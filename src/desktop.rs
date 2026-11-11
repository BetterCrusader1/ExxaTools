//! Window and desktop integration without a GUI toolkit: the UI is the web page,
//! the window is whatever browser the user has, and the launcher entry points at
//! the same binary.

use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::paths;

fn port_answers(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

fn spawn_open(url: &str) -> Result<(), String> {
    let mut command = Command::new("xdg-open");
    crate::steamctl::apply_session_env(&mut command);
    command
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot open {url}: {error}"))
}

/// Open the dashboard, starting the server first if nothing answers.
pub fn launch_app() -> Value {
    let port = 8765u16;
    let url = format!("http://127.0.0.1:{port}/");
    if port_answers(port) {
        let _ = spawn_open(&url);
        return json!({ "started": false, "opened": url, "running": true });
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let spawned = Command::new(&exe)
        .args(["serve", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok();
    if !spawned {
        return json!({ "started": false, "opened": Value::Null, "error": "cannot start the server" });
    }
    std::thread::sleep(std::time::Duration::from_millis(700));
    let opened = spawn_open(&url).is_ok();
    json!({ "started": true, "opened": if opened { json!(url) } else { Value::Null }, "running": true })
}

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
    let exe = if launcher.is_file() { launcher } else { std::env::current_exe().map_err(|error| error.to_string())? };
    let icon = paths::web_dir().join("logo.png");
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=ExxaTools\nComment=Steam depots and manifests on Vapor Forge\nExec={} serve --window\nIcon={}\nTerminal=false\nCategories=Game;Utility;\n",
        exe.display(),
        icon.display()
    );
    std::fs::write(&target, content).map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    Ok(json!({ "path": target.display().to_string(), "exec": format!("{} serve --window", exe.display()) }))
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
    let exe = if launcher.is_file() { launcher } else { std::env::current_exe().map_err(|error| error.to_string())? };
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=ExxaTools\nComment=Steam depots and manifests on Vapor Forge\nExec={} serve --port 8765\nIcon={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exe.display(),
        paths::web_dir().join("logo.png").display()
    );
    std::fs::write(&target, content).map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    Ok(json!({ "path": target.display().to_string(), "enabled": true }))
}

pub fn remove_autostart() -> Value {
    let target = autostart_path();
    let removed = target.exists() && std::fs::remove_file(&target).is_ok();
    json!({ "path": target.display().to_string(), "removed": removed })
}

/// `serve --window` has no toolkit: start the server and hand the URL to the
/// browser, which is the same page the GTK helper shows.
pub fn open_window(url: &str) -> Result<(), String> {
    spawn_open(url)
}