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

/// A freshly created CEF window can report (0,0) for a moment while the window
/// manager has not placed it yet. In that case lay the panel out against the
/// shell's origin instead of the corner of the screen.
/// Body height for layout decisions: header, an alert row, up to four plan rows,
/// never taller than the space under the menu icon.
pub fn panel_height(plans: i64, alert: bool, screen_height: i64) -> i64 {
    let mut height = 300 + plans.clamp(0, 4) * 30 + if alert { 46 } else { 0 };
    let room = if screen_height > 0 { screen_height - 80 } else { 900 };
    height = height.min(room.max(240));
    height.max(240)
}

fn panel_origin(host: &Frame, reference: Option<&Frame>) -> (i64, i64) {
    if host.x == 0 && host.y == 0 {
        if let Some(main) = reference.filter(|main| main.ws != host.ws && main.x + main.y > 0) {
            return (main.x, main.y + (main.h - host.h).max(0));
        }
    }
    (host.x, host.y)
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
            let mut view = crate::state::app_summary(&st, record);
            view["launch_options"] = json!(crate::steamctl::read_launch_options(appid));
            view
        })
        .collect();
    let incidents = crate::diagnose::from_files();
    json!({
        "engine": crate::engine::detect(),
        "apps": apps,
        "diagnose": incidents.get("incidents").cloned().unwrap_or_else(|| json!([])),
    })
}

fn run_action(action: &str) -> String {
    if let Some(rest) = action.strip_prefix("install:") {
        let Ok(appid) = rest.parse::<u64>() else { return "bad app id".into() };
        return match crate::plans::write_script(appid) {
            Ok(true) => format!("plan {appid} installed"),
            Ok(false) => format!("no saved plan for {appid}"),
            Err(error) => error,
        };
    }
    if let Some(rest) = action.strip_prefix("rebuild:") {
        let Ok(appid) = rest.parse::<u64>() else { return "bad app id".into() };
        return match crate::plans::refresh_plan(appid) {
            Ok(report) => format!(
                "plan {appid} rebuilt ({} depot set(s))",
                report.get("updated").and_then(Value::as_u64).unwrap_or(0)
            ),
            Err(error) => error,
        };
    }
    if let Some(rest) = action.strip_prefix("launch:") {
        let Ok(appid) = rest.parse::<u64>() else { return "bad app id".into() };
        let current = crate::steamctl::read_launch_options(appid).unwrap_or_default();
        let has = current.split_whitespace().any(|part| part == "-onlinefix");
        let next = if has {
            current.split_whitespace().filter(|part| *part != "-onlinefix").collect::<Vec<_>>().join(" ")
        } else {
            crate::verify::merge_launch_option(Some(&current), "-onlinefix")
        };
        return match crate::steamctl::set_launch_options(appid, &next) {
            Ok(_) => format!("-onlinefix {}", if has { "removed" } else { "added" }),
            Err(error) => error,
        };
    }
    match action {
        "disk" => match crate::storage::report().get("lowest").cloned() {
            Some(lowest) => format!(
                "Lowest free space: {} ({})",
                lowest.get("free_human").and_then(Value::as_str).unwrap_or("?"),
                lowest.get("path").and_then(Value::as_str).unwrap_or("")
            ),
            None => "Could not read free space".into(),
        },
        "engine" => match crate::engine::install(None) {
            Ok(report) => format!("engine {} installed", report.get("version").and_then(Value::as_str).unwrap_or("")),
            Err(error) => error,
        },
        "keys" => {
            let st = crate::state::load().unwrap_or_default();
            let mut refreshed = 0;
            for record in st.apps.values() {
                let appid = record.get("appid").and_then(Value::as_u64).unwrap_or(0);
                if crate::vfsys::script_path(appid).exists() && crate::plans::write_script(appid).unwrap_or(false) {
                    refreshed += 1;
                }
            }
            format!("{refreshed} script(s) refreshed")
        }
        "open" => match open_dashboard() {
            Ok(()) => "dashboard opened in Steam".into(),
            Err(error) => error,
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
/// One pump per process: it turns page clicks into Rust calls and opens the panel
/// when the icon sets the flag, so it must run before the panel is ever shown.
pub fn start_pump() {
    if !PUMP.swap(true, std::sync::atomic::Ordering::Relaxed) {
        std::thread::spawn(pump);
    }
}

fn pump() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(400));
        // The icon and the hotkey set a flag instead of calling the API, because
        // the page cannot hold the token and its CSP blocks loopback requests.
        if let Some(target) = main_target() {
            if let Some(ws) = target.get("webSocketDebuggerUrl").and_then(Value::as_str) {
                if let Ok(want) = cdp::evaluate(ws, "window.__vdWantPanel || null", std::time::Duration::from_millis(800)) {
                    if want.is_object() {
                        let _ = cdp::evaluate(ws, "window.__vdWantPanel = null", std::time::Duration::from_millis(800));
                        if want.get("close").and_then(Value::as_bool).unwrap_or(false) {
                            hide_panel();
                        } else {
                            let field = |name: &str| want.get(name).and_then(Value::as_i64).unwrap_or(0);
                            if let Err(error) = panel(
                                true,
                                want.get("toggle").and_then(Value::as_bool).unwrap_or(false),
                                field("x"),
                                field("y"),
                                field("screen_x"),
                                field("screen_y"),
                            ) {
                                eprintln!("panel from icon: {error}");
                            }
                        }
                        continue;
                    }
                }
            }
        }
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
    let plans = crate::state::load().map(|state| state.apps.len()).unwrap_or(0) as i64;
    let alerts = crate::diagnose::from_files()
        .get("incidents")
        .and_then(Value::as_array)
        .map(|list| !list.is_empty())
        .unwrap_or(false);
    let height = panel_height(plans, alerts, screen_y.max(0));
    let icon_abs = screen_x + x;
    let mut left = icon_abs - width + 24;
    let top = screen_y + y + 10;
    let host = host_for(left + width / 2, top + height / 2).ok_or("no Steam window covers that spot")?;

    ensure_panel(&host)?;
    let reference = frames().into_iter().max_by_key(|frame| frame.w * frame.h);
    let (origin_x, origin_y) = panel_origin(&host, reference.as_ref());
    if left < origin_x + 8 {
        left = icon_abs - 16;
    }
    let local_x = (left - origin_x).clamp(8, (host.w - width - 8).max(8));
    let local_y = (top - origin_y).max(8);
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

    start_pump();

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
    list.into_iter().find(|target| {
        target.get("title").and_then(Value::as_str) == Some("Steam")
            && target.get("type").and_then(Value::as_str) == Some("page")
    })
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

/// Show the dashboard as an overlay inside Steam. Callers that used to spawn a
/// desktop window (the panel button, an older injected panel) go through here so
/// the app never splits into a second application.
pub fn open_dashboard() -> Result<(), String> {
    if !steamctl::is_running() {
        return Err("Steam is not running".into());
    }
    if !debug_ready() {
        return Err(format!("Steam has no debugging port: restart Steam with {FLAG}"));
    }
    let mut shown = 0;
    for target in page_targets() {
        let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
        if ws.is_empty() {
            continue;
        }
        let script = "window.__vdOpenDashboard ? (window.__vdOpenDashboard(), 'opened') : 'missing'";
        if let Ok(value) = cdp::evaluate(ws, script, std::time::Duration::from_secs(4)) {
            if value.as_str() == Some("opened") {
                shown += 1;
            }
        }
    }
    if shown == 0 {
        return Err("the Steam menu is not injected yet".into());
    }
    Ok(())
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
    let script = script();
    let build = script_hash(&script);
    let plain = format!("{script}\nwindow.__vdBuild = {build:?};\nwindow.__vdInject();");
    let main_ws = main_target()
        .and_then(|target| target.get("webSocketDebuggerUrl").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    // Every page window gets the current script: the panel lives in the content
    // window, which is not the shell that carries the menu bar.
    let mut text = String::new();
    let mut reached = 0;
    for target in page_targets() {
        let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
        if ws.is_empty() {
            continue;
        }
        // Only the window that owns the menu bar gets the icon; every other page
        // window stays panel-only, or the icon would appear more than once.
        let body = if ws == main_ws {
            format!("window.__vdNoIcon = false;\n{plain}")
        } else {
            format!("window.__vdNoIcon = true;\n{plain}")
        };
        if let Ok(value) = cdp::evaluate(ws, &body, std::time::Duration::from_secs(20)) {
            let answer = value.as_str().unwrap_or("").to_string();
            if answer == "injected" || answer == "present" {
                reached += 1;
                text = answer;
            }
        }
    }
    if reached == 0 {
        return Err(format!(
            "injection failed: {}",
            if text.is_empty() { "no page window answered" } else { &text }
        ));
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


/// The flag is an argument of the steam binary, not a command: putting it before
/// `env` breaks the whole entry and Steam starts without a debugging port.
fn add_debug_flag(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let broken = format!("Exec={FLAG} ");
    for raw in text.lines() {
        // Repair entries broken by the old rule, which put the flag before `env`.
        let line = match raw.strip_prefix(&broken) {
            Some(rest) => format!("Exec={rest}"),
            None => raw.to_string(),
        };
        if line.starts_with("Exec=") && !line.contains(FLAG) {
            if let Some(index) = line.find("/usr/bin/steam") {
                let split = index + "/usr/bin/steam".len();
                out.push_str(&line[..split]);
                out.push(' ');
                out.push_str(FLAG);
                out.push_str(&line[split..]);
            } else {
                out.push_str(&line);
            }
        } else {
            out.push_str(&line);
        }
        out.push('\n');
    }
    out
}

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
        if !text.contains("Exec=") {
            continue;
        }
        if path.file_name().and_then(|value| value.to_str()) == Some("steam.desktop") && !text.contains(DESKTOP_MARK) {
            continue;
        }
        let updated = add_debug_flag(&text);
        let written = updated == text || std::fs::write(&path, updated).is_ok();
        if written {
            patched.push(path.display().to_string());
        }
    }
    let restarted = steamctl::restart(&[FLAG.to_string()])?;
    Ok(json!({ "patched": patched, "restarted": restarted, "flag": FLAG }))
}

static WATCHER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Keeps the menu alive for the whole session: Steam rebuilds its window on every
/// restart and drops whatever was injected, so the icon has to be put back.
pub fn ensure_started() {
    use std::sync::atomic::Ordering;
    if WATCHER.swap(true, Ordering::Relaxed) {
        return;
    }
    std::thread::spawn(|| {
        loop {
            let running = steamctl::is_running();
            if !running {
                std::thread::sleep(std::time::Duration::from_secs(2));
                continue;
            }
            if !debug_ready() {
                // A Steam started without the debugging flag cannot be reached; the
                // launcher entry is repaired so the next start works.
                let _ = enable_debug_if_possible();
                std::thread::sleep(std::time::Duration::from_secs(10));
                continue;
            }
            let injected = status().get("injected").and_then(Value::as_bool).unwrap_or(false);
            if !injected {
                match inject() {
                    Ok(_) => eprintln!("steamui: menu injected"),
                    Err(error) => eprintln!("steamui: inject failed: {error}"),
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(if injected { 20 } else { 3 }));
        }
    });
}

/// Writes the flag into the launcher without restarting anything.
fn enable_debug_if_possible() -> Result<(), String> {
    for path in [desktop_override(), Some(PathBuf::from("/usr/share/applications/steam.desktop"))]
        .into_iter()
        .flatten()
    {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if !text.contains("Exec=") {
            continue;
        }
        if path.file_name().and_then(|value| value.to_str()) == Some("steam.desktop") && !text.contains(DESKTOP_MARK) {
            continue;
        }
        let updated = add_debug_flag(&text);
        if updated != text {
            let _ = std::fs::write(&path, updated);
        }
    }
    Ok(())
}

/// Kept for callers that used to start the loop by hand.
pub fn watch_loop() {
    ensure_started();
}


#[cfg(test)]
mod tests {
    use super::add_debug_flag;

    #[test]
    fn flag_goes_after_the_binary() {
        let entry = "[Desktop Entry]\nExec=env \"LD_AUDIT=/x\" /usr/bin/steam %U\n";
        let patched = add_debug_flag(entry);
        assert!(patched.contains("/usr/bin/steam -cef-enable-debugging %U"));
        assert!(!patched.starts_with("Exec=-cef"));
    }

    #[test]
    fn broken_entry_is_repaired() {
        let entry = "Exec=-cef-enable-debugging env \"LD_AUDIT=/x\" /usr/bin/steam %U\n";
        let patched = add_debug_flag(entry);
        assert!(patched.starts_with("Exec=env "));
        assert!(patched.contains("/usr/bin/steam -cef-enable-debugging %U"));
    }

    #[test]
    fn flag_is_not_added_twice() {
        let entry = "Exec=/usr/bin/steam -cef-enable-debugging %U\n";
        assert_eq!(add_debug_flag(entry), entry);
    }
}


#[cfg(test)]
mod origin_tests {
    use super::{panel_origin, Frame};

    fn frame(x: i64, y: i64, w: i64, h: i64, ws: &str) -> Frame {
        Frame { ws: ws.to_string(), x, y, w, h }
    }

    #[test]
    fn placed_window_uses_its_own_origin() {
        let host = frame(462, 448, 1278, 657, "host");
        let main = frame(461, 356, 1280, 800, "main");
        assert_eq!(panel_origin(&host, Some(&main)), (462, 448));
    }

    #[test]
    fn origin_falls_back_to_the_shell() {
        let host = frame(0, 0, 1278, 657, "host");
        let main = frame(461, 356, 1280, 800, "main");
        assert_eq!(panel_origin(&host, Some(&main)), (461, 356 + (800 - 657)));
    }

    #[test]
    fn no_reference_keeps_zero() {
        let host = frame(0, 0, 1278, 657, "host");
        assert_eq!(panel_origin(&host, None), (0, 0));
    }
}


#[cfg(test)]
mod height_tests {
    use super::panel_height;

    #[test]
    fn height_grows_with_plans() {
        assert_eq!(panel_height(0, false, 0), 300);
        assert_eq!(panel_height(2, false, 0), 360);
    }

    #[test]
    fn alert_adds_a_row() {
        assert_eq!(panel_height(0, true, 0), 346);
    }

    #[test]
    fn plans_are_capped_at_four() {
        assert_eq!(panel_height(9, false, 0), 420);
    }

    #[test]
    fn short_screens_clamp() {
        assert_eq!(panel_height(4, true, 400), 320);
        assert_eq!(panel_height(0, false, 200), 240);
    }
}
