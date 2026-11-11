"""Make ExxaTools behave like a desktop app instead of a browser tab."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

PROJECT_DIR = Path(__file__).resolve().parent.parent
ICON = PROJECT_DIR / "web" / "logo.png"
CONFIG_HOME = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
DATA_HOME = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share")
PROFILE_DIR = CONFIG_HOME / "exxatools" / "window"
LAUNCHER = PROJECT_DIR / "exxa"

CHROMIUM_BINARIES = [
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "brave-browser",
    "vivaldi",
    "microsoft-edge",
]

PREFS = """
user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.sessionstore.resume_from_crash", false);
user_pref("datareporting.policy.dataSubmissionPolicyAccepted", true);
user_pref("browser.tabs.warnOnClose", false);
user_pref("browser.startup.homepage", "about:blank");
"""

USER_CHROME = """/* ExxaTools app window: no tabs, no address bar. */
#TabsToolbar, #nav-bar, #PersonalToolbar { visibility: collapse !important; }
"""


def _find_chromium() -> str | None:
    for name in CHROMIUM_BINARIES:
        path = shutil.which(name)
        if path:
            return path
    return None


WEBKIT_APP = r'''"""ExxaTools window: GTK 3 + WebKit2 rendering the local UI."""
import os
import sys

# WebKit's DMA-BUF renderer fails on a lot of X11/nvidia setups with
# "Failed to create GBM buffer", which paints an empty window. Fall back to the
# shared-memory path; the UI is plain DOM, nothing needs the GPU.
os.environ.setdefault("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
os.environ.setdefault("WEBKIT_DISABLE_COMPOSITING_MODE", "1")

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")

from gi.repository import Gdk, Gtk, WebKit2

Gdk.set_program_class("ExxaTools")

url = sys.argv[1]
title = sys.argv[2] if len(sys.argv) > 2 else "ExxaTools"

settings = Gtk.Settings.get_default()
if settings is not None:
    settings.set_property("gtk-application-prefer-dark-theme", True)

window = Gtk.Window(title=title)
window.set_default_size(1400, 950)
window.set_position(Gtk.WindowPosition.CENTER)

context = WebKit2.WebContext.get_default()
view = WebKit2.WebView.new_with_context(context)
view.set_vexpand(True)
view.set_hexpand(True)
web_settings = view.get_settings()
web_settings.set_property("enable-accelerated-2d-canvas", False)
try:
    web_settings.set_property(
        "hardware-acceleration-policy", WebKit2.HardwareAccelerationPolicy.NEVER
    )
except (TypeError, AttributeError):
    pass

view.load_uri(url)
window.add(view)
window.connect("destroy", Gtk.main_quit)
window.show_all()
Gtk.main()
'''

_GI_CACHE: dict[str, str | None] = {}


def _gi_interpreter() -> str | None:
    """A python that can import Gtk and WebKit2 (this one may be a venv)."""
    if "path" in _GI_CACHE:
        return _GI_CACHE["path"]
    probe = (
        "import gi; gi.require_version('Gtk','3.0'); "
        "gi.require_version('WebKit2','4.1'); "
        "from gi.repository import Gtk, WebKit2"
    )
    candidates = ["/usr/bin/python3", sys.executable, "/usr/local/bin/python3"]
    for candidate in candidates:
        if not candidate or not Path(candidate).exists():
            continue
        result = subprocess.run(
            [candidate, "-c", probe], capture_output=True, text=True, check=False, timeout=30
        )
        if result.returncode == 0:
            _GI_CACHE["path"] = candidate
            return candidate
    _GI_CACHE["path"] = None
    return None


def _webkit_window(url: str, title: str) -> dict | None:
    interpreter = _gi_interpreter()
    if interpreter is None:
        return None
    script = PROFILE_DIR / "window.py"
    script.parent.mkdir(parents=True, exist_ok=True)
    script.write_text(WEBKIT_APP, "utf-8")
    process = subprocess.Popen(
        [interpreter, str(script), url, title],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return {
        "mode": "webkit",
        "binary": interpreter,
        "profile": str(PROFILE_DIR),
        "pid": process.pid,
        "script": str(script),
    }


def _firefox_profile() -> Path:
    profile = PROFILE_DIR / "firefox"
    (profile / "chrome").mkdir(parents=True, exist_ok=True)
    prefs = profile / "prefs.js"
    if not prefs.exists() or "legacyUserProfileCustomizations" not in prefs.read_text("utf-8", "replace"):
        prefs.write_text(PREFS.strip() + "\n", "utf-8")
    chrome = profile / "chrome" / "userChrome.css"
    if not chrome.exists():
        chrome.write_text(USER_CHROME, "utf-8")
    return profile


def open_window(url: str, prefer: str = "auto") -> dict:
    """Open `url` in an app-like window, falling back to the default browser."""
    if prefer in ("auto", "webkit"):
        window = _webkit_window(url, "ExxaTools")
        if window is not None:
            return window

    chromium = _find_chromium()
    if chromium:
        profile = PROFILE_DIR / "chromium"
        profile.mkdir(parents=True, exist_ok=True)
        process = subprocess.Popen(
            [
                chromium,
                f"--app={url}",
                f"--user-data-dir={profile}",
                "--class=ExxaTools",
                "--no-first-run",
                "--no-default-browser-check",
                "--window-size=1400,950",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return {
            "mode": "chromium",
            "binary": chromium,
            "profile": str(profile),
            "pid": process.pid,
        }

    firefox = shutil.which("firefox")
    if firefox:
        profile = _firefox_profile()
        process = subprocess.Popen(
            [
                firefox,
                "--no-remote",
                "--new-instance",
                "--profile",
                str(profile),
                "--width",
                "1400",
                "--height",
                "950",
                url,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return {
            "mode": "firefox",
            "binary": firefox,
            "profile": str(profile),
            "pid": process.pid,
        }

    import webbrowser

    webbrowser.open(url)
    return {"mode": "browser", "binary": None, "profile": None}


def window_alive(pid: int | None) -> bool:
    if not pid:
        return False
    try:
        os.kill(int(pid), 0)
    except (OSError, ValueError):
        return False
    return True


def wait_for_window(pid: int | None, timeout: float | None = None, poll: float = 0.4) -> bool:
    """Block until the app window closes. Returns True when it closed by itself."""
    started = time.monotonic()
    while window_alive(pid):
        if timeout is not None and time.monotonic() - started > timeout:
            return False
        time.sleep(poll)
    return True


def desktop_entry() -> str:
    return "\n".join(
        [
            "[Desktop Entry]",
            "Type=Application",
            "Name=ExxaTools",
            "Comment=Steam depots and manifests for Vapor Forge",
            f"Exec={LAUNCHER} serve --window",
            f"Icon={ICON}",
            "Terminal=false",
            "Categories=Game;Utility;",
            "StartupWMClass=ExxaTools",
            f"Path={PROJECT_DIR}",
            "",
        ]
    )


def open_target(target: str) -> dict:
    """Open one whitelisted file or directory with the desktop handler."""
    from . import vfsys

    targets = {
        "readme": PROJECT_DIR / "README.md",
        "design": PROJECT_DIR / "DESIGN.md",
        "scripts": vfsys.VF_SCRIPTS,
        "config": vfsys.VF_CONFIG,
        "log": vfsys.DATA_HOME / "Steam" / "logs" / "console-linux.txt",
        "state": PROJECT_DIR / "exxatools" / "store.py",
    }
    path = targets.get(target)
    if path is None:
        raise ValueError(f"unknown target {target!r}")
    if not path.exists():
        raise ValueError(f"{path} does not exist yet")
    opener = shutil.which("xdg-open")
    if opener is None:
        return {"opened": False, "path": str(path), "reason": "xdg-open not found"}
    subprocess.Popen(
        [opener, str(path)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return {"opened": True, "path": str(path)}


def app_running(port: int = 8765) -> bool:
    import socket

    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.6):
            return True
    except OSError:
        return False


def launch_app(port: int = 8765) -> dict:
    """Start the dashboard window if it is not already listening."""
    if app_running(port):
        return {"running": True, "started": False, "url": f"http://127.0.0.1:{port}/"}
    subprocess.Popen(
        [str(LAUNCHER), "serve", "--port", str(port), "--window"],
        cwd=str(PROJECT_DIR),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return {"running": True, "started": True, "url": f"http://127.0.0.1:{port}/"}


def install_desktop_entry() -> dict:
    legacy = DATA_HOME / "applications" / "vapordepot.desktop"
    if legacy.exists():
        legacy.unlink(missing_ok=True)
    target = DATA_HOME / "applications" / "exxatools.desktop"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(desktop_entry(), "utf-8")
    os.chmod(target, 0o755)
    os.chmod(LAUNCHER, 0o755)
    updated = False
    if shutil.which("update-desktop-database"):
        result = subprocess.run(
            ["update-desktop-database", str(target.parent)],
            capture_output=True,
            check=False,
        )
        updated = result.returncode == 0
    return {"path": str(target), "icon": str(ICON), "database_updated": updated}


def uninstall_desktop_entry() -> dict:
    target = DATA_HOME / "applications" / "exxatools.desktop"
    removed = False
    if target.exists():
        target.unlink()
        removed = True
    return {"path": str(target), "removed": removed}
