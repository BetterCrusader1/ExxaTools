//! Inject our icon and menu into the Steam client's own menu bar.
//!
//! Clone the Help menu item, swap its contents for our mark, and hang a panel
//! off it that talks to the local API. A MutationObserver puts it back when
//! Steam re-renders.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::base64_encode;
use crate::cdp;
use crate::paths;
use crate::steamctl;

pub const DEBUG_PORT: u16 = 8080;
pub const FLAG: &str = "-cef-enable-debugging";
const MAIN_TARGET_PREFIX: &str = "about:blank?createflags=18";
pub const ROOT_ID: &str = "vd-menu-root";
const PANEL_ID: &str = "vd-menu-panel";
const ICON_ID: &str = "vd-menu-icon";
fn api_base() -> String {
    format!("http://127.0.0.1:{}", crate::server::port())
}
const MENU_SCRIPT: &str = include_str!("../assets/steam-menu.js");

const GLYPHS: &[(&str, &str)] = &[
    (
        "app",
        r#"<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.5 3.5h11v9h-11z"/><path d="M2.5 6.5h11"/></svg>"#,
    ),
    (
        "restart",
        r#"<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.8 8a5.2 5.2 0 1 0 1.9-4"/><path d="M2.6 2.6v3.2h3.2"/></svg>"#,
    ),
    (
        "provider",
        r#"<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8" cy="8" r="2.1"/><path d="M8 1.5v3M8 11.5v3M1.5 8h3M11.5 8h3"/></svg>"#,
    ),
    (
        "about",
        r#"<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8" cy="8" r="6"/><path d="M8 7.2v4M8 5.1v.2"/></svg>"#,
    ),
];

fn icon_markup() -> String {
    let path: PathBuf = paths::web_dir().join("logo-48.png");
    let Ok(raw) = std::fs::read(path) else { return String::new() };
    format!(
        r#"<img src="data:image/png;base64,{}" style="width:16px;height:16px;border-radius:50%;display:block" alt="">"#,
        base64_encode(&raw)
    )
}

fn script_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn script() -> String {
    let glyphs = serde_json::to_string(
        &GLYPHS
            .iter()
            .map(|(name, svg)| ((*name).to_string(), Value::String((*svg).to_string())))
            .collect::<std::collections::BTreeMap<_, _>>(),
    )
    .unwrap_or_else(|_| "{}".into());
    MENU_SCRIPT
        .replace("__GLYPHS__", &glyphs)
        .replace("__API__", &api_base())
        .replace("__ROOT__", ROOT_ID)
        .replace("__PANEL__", PANEL_ID)
        .replace("__ICON__", ICON_ID)
        .replace("ICON_ICON", &serde_json::to_string(&icon_markup()).unwrap_or_else(|_| "\"\"".into()))
}

fn page_targets() -> Vec<Value> {
    cdp::targets(DEBUG_PORT)
        .into_iter()
        .filter(|target| target.get("type").and_then(Value::as_str) == Some("page"))
        .collect()
}

fn evaluate_on(target: &Value, expression: &str, timeout: std::time::Duration) -> Result<Value, String> {
    let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
    if ws.is_empty() {
        return Err("target has no websocket url".into());
    }
    cdp::evaluate(ws, expression, timeout)
}

#[derive(Debug, Clone)]
struct Frame {
    ws: String,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
}

fn frame_of(target: &Value) -> Option<Frame> {
    let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str)?.to_string();
    let raw = cdp::evaluate(
        &ws,
        "JSON.stringify({x: window.screenX, y: window.screenY, w: window.innerWidth, h: window.innerHeight})",
        std::time::Duration::from_secs(4),
    )
    .ok()?;
    let text = raw.as_str()?;
    let parsed: Value = serde_json::from_str(text).ok()?;
    Some(Frame {
        ws,
        x: parsed.get("x").and_then(Value::as_i64).unwrap_or(0),
        y: parsed.get("y").and_then(Value::as_i64).unwrap_or(0),
        w: parsed.get("w").and_then(Value::as_i64).unwrap_or(0),
        h: parsed.get("h").and_then(Value::as_i64).unwrap_or(0),
    })
}

fn frames() -> Vec<Frame> {
    page_targets().iter().filter_map(frame_of).filter(|frame| frame.w > 50 && frame.h > 50).collect()
}

/// The window the user is looking at is the smallest one that covers the point
/// where the panel would appear: Steam layers the store and library above the
/// shell, and both report the point.
fn host_for(cx: i64, cy: i64) -> Option<Frame> {
    frames()
        .into_iter()
        .filter(|frame| cx >= frame.x && cx <= frame.x + frame.w && cy >= frame.y && cy <= frame.y + frame.h)
        .min_by_key(|frame| frame.w * frame.h)
}

fn ensure_panel(target: &Frame) -> Result<(), String> {
    let script = script();
    let build = script_hash(&script);
    let query = format!("window.__vdBuild === {build:?}");
    let current = cdp::evaluate(&target.ws, &query, std::time::Duration::from_secs(4))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if current {
        return Ok(());
    }
    let bootstrap = format!("window.__vdNoIcon = true;\n{script}\nwindow.__vdBuild = {build:?};");
    cdp::evaluate(&target.ws, &bootstrap, std::time::Duration::from_secs(8)).map(|_| ())
}

static PANEL_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PANEL_HOST: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static PUMP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn status_snapshot() -> Value {
    let st = crate::state::load().unwrap_or_default();
    let apps: Vec<Value> = st
        .apps
        .values()
        .map(|record| {
            let appid = record.get("appid").and_then(Value::as_u64).unwrap_or(0);
            let depots = record.get("depots").and_then(Value::as_array).cloned().unwrap_or_default();
            let missing: Vec<u64> = depots
                .iter()
                .filter_map(|depot| depot.get("depot_id").and_then(Value::as_u64))
                .filter(|id| !st.depot_keys.contains_key(&id.to_string()))
                .collect();
            let mut view = record.clone();
            view["installed"] = json!(crate::vfsys::script_path(appid).exists());
            view["missing_keys"] = json!(missing);
            view
        })
        .collect();
    json!({ "engine": crate::engine::detect(), "apps": apps })
}

fn run_action(action: &str) -> String {
    match action {
        "open" => match crate::desktop::launch_app().get("error").and_then(Value::as_str) {
            Some(error) => error.to_string(),
            None => "opening the dashboard".into(),
        },
        "restart" => match crate::steamctl::restart(&[]) {
            Ok(_) => "Steam is restarting".into(),
            Err(error) => error,
        },
        "provider" => {
            let st = crate::state::load().unwrap_or_default();
            let order: Vec<Value> = st.settings.manifest_providers.iter().map(|name| json!(name)).collect();
            match crate::lua::render_provider(&order) {
                Ok(text) => match crate::vfsys::write_provider(&text) {
                    Ok(_) => "provider script written".into(),
                    Err(error) => error,
                },
                Err(error) => error,
            }
        }
        "close" => {
            hide_panel();
            String::new()
        }
        _ => String::new(),
    }
}

/// Steam's pages block loopback requests with their CSP, so the panel has no
/// network at all: the pump reads clicks and pushes fresh state over CDP.
fn pump() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(400));
        if !PANEL_SHOWN.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }
        let Some(ws) = PANEL_HOST.lock().ok().and_then(|guard| guard.clone()) else { continue };
        let action = cdp::evaluate(&ws, "window.__vdAction || ''", std::time::Duration::from_secs(3))
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        if !action.is_empty() {
            let _ = cdp::evaluate(&ws, "window.__vdAction = ''", std::time::Duration::from_secs(3));
            let message = run_action(&action);
            let state = status_snapshot();
            let payload = json!({ "engine": state["engine"], "apps": state["apps"], "msg": message });
            let expression = format!("window.__vdUpdate({payload})");
            let _ = cdp::evaluate(&ws, &expression, std::time::Duration::from_secs(3));
            continue;
        }
        let state = status_snapshot();
        let expression = format!("window.__vdUpdate({state})");
        let _ = cdp::evaluate(&ws, &expression, std::time::Duration::from_secs(3));
    }
}

fn hide_panel() {
    use std::sync::atomic::Ordering;
    for frame in frames() {
        let _ = cdp::evaluate(&frame.ws, "window.__vdPanel && window.__vdPanel({ open: false })", std::time::Duration::from_secs(3));
    }
    PANEL_SHOWN.store(false, Ordering::Relaxed);
    if let Ok(mut guard) = PANEL_HOST.lock() {
        *guard = None;
    }
}

/// The panel lives inside Steam: it is injected into whichever window sits under
/// the menu, so it scrolls with that window and cannot float over other apps.
pub fn panel(open: bool, toggle: bool, x: i64, y: i64, screen_x: i64, screen_y: i64) -> Result<Value, String> {
    use std::sync::atomic::Ordering;

    if !open || (toggle && PANEL_SHOWN.load(Ordering::Relaxed)) {
        hide_panel();
        return Ok(json!({ "open": false }));
    }
    if !steamctl::is_running() {
        return Err("Steam is not running".into());
    }

    let width = 300i64;
    let plans = crate::state::load().map(|state| state.apps.len()).unwrap_or(0).min(4) as i64;
    let height = 300 + plans * 30;
    let icon_abs = screen_x + x;
    let mut left = icon_abs - width + 24;
    let top = screen_y + y + 10;
    let host = host_for(left + width / 2, top + height / 2).ok_or("no Steam window covers that spot")?;

    ensure_panel(&host)?;
    if left < host.x + 8 {
        left = icon_abs - 16;
    }
    let local_x = (left - host.x).clamp(8, (host.w - width - 8).max(8));
    let local_y = (top - host.y).max(8);
    let state = status_snapshot();
    let expression = format!(
        "window.__vdPanel({{ open: true, x: {local_x}, y: {local_y}, state: {state} }})"
    );
    cdp::evaluate(&host.ws, &expression, std::time::Duration::from_secs(4))?;
    PANEL_SHOWN.store(true, Ordering::Relaxed);
    if let Ok(mut guard) = PANEL_HOST.lock() {
        *guard = Some(host.ws.clone());
    }

    for frame in frames() {
        if frame.ws == host.ws {
            continue;
        }
        let _ = cdp::evaluate(&frame.ws, "window.__vdPanel && window.__vdPanel({ open: false })", std::time::Duration::from_secs(3));
    }

    if !PUMP.swap(true, Ordering::Relaxed) {
        std::thread::spawn(pump);
    }

    Ok(json!({ "open": true, "host": [host.x, host.y, host.w, host.h], "left": local_x, "top": local_y }))
}

pub fn debug_ready() -> bool {
    cdp::version(DEBUG_PORT).is_some()
}

pub fn main_target() -> Option<Value> {
    let list = cdp::targets(DEBUG_PORT);
    for target in &list {
        let url = target.get("url").and_then(Value::as_str).unwrap_or("");
        if url.starts_with(MAIN_TARGET_PREFIX) {
            return Some(target.clone());
        }
    }
    for target in list {
        if target.get("title").and_then(Value::as_str) == Some("Steam")
            && target.get("type").and_then(Value::as_str) == Some("page")
        {
            return Some(target);
        }
    }
    None
}

pub fn status() -> Value {
    let running = steamctl::is_running();
    let ready = running && debug_ready();
    let target = if ready { main_target() } else { None };
    let mut injected = false;
    let mut error: Option<String> = None;
    if let Some(target) = &target {
        let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
        match cdp::evaluate(ws, &format!("Boolean(document.getElementById({ROOT_ID:?}))"), std::time::Duration::from_secs(6)) {
            Ok(value) => injected = value.as_bool().unwrap_or(false),
            Err(message) => error = Some(message),
        }
    }
    json!({
        "steam_running": running,
        "debug_port": DEBUG_PORT,
        "debug_ready": ready,
        "target": target.as_ref().and_then(|value| value.get("url")).cloned(),
        "injected": injected,
        "flag": FLAG,
        "error": error,
    })
}

pub fn inject() -> Result<Value, String> {
    if !steamctl::is_running() {
        return Err("Steam is not running".into());
    }
    if !debug_ready() {
        return Err(format!(
            "Steam has no debugging port: press Enable, which restarts Steam with {FLAG}"
        ));
    }
    let target = main_target().ok_or("the Steam main window is not available over CDP".to_string())?;
    let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
    let script = script();
    let build = script_hash(&script);
    let body = format!("window.__vdNoIcon = false;\n{script}\nwindow.__vdBuild = {build:?};");
    let result = cdp::evaluate(ws, &body, std::time::Duration::from_secs(20))?;
    let text = result.as_str().unwrap_or("");
    if text != "injected" && text != "present" {
        return Err(format!("injection failed: {text}"));
    }
    let mut out = status();
    out["injected"] = json!(true);
    out["result"] = json!(text);
    Ok(out)
}

pub fn remove() -> Value {
    let mut removed = 0;
    for target in page_targets() {
        let expression = format!(
            r#"(() => {{
              const root = document.getElementById({ROOT_ID:?});
              const panel = document.getElementById({PANEL_ID:?});
              const style = document.getElementById({PANEL_ID:?} + "-style");
              const iconStyle = document.getElementById({ROOT_ID:?} + "-style");
              if (root) root.remove();
              if (panel) panel.remove();
              if (style) style.remove();
              if (iconStyle) iconStyle.remove();
              window.__vdOpen = false;
              return true;
            }})()"#
        );
        if evaluate_on(&target, &expression, std::time::Duration::from_secs(4)).is_ok() {
            removed += 1;
        }
    }
    let mut out = status();
    out["removed"] = json!(removed);
    out
}

pub fn desktop_override() -> Option<PathBuf> {
    let path = paths::data_home().join("applications/steam.desktop");
    path.is_file().then_some(path)
}

const DESKTOP_MARK: &str = "X-Vapor-Forge-Override=true";

/// Persist the debugging flag in the launcher, then restart Steam.
pub fn enable_debug() -> Result<Value, String> {
    let mut patched: Vec<String> = Vec::new();
    for path in [desktop_override(), Some(PathBuf::from("/usr/share/applications/steam.desktop"))]
        .into_iter()
        .flatten()
    {
        if !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if text.contains(FLAG) {
            patched.push(path.display().to_string());
            continue;
        }
        if !text.contains("Exec=") {
            continue;
        }
        if path.file_name().and_then(|value| value.to_str()) == Some("steam.desktop") && !text.contains(DESKTOP_MARK) {
            continue;
        }
        let updated = text.replacen("Exec=", &format!("Exec={FLAG} "), 1);
        if std::fs::write(&path, updated).is_ok() {
            patched.push(path.display().to_string());
        }
    }
    let restarted = steamctl::restart(&[FLAG.to_string()])?;
    Ok(json!({ "patched": patched, "restarted": restarted, "flag": FLAG }))
}

/// Puts the menu back after Steam re-renders or updates itself.
pub fn watch_loop() {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(45));
        if !steamctl::is_running() {
            continue;
        }
        if !debug_ready() {
            continue;
        }
        let injected = status().get("injected").and_then(Value::as_bool).unwrap_or(false);
        if injected {
            continue;
        }
        match inject() {
            Ok(_) => eprintln!("steamui: menu re-injected"),
            Err(error) => eprintln!("steamui: re-inject failed: {error}"),
        }
    }
}
