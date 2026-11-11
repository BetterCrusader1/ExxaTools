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
    let result = cdp::evaluate(ws, &script(), std::time::Duration::from_secs(20))?;
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
    let Some(target) = main_target() else {
        return json!({ "injected": false, "removed": false });
    };
    let ws = target.get("webSocketDebuggerUrl").and_then(Value::as_str).unwrap_or("");
    let expression = format!(
        r#"(() => {{
          const root = document.getElementById({ROOT_ID:?});
          const panel = document.getElementById({PANEL_ID:?});
          const style = document.getElementById({ROOT_ID:?} + "-style");
          if (root) root.remove();
          if (panel) panel.remove();
          if (style) style.remove();
          return true;
        }})()"#
    );
    let removed = cdp::evaluate(ws, &expression, std::time::Duration::from_secs(10))
        .map(|value| value.as_bool().unwrap_or(false))
        .unwrap_or(false);
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
        if !steamctl::is_running() || !debug_ready() {
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
