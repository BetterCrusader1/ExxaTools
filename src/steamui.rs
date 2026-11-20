//! Inject our icon and menu into the Steam client's own menu bar.
//!
//! Clone the Help menu item, swap its contents for our mark, and let it open the
//! dashboard. A MutationObserver puts it back when Steam re-renders.

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
    let Ok(raw) = std::fs::read(path) else {
        return String::new();
    };
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
        .replace("__ROOT__", ROOT_ID)
        .replace("__ICON__", ICON_ID)
        .replace(
            "ICON_ICON",
            &serde_json::to_string(&icon_markup()).unwrap_or_else(|_| "\"\"".into()),
        )
}

fn page_targets() -> Vec<Value> {
    cdp::targets(DEBUG_PORT)
        .into_iter()
        .filter(|target| target.get("type").and_then(Value::as_str) == Some("page"))
        .filter(|target| {
            // Skip the dashboard's own frames: injecting there would nest our
            // mark inside the dashboard and answer its own requests.
            let url = target.get("url").and_then(Value::as_str).unwrap_or("");
            !url.contains("127.0.0.1:")
        })
        .collect()
}

fn evaluate_on(
    target: &Value,
    expression: &str,
    timeout: std::time::Duration,
) -> Result<Value, String> {
    let ws = target
        .get("webSocketDebuggerUrl")
        .and_then(Value::as_str)
        .unwrap_or("");
    if ws.is_empty() {
        return Err("target has no websocket url".into());
    }
    cdp::evaluate(ws, expression, timeout)
}

static PUMP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn open_dashboard_page(page: &str) -> Result<(), String> {
    let url = if page.is_empty() {
        format!("http://127.0.0.1:{}/", crate::server::port())
    } else {
        format!("http://127.0.0.1:{}/?page={page}", crate::server::port())
    };
    crate::desktop::open_window(&url)
}

fn run_action(action: &str) {
    // The only live trigger is the Help-menu icon, which opens the full app.
    // The retired panel's install/play/sync/fix/restart arms are gone with it.
    if matches!(action, "open" | "dashboard") {
        let _ = open_dashboard_page("");
    }
}

/// Steam's pages block loopback requests with their CSP, so the injected script
/// has no network at all: the pump reads the icon's clicks and the store-page
/// actions over CDP and turns them into Rust calls. One pump per process.
pub fn start_pump() {
    if !PUMP.swap(true, std::sync::atomic::Ordering::Relaxed) {
        std::thread::spawn(pump);
    }
}

/// Answers the inline store-page actions: a probe reports whether the app is in
/// the library, Add fetches a plan, Restart applies it, Sync refreshes manifests.
/// Navigation actions (opening Settings or the dashboard) must not hide the
/// library affordances: Add/Sync stay exactly as the library state dictates.
fn page_affordances(planned: bool, setup_needed: bool) -> (bool, bool, bool) {
    (!planned, planned, setup_needed)
}

/// Failures the user fixes in Settings (missing/expired key, no source, rate
/// limit) should surface the Setup source action next to the error.
fn error_needs_setup(error: &str) -> bool {
    error.contains("plan source")
        || error.contains("API key")
        || error.contains("daily limit")
        || error.contains("ryuu")
}

fn handle_page_request(ws: &str, request: &Value) {
    let appid = request.get("appid").and_then(Value::as_u64).unwrap_or(0);
    if appid == 0 {
        return;
    }
    let want = request.get("want").and_then(Value::as_str).unwrap_or("");
    let name = crate::steam::catalog(appid)
        .ok()
        .and_then(|catalog| {
            catalog
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| format!("App {appid}"));
    let state = crate::state::load().ok();
    let planned = state
        .as_ref()
        .map(|state| state.apps.contains_key(&appid.to_string()))
        .unwrap_or(false);
    let setup_needed = state
        .as_ref()
        .map(|state| crate::plans::sources(&state.settings).is_empty())
        .unwrap_or(false);
    let (status, can_add, can_sync, can_setup, can_restart, error) = match want {
        "dashboard" => {
            let message = match open_dashboard_page("") {
                Ok(()) => "Dashboard opened".to_string(),
                Err(error) => error,
            };
            (message, !planned, planned, setup_needed, false, false)
        }
        "settings" => {
            let message = match open_dashboard_page("settings") {
                Ok(()) => "Settings opened".to_string(),
                Err(error) => error,
            };
            (message, !planned, planned, setup_needed, false, false)
        }
        "launch" => {
            let message = match crate::desktop::spawn_reaped(
                std::process::Command::new("steam").arg(format!("steam://rungameid/{appid}")),
            ) {
                Ok(_) => format!("Launching {name}"),
                Err(error) => format!("Steam did not open: {error}"),
            };
            (message, !planned, planned, setup_needed, false, false)
        }
        "restart" => match crate::steamctl::restart(&[]) {
            Ok(_) => (
                "Steam restarted".to_string(),
                false,
                true,
                false,
                false,
                false,
            ),
            Err(error) => (error, false, true, false, true, true),
        },
        "fix" => {
            match crate::appmanifest::patch_with_restart(appid, &crate::appmanifest::Mode::Fix) {
                Ok(_) => (
                    format!("{name} update flags repaired; Steam restarted"),
                    false,
                    true,
                    false,
                    false,
                    false,
                ),
                Err(error) => (error, false, true, false, false, true),
            }
        }
        "add" => match crate::plans::fetch_plan(appid, Some(&name)) {
            Ok(_) => (
                format!("{name} added — restart Steam to install it"),
                false,
                true,
                false,
                true,
                false,
            ),
            Err(error) => {
                let setup = setup_needed || error_needs_setup(&error);
                (error, true, planned, setup, false, true)
            }
        },
        "sync" => match crate::plans::refresh_plan(appid) {
            Ok(_) => (
                format!("{name} manifests synced"),
                false,
                true,
                false,
                false,
                false,
            ),
            Err(error) => (error, false, true, false, false, true),
        },
        _ => {
            let (can_add, can_sync, can_setup) = page_affordances(planned, setup_needed);
            (
                if planned {
                    "In your library. Sync to pick up the newest manifests.".to_string()
                } else {
                    "Not in your library yet.".to_string()
                },
                can_add,
                can_sync,
                can_setup,
                false,
                false,
            )
        }
    };
    let payload = json!({
        "appid": appid,
        "name": name,
        "status": status,
        "can_add": can_add,
        "can_sync": can_sync,
        "can_setup": can_setup,
        "can_restart": can_restart,
        "error": error,
        "can_launch": planned,
        "can_fix": planned && crate::appmanifest::manifest_path(appid).is_some(),
    });
    let expression = format!("window.__vdPageState = {payload};");
    let _ = cdp::evaluate(ws, &expression, std::time::Duration::from_secs(2));
}

fn pump() {
    let menu_script = script();
    let expected_build = script_hash(&menu_script);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(400));
        if !steamctl::is_running() || !debug_ready() {
            continue;
        }
        let Some(target) = main_target() else {
            continue;
        };
        let ws = target
            .get("webSocketDebuggerUrl")
            .and_then(Value::as_str)
            .unwrap_or("");
        if ws.is_empty() {
            continue;
        }
        for target in page_targets() {
            let page_ws = target
                .get("webSocketDebuggerUrl")
                .and_then(Value::as_str)
                .unwrap_or("");
            if page_ws.is_empty() {
                continue;
            }
            let batch = cdp::evaluate(
                page_ws,
                r#"(() => {
                  const batch = {
                    build: window.__vdBuild || '',
                    page: window.__vdPageRequest || null,
                    action: window.__vdAction || ''
                  };
                  if (batch.page) window.__vdPageRequest = null;
                  if (batch.action) window.__vdAction = '';
                  return JSON.stringify(batch);
                })()"#,
                std::time::Duration::from_millis(700),
            )
            .ok()
            .and_then(|value| {
                value
                    .as_str()
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
            })
            .unwrap_or_else(|| json!({}));
            if batch.get("build").and_then(Value::as_str) != Some(expected_build.as_str()) {
                // A CEF navigation creates a fresh JS context. Reinject in the
                // pump itself instead of leaving the icon absent until the
                // slower filesystem watcher notices.
                let bootstrap = format!(
                    "window.__vdNoIcon = {};\n{}\nwindow.__vdBuild = {expected_build:?};\nwindow.__vdInject();",
                    page_ws != ws, menu_script
                );
                let _ = cdp::evaluate(page_ws, &bootstrap, std::time::Duration::from_secs(4));
                continue;
            }
            if let Some(request) = batch.get("page").filter(|value| value.is_object()) {
                handle_page_request(page_ws, request);
            }
            let action = batch.get("action").and_then(Value::as_str).unwrap_or("");
            if !action.is_empty() {
                run_action(action);
            }
        }
    }
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
        let ws = target
            .get("webSocketDebuggerUrl")
            .and_then(Value::as_str)
            .unwrap_or("");
        match cdp::evaluate(
            ws,
            &format!("Boolean(document.getElementById({ROOT_ID:?}))"),
            std::time::Duration::from_secs(6),
        ) {
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
    let server = crate::http::get_json(&format!(
        "http://127.0.0.1:{}/api/session",
        crate::server::port()
    ))
    .is_ok();
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

/// Puts the bus into the page and one isolated world per enabled plugin into the
/// main window. A plugin that throws is reported and skipped; it cannot stop the
/// others or the page.
pub fn inject_plugins(ws: &str) -> Result<usize, String> {
    cdp::evaluate(ws, PLUGIN_BUS_JS, std::time::Duration::from_secs(5))?;
    let frontends = crate::plugins::frontends();
    let keep: Vec<String> = frontends
        .iter()
        .map(|(plugin, _)| plugin.id.clone())
        .collect();
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
        match cdp::evaluate_isolated(
            ws,
            &format!("exxa-{}", plugin.id),
            &body,
            std::time::Duration::from_secs(20),
        ) {
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
    let ws = target
        .get("webSocketDebuggerUrl")
        .and_then(Value::as_str)
        .unwrap_or("");
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
        let id = call
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
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

fn injection_current(expected: &str) -> bool {
    let targets = page_targets();
    !targets.is_empty()
        && targets.iter().all(|target| {
            let ws = target
                .get("webSocketDebuggerUrl")
                .and_then(Value::as_str)
                .unwrap_or("");
            !ws.is_empty()
                && cdp::evaluate(
                    ws,
                    &format!("window.__vdBuild === {expected:?}"),
                    std::time::Duration::from_secs(2),
                )
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
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
    let script = script();
    let build = script_hash(&script);
    let plain = format!("{script}\nwindow.__vdBuild = {build:?};\nwindow.__vdInject();");
    let main_ws = main_target()
        .and_then(|target| {
            target
                .get("webSocketDebuggerUrl")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    // Every page window gets the current script: the store pages carry the inline
    // actions, and they are not the shell that carries the menu bar.
    let mut text = String::new();
    let mut reached = 0;
    for target in page_targets() {
        let ws = target
            .get("webSocketDebuggerUrl")
            .and_then(Value::as_str)
            .unwrap_or("");
        if ws.is_empty() {
            continue;
        }
        // Only the window that owns the menu bar gets the icon; every other page
        // window runs icon-less, or the icon would appear more than once.
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
            if text.is_empty() {
                "no page window answered"
            } else {
                &text
            }
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

/// The Steam launcher runs the installed companion, so starting Steam never
/// depends on the source tree or a cargo build.
fn wrapper_exe() -> std::path::PathBuf {
    crate::steamctl::install_companion().unwrap_or_else(|_| crate::paths::companion_bin())
}

fn launcher_backup(path: &std::path::Path) -> std::path::PathBuf {
    let name = format!(
        "{}.{BACKUP_SUFFIX}",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("steam.desktop")
    );
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
        std::fs::write(&backup, &original)
            .map_err(|error| format!("cannot back up {}: {error}", backup.display()))?;
    }
    let text = original;
    let exe = wrapper_exe();
    let updated = rewrite_steam_entry(&text, &exe);
    std::fs::write(&path, &updated)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(
        json!({ "path": path.display().to_string(), "backup": backup.display().to_string(), "exec": format!("{} steam", exe.display()) }),
    )
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
    text.lines()
        .any(|line| line.contains(&format!("{} steam", wrapper_exe().display())))
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
    for path in [
        desktop_override(),
        Some(PathBuf::from("/usr/share/applications/steam.desktop")),
    ]
    .into_iter()
    .flatten()
    {
        if !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains("Exec=") {
            continue;
        }
        if path.file_name().and_then(|value| value.to_str()) == Some("steam.desktop")
            && !text.contains(DESKTOP_MARK)
        {
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
                // launcher entry and Steam's own marker are repaired so the next
                // start works, however Steam gets started.
                let _ = enable_debug_if_possible();
                let _ = steamctl::ensure_debug_marker();
                std::thread::sleep(std::time::Duration::from_secs(10));
                continue;
            }
            let expected = script_hash(&script());
            let injected = status()
                .get("injected")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && injection_current(&expected);
            if !injected {
                match inject() {
                    Ok(_) => eprintln!("steamui: menu injected"),
                    Err(error) => eprintln!("steamui: inject failed: {error}"),
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(if injected {
                20
            } else {
                3
            }));
        }
    });
}

/// Writes the flag into the launcher without restarting anything.
fn enable_debug_if_possible() -> Result<(), String> {
    for path in [
        desktop_override(),
        Some(PathBuf::from("/usr/share/applications/steam.desktop")),
    ]
    .into_iter()
    .flatten()
    {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains("Exec=") {
            continue;
        }
        if path.file_name().and_then(|value| value.to_str()) == Some("steam.desktop")
            && !text.contains(DESKTOP_MARK)
        {
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
        let entry =
            "[Desktop Entry]\nExec=env \"LD_AUDIT=/x\" /usr/bin/steam -cef-enable-debugging %U\n"
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
mod store_tests {
    use super::MENU_SCRIPT;

    #[test]
    fn store_actions_are_inline_not_a_fixed_card() {
        assert!(MENU_SCRIPT.contains("vd-store-actions"));
        assert!(MENU_SCRIPT.contains("exxa-store-btn"));
        assert!(MENU_SCRIPT.contains("Restart Steam"));
        assert!(MENU_SCRIPT.contains("vd-app-card"));
        assert!(!MENU_SCRIPT.contains("#vd-app-card { position: fixed"));
    }

    #[test]
    fn store_actions_use_steam_font_and_exxa_accent() {
        assert!(MENU_SCRIPT.contains("\"Motiva Sans\""));
        assert!(MENU_SCRIPT.contains("#8b5cf6"));
        assert!(MENU_SCRIPT.contains("#a78bfa"));
    }
}

#[cfg(test)]
mod page_tests {
    use super::{error_needs_setup, page_affordances};

    #[test]
    fn navigation_keeps_add_for_games_outside_the_library() {
        assert_eq!(page_affordances(false, false), (true, false, false));
        assert_eq!(page_affordances(false, true), (true, false, true));
    }

    #[test]
    fn planned_games_keep_sync_instead_of_add() {
        assert_eq!(page_affordances(true, false), (false, true, false));
        assert_eq!(page_affordances(true, true), (false, true, true));
    }

    #[test]
    fn key_and_source_failures_offer_setup() {
        assert!(error_needs_setup(
            "no plan source configured: add a Hubcap or Ryuu key in Settings"
        ));
        assert!(error_needs_setup("Hubcap rejected the API key"));
        assert!(error_needs_setup("no Hubcap API key set"));
        assert!(error_needs_setup("Hubcap daily limit reached"));
        assert!(error_needs_setup(
            "ryuu answered with an HTML page (login or challenge)"
        ));
        assert!(!error_needs_setup(
            "app 5196330 is not in the Hubcap database"
        ));
        assert!(!error_needs_setup("cannot reach Hubcap: timeout"));
    }
}
