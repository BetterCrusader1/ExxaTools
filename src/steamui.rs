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

const PLUGIN_BUS_JS: &str = r#"(() => {
  if (!document.getElementById("exxa-queue")) {
    const queue = document.createElement("script");
    queue.type = "application/json";
    queue.id = "exxa-queue";
    queue.hidden = true;
    queue.textContent = "[]";
    document.body.appendChild(queue);
  }
  if (!document.getElementById("exxa-results")) {
    const results = document.createElement("script");
    results.type = "application/json";
    results.id = "exxa-results";
    results.hidden = true;
    results.textContent = "{}";
    document.body.appendChild(results);
  }
  if (!document.getElementById("exxa-plugin-root")) {
    const root = document.createElement("div");
    root.id = "exxa-plugin-root";
    document.body.appendChild(root);
  }
  return "bus";
})()"#;

/// The `exxa` object a plugin sees. Calls go into a DOM queue that the Rust bridge
/// drains; results come back the same way. The page's own globals are untouched.
fn plugin_bootstrap(plugin: &crate::plugins::Plugin) -> String {
    let meta = serde_json::to_string(&json!({
        "id": plugin.id,
        "name": plugin.name,
        "version": plugin.version,
        "permissions": plugin.permissions,
    }))
    .unwrap_or_else(|_| "{}".into());
    format!(
        r#"(() => {{
  const PLUGIN = {meta};
  const host = (id) => document.getElementById(id);
  let seq = 0;
  const pending = new Map();
  const call = (method, params) => new Promise((resolve, reject) => {{
    const id = PLUGIN.id + ":" + (++seq) + ":" + Math.random().toString(36).slice(2, 8);
    pending.set(id, {{ resolve, reject }});
    const queue = host("exxa-queue");
    if (!queue) {{ pending.delete(id); reject(new Error("host is not ready")); return; }}
    let list = [];
    try {{ list = JSON.parse(queue.textContent || "[]"); }} catch (error) {{ list = []; }}
    list.push({{ id: id, plugin: PLUGIN.id, method: method, params: params || {{}} }});
    queue.textContent = JSON.stringify(list);
  }});
  setInterval(() => {{
    const box = host("exxa-results");
    if (!box || !pending.size) return;
    let map = {{}};
    try {{ map = JSON.parse(box.textContent || "{{}}"); }} catch (error) {{ return; }}
    const ids = Object.keys(map);
    if (!ids.length) return;
    for (const id of ids) {{
      const entry = pending.get(id);
      if (!entry) {{ delete map[id]; continue; }}
      pending.delete(id);
      const outcome = map[id] || {{}};
      (outcome.ok ? entry.resolve : entry.reject)(outcome.value);
      delete map[id];
    }}
    box.textContent = JSON.stringify(map);
  }}, 150);
  const toast = (text) => {{
    const node = document.createElement("div");
    node.textContent = String(text);
    node.style.cssText = "position:fixed;right:18px;bottom:18px;z-index:2147483647;padding:10px 14px;border-radius:10px;background:#151A21;border:1px solid #2E3A49;color:#E9EDF2;font:500 12.5px Montserrat,Arial,sans-serif";
    document.body.appendChild(node);
    setTimeout(() => node.remove(), 2600);
  }};
  const section = (title, build) => {{
    const root = host("exxa-plugin-root");
    if (!root) return false;
    // Re-injection creates a fresh world for the same plugin; drop the section the
    // previous world rendered so the panel never shows two copies.
    root.querySelectorAll('[data-exxa-plugin="' + PLUGIN.id + '"]').forEach((old) => old.remove());
    const wrap = document.createElement("section");
    wrap.setAttribute("data-exxa-plugin", PLUGIN.id);
    if (title) {{
      const label = document.createElement("div");
      label.className = "vd-label";
      label.textContent = String(title);
      wrap.append(label);
    }}
    let node = null;
    try {{ node = typeof build === "function" ? build() : build; }} catch (error) {{ return false; }}
    if (node) wrap.append(node);
    root.append(wrap);
    return true;
  }};
  window.exxa = {{
    version: "1",
    plugin: {{ id: PLUGIN.id, name: PLUGIN.name, version: PLUGIN.version }},
    config: {{
      get: (key) => call("config.get", {{ key: key }}),
      set: (key, value) => call("config.set", {{ key: key, value: value }}),
    }},
    steam: {{ status: () => call("steam.status") }},
    ui: {{ section: section, toast: toast }},
    log: (...args) => console.log("[" + PLUGIN.id + "]", ...args),
  }};
  return PLUGIN.id;
}})()"#
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
        .filter(|target| {
            // Skip the dashboard's own frames: injecting there would nest a panel
            // inside the dashboard and answer its own requests.
            let url = target.get("url").and_then(Value::as_str).unwrap_or("");
            !url.contains("127.0.0.1:")
        })
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
            match refreshed {
                0 => "no scripts needed a refresh".into(),
                1 => "refreshed 1 script".into(),
                count => format!("refreshed {count} scripts"),
            }
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
        "dashboard" => match open_dashboard() {
            Ok(()) => "dashboard opened in Steam".into(),
            Err(error) => error,
        },
        "settings" => match open_dashboard_at("/#settings") {
            Ok(()) => "settings opened in Steam".into(),
            Err(error) => error,
        },
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

/// Answers the store-page card: a probe reports whether the app is in the library,
/// Add fetches a plan through the configured sources, Sync refreshes manifests.
fn handle_page_request(ws: &str, request: &Value) {
    let appid = request.get("appid").and_then(Value::as_u64).unwrap_or(0);
    if appid == 0 {
        return;
    }
    let want = request.get("want").and_then(Value::as_str).unwrap_or("");
    let name = crate::steam::catalog(appid)
        .ok()
        .and_then(|catalog| catalog.get("name").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| format!("App {appid}"));
    let planned = crate::state::load()
        .map(|state| state.apps.contains_key(&appid.to_string()))
        .unwrap_or(false);
    let (status, can_add, can_sync, can_setup) = match want {
        "dashboard" => {
            let message = match open_dashboard() {
                Ok(()) => "Dashboard opened in Steam".to_string(),
                Err(error) => error,
            };
            (message, false, planned, false)
        }
        "settings" => {
            let message = match open_dashboard_at("/#settings") {
                Ok(()) => "Settings opened in Steam".to_string(),
                Err(error) => error,
            };
            (message, false, planned, false)
        }
        "add" => match crate::plans::fetch_plan(appid, Some(&name)) {
            Ok(_) => (
                format!("{name} added — restart Steam to install it"),
                false,
                true,
                false,
            ),
            Err(error) => {
                let setup = error.contains("plan source");
                (error, true, planned, setup)
            }
        },
        "sync" => match crate::plans::refresh_plan(appid) {
            Ok(_) => (format!("{name} manifests synced"), false, true, false),
            Err(error) => (error, false, true, false),
        },
        _ => (
            if planned {
                "In your library. Sync to pick up the newest manifests.".to_string()
            } else {
                "Not in your library yet.".to_string()
            },
            !planned,
            planned,
            false,
        ),
    };
    let payload = json!({
        "appid": appid,
        "name": name,
        "status": status,
        "can_add": can_add,
        "can_sync": can_sync,
        "can_setup": can_setup,
    });
    let expression = format!("window.__vdPageState = {payload};");
    let _ = cdp::evaluate(ws, &expression, std::time::Duration::from_secs(2));
}

fn pump() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(400));
        if !steamctl::is_running() || !debug_ready() {
            continue;
        }
        let Some(target) = main_target() else { continue };
        let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
        if ws.is_empty() {
            continue;
        }
        // The icon opens and closes the panel inside the page; the pump only
        // refreshes what is on screen and turns clicks into Rust calls.
        let _ = cdp::evaluate(ws, "window.__vdWantPanel = null", std::time::Duration::from_millis(600));
        for target in page_targets() {
            let page_ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
            if page_ws.is_empty() {
                continue;
            }
            let request = cdp::evaluate(page_ws, "window.__vdPageRequest || null", std::time::Duration::from_millis(600))
                .ok()
                .unwrap_or(Value::Null);
            if request.is_object() {
                let _ = cdp::evaluate(page_ws, "window.__vdPageRequest = null", std::time::Duration::from_millis(600));
                handle_page_request(page_ws, &request);
            }
        }
        let open = cdp::evaluate(ws, "Boolean(window.__vdPanelOpen)", std::time::Duration::from_millis(800))
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !open {
            continue;
        }
        let action = cdp::evaluate(ws, "window.__vdAction || ''", std::time::Duration::from_secs(2))
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        if !action.is_empty() {
            let _ = cdp::evaluate(ws, "window.__vdAction = ''", std::time::Duration::from_secs(2));
            let message = run_action(&action);
        if action == "dashboard" || action == "settings" {
            let _ = cdp::evaluate(ws, "window.__vdPanel && window.__vdPanel({open:false})", std::time::Duration::from_millis(800));
        }
            let state = status_snapshot();
            let payload = json!({ "engine": state["engine"], "apps": state["apps"], "msg": message });
            let expression = format!("window.__vdUpdate({payload})");
            let _ = cdp::evaluate(ws, &expression, std::time::Duration::from_secs(2));
            continue;
        }
        let state = status_snapshot();
        let expression = format!("window.__vdUpdate({state})");
        let _ = cdp::evaluate(ws, &expression, std::time::Duration::from_secs(2));
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
    // A click left in the page from a previous panel must not fire against the
    // one we just opened (a stale "close" used to shut it instantly).
    let _ = cdp::evaluate(&host.ws, "window.__vdAction = ''", std::time::Duration::from_secs(2));
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
    let desktop = desktop_override();
    let launcher = desktop
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| launcher_uses_wrapper(&text))
        .unwrap_or(false);
    let autostart = crate::desktop::autostart_path().is_file();
    let server = crate::http::get_json(&format!("http://127.0.0.1:{}/api/session", crate::server::port())).is_ok();
    json!({
        "steam_running": running,
        "debug_port": DEBUG_PORT,
        "debug_ready": ready,
        "target": target.as_ref().and_then(|value| value.get("url")).cloned(),
        "injected": injected,
        "flag": FLAG,
        "autostart": autostart,
        "desktop_entry": desktop.map(|path| path.display().to_string()),
        "launcher_wrapper": launcher,
        "server": server,
        "error": error,
    })
}

/// Show the dashboard as an overlay inside Steam. Callers that used to spawn a
/// desktop window (the panel button, an older injected panel) go through here so
/// the app never splits into a second application.
pub fn open_dashboard() -> Result<(), String> {
    open_dashboard_at("/")
}

pub fn open_dashboard_at(route: &str) -> Result<(), String> {
    if !steamctl::is_running() {
        return Err("Steam is not running".into());
    }
    if !debug_ready() {
        return Err(format!("Steam has no debugging port: restart Steam with {FLAG}"));
    }
    // Only the shell window can host the dashboard: store and community pages
    // refuse a loopback iframe with their CSP, which reads as "content blocked".
    let target = main_target().ok_or_else(|| "the Steam menu is not injected yet".to_string())?;
    let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
    if ws.is_empty() {
        return Err("the Steam menu is not injected yet".into());
    }
    let script = format!(
        "window.__vdOpenDashboard ? (window.__vdOpenDashboard({route:?}), 'opened') : 'missing'"
    );
    let value = cdp::evaluate(ws, &script, std::time::Duration::from_secs(4))?;
    if value.as_str() != Some("opened") {
        return Err("the Steam menu is not injected yet".into());
    }
    Ok(())
}

/// Puts the bus into the page and one isolated world per enabled plugin into the
/// main window. A plugin that throws is reported and skipped; it cannot stop the
/// others or the page.
pub fn inject_plugins(ws: &str) -> Result<usize, String> {
    cdp::evaluate(ws, PLUGIN_BUS_JS, std::time::Duration::from_secs(5))?;
    let frontends = crate::plugins::frontends();
    let keep: Vec<String> = frontends.iter().map(|(plugin, _)| plugin.id.clone()).collect();
    let cleanup = format!(
        "(() => {{ const keep = new Set({}); document.querySelectorAll('#exxa-plugin-root > section[data-exxa-plugin]').forEach((node) => {{ if (!keep.has(node.getAttribute('data-exxa-plugin'))) node.remove(); }}); }})()",
        serde_json::to_string(&keep).unwrap_or_else(|_| "[]".into())
    );
    let _ = cdp::evaluate(ws, &cleanup, std::time::Duration::from_secs(5));
    let mut loaded = 0;
    for (plugin, source) in frontends {
        let body = format!(
            "{}\ntry {{\n{}\n}} catch (error) {{ console.error('[exxa] plugin {} failed', error); }}\n",
            plugin_bootstrap(&plugin),
            source,
            plugin.id
        );
        match cdp::evaluate_isolated(ws, &format!("exxa-{}", plugin.id), &body, std::time::Duration::from_secs(20)) {
            Ok(_) => loaded += 1,
            Err(error) => eprintln!("steamui: plugin {}: {error}", plugin.id),
        }
    }
    Ok(loaded)
}

/// Drains queued plugin calls, checks permissions in Rust and writes the answers
/// back. Runs on a slow tick; the page is never asked to trust plugin input.
pub fn bridge_tick() {
    if !steamctl::is_running() || !debug_ready() {
        return;
    }
    let Some(target) = main_target() else { return };
    let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
    if ws.is_empty() {
        return;
    }
    let drain = r#"(() => { const q = document.getElementById("exxa-queue"); if (!q) return "[]"; const text = q.textContent || "[]"; q.textContent = "[]"; return text; })()"#;
    let Ok(Value::String(raw)) = cdp::evaluate(ws, drain, std::time::Duration::from_secs(3)) else {
        return;
    };
    let calls: Vec<Value> = serde_json::from_str(&raw).unwrap_or_default();
    if calls.is_empty() {
        return;
    }
    let mut answers = serde_json::Map::new();
    for call in calls {
        let id = call.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        let plugin = call.get("plugin").and_then(Value::as_str).unwrap_or("");
        let method = call.get("method").and_then(Value::as_str).unwrap_or("");
        let params = call.get("params").cloned().unwrap_or_else(|| json!({}));
        if id.is_empty() {
            continue;
        }
        let answer = match crate::plugins::dispatch(plugin, method, &params) {
            Ok(value) => json!({ "ok": true, "value": value }),
            Err(error) => json!({ "ok": false, "value": error }),
        };
        answers.insert(id, answer);
    }
    if answers.is_empty() {
        return;
    }
    let add = Value::Object(answers).to_string();
    let expression = format!(
        r#"(() => {{ const box = document.getElementById("exxa-results"); if (!box) return 0; let map = {{}}; try {{ map = JSON.parse(box.textContent || "{{}}"); }} catch (error) {{ map = {{}}; }} Object.assign(map, {add}); box.textContent = JSON.stringify(map); return Object.keys(map).length; }})()"#
    );
    let _ = cdp::evaluate(ws, &expression, std::time::Duration::from_secs(3));
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
    if !main_ws.is_empty() {
        match inject_plugins(&main_ws) {
            Ok(count) => eprintln!("steamui: {count} plugin(s) loaded"),
            Err(error) => eprintln!("steamui: plugin injection: {error}"),
        }
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
const WRAPPER_MARK: &str = "X-Exxa-Steam-Wrapper=true";
const BACKUP_SUFFIX: &str = "exxatools.bak";

fn wrapper_exe() -> std::path::PathBuf {
    let launcher = paths::project_dir().join("exxa");
    if launcher.is_file() {
        launcher
    } else {
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("exxa"))
    }
}

fn launcher_backup(path: &std::path::Path) -> std::path::PathBuf {
    let name = format!("{}.{BACKUP_SUFFIX}", path.file_name().and_then(|value| value.to_str()).unwrap_or("steam.desktop"));
    path.with_file_name(name)
}

/// The command part changes, the arguments do not: `%U` stays `%U`, and every
/// `steam://` action keeps its target.
fn rewrite_steam_entry(text: &str, exe: &std::path::Path) -> String {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        if let Some(rest) = raw.strip_prefix("Exec=") {
            if let Some(index) = rest.find("/usr/bin/steam") {
                let tail = rest[index + "/usr/bin/steam".len()..].trim_start();
                let tail = tail.strip_prefix(FLAG).unwrap_or(tail).trim_start();
                let line = if tail.is_empty() {
                    format!("Exec={} steam", exe.display())
                } else {
                    format!("Exec={} steam {tail}", exe.display())
                };
                lines.push(line);
                continue;
            }
        }
        lines.push(raw.to_string());
    }
    if !lines.iter().any(|line| line == WRAPPER_MARK) {
        lines.push(WRAPPER_MARK.to_string());
    }
    format!("{}\n", lines.join("\n"))
}

/// Point every Steam desktop action at `exxa steam`, which starts the server and
/// then replaces itself with Steam. Arguments such as `%U` and `steam://...` are
/// kept verbatim; the wrapper sets LD_AUDIT and the debug flag itself.
pub fn install_launcher() -> Result<Value, String> {
    let path = paths::data_home().join("applications/steam.desktop");
    let stock = std::path::PathBuf::from("/usr/share/applications/steam.desktop");
    let backup = launcher_backup(&path);
    let original = if backup.is_file() {
        std::fs::read_to_string(&backup).map_err(|error| error.to_string())?
    } else if path.is_file() {
        std::fs::read_to_string(&path).map_err(|error| error.to_string())?
    } else if stock.is_file() {
        std::fs::read_to_string(&stock).map_err(|error| error.to_string())?
    } else {
        return Err("no Steam desktop entry to rewrite".into());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if !backup.is_file() {
        std::fs::write(&backup, &original).map_err(|error| format!("cannot back up {}: {error}", backup.display()))?;
    }
    let text = original;
    let exe = wrapper_exe();
    let updated = rewrite_steam_entry(&text, &exe);
    std::fs::write(&path, &updated).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(json!({ "path": path.display().to_string(), "backup": backup.display().to_string(), "exec": format!("{} steam", exe.display()) }))
}

pub fn uninstall_launcher() -> Value {
    let Some(path) = desktop_override() else {
        return json!({ "restored": false, "error": "no local Steam desktop entry" });
    };
    let backup = launcher_backup(&path);
    if !backup.is_file() {
        return json!({ "restored": false, "path": path.display().to_string(), "error": "no backup to restore" });
    }
    let backed_up = std::fs::read_to_string(&backup).unwrap_or_default();
    // The local override is ours when it carries the Vapor Forge marker. Removing
    // it lets Steam fall back to the stock entry instead of restoring our own file.
    let restored = if backed_up.contains(DESKTOP_MARK) {
        let removed = std::fs::remove_file(&path).is_ok();
        let _ = std::fs::remove_file(&backup);
        removed
    } else {
        let copied = std::fs::copy(&backup, &path).is_ok();
        if copied {
            let _ = std::fs::remove_file(&backup);
        }
        copied
    };
    json!({ "restored": restored, "path": path.display().to_string(), "stock": restored })
}

fn launcher_uses_wrapper(text: &str) -> bool {
    text.lines().any(|line| line.contains(&format!("{} steam", wrapper_exe().display())))
        || text.contains(WRAPPER_MARK)
}



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
    start_pump();
    static BRIDGE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !BRIDGE.swap(true, Ordering::Relaxed) {
        std::thread::spawn(|| loop {
            std::thread::sleep(std::time::Duration::from_millis(900));
            bridge_tick();
        });
    }
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

    #[test]
    fn wrapper_keeps_placeholders_and_actions() {
        let entry = "[Desktop Entry]\nExec=env \"LD_AUDIT=/x\" /usr/bin/steam -cef-enable-debugging %U\n"
            .to_string()
            + "Exec=env \"LD_AUDIT=/x\" /usr/bin/steam steam://store\n";
        let out = super::rewrite_steam_entry(&entry, std::path::Path::new("/opt/exxa"));
        assert!(out.contains("Exec=/opt/exxa steam %U"));
        assert!(out.contains("Exec=/opt/exxa steam steam://store"));
        assert!(!out.contains("/usr/bin/steam"));
        assert!(out.contains(super::WRAPPER_MARK));
    }

    #[test]
    fn wrapper_ignores_entries_that_do_not_run_steam() {
        let entry = "Exec=/usr/bin/other %U\n";
        let out = super::rewrite_steam_entry(entry, std::path::Path::new("/opt/exxa"));
        assert!(out.contains("Exec=/usr/bin/other %U"));
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
