"""Inspect a machine's Vapor Forge install, and own its Lua script directory."""

from __future__ import annotations

import os
import re
import time
from pathlib import Path

from . import store

HOME = Path.home()
CONFIG_HOME = Path(os.environ.get("XDG_CONFIG_HOME") or HOME / ".config")
DATA_HOME = Path(os.environ.get("XDG_DATA_HOME") or HOME / ".local" / "share")

VF_CONFIG_DIR = CONFIG_HOME / "vapor-forge"
VF_CONFIG = VF_CONFIG_DIR / "config.toml"
VF_SCRIPTS = VF_CONFIG_DIR / "scripts"
VF_DATA_DIR = DATA_HOME / "vapor-forge"
DEFAULT_STEAM_PATHS = [
    HOME / ".steam" / "steam" / "config" / "lua",
    HOME / ".local" / "share" / "Steam" / "config" / "lua",
]
SYSTEMD_DROPINS = [
    CONFIG_HOME / "systemd" / "user" / "steam-launcher.service.d" / "10-vapor-forge.conf",
    CONFIG_HOME / "systemd" / "user" / "app-steam@autostart.service.d" / "10-vapor-forge.conf",
]
STEAM_ENV_FILE = HOME / ".env.steam"
LAUNCHER = DATA_HOME / "applications" / "steam.desktop"

DISABLED_SUFFIX = ".disabled"


def script_path(appid: int | str) -> Path:
    return VF_SCRIPTS / f"{appid}.lua"


def _lib_files() -> dict[str, list[str]]:
    found: dict[str, list[str]] = {}
    for arch in ("i686", "x86_64"):
        directory = VF_DATA_DIR / arch
        names = []
        if directory.is_dir():
            names = sorted(path.name for path in directory.glob("*.so"))
        found[arch] = names
    return found


def _ld_audit_configured() -> dict:
    evidence = []
    for dropin in SYSTEMD_DROPINS:
        if dropin.is_file():
            try:
                text = dropin.read_text("utf-8", "replace")
            except OSError:
                continue
            if "LD_AUDIT" in text:
                evidence.append(str(dropin))
    if STEAM_ENV_FILE.is_file():
        try:
            text = STEAM_ENV_FILE.read_text("utf-8", "replace")
        except OSError:
            text = ""
        if "LD_AUDIT" in text and "vapor_forge" in text:
            evidence.append(str(STEAM_ENV_FILE))
    if LAUNCHER.is_file():
        try:
            text = LAUNCHER.read_text("utf-8", "replace")
        except OSError:
            text = ""
        if "LD_AUDIT" in text and "vapor_forge" in text:
            evidence.append(str(LAUNCHER))
    env = os.environ.get("LD_AUDIT", "")
    if "vapor_forge" in env:
        evidence.append("LD_AUDIT environment")
    return {"configured": bool(evidence), "evidence": evidence}


def ld_audit() -> dict:
    """Public view of the LD_AUDIT wiring check."""
    return _ld_audit_configured()


def script_dirs() -> list[Path]:
    dirs = [VF_SCRIPTS]
    for path in DEFAULT_STEAM_PATHS:
        if path.is_dir():
            dirs.append(path)
    for extra in _config_script_paths():
        path = Path(extra).expanduser()
        if path.is_dir():
            dirs.append(path)
    seen, unique = set(), []
    for path in dirs:
        if path in seen:
            continue
        seen.add(path)
        unique.append(path)
    return unique


def _config_script_paths() -> list[str]:
    if not VF_CONFIG.is_file():
        return []
    try:
        text = VF_CONFIG.read_text("utf-8", "replace")
    except OSError:
        return []
    match = re.search(r"^\[scripting\](.*?)(?=^\[|\Z)", text, re.MULTILINE | re.DOTALL)
    if not match:
        return []
    body = match.group(1)
    paths_match = re.search(r"^paths\s*=\s*\[(.*?)\]", body, re.MULTILINE | re.DOTALL)
    if not paths_match:
        return []
    return re.findall(r'"([^"]+)"', paths_match.group(1))


META_RE = re.compile(r"--\s*@(\w+)\s+(.+)")


def read_meta(path: Path) -> dict:
    meta: dict[str, str] = {}
    try:
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            for _ in range(40):
                line = handle.readline()
                if not line:
                    break
                match = META_RE.search(line)
                if match:
                    meta[match.group(1).lower()] = match.group(2).strip()
    except OSError:
        pass
    return meta


def installed_scripts() -> list[dict]:
    out = []
    for directory in script_dirs():
        try:
            entries = sorted(directory.iterdir())
        except OSError:
            continue
        for path in entries:
            name = path.name
            enabled = name.endswith(".lua")
            if not enabled and not name.endswith(".lua" + DISABLED_SUFFIX):
                continue
            meta = read_meta(path)
            appid = meta.get("appid") or name.split(".")[0]
            out.append(
                {
                    "path": str(path),
                    "file": name,
                    "appid": appid,
                    "name": meta.get("name", ""),
                    "branch": meta.get("branch", ""),
                    "depots": meta.get("depots", ""),
                    "enabled": enabled,
                    "editable": path.parent == VF_SCRIPTS,
                    "mtime": int(path.stat().st_mtime) if path.exists() else 0,
                }
            )
    out.sort(key=lambda item: (item["name"] or item["appid"]).lower())
    return out


def status() -> dict:
    libs = _lib_files()
    installed = any(libs.values())
    audit = _ld_audit_configured()
    state = store.load()
    return {
        "vapor_forge": {
            "installed": installed,
            "libs": libs,
            "data_dir": str(VF_DATA_DIR),
            "config_dir": str(VF_CONFIG_DIR),
            "config_exists": VF_CONFIG.is_file(),
            "ld_audit": audit,
            "scripts_dir": str(VF_SCRIPTS),
            "scripts_dir_exists": VF_SCRIPTS.is_dir(),
            "script_dirs": [str(path) for path in script_dirs()],
        },
        "scripts": installed_scripts(),
        "apps": list(state["apps"].values()),
        "settings": state["settings"],
        "state_path": str(store.STATE_PATH),
    }


def write_script(appid: int, text: str, enabled: bool = True) -> Path:
    VF_SCRIPTS.mkdir(parents=True, exist_ok=True)
    target = script_path(appid)
    disabled = VF_SCRIPTS / f"{appid}.lua{DISABLED_SUFFIX}"
    if enabled:
        if disabled.exists():
            disabled.unlink()
    else:
        if target.exists():
            target.unlink()
        target = disabled
    target.write_text(text, "utf-8")
    os.chmod(target, 0o644)
    return target


def remove_script(appid: int) -> bool:
    removed = False
    for path in (script_path(appid), VF_SCRIPTS / f"{appid}.lua{DISABLED_SUFFIX}"):
        if path.exists():
            path.unlink()
            removed = True
    return removed


def toggle_script(appid: int, enabled: bool) -> Path | None:
    live = script_path(appid)
    off = VF_SCRIPTS / f"{appid}.lua{DISABLED_SUFFIX}"
    if enabled:
        if off.exists():
            off.rename(live)
        return live if live.exists() else None
    if live.exists():
        live.rename(off)
        return off
    return None


def ensure_script_dir_registered() -> str:
    """Vapor Forge scans its own scripts directory; report what it scans."""
    dirs = [str(path) for path in script_dirs()]
    return "Vapor Forge scans: " + ", ".join(dirs)


PROVIDER_FILE = "zz-exxatools-provider.lua"


def provider_path() -> Path:
    return VF_SCRIPTS / PROVIDER_FILE


def write_provider(text: str) -> Path:
    VF_SCRIPTS.mkdir(parents=True, exist_ok=True)
    legacy = VF_SCRIPTS / "zz-vapordepot-provider.lua"
    if legacy.exists():
        legacy.unlink(missing_ok=True)
    path = provider_path()
    temp = path.with_suffix(".tmp")
    temp.write_text(text, "utf-8")
    os.chmod(temp, 0o644)
    os.replace(temp, path)
    return path


def remove_provider() -> bool:
    path = provider_path()
    if path.exists():
        path.unlink()
        return True
    return False


def provider_state() -> dict:
    path = provider_path()
    info = {"installed": path.is_file(), "path": str(path), "providers": [], "mtime": 0, "stamp": ""}
    if not path.is_file():
        return info
    info["mtime"] = int(path.stat().st_mtime)
    try:
        text = path.read_text("utf-8", "replace")
    except OSError:
        return info
    info["providers"] = re.findall(r'name\s*=\s*"([^"]+)"', text)
    match = re.search(r"generated ([0-9-]+ [0-9:]+)", text)
    info["stamp"] = match.group(1) if match else ""
    return info


def stamp() -> str:
    return time.strftime("%Y-%m-%d %H:%M:%S")
