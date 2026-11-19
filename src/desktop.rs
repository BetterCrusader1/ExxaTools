//! Desktop integration: the dashboard opens in the GTK window defined in
//! `window.rs`, and the launcher entry points at this binary so the same window
//! starts from the menu.

use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::paths;

pub fn install_desktop_entry() -> Result<Value, String> {
    let legacy = paths::data_home().join("applications/vapordepot.desktop");
    if legacy.exists() {
        let _ = std::fs::remove_file(legacy);
    }
    let target = paths::data_home().join("applications/exxatools.desktop");
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let launcher = paths::project_dir().join("exxa");
    let exe = if launcher.is_file() {
        launcher
    } else {
        paths::executable()?
    };
    let icon = paths::web_dir().join("logo.png");
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=ExxaTools\nComment=Steam depots and manifests on Vapor Forge\nExec={} window\nIcon={}\nStartupWMClass=exxa\nTerminal=false\nCategories=Game;Utility;\n",
        exe.display(),
        icon.display()
    );
    std::fs::write(&target, content)
        .map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    Ok(json!({ "path": target.display().to_string(), "exec": format!("{} window", exe.display()) }))
}

pub fn uninstall_desktop_entry() -> Value {
    let target = paths::data_home().join("applications/exxatools.desktop");
    let removed = target.exists() && std::fs::remove_file(&target).is_ok();
    json!({ "removed": removed, "path": target.display().to_string() })
}

pub fn autostart_path() -> std::path::PathBuf {
    paths::config_home().join("autostart/exxatools.desktop")
}

pub fn install_autostart() -> Result<Value, String> {
    let target = autostart_path();
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let launcher = paths::project_dir().join("exxa");
    let exe = if launcher.is_file() {
        launcher
    } else {
        paths::executable()?
    };
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=ExxaTools\nComment=Steam depots and manifests on Vapor Forge\nExec={} serve --port 8765\nIcon={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exe.display(),
        paths::web_dir().join("logo.png").display()
    );
    std::fs::write(&target, content)
        .map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    Ok(json!({ "path": target.display().to_string(), "enabled": true }))
}

pub fn remove_autostart() -> Value {
    let target = autostart_path();
    let removed = target.exists() && std::fs::remove_file(&target).is_ok();
    json!({ "path": target.display().to_string(), "removed": removed })
}

/// Opens the dashboard as a real window. The server keeps running on its own, so
/// closing the window leaves the Steam integration alive.
pub fn open_window(url: &str) -> Result<(), String> {
    let exe = paths::executable()?;
    let mut command = Command::new(exe);
    crate::steamctl::apply_session_env(&mut command);
    command
        .args(["window", "--url", url])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot open a window: {error}"))
}
