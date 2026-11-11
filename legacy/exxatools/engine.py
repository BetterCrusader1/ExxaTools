"""Vapor Forge engine lifecycle: detect, install, update, uninstall, validate.

Vapor Forge is the piece that actually hooks the Steam client: it makes Steam
accept a license it does not own, serve pinned manifests, and decrypt depots.
Without it a plan is just a file nobody reads, so this module owns the whole
"does the engine run" question and can install it without a terminal.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
import time
from pathlib import Path

from . import steamctl, vfsys

REPO = "Hintay/vapor-forge"
RELEASES_API = f"https://api.github.com/repos/{REPO}/releases"
ARCHES = ("i686", "x86_64")
LIBS = ("libvapor_forge.so", "libvapor_forge_proton_inject.so")
CACHE_HOME = Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache")
CACHE_DIR = CACHE_HOME / "exxatools" / "engine"
META = vfsys.VF_DATA_DIR / "exxatools-engine.json"
INSTALL_TIMEOUT = 180
LUA_CHECK_TIMEOUT = 30


class EngineError(RuntimeError):
    pass


# -- remote releases ------------------------------------------------------


def releases(limit: int = 5) -> list[dict]:
    """Newest releases first, each with its linux tarball resolved."""
    from . import http

    try:
        payload = http.get_json(f"{RELEASES_API}?per_page={max(1, min(limit, 20))}")
    except http.HttpError as error:
        raise EngineError(f"cannot reach the GitHub release API: {error}") from error
    out = []
    for release in payload if isinstance(payload, list) else []:
        asset = _linux_asset(release)
        if asset is None:
            continue
        out.append(
            {
                "tag": release.get("tag_name"),
                "name": release.get("name") or release.get("tag_name"),
                "published": release.get("published_at"),
                "notes_url": release.get("html_url"),
                "asset": asset["name"],
                "size": asset["size"],
                "url": asset["browser_download_url"],
            }
        )
    return out


def _linux_asset(release: dict) -> dict | None:
    best = None
    for asset in release.get("assets") or []:
        name = str(asset.get("name") or "")
        if not name.endswith(".tar.gz") or "linux" not in name:
            continue
        if "x86_64" in name or "amd64" in name:
            return asset
        best = best or asset
    return best


def latest() -> dict:
    found = releases(1)
    if not found:
        raise EngineError("no linux release published for Vapor Forge")
    return found[0]


# -- local state ----------------------------------------------------------


def meta() -> dict:
    if not META.is_file():
        legacy = META.with_name("vapordepot-engine.json")
        if legacy.is_file():
            try:
                legacy.replace(META)
            except OSError:
                return _read_meta(legacy)
        else:
            return {}
    return _read_meta(META)


def _read_meta(path) -> dict:
    try:
        return json.loads(path.read_text("utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def libs() -> dict[str, list[dict]]:
    out: dict[str, list[dict]] = {}
    for arch in ARCHES:
        directory = vfsys.VF_DATA_DIR / arch
        entries = []
        for name in LIBS:
            path = directory / name
            if path.is_file():
                stat = path.stat()
                entries.append(
                    {
                        "name": name,
                        "path": str(path),
                        "size": stat.st_size,
                        "mtime": int(stat.st_mtime),
                        "current": int(time.time()) - int(stat.st_mtime) < 900,
                    }
                )
        out[arch] = entries
    return out


def installed() -> bool:
    return any(entries for entries in libs().values())


def detect() -> dict:
    present = libs()
    audit = vfsys.ld_audit()
    info = meta()
    running = steamctl.is_running()
    loaded = steamctl.audit_loaded() if running else {"loaded": False, "pid": None, "ld_audit": ""}
    missing = [
        f"{arch}/{name}" for arch in ARCHES for name in LIBS
        if not (vfsys.VF_DATA_DIR / arch / name).is_file()
    ]
    return {
        "installed": bool(present.get("x86_64")) and bool(present.get("i686")),
        "partial": bool(present.get("x86_64")) != bool(present.get("i686")),
        "missing": missing,
        "version": info.get("tag"),
        "sha256": info.get("sha256"),
        "installed_at": info.get("installed_at"),
        "source": info.get("url"),
        "libs": present,
        "ld_audit": audit,
        "config_exists": vfsys.VF_CONFIG.is_file(),
        "config_path": str(vfsys.VF_CONFIG),
        "scripts_dir": str(vfsys.VF_SCRIPTS),
        "data_dir": str(vfsys.VF_DATA_DIR),
        "steam_running": running,
        "steam_pid": loaded.get("pid"),
        "steam_engine_loaded": bool(loaded.get("loaded")),
        "steam_ld_audit": loaded.get("ld_audit", ""),
        "needs_restart": bool(running and present.get("x86_64") and not loaded.get("loaded")),
        "lua": _lua_tooling(),
    }


def _lua_tooling() -> dict:
    luac = shutil.which("luac") or shutil.which("luac5.4") or shutil.which("luac5.3")
    return {"luac": luac, "syntax_check": bool(luac)}


# -- download -------------------------------------------------------------


def _cache_tarball(url: str, target: Path) -> dict:
    import urllib.request

    target.parent.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256()
    temp = target.with_suffix(target.suffix + ".part")
    request = urllib.request.Request(url, headers={"User-Agent": "ExxaTools/1.0"})
    try:
        with urllib.request.urlopen(request, timeout=60) as response, temp.open("wb") as handle:
            while True:
                chunk = response.read(1 << 20)
                if not chunk:
                    break
                digest.update(chunk)
                handle.write(chunk)
    except Exception as error:  # noqa: BLE001 - transport surface
        temp.unlink(missing_ok=True)
        raise EngineError(f"download failed: {error}") from error
    os.replace(temp, target)
    return {"path": str(target), "sha256": digest.hexdigest(), "size": target.stat().st_size}


def _extract(archive: Path, tag: str) -> Path:
    target = CACHE_DIR / "releases" / tag
    if (target / "install.sh").is_file():
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix="vf-", dir=target.parent))
    try:
        with tarfile.open(archive, "r:gz") as tar:
            for member in tar.getmembers():
                name = member.name
                if name.startswith("/") or ".." in Path(name).parts:
                    raise EngineError(f"refusing unsafe archive member: {name}")
                if member.isdev():
                    continue
                tar.extract(member, staging)
    except (tarfile.TarError, OSError) as error:
        shutil.rmtree(staging, ignore_errors=True)
        raise EngineError(f"cannot extract {archive.name}: {error}") from error
    roots = [item for item in staging.iterdir() if item.is_dir()]
    source = roots[0] if len(roots) == 1 else staging
    if target.exists():
        shutil.rmtree(target, ignore_errors=True)
    shutil.move(str(source), str(target))
    shutil.rmtree(staging, ignore_errors=True)
    return target


def _run_installer(directory: Path, action: str = "install") -> dict:
    script = directory / "install.sh"
    if not script.is_file():
        raise EngineError(f"{script} is missing from the release archive")
    try:
        result = subprocess.run(
            ["bash", str(script), action],
            cwd=str(directory),
            capture_output=True,
            text=True,
            timeout=INSTALL_TIMEOUT,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise EngineError(f"installer timed out after {INSTALL_TIMEOUT}s") from error
    output = (result.stdout or "") + (result.stderr or "")
    if result.returncode != 0:
        raise EngineError(f"installer exited {result.returncode}\n{output.strip()}")
    return {"returncode": result.returncode, "output": output.strip().splitlines()}


# -- public actions -------------------------------------------------------


def install(tag: str | None = None) -> dict:
    release = latest() if tag is None else _release_by_tag(tag)
    archive_name = Path(release["asset"]).name
    archive = CACHE_DIR / "downloads" / archive_name
    recorded = meta()
    if archive.is_file() and recorded.get("sha256") and recorded.get("tag") == release["tag"]:
        if _sha256(archive) != recorded["sha256"]:
            archive.unlink()
    fetched = _cache_tarball(release["url"], archive) if not archive.is_file() else {
        "path": str(archive),
        "sha256": _sha256(archive),
        "size": archive.stat().st_size,
    }
    directory = _extract(archive, release["tag"])
    installer = _run_installer(directory, "install")
    present = libs()
    if not present.get("x86_64"):
        raise EngineError("installer finished but the x86_64 library is not in place")
    META.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "tag": release["tag"],
        "url": release["url"],
        "asset": archive_name,
        "sha256": fetched["sha256"],
        "size": fetched["size"],
        "installed_at": vfsys.stamp(),
        "release_dir": str(directory),
    }
    META.write_text(json.dumps(payload, indent=2) + "\n", "utf-8")
    return {
        "installed": True,
        "version": release["tag"],
        "sha256": fetched["sha256"],
        "installer": installer,
        "status": detect(),
    }


def _release_by_tag(tag: str) -> dict:
    for release in releases(20):
        if release["tag"] == tag:
            return release
    raise EngineError(f"release {tag} has no linux tarball")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def uninstall() -> dict:
    info = meta()
    directory = Path(info.get("release_dir") or "")
    output: list[str] = []
    if (directory / "install.sh").is_file():
        result = _run_installer(directory, "uninstall")
        output = result["output"]
    else:
        output = _manual_uninstall()
    META.unlink(missing_ok=True)
    return {"installed": False, "removed": True, "output": output, "status": detect()}


def _manual_uninstall() -> list[str]:
    """Fallback when the release directory is gone: undo exactly what it writes."""
    done = []
    for arch in ARCHES:
        for name in LIBS:
            path = vfsys.VF_DATA_DIR / arch / name
            if path.exists():
                path.unlink()
                done.append(f"removed {path}")
    for dropin in vfsys.SYSTEMD_DROPINS:
        if dropin.exists():
            dropin.unlink()
            done.append(f"removed {dropin}")
    if shutil.which("systemctl"):
        subprocess.run(["systemctl", "--user", "daemon-reload"], capture_output=True, check=False)
    return done


# -- log ------------------------------------------------------------------

STEAM_LOG = vfsys.DATA_HOME / "Steam" / "logs" / "console-linux.txt"
LOG_TAG = "[vapor-forge]"
NOISE_TAG = "feat: sharing unlocked app_id="


def log(lines: int = 200) -> dict:
    """The engine's own lines out of Steam's console log."""
    if not STEAM_LOG.is_file():
        return {"path": str(STEAM_LOG), "lines": [], "error": "Steam console log not found"}
    try:
        with STEAM_LOG.open("r", encoding="utf-8", errors="replace") as handle:
            tail = handle.readlines()[-6000:]
    except OSError as error:
        return {"path": str(STEAM_LOG), "lines": [], "error": str(error)}
    picked, skipped = [], 0
    for line in tail:
        if LOG_TAG not in line:
            continue
        if NOISE_TAG in line:
            skipped += 1
            continue
        picked.append(line.rstrip("\n"))
    return {
        "path": str(STEAM_LOG),
        "lines": picked[-max(1, min(lines, 1000)):],
        "total": len(picked),
        "skipped": skipped,
    }


# -- plan validation ------------------------------------------------------


def check_script(text: str, path_hint: str = "plan.lua", plan: bool = True) -> dict:
    """Parse Lua the way the runtime will, without needing Steam running."""
    report = {"path": path_hint, "ok": False, "tool": None, "output": "", "lint": []}
    from . import verify

    report["lint"] = verify.lint_lua(text) if plan else []
    luac = _lua_tooling()["luac"]
    if luac:
        with tempfile.TemporaryDirectory(prefix="vd-lua-") as workdir:
            target = Path(workdir) / "plan.lua"
            target.write_text(text, "utf-8")
            try:
                result = subprocess.run(
                    [luac, "-p", str(target)],
                    capture_output=True,
                    text=True,
                    timeout=LUA_CHECK_TIMEOUT,
                    check=False,
                )
            except subprocess.TimeoutExpired:
                report["output"] = "luac timed out"
                return report
        report["tool"] = luac
        report["output"] = ((result.stdout or "") + (result.stderr or "")).strip()
        report["ok"] = result.returncode == 0 and not report["lint"]
        return report
    report["tool"] = None
    report["ok"] = not report["lint"]
    report["output"] = "luac not installed; only the statement lint ran"
    return report