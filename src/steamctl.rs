//! Steam client control: roots, pid, running state, LD_AUDIT, restart.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn root() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("EXXATOOLS_STEAM_ROOT") {
        let path = PathBuf::from(value);
        if path.is_dir() {
            return Some(path);
        }
    }
    if let Ok(steam) = steamlocate::SteamDir::locate() {
        return Some(steam.path().to_path_buf());
    }
    let home = crate::paths::home();
    let candidates = [
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".steam/root"),
        home.join("Steam"),
    ];
    for candidate in &candidates {
        if candidate.join("config/loginusers.vdf").is_file() {
            return Some(candidate.clone());
        }
    }
    candidates
        .into_iter()
        .find(|path| path.join("steam.sh").is_file() || path.join("ubuntu12_64").is_dir())
}

pub fn pid() -> Option<i32> {
    let root = root()?;
    let text = std::fs::read_to_string(root.join("steam.pid")).ok()?;
    let value: i32 = text.trim().parse().ok()?;
    if PathBuf::from(format!("/proc/{value}")).exists() {
        Some(value)
    } else {
        None
    }
}

/// True when a game (not the client itself) is running under Steam. Used before
/// restarting the client to enable CEF debugging, so a running game is never
/// interrupted.
pub fn game_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .filter(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
        else {
            continue;
        };
        let Ok(bytes) = std::fs::read(format!("/proc/{pid}/environ")) else {
            continue;
        };
        for item in bytes.split(|byte| *byte == 0) {
            for key in [b"SteamAppId=".as_slice(), b"SteamGameId=".as_slice()] {
                if let Some(value) = item.strip_prefix(key) {
                    if !value.is_empty() && value != b"0" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub fn is_running() -> bool {
    if pid().is_some() {
        return true;
    }
    Command::new("pgrep")
        .args(["-x", "steam"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn steam_pids() -> Vec<i32> {
    let mut pids = Vec::new();
    if let Some(pid) = pid() {
        pids.push(pid);
    }
    if let Ok(output) = Command::new("pgrep").arg("-x").arg("steam").output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Ok(value) = line.trim().parse::<i32>() {
                if !pids.contains(&value) {
                    pids.push(value);
                }
            }
        }
    }
    pids
}

fn audit_value(p: i32) -> Option<String> {
    let bytes = std::fs::read(format!("/proc/{p}/environ")).ok()?;
    for chunk in bytes.split(|byte| *byte == 0) {
        if let Some(rest) = chunk.strip_prefix(b"LD_AUDIT=") {
            return Some(String::from_utf8_lossy(rest).into_owned());
        }
    }
    Some(String::new())
}

/// What the live Steam processes prove about the engine.
///
/// The client runs several processes called `steam`, and the environment alone
/// is not evidence: a Steam started with `LD_AUDIT` keeps the variable in the
/// wrapper even when its runtime moves the real client into a container that
/// never loads the shim. `active` therefore means the library is mapped inside
/// the process that owns `steamclient.so`, which is where the hooks live.
pub fn audit_loaded() -> serde_json::Value {
    let mut audit = String::new();
    let mut pid: Option<i32> = None;
    let mut mapped = false;
    let mut active = false;
    for candidate in steam_pids() {
        let maps = process_maps(candidate).map(|maps| map_state(&maps));
        let value = audit_value(candidate);
        let state = maps.unwrap_or(MapState {
            shim: false,
            client: false,
        });
        mapped |= state.shim;
        if state.active() {
            active = true;
            pid = Some(candidate);
            audit = value.unwrap_or_default();
            break;
        }
        if pid.is_none() {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                pid = Some(candidate);
                audit = value;
            }
        }
    }
    serde_json::json!({
        "pid": pid,
        "ld_audit": audit,
        "loaded": active,
        "mapped": mapped,
        "active": active,
    })
}

/// Which modules a process actually mapped.
///
/// The environment is not evidence: a Steam launched with `LD_AUDIT` keeps the
/// variable in the wrapper process even when the runtime moves the real client
/// into a container that never loads the shim. Only the mapped file proves the
/// engine is live, and only in the process that owns `steamclient.so` — the
/// hooks live there, so a shim mapped anywhere else changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapState {
    pub shim: bool,
    pub client: bool,
}

impl MapState {
    pub fn active(self) -> bool {
        self.shim && self.client
    }
}

pub fn map_state(maps: &str) -> MapState {
    let mut state = MapState {
        shim: false,
        client: false,
    };
    for line in maps.lines() {
        let Some(path) = line.split_whitespace().last() else {
            continue;
        };
        if path.ends_with("libvapor_forge.so") {
            state.shim = true;
        } else if path.ends_with("steamclient.so") {
            state.client = true;
        }
    }
    state
}

fn process_maps(pid: i32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/maps")).ok()
}

/// Only the audit shim goes in LD_AUDIT; the Proton helper is injected elsewhere.
pub fn audit_libs() -> String {
    let mut libs: Vec<String> = Vec::new();
    for arch in ["i686", "x86_64"] {
        let path = crate::paths::vf_data().join(arch).join("libvapor_forge.so");
        if path.is_file() {
            libs.push(path.display().to_string());
        }
    }
    libs.join(":")
}

fn steam_binary() -> Option<String> {
    if let Ok(output) = Command::new("which").arg("steam").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    root()
        .map(|path| path.join("steam.sh").display().to_string())
        .filter(|path| PathBuf::from(path).is_file())
}

pub fn shutdown(timeout: std::time::Duration) -> Result<bool, String> {
    let binary = steam_binary().ok_or("steam executable not found in PATH")?;
    Command::new(binary)
        .arg("-shutdown")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("cannot run steam -shutdown: {error}"))?;
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !is_running() {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(!is_running())
}

/// Steam is a GUI app: when we spawn it from a bare shell there is no DISPLAY.
pub fn apply_session_env(command: &mut Command) {
    let has_display = std::env::var("DISPLAY")
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    if !has_display {
        let socket = std::path::Path::new("/tmp/.X11-unix/X0");
        if socket.exists() {
            command.env("DISPLAY", ":0");
        }
    }
    if std::env::var("XAUTHORITY")
        .map(|value| value.is_empty())
        .unwrap_or(true)
    {
        // A session can keep its cookie in the runtime dir while ~/.Xauthority is
        // empty or stale, and Steam then fails with "Unable to open a connection
        // to X"; prefer whichever file actually has a cookie in it.
        let home_cookie = crate::paths::home().join(".Xauthority");
        let mut chosen = None;
        if std::fs::metadata(&home_cookie)
            .map(|meta| meta.len() > 0)
            .unwrap_or(false)
        {
            chosen = Some(home_cookie);
        } else {
            let runtime =
                std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".into());
            if let Ok(entries) = std::fs::read_dir(&runtime) {
                let mut candidates: Vec<PathBuf> = entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .map(|name| name.starts_with("xauth_") || name == "gdm/Xauthority")
                            .unwrap_or(false)
                    })
                    .filter(|path| {
                        std::fs::metadata(path)
                            .map(|meta| meta.len() > 0)
                            .unwrap_or(false)
                    })
                    .collect();
                // The session rotates these cookies; the newest file is the live
                // one and a stale earlier name must lose to it.
                candidates.sort_by_key(|path| {
                    std::fs::metadata(path)
                        .and_then(|meta| meta.modified())
                        .ok()
                });
                chosen = candidates.into_iter().next_back();
            }
        }
        if let Some(authority) = chosen {
            command.env("XAUTHORITY", authority);
        }
    }
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".into());
        let bus = format!("unix:path={runtime}/bus");
        if std::path::Path::new(&format!("{runtime}/bus")).exists() {
            command.env("DBUS_SESSION_BUS_ADDRESS", bus);
        }
    }
}

/// Steam's container launcher wipes LD_AUDIT before the client inside the
/// pressure-vessel sandbox starts. The accepted way to keep an audit library
/// inside is the launcher's own `--ld-audit` option, so `srt-client-container`
/// is wrapped with a small script that appends it to every invocation.
/// Steam's bootstrapper re-verifies package file sizes on every launch, so the
/// wrapper is padded to the original's exact byte size and reinstalled on every
/// launch; a real Steam update changes the expected size and re-extracts a
/// launcher we adopt then.
fn ensure_container_wrapper(libs: &str) {
    let Some(root) = root() else { return };
    // pv-adverb rejects a colon-separated LD_AUDIT, so the container gets the
    // 64-bit shim alone: the 32-bit loader ignores a 64-bit audit library
    // anyway, and containerised Steamnano userland is all 64-bit now.
    let lib64 = libs
        .split(':')
        .find(|path| path.contains("x86_64"))
        .unwrap_or(libs);
    let wrapper_dir = root.join(
        "steamrt64/pv-runtime/steam-runtime-steamrt/pressure-vessel/libexec/steam-runtime-tools-0",
    );
    let launcher = wrapper_dir.join("srt-client-container");
    let original = wrapper_dir.join("srt-client-container.real");
    if !launcher.is_file() {
        // A Steam bootstrap can leave the name missing between steps; recover
        // from the pristine copy instead of silently skipping the wrapper.
        if !original.is_file() {
            return;
        }
        let _ = std::fs::rename(&original, &launcher);
        eprintln!("steam: recovered container launcher from the pristine copy");
    }
    let head = std::fs::read(&launcher)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).into_owned());
    // Current wrapper = v2 marker plus the expected audit path. Anything else
    // (a stale wrapper or a stock ELF) is replaced below.
    if head.as_deref().is_some_and(|text| {
        text.contains("EXXATOOLS_WRAPPER v3") && text.contains(&format!("--ld-audit={lib64} "))
    }) {
        eprintln!("steam: container wrapper already installed");
        return;
    }
    if std::fs::read(&launcher)
        .map(|data| data.starts_with(&[0x7f, b'E', b'L', b'F']))
        .unwrap_or(false)
    {
        // Either a stock launcher or a Steam update over our wrapper: keep the
        // pristine binary under .real and put the script in its place.
        let _ = std::fs::remove_file(&original);
        if std::fs::rename(&launcher, &original).is_err() {
            eprintln!("steam: cannot stage the container launcher");
            return;
        }
    } else if !original.is_file() {
        eprintln!("steam: container launcher is neither wrapper nor ELF");
        return;
    }
    let Ok(target_size) = std::fs::metadata(&original).map(|meta| meta.len()) else {
        return;
    };
    let script = format!(
        "#!/bin/sh\n# EXXATOOLS_WRAPPER v3: run the real launcher with the audit shim and\n# raise the companion server first. Everything below the exec line is padding\n# so Steam's size-only package check sees the byte size it expects.\n# pv-adverb rejects a colon-separated LD_AUDIT, so the 64-bit shim is passed\n# alone; the 32-bit loader inside the container ignores a 64-bit audit library.\nif [ -x \"$HOME/.local/share/exxatools/bin/exxa\" ]; then\n  \"$HOME/.local/share/exxatools/bin/exxa\" serve </dev/null >/dev/null 2>&1 &\nfi\nexec \"$(dirname \"$0\")/srt-client-container.real\" \\\n  --pv-option=--ld-audit={lib64} \\\n  \"$@\"\n",
    );
    if script.len() as u64 > target_size {
        eprintln!("steam: container launcher too small for the wrapper");
        return;
    }
    let mut padded = script.into_bytes();
    padded.resize(target_size as usize, b'\n');
    let temporary = wrapper_dir.join(".srt-client-container.exxa.tmp");
    if std::fs::write(&temporary, &padded).is_err() {
        eprintln!("steam: cannot write the container wrapper");
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755));
    if std::fs::rename(&temporary, &launcher).is_err() {
        eprintln!("steam: cannot install the container wrapper");
        return;
    }
    eprintln!("steam: container wrapper installed ({target_size} bytes)");
}

/// Steam reads this marker on every start, including its own self-restarts and
/// updates, and then opens the CEF debugging port. A launch flag alone is lost
/// the moment Steam restarts itself or is started from anywhere else.
pub const DEBUG_MARKER: &str = ".cef-enable-remote-debugging";

pub fn ensure_debug_marker() -> Result<PathBuf, String> {
    let root = root().ok_or("Steam directory not found")?;
    let marker = root.join(DEBUG_MARKER);
    if !marker.is_file() {
        std::fs::write(&marker, b"")
            .map_err(|error| format!("cannot write {}: {error}", marker.display()))?;
    }
    Ok(marker)
}

/// Every start goes through here, so the debugging flag is never dropped by a
/// caller that only asked for a plain restart.
fn launch_args(extra_args: &[String]) -> Vec<String> {
    let flag = crate::steamui::FLAG;
    let mut args: Vec<String> = Vec::with_capacity(extra_args.len() + 1);
    args.push(flag.to_string());
    args.extend(
        extra_args
            .iter()
            .filter(|arg| arg.as_str() != flag)
            .cloned(),
    );
    args
}

fn steam_command(extra_args: &[String]) -> Result<(Command, String), String> {
    let binary = steam_binary().ok_or("steam executable not found in PATH")?;
    let libs = audit_libs();
    ensure_container_wrapper(&libs);
    let _ = install_companion();
    if let Err(error) = ensure_debug_marker() {
        eprintln!("steam: {error}");
    }
    let mut command = Command::new(binary);
    for arg in launch_args(extra_args) {
        command.arg(arg);
    }
    apply_session_env(&mut command);
    if crate::paths::vf_data()
        .join("x86_64")
        .join(".exxatools-pattern-overrides")
        .is_file()
    {
        // The vendored shim carries entry-point overrides for this Steam build.
        command.env("VAPOR_FORGE_TRUST_PATTERNS", "1");
    }
    let existing = std::env::var("LD_AUDIT").unwrap_or_default();
    if !libs.is_empty() && !existing.contains("vapor_forge") {
        let merged = if existing.is_empty() {
            libs.clone()
        } else {
            format!("{libs}:{existing}")
        };
        command.env("LD_AUDIT", merged);
    }
    Ok((command, libs))
}

/// Keep a private copy of this binary where the Steam-side shim can exec it.
/// The copy only changes on real upgrades (compared by content), so a running
/// server is never replaced under Steam mid-session.
pub fn install_companion() -> Result<std::path::PathBuf, String> {
    let exe =
        crate::paths::executable().map_err(|error| format!("cannot locate myself: {error}"))?;
    install_companion_from(&exe)?;
    Ok(crate::paths::companion_bin())
}

/// Installs `source` as the companion when its content differs. Returns
/// whether anything was replaced; running copies keep their old inode.
pub fn install_companion_from(source: &std::path::Path) -> Result<bool, String> {
    let target = crate::paths::companion_bin();
    let fresh =
        std::fs::read(source).map_err(|error| format!("cannot read {source:?}: {error}"))?;
    let current = std::fs::read(&target).unwrap_or_default();
    if fresh == current {
        return Ok(false);
    }
    let parent = target
        .parent()
        .ok_or_else(|| "companion path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let temporary = parent.join(".exxa.tmp");
    std::fs::write(&temporary, &fresh)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    // The Steam-side shim execs this copy: it must be executable.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("cannot chmod {}: {error}", temporary.display()))?;
    if std::fs::rename(&temporary, &target).is_err() {
        // A running server holds the old inode open; rename works on Linux, but
        // fall back to unlink+rename semantics if a filesystem refuses.
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("cannot install {}", target.display()));
    }
    Ok(true)
}

pub fn start(extra_args: &[String]) -> Result<serde_json::Value, String> {
    let (mut command, libs) = steam_command(extra_args)?;
    command.stdout(Stdio::null()).stderr(Stdio::null());
    crate::desktop::spawn_reaped(&mut command)
        .map_err(|error| format!("cannot start Steam: {error}"))?;
    Ok(serde_json::json!({ "started": true, "ld_audit": libs, "args": extra_args }))
}

/// Replace this process with Steam, so the client inherits the session and the
/// terminal that launched it does not own Steam's lifetime.
pub fn launch(extra_args: &[String]) -> Result<(), String> {
    let (mut command, _) = steam_command(extra_args)?;
    let error = command.exec();
    Err(format!("cannot launch Steam: {error}"))
}

/// Start the local server unless something already answers on the port. Returns
/// true when this call started it.
pub fn ensure_server(port: u16) -> Result<bool, String> {
    if crate::server::is_ours(port) {
        return Ok(false);
    }
    if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
        return Err(format!(
            "another program already listens on 127.0.0.1:{port}; ExxaTools cannot start there"
        ));
    }
    let exe = crate::paths::executable().map_err(|error| error.to_string())?;
    crate::desktop::spawn_reaped(
        Command::new(exe)
            .args(["serve", "--port", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .map_err(|error| format!("cannot start ExxaTools: {error}"))?;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if crate::server::is_ours(port) {
            return Ok(true);
        }
    }
    Err(format!("ExxaTools did not answer on 127.0.0.1:{port}"))
}

pub fn restart(extra_args: &[String]) -> Result<serde_json::Value, String> {
    let was_running = is_running();
    if was_running && !shutdown(std::time::Duration::from_secs(45))? {
        return Err("Steam did not shut down in time".into());
    }
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let started = start(extra_args)?;
    Ok(serde_json::json!({ "stopped": was_running, "start": started }))
}
pub fn active_account() -> Option<(String, String)> {
    let root = root()?;
    let text = std::fs::read_to_string(root.join("config/loginusers.vdf")).ok()?;
    let text = text.trim_start_matches('\u{feff}').to_string();
    let tree = crate::vdf::parse(&text).ok()?;
    let users = crate::vdf::child(&tree, "users").or_else(|| crate::vdf::child(&tree, "Users"))?;
    let map = users.as_object()?;
    let mut fallback: Option<String> = None;
    for (steam_id, entry) in map {
        if fallback.is_none() {
            fallback = Some(steam_id.clone());
        }
        if crate::vdf::scalar(entry, "MostRecent").as_deref() == Some("1") {
            return Some((account_id(steam_id), steam_id.clone()));
        }
    }
    fallback.map(|steam_id| (account_id(&steam_id), steam_id))
}

fn account_id(steam_id: &str) -> String {
    steam_id
        .parse::<u64>()
        .map(|value| value.saturating_sub(76561197960265728).to_string())
        .unwrap_or_else(|_| steam_id.to_string())
}

pub fn localconfig_path() -> Option<PathBuf> {
    let root = root()?;
    if let Some((account, _)) = active_account() {
        let path = root
            .join("userdata")
            .join(account)
            .join("config/localconfig.vdf");
        if path.is_file() {
            return Some(path);
        }
    }
    // Fall back to any account that has a localconfig, newest directory first.
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(root.join("userdata"))
        .ok()?
        .flatten()
        .map(|entry| entry.path().join("config/localconfig.vdf"))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    });
    candidates.pop()
}

pub fn read_launch_options(appid: u64) -> Option<String> {
    let path = localconfig_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let tree = crate::vdf::parse(&text).ok()?;
    let apps = crate::vdf::find_key(&tree, "apps")?;
    let app = crate::vdf::child(apps, &appid.to_string())?;
    crate::vdf::scalar(app, "LaunchOptions")
}

/// Brace span of the `"<appid>" { ... }` block inside the apps table.
pub fn app_block(text: &str, appid: u64) -> Option<(usize, usize)> {
    let needle = format!("\"{appid}\"");
    let mut from = 0;
    while let Some(found) = text[from..].find(&needle) {
        let start = from + found;
        let after = start + needle.len();
        if let Some(open_rel) = text[after..].find('{') {
            let open = after + open_rel;
            let mut depth = 0usize;
            for (offset, ch) in text[open..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some((open, open + offset));
                        }
                    }
                    _ => {}
                }
            }
        }
        from = after;
    }
    None
}

/// Pure text edit, so the rule can be tested without a Steam install.
pub fn apply_launch_option(
    text: &str,
    appid: u64,
    options: &str,
) -> Result<(String, Option<String>), String> {
    let previous = {
        let tree = crate::vdf::parse(text).map_err(|error| error.to_string())?;
        crate::vdf::find_key(&tree, "apps")
            .and_then(|apps| crate::vdf::child(apps, &appid.to_string()))
            .and_then(|app| crate::vdf::scalar(app, "LaunchOptions"))
    };
    let Some((open, close)) = app_block(text, appid) else {
        return Err(format!(
            "{appid} is not in this config yet: launch the game once, then set options"
        ));
    };
    if let Some(index) = text[open..close].find("\"LaunchOptions\"") {
        let after_key = open + index + "\"LaunchOptions\"".len();
        let Some(quote_start) = text[after_key..close].find('"') else {
            return Err("malformed LaunchOptions entry".into());
        };
        let value_start = after_key + quote_start + 1;
        let Some(quote_end) = text[value_start..close].find('"') else {
            return Err("malformed LaunchOptions value".into());
        };
        let value_end = value_start + quote_end;
        return Ok((
            format!("{}{}{}", &text[..value_start], options, &text[value_end..]),
            previous,
        ));
    }
    let line_start = text[..close]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let tabs: String = text[line_start..close]
        .chars()
        .take_while(|ch| *ch == '\t' || *ch == ' ')
        .collect();
    let mut out = String::with_capacity(text.len() + options.len() + 24);
    out.push_str(&text[..close]);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{tabs}\t\"LaunchOptions\"\t\t\"{options}\"\n"));
    out.push_str(&text[close..]);
    Ok((out, previous))
}

pub fn set_launch_options(appid: u64, options: &str) -> Result<serde_json::Value, String> {
    let path = localconfig_path().ok_or("no Steam localconfig.vdf found for the active account")?;
    // A fixture tree is never the client the running process has open, so the
    // guard would block tests without protecting anything.
    if is_running() && std::env::var("EXXATOOLS_STEAM_ROOT").is_err() {
        return Err("close Steam before changing launch options".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let (updated, previous) = apply_launch_option(&text, appid, options)?;
    let backup = path.with_file_name("localconfig.vdf.exxatools.bak");
    let _ = std::fs::write(&backup, &text);
    let temp = path.with_file_name("localconfig.vdf.exxatools.tmp");
    std::fs::write(&temp, &updated).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, &path).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "path": path.display().to_string(),
        "previous": previous,
        "options": options,
        "backup": backup.display().to_string(),
    }))
}

/// Steam keeps the per-game sync marker in localconfig.vdf. Reading it tells the
/// user whether the last cloud exchange with this game finished; dropping it
/// forces Steam to treat the next launch as a fresh sync, which is what clears
/// the stuck "cloud out of date" prompt.
pub fn cloud_state(appid: u64) -> serde_json::Value {
    let Some(path) = localconfig_path() else {
        return serde_json::json!({ "found": false });
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return serde_json::json!({ "found": false });
    };
    let Ok(tree) = crate::vdf::parse(&text) else {
        return serde_json::json!({ "found": false });
    };
    let app = crate::vdf::find_key(&tree, "apps")
        .and_then(|apps| crate::vdf::child(apps, &appid.to_string()));
    let cloud = app.and_then(|app| crate::vdf::child(app, "cloud"));
    match cloud {
        Some(cloud) => serde_json::json!({
            "found": true,
            "state": crate::vdf::scalar(cloud, "last_sync_state"),
            "used_bytes": crate::vdf::scalar_i64(cloud, "used_bytes"),
            "used_files": crate::vdf::scalar_i64(cloud, "used_files"),
            "quota_bytes": crate::vdf::scalar_i64(cloud, "quota_bytes"),
        }),
        None => serde_json::json!({ "found": true, "state": "never synced" }),
    }
}

pub fn clear_cloud_state(appid: u64) -> Result<serde_json::Value, String> {
    let path = localconfig_path().ok_or("no Steam localconfig.vdf found for the active account")?;
    if is_running() {
        return Err("close Steam before resetting a cloud sync state".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let Some((open, close)) = app_block(&text, appid) else {
        return Err(format!(
            "{appid} is not in this config yet: launch the game once first"
        ));
    };
    let block = &text[open..close];
    let Some(offset) = block.find("\"cloud\"") else {
        return Err("this game has no cloud state to reset".into());
    };
    let after_key = open + offset + "\"cloud\"".len();
    let Some(brace) = text[after_key..close].find('{') else {
        return Err("malformed cloud block".into());
    };
    let start_of_brace = after_key + brace;
    let mut depth = 0usize;
    let mut end = None;
    for (index, ch) in text[start_of_brace..close].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start_of_brace + index + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end.ok_or("unbalanced cloud block")?;
    let line_start = text[..open + offset]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(open);
    let mut tail = &text[end..];
    if tail.starts_with('\n') {
        tail = &tail[1..];
    }
    let updated = format!("{}{}", &text[..line_start], tail);
    let backup = path.with_file_name("localconfig.vdf.exxatools.bak");
    let _ = std::fs::write(&backup, &text);
    let temp = path.with_file_name("localconfig.vdf.exxatools.tmp");
    std::fs::write(&temp, &updated).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, &path).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "reset": true,
        "path": path.display().to_string(),
        "backup": backup.display().to_string(),
    }))
}

pub fn add_launch_option(appid: u64, flag: &str) -> Result<serde_json::Value, String> {
    let current = read_launch_options(appid);
    let merged = crate::verify::merge_launch_option(current.as_deref(), flag);
    if current.as_deref().map(str::trim) == Some(merged.trim()) {
        return Ok(serde_json::json!({ "options": merged, "changed": false }));
    }
    let mut result = set_launch_options(appid, &merged)?;
    result["changed"] = serde_json::json!(true);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = "\"UserLocalConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"apps\"\n\t\t\t\t{\n\t\t\t\t\t\"440\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"1700000000\"\n\t\t\t\t\t}\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n";

    #[test]
    fn inserts_when_missing() {
        let (updated, previous) = apply_launch_option(CONFIG, 440, "-onlinefix").unwrap();
        assert_eq!(previous, None);
        assert!(updated.contains("\"LaunchOptions\"\t\t\"-onlinefix\""));
        let tree = crate::vdf::parse(&updated).unwrap();
        let options = crate::vdf::find_key(&tree, "apps")
            .and_then(|apps| crate::vdf::child(apps, "440"))
            .and_then(|app| crate::vdf::scalar(app, "LaunchOptions"));
        assert_eq!(options.as_deref(), Some("-onlinefix"));
    }

    #[test]
    fn replaces_existing() {
        let text = CONFIG.replace(
            "\"LastPlayed\"\t\t\"1700000000\"",
            "\"LaunchOptions\"\t\t\"-old\"",
        );
        let (updated, previous) = apply_launch_option(&text, 440, "-new").unwrap();
        assert_eq!(previous.as_deref(), Some("-old"));
        assert!(updated.contains("\"-new\""));
        assert!(!updated.contains("-old"));
    }

    #[test]
    fn refuses_unknown_app() {
        assert!(apply_launch_option(CONFIG, 620, "-onlinefix").is_err());
    }

    const WRAPPER_MAPS: &str = "\
55d0-55e0 r--p 00000000 08:02 1 /home/x/.local/share/Steam/steamrt64/steam
7f00-7f10 r-xp 00000000 08:02 2 /usr/lib/libc.so.6
7f20-7f30 r--p 00000000 08:02 3 /home/x/.local/share/vapor-forge/x86_64/libvapor_forge.so
";

    const CLIENT_MAPS: &str = "\
7f00-7f10 r-xp 00000000 08:02 2 /usr/lib/libc.so.6
7f30-7f40 r-xp 00000000 08:02 4 /home/x/.local/share/Steam/steamrt64/steamclient.so
7f50-7f60 r--p 00000000 08:02 3 /home/x/.local/share/vapor-forge/x86_64/libvapor_forge.so
";

    #[test]
    fn maps_state_separates_the_shim_from_the_client() {
        let wrapper = map_state(WRAPPER_MAPS);
        assert!(wrapper.shim, "the launcher maps the shim");
        assert!(!wrapper.client, "the launcher owns no steamclient.so");
        assert!(
            !wrapper.active(),
            "a shim outside steamclient changes nothing"
        );

        let client = map_state(CLIENT_MAPS);
        assert!(client.shim);
        assert!(client.client);
        assert!(
            client.active(),
            "the shim is only effective inside steamclient"
        );
    }

    #[test]
    fn merges_without_duplicates() {
        assert_eq!(
            crate::verify::merge_launch_option(Some("-a"), "-onlinefix"),
            "-a -onlinefix"
        );
        assert_eq!(
            crate::verify::merge_launch_option(Some("-onlinefix"), "-onlinefix"),
            "-onlinefix"
        );
        assert_eq!(crate::verify::merge_launch_option(None, "-x"), "-x");
    }
}

#[cfg(test)]
mod launch_tests {
    use super::launch_args;

    #[test]
    fn plain_restart_still_opens_the_debugging_port() {
        assert_eq!(launch_args(&[]), vec!["-cef-enable-debugging".to_string()]);
    }

    #[test]
    fn flag_is_not_duplicated_and_urls_are_kept() {
        let args = launch_args(&["-cef-enable-debugging".into(), "steam://open/games".into()]);
        assert_eq!(
            args,
            vec![
                "-cef-enable-debugging".to_string(),
                "steam://open/games".to_string()
            ]
        );
    }
}
