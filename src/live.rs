//! Live coupling with Steam and with the source checkout.
//!
//! The server already starts with Steam (the container wrapper raises it).
//! This module makes the rest automatic: the dashboard opens when Steam's UI
//! comes up and closes when Steam goes away, and a server built from a
//! checkout rebuilds itself when the sources change, installs the result as
//! the companion binary and re-executes into it. The open dashboard notices
//! the new companion and restarts on its own page.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Set on a process that re-executed itself after an update, so it does not
/// repeat its start-up side effects (like opening the dashboard again).
pub const REEXEC_ENV: &str = "EXXA_REEXEC";

/// Sources go quiet for this long before a build starts: editors write a
/// file several times per save, and a save often touches several files.
const DEBOUNCE: Duration = Duration::from_millis(1200);
const POLL: Duration = Duration::from_secs(2);

/// The checkout this binary was built from, when it still exists. Packaged
/// builds point at a build directory that is gone, which turns rebuilding off.
pub fn checkout() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    (dir.join("Cargo.toml").is_file() && dir.join("src/main.rs").is_file()).then_some(dir)
}

fn status_path() -> PathBuf {
    crate::paths::cache_home().join("exxatools/live.json")
}

fn log_path() -> PathBuf {
    crate::paths::cache_home().join("exxatools/live-build.log")
}

/// The last automatic build: `state` is `building`, `ok` or `failed`.
pub fn status() -> Value {
    std::fs::read(status_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({ "state": "idle" }))
}

fn write_status(state: &str, message: &str) {
    let at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let path = status_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = json!({ "state": state, "message": message, "at": at, "log": log_path() });
    let _ = std::fs::write(path, body.to_string());
}

fn sha256_file(path: &Path) -> Option<[u8; 32]> {
    let bytes = std::fs::read(path).ok()?;
    Some(Sha256::digest(&bytes).into())
}

/// Fingerprint of the binary this process is running. `/proc/self/exe` still
/// reads the old file after an update replaced it on disk.
fn own_hash() -> Option<[u8; 32]> {
    sha256_file(Path::new("/proc/self/exe"))
}

/// Watches the companion binary and reports once it holds a different build
/// than the running one. Cheap: hashes only when size or mtime moved.
pub struct UpdateWatch {
    own: Option<[u8; 32]>,
    seen: Option<(u64, SystemTime)>,
}

impl UpdateWatch {
    pub fn new() -> Self {
        Self {
            own: own_hash(),
            seen: stamp(&crate::paths::companion_bin()),
        }
    }

    /// True when the companion changed since the last call and differs from
    /// this process. A companion mid-write is skipped until it settles.
    pub fn ready(&mut self) -> bool {
        let companion = crate::paths::companion_bin();
        let current = stamp(&companion);
        if current.is_none() || current == self.seen {
            return false;
        }
        self.seen = current;
        match (self.own, sha256_file(&companion)) {
            (Some(own), Some(theirs)) => own != theirs,
            _ => false,
        }
    }
}

impl Default for UpdateWatch {
    fn default() -> Self {
        Self::new()
    }
}

fn stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

/// Server side: re-execute into the companion whenever it is replaced, so
/// the API, the Steam panel script and the plans code are always current.
pub fn start_server_reexec(port: u16) {
    std::thread::spawn(move || {
        let mut watch = UpdateWatch::new();
        loop {
            std::thread::sleep(POLL);
            if !watch.ready() {
                continue;
            }
            eprintln!("live: a new build is installed; restarting the server");
            let mut command = Command::new(crate::paths::companion_bin());
            command
                .args(["serve", "--port", &port.to_string()])
                .env(REEXEC_ENV, "1");
            use std::os::unix::process::CommandExt;
            let error = command.exec();
            eprintln!("live: cannot restart into the new build: {error}");
        }
    });
}

/// Server side: when the settings allow it, open the dashboard once Steam's
/// UI is up. The first observation counts as a start unless this process is a
/// re-exec, which would otherwise reopen a dashboard that is already there.
pub fn start_steam_follower() {
    std::thread::spawn(|| {
        let mut was_up = std::env::var_os(REEXEC_ENV).is_some() && steam_ui_up();
        loop {
            let up = steam_ui_up();
            if up && !was_up && open_with_steam() {
                eprintln!("live: Steam is up; opening the dashboard");
                let _ = crate::desktop::open_window("");
            }
            was_up = up;
            std::thread::sleep(POLL);
        }
    });
}

/// Steam counts as up once its UI answers on the debugging port, so the
/// dashboard does not race Steam's own login and update screens.
pub fn steam_ui_up() -> bool {
    crate::steamctl::is_running() && crate::steamui::main_target().is_some()
}

pub fn open_with_steam() -> bool {
    crate::state::load()
        .map(|state| state.settings.open_with_steam)
        .unwrap_or(true)
}

pub fn close_with_steam() -> bool {
    crate::state::load()
        .map(|state| state.settings.close_with_steam)
        .unwrap_or(true)
}

fn live_rebuild() -> bool {
    crate::state::load()
        .map(|state| state.settings.live_rebuild)
        .unwrap_or(true)
}

/// Server side: rebuild from the checkout whenever its sources change and
/// install the result as the companion. Off for packaged builds.
pub fn start_builder() {
    let Some(root) = checkout() else { return };
    let Some(cargo) = cargo() else {
        eprintln!("live: cargo not found; automatic rebuilds are off");
        return;
    };
    std::thread::spawn(move || {
        if let Err(error) = builder_loop(&root, &cargo) {
            eprintln!("live: {error}");
        }
    });
}

/// The server inherits Steam's environment, whose PATH often lacks rustup.
fn cargo() -> Option<PathBuf> {
    let home = crate::paths::home().join(".cargo/bin/cargo");
    if home.is_file() {
        return Some(home);
    }
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("cargo"))
            .find(|candidate| candidate.is_file())
    })
}

/// Paths whose change can alter the binary. `target/` and VCS metadata never
/// trigger a build, or every build would schedule the next one.
fn is_source(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut parts = relative.components();
    let Some(first) = parts.next() else {
        return false;
    };
    let first = first.as_os_str().to_string_lossy();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = name.starts_with('.')
        || name.ends_with('~')
        || name.ends_with(".swp")
        || name.ends_with(".tmp");
    match first.as_ref() {
        "src" | "assets" | "web" | "patches" => !temporary,
        "Cargo.toml" | "Cargo.lock" | "build.rs" => true,
        _ => false,
    }
}

fn builder_loop(root: &Path, cargo: &Path) -> Result<(), String> {
    use notify::{RecursiveMode, Watcher};

    let (sender, receiver) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })
    .map_err(|error| format!("cannot watch the checkout: {error}"))?;
    for entry in ["src", "assets", "web", "patches"] {
        let path = root.join(entry);
        if path.is_dir() {
            watcher
                .watch(&path, RecursiveMode::Recursive)
                .map_err(|error| format!("cannot watch {}: {error}", path.display()))?;
        }
    }
    watcher
        .watch(root, RecursiveMode::NonRecursive)
        .map_err(|error| format!("cannot watch {}: {error}", root.display()))?;
    eprintln!("live: watching {} for changes", root.display());

    // Catch up with edits made while no server was running.
    let mut dirty: Option<Instant> = Some(Instant::now() - DEBOUNCE);
    loop {
        let wait = dirty
            .map(|since| DEBOUNCE.saturating_sub(since.elapsed()))
            .unwrap_or(Duration::from_secs(3600))
            .max(Duration::from_millis(50));
        match receiver.recv_timeout(wait) {
            Ok(Ok(event)) => {
                if event.paths.iter().any(|path| is_source(root, path)) {
                    dirty = Some(Instant::now());
                }
            }
            Ok(Err(error)) => eprintln!("live: watch error: {error}"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if dirty.is_some_and(|since| since.elapsed() >= DEBOUNCE) {
                    dirty = None;
                    if live_rebuild() {
                        build_and_install(root, cargo);
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("the checkout watcher stopped".into());
            }
        }
    }
}

fn build_and_install(root: &Path, cargo: &Path) {
    write_status("building", "Building the latest sources…");
    let log = log_path();
    if let Some(parent) = log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut command = Command::new("nice");
    command
        .arg("-n")
        .arg("10")
        .arg(cargo)
        .args(["build", "--release", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .current_dir(root)
        .stdin(Stdio::null())
        // Steam's environment carries its runtime's loader settings, which
        // break rustc and the linker.
        .env_remove("LD_AUDIT")
        .env_remove("LD_PRELOAD")
        .env_remove("LD_LIBRARY_PATH");
    if let Some(bin) = cargo.parent() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![bin.to_path_buf()];
        paths.extend(std::env::split_paths(&path));
        if let Ok(joined) = std::env::join_paths(paths) {
            command.env("PATH", joined);
        }
    }
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            write_status("failed", &format!("cannot run cargo: {error}"));
            return;
        }
    };
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let _ = std::fs::write(&log, &text);
    if !output.status.success() {
        let message = first_error(&text).unwrap_or_else(|| "cargo build failed".into());
        eprintln!("live: build failed: {message}");
        write_status("failed", &message);
        return;
    }
    let built = root.join("target/release/exxa");
    match crate::steamctl::install_companion_from(&built) {
        Ok(true) => {
            eprintln!("live: installed a new build");
            write_status("ok", "Updated to the latest build");
        }
        Ok(false) => write_status("ok", "Already up to date"),
        Err(error) => write_status("failed", &error),
    }
}

/// The first compiler error with its location, for a one-line toast.
fn first_error(log: &str) -> Option<String> {
    let mut lines = log.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("error") {
            let location = lines
                .next()
                .map(str::trim)
                .filter(|next| next.starts_with("-->"))
                .map(|next| format!(" ({})", next.trim_start_matches("-->").trim()))
                .unwrap_or_default();
            return Some(format!("{line}{location}"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_sources_trigger_a_build() {
        let root = Path::new("/c");
        assert!(is_source(root, Path::new("/c/src/ui/art.rs")));
        assert!(is_source(root, Path::new("/c/assets/steam-menu.js")));
        assert!(is_source(root, Path::new("/c/Cargo.toml")));
        assert!(!is_source(root, Path::new("/c/target/release/exxa")));
        assert!(!is_source(root, Path::new("/c/.git/index")));
        assert!(!is_source(root, Path::new("/c/README.md")));
        assert!(!is_source(root, Path::new("/c/src/.art.rs.swp")));
        assert!(!is_source(root, Path::new("/c/src/art.rs~")));
        assert!(!is_source(root, Path::new("/elsewhere/src/main.rs")));
    }

    #[test]
    fn the_first_compiler_error_is_reported_with_its_location() {
        let log = "   Compiling exxa v0.3.0\nwarning: unused\nerror[E0425]: cannot find value `x`\n  --> src/live.rs:10:5\n   |\n";
        assert_eq!(
            first_error(log).as_deref(),
            Some("error[E0425]: cannot find value `x` (src/live.rs:10:5)")
        );
        assert_eq!(first_error("Finished"), None);
    }

    #[test]
    fn this_checkout_is_found() {
        assert!(checkout().is_some_and(|dir| dir.join("src/live.rs").is_file()));
    }
}
