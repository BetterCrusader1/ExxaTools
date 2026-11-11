"""Steam client control: roots, accounts, launch options, restart."""

from __future__ import annotations

import os
import shutil
import subprocess
import time
from pathlib import Path

from . import vdf, vfsys

CANDIDATES = [
    Path.home() / ".steam" / "steam",
    Path.home() / ".local" / "share" / "Steam",
    Path.home() / ".steam" / "root",
    Path.home() / "Steam",
]


class SteamError(RuntimeError):
    pass


def root() -> Path | None:
    for candidate in CANDIDATES:
        if (candidate / "config" / "loginusers.vdf").is_file():
            return candidate
    for candidate in CANDIDATES:
        if (candidate / "ubuntu12_64").is_dir() or (candidate / "steam.sh").is_file():
            return candidate
    return None


def _login_users(path: Path) -> dict:
    try:
        return vdf.parse(path.read_text("utf-8", "replace"))
    except (OSError, vdf.VdfError):
        return vdf.Node()


def active_account(steam_root: Path | None = None) -> tuple[str, str] | None:
    """Return (account_id, steam_id64) for the most recent account."""
    steam_root = steam_root or root()
    if steam_root is None:
        return None
    users = _login_users(steam_root / "config" / "loginusers.vdf")
    container = users.child("users") or users.child("Users")
    if container is None:
        return None
    fallback = None
    for entry in container.children:
        if entry.key is None:
            continue
        steam_id = entry.key
        most_recent = entry.child("MostRecent")
        if fallback is None:
            fallback = steam_id
        if most_recent is not None and str(most_recent.value).strip() == "1":
            return _account_id(steam_id), steam_id
    if fallback is not None:
        return _account_id(fallback), fallback
    return None


def _account_id(steam_id: str) -> str:
    try:
        return str(int(steam_id) - 76561197960265728)
    except (TypeError, ValueError):
        return steam_id


def localconfig_path(steam_root: Path | None = None) -> Path | None:
    steam_root = steam_root or root()
    if steam_root is None:
        return None
    account = active_account(steam_root)
    if account is None:
        return None
    path = steam_root / "userdata" / account[0] / "config" / "localconfig.vdf"
    return path if path.is_file() else None


def pid() -> int | None:
    steam_root = root()
    if steam_root is None:
        return None
    pid_file = steam_root / "steam.pid"
    try:
        value = int(pid_file.read_text("utf-8").strip())
    except (OSError, ValueError):
        return None
    try:
        os.kill(value, 0)
    except ProcessLookupError:
        return None
    except PermissionError:
        return value
    return value


def is_running() -> bool:
    if pid() is not None:
        return True
    return bool(shutil.which("pgrep") and subprocess.run(
        ["pgrep", "-x", "steam"], capture_output=True
    ).returncode == 0)


def audit_loaded() -> dict:
    """Whether the running client actually has the Vapor Forge shim in LD_AUDIT."""
    target = pid()
    if target is None:
        try:
            result = subprocess.run(
                ["pgrep", "-f", "ubuntu12_32/steam "], capture_output=True, text=True, check=False
            )
            found = [line.strip() for line in result.stdout.splitlines() if line.strip().isdigit()]
            target = int(found[0]) if found else None
        except (OSError, ValueError):
            target = None
    if target is None:
        return {"pid": None, "ld_audit": "", "loaded": False}
    try:
        environ = Path(f"/proc/{target}/environ").read_bytes()
    except OSError as error:
        return {"pid": target, "ld_audit": "", "loaded": False, "error": str(error)}
    value = ""
    for chunk in environ.split(b"\0"):
        if chunk.startswith(b"LD_AUDIT="):
            value = chunk.split(b"=", 1)[1].decode("utf-8", "replace")
            break
    return {"pid": target, "ld_audit": value, "loaded": "libvapor_forge.so" in value}


def read_launch_options(appid: int) -> dict:
    path = localconfig_path()
    if path is None:
        return {"found": False, "path": None, "options": None}
    try:
        text = path.read_text("utf-8", "replace")
        options = vdf.read_launch_options(text, appid)
    except (OSError, vdf.VdfError) as error:
        raise SteamError(str(error)) from error
    return {"found": True, "path": str(path), "options": options}


def set_launch_options(appid: int, options: str) -> dict:
    path = localconfig_path()
    if path is None:
        raise SteamError("no Steam localconfig.vdf found for the active account")
    if is_running():
        raise SteamError("close Steam before changing launch options")
    try:
        text = path.read_text("utf-8", "replace")
        updated, previous = vdf.set_launch_options(text, appid, options)
    except (OSError, vdf.VdfError) as error:
        raise SteamError(str(error)) from error
    backup = path.with_name("localconfig.vdf.exxatools.bak")
    backup.write_text(text, "utf-8")
    temp = path.with_name("localconfig.vdf.exxatools.tmp")
    temp.write_text(updated, "utf-8")
    temp.replace(path)
    return {"path": str(path), "previous": previous, "options": options, "backup": str(backup)}


def add_launch_option(appid: int, flag: str) -> dict:
    current = read_launch_options(appid)
    merged = vdf.merge_launch_option(current.get("options"), flag)
    if current.get("options") == merged:
        return {"path": current.get("path"), "options": merged, "changed": False}
    result = set_launch_options(appid, merged)
    result["changed"] = True
    return result


def _audit_libs() -> str:
    """Only the audit shim goes in LD_AUDIT; the Proton helper is injected elsewhere."""
    libs = []
    for arch in ("i686", "x86_64"):
        path = vfsys.VF_DATA_DIR / arch / "libvapor_forge.so"
        if path.is_file():
            libs.append(str(path))
    return ":".join(libs)


def shutdown(timeout: float = 45.0) -> bool:
    binary = shutil.which("steam") or (str(root() / "steam.sh") if root() else None)
    if not binary:
        raise SteamError("steam executable not found in PATH")
    subprocess.run([binary, "-shutdown"], capture_output=True, timeout=timeout, check=False)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not is_running():
            return True
        time.sleep(0.5)
    return not is_running()


def start(extra_args: list[str] | None = None) -> dict:
    binary = shutil.which("steam") or (str(root() / "steam.sh") if root() else None)
    if not binary:
        raise SteamError("steam executable not found in PATH")
    env = dict(os.environ)
    libs = _audit_libs()
    if libs and "vapor_forge" not in env.get("LD_AUDIT", ""):
        existing = env.get("LD_AUDIT", "")
        env["LD_AUDIT"] = f"{libs}:{existing}" if existing else libs
    command = [binary, *(extra_args or [])]
    subprocess.Popen(
        command,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return {"started": True, "ld_audit": env.get("LD_AUDIT", ""), "args": extra_args or []}


def restart(extra_args: list[str] | None = None) -> dict:
    was_running = is_running()
    stopped = shutdown() if was_running else True
    if not stopped:
        raise SteamError("Steam did not shut down in time")
    time.sleep(1.5)
    return {"stopped": was_running, **start(extra_args)}
