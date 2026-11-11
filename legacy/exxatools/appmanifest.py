"""Steam appmanifest handling: read what Steam thinks, and stop the loops.

Two failures ruin a manifest install on Linux and both live in
``steamapps/appmanifest_<appid>.acf``:

* "update required" forever, because ``StateFlags`` is not 4 or
  ``BytesDownloaded`` lags ``BytesToDownload``;
* a silent re-download after restart, because the client is allowed to
  schedule its own updates.

Vapor Forge makes the license and the manifests valid; this module makes Steam
believe the install is finished and keeps it that way. Steam rewrites the file
while it runs, so every write reports whether a restart is needed first.
"""

from __future__ import annotations

import os
import re
import shutil
import time
from pathlib import Path

from . import steamctl, vdf

SCALARS = (
    "appid",
    "name",
    "StateFlags",
    "LastUpdated",
    "SizeOnDisk",
    "StagingSize",
    "buildid",
    "LastOwner",
    "BytesToDownload",
    "BytesDownloaded",
    "AutoUpdateBehavior",
    "AllowOtherDownloadsWhileRunning",
    "ScheduledAutoUpdate",
    "InstalledDepots",
    "SharedDepots",
)

FLAG_BITS = {
    1: "uninstalled",
    2: "update required",
    4: "fully installed",
    8: "encrypted",
    16: "locked",
    32: "files missing",
    64: "app running",
    128: "files corrupt",
    256: "update running",
    512: "update paused",
    1024: "update started",
    2048: "uninstalling",
    4096: "backup running",
    8192: "reconfiguring",
    16384: "validating",
    32768: "adding files",
    65536: "preallocating",
    131072: "downloading",
    262144: "staging",
    524288: "committing",
    1048576: "update stopping",
}

FIXES = {
    "fix": {
        "StateFlags": "4",
        "AutoUpdateBehavior": "1",
        "AllowOtherDownloadsWhileRunning": "2",
        "ScheduledAutoUpdate": "0",
        "match_bytes": True,
    },
    "block": {
        "AutoUpdateBehavior": "1",
        "ScheduledAutoUpdate": "0",
    },
    "unblock": {
        "AutoUpdateBehavior": "0",
        "ScheduledAutoUpdate": "1",
    },
}


class ManifestError(RuntimeError):
    pass


# -- discovery ------------------------------------------------------------


def libraries() -> list[Path]:
    """Every ``steamapps`` directory this client knows about."""
    found: list[Path] = []
    root = steamctl.root()
    if root is not None:
        direct = root / "steamapps"
        if direct.is_dir():
            found.append(direct)
        for name in ("libraryfolders.vdf", "libraryfolders.acf"):
            path = direct / name
            if not path.is_file():
                continue
            try:
                tree = vdf.parse(path.read_text("utf-8", "replace"))
            except (OSError, vdf.VdfError):
                continue
            for node in _walk(tree):
                if node.key == "path" and node.value:
                    candidate = Path(node.value) / "steamapps"
                    if candidate.is_dir():
                        found.append(candidate)
    seen, unique = set(), []
    for path in found:
        resolved = path.resolve()
        if resolved in seen:
            continue
        seen.add(resolved)
        unique.append(resolved)
    return unique


def _walk(node: vdf.Node):
    for child in node.children:
        yield child
        yield from _walk(child)


def manifest_path(appid: int | str) -> Path | None:
    name = f"appmanifest_{appid}.acf"
    for library in libraries():
        candidate = library / name
        if candidate.is_file():
            return candidate
    return None


# -- reading --------------------------------------------------------------


def _scalar(tree: vdf.Node, key: str) -> str | None:
    app_state = tree.child("AppState") or tree
    node = app_state.child(key)
    if node is None or node.value is None:
        return None
    return str(node.value).strip().strip('"')


def _depots(tree: vdf.Node) -> list[dict]:
    app_state = tree.child("AppState") or tree
    block = app_state.child("InstalledDepots")
    if block is None:
        return []
    depots = []
    for node in block.children:
        if node.key is None:
            continue
        manifest = node.child("manifest")
        depots.append(
            {
                "depot_id": int(node.key) if str(node.key).isdigit() else node.key,
                "manifest": (manifest.value if manifest else None),
            }
        )
    depots.sort(key=lambda item: str(item["depot_id"]))
    return depots


def read(appid: int | str) -> dict | None:
    path = manifest_path(appid)
    if path is None:
        return None
    try:
        text = path.read_text("utf-8", "replace")
        tree = vdf.parse(text)
    except (OSError, vdf.VdfError) as error:
        raise ManifestError(f"cannot read {path}: {error}") from error
    values = {key: _scalar(tree, key) for key in SCALARS}
    flags = int(values["StateFlags"] or 0) if str(values["StateFlags"] or "0").isdigit() else 0
    return {
        "path": str(path),
        "library": str(path.parent),
        "appid": int(values["appid"] or appid) if str(values["appid"] or appid).isdigit() else appid,
        "name": values["name"],
        "state_flags": flags,
        "state": [label for bit, label in FLAG_BITS.items() if flags & bit],
        "bytes_to_download": _int(values["BytesToDownload"]),
        "bytes_downloaded": _int(values["BytesDownloaded"]),
        "size_on_disk": _int(values["SizeOnDisk"]),
        "last_updated": _int(values["LastUpdated"]),
        "buildid": values["buildid"],
        "auto_update": values["AutoUpdateBehavior"],
        "scheduled_auto_update": values["ScheduledAutoUpdate"],
        "allow_other_downloads": values["AllowOtherDownloadsWhileRunning"],
        "depots": _depots(tree),
        "mtime": int(path.stat().st_mtime),
    }


def _int(value: str | None) -> int | None:
    text = str(value or "").strip()
    if not text.isdigit():
        return None
    return int(text)


# -- reporting ------------------------------------------------------------


def running_apps() -> list[dict]:
    """Apps Steam currently has open (StateFlags bit 64)."""
    out = []
    for library in libraries():
        for path in library.glob("appmanifest_*.acf"):
            try:
                data = read(path.name.split("_", 1)[1].split(".")[0])
            except (ManifestError, OSError):
                continue
            if not data or not data["state_flags"] & 64:
                continue
            out.append({"appid": data["appid"], "name": data["name"]})
    return out


def report(appid: int | str, plan: dict | None = None) -> dict:
    data = read(appid)
    running = steamctl.is_running()
    if data is None:
        return {
            "appid": appid,
            "installed": False,
            "steam_running": running,
            "libraries": [str(path) for path in libraries()],
            "problems": [
                "no appmanifest yet: let Steam download the app once, then fix it here"
            ],
            "depots": [],
        }
    problems = []
    if data["state_flags"] and not data["state_flags"] & 4:
        problems.append(f"StateFlags {data['state_flags']} is not fully installed ({', '.join(data['state']) or 'none'})")
    if (
        data["bytes_to_download"] is not None
        and data["bytes_downloaded"] is not None
        and data["bytes_downloaded"] < data["bytes_to_download"]
    ):
        problems.append(
            f"{data['bytes_to_download'] - data['bytes_downloaded']} bytes still marked pending"
        )
    if str(data["auto_update"] or "") in ("", "0"):
        problems.append("automatic updates enabled: Steam may re-download on its own schedule")
    if str(data["scheduled_auto_update"] or "0") not in ("0", ""):
        problems.append("scheduled auto update enabled")

    planned = []
    if plan:
        installed_ids = {str(item["depot_id"]) for item in data["depots"]}
        for depot in plan.get("depots") or []:
            depot_id = str(depot.get("depot_id"))
            current = next(
                (item for item in data["depots"] if str(item["depot_id"]) == depot_id), None
            )
            planned.append(
                {
                    "depot_id": depot.get("depot_id"),
                    "planned_gid": depot.get("gid"),
                    "steam_gid": current["manifest"] if current else None,
                    "in_appmanifest": depot_id in installed_ids,
                    "pinned": bool(current and depot.get("gid") and str(current["manifest"]) == str(depot["gid"])),
                }
            )
    return {
        "appid": data["appid"],
        "installed": True,
        "name": data["name"],
        "path": data["path"],
        "library": data["library"],
        "steam_running": running,
        "state_flags": data["state_flags"],
        "state": data["state"],
        "bytes_to_download": data["bytes_to_download"],
        "bytes_downloaded": data["bytes_downloaded"],
        "size_on_disk": data["size_on_disk"],
        "last_updated": data["last_updated"],
        "auto_update": data["auto_update"],
        "scheduled_auto_update": data["scheduled_auto_update"],
        "allow_other_downloads": data["allow_other_downloads"],
        "depots": data["depots"],
        "planned_depots": planned,
        "problems": problems,
        "libraries": [str(path) for path in libraries()],
    }


# -- writing --------------------------------------------------------------


def _set_key(text: str, key: str, value: str) -> tuple[str, bool]:
    pattern = re.compile(rf'("{re.escape(key)}"\s*)"[^"]*"')
    if pattern.search(text):
        return pattern.sub(lambda match: match.group(1) + f'"{value}"', text, count=1), True
    close = text.rstrip().rfind("}")
    if close == -1:
        raise ManifestError("appmanifest has no closing brace")
    line_start = text.rfind("\n", 0, close) + 1
    indent = text[line_start:close] if set(text[line_start:close]) <= set("\t ") else "\t"
    inserted = f'{indent}"{key}"\t\t"{value}"\n'
    return text[:line_start] + inserted + text[line_start:], False


def patch(appid: int | str, mode: str = "fix", steam_check: bool = True) -> dict:
    if mode not in FIXES:
        raise ManifestError(f"unknown mode {mode!r}; use {', '.join(FIXES)}")
    path = manifest_path(appid)
    if path is None:
        raise ManifestError("no appmanifest for that app yet")
    running = steamctl.is_running()
    if steam_check and running:
        raise ManifestError(
            "Steam is running and will overwrite the file: close Steam, patch, then start it"
        )
    text = path.read_text("utf-8", "replace")
    before = read(appid) or {}
    changes = []
    for key, value in FIXES[mode].items():
        if key == "match_bytes":
            continue
        text, replaced = _set_key(text, key, value)
        changes.append({"key": key, "value": value, "updated": replaced})
    if FIXES[mode].get("match_bytes"):
        to_download = before.get("bytes_to_download")
        if to_download is not None:
            text, replaced = _set_key(text, "BytesDownloaded", str(to_download))
            changes.append({"key": "BytesDownloaded", "value": str(to_download), "updated": replaced})
        text, _ = _set_key(text, "LastUpdated", str(int(time.time())))
    backup = path.with_suffix(path.suffix + ".exxatools.bak")
    if not backup.exists():
        shutil.copy2(path, backup)
    temp = path.with_suffix(".tmp")
    temp.write_text(text, "utf-8")
    os.replace(temp, path)
    try:
        tree = vdf.parse(text)
        if tree.child("AppState") is None:
            raise vdf.VdfError("AppState block disappeared")
    except vdf.VdfError as error:
        shutil.copy2(backup, path)
        raise ManifestError(f"patched file failed validation, restored backup: {error}") from error
    return {
        "appid": appid,
        "path": str(path),
        "mode": mode,
        "backup": str(backup),
        "changes": changes,
        "before": {
            "state_flags": before.get("state_flags"),
            "auto_update": before.get("auto_update"),
            "scheduled_auto_update": before.get("scheduled_auto_update"),
        },
        "after": report(appid),
        "steam_running": running,
        "warning": "start Steam again so the patched appmanifest is the one it loads" if not running else None,
    }