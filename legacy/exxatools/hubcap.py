"""Hubcap Manifest: depot keys and pinned manifests, one app at a time.

Hubcap hands out a free API key (``smm_...``) and returns a zip per app that
contains a ``.lua`` with the depot list, depot keys, and the manifest ids of a
snapshot build. That is exactly the missing half of a plan: keys are the thing
Steam never gives you and nobody can derive.

The client here stays small on purpose: no Hubcap-specific state on disk beyond
the key, no background refresh. Fetch, parse, hand the result to the store.
"""

from __future__ import annotations

import io
import json
import re
import urllib.error
import urllib.parse
import urllib.request
import zipfile

BASE = "https://hubcapmanifest.com/api/v1"
USER_AGENT = "ExxaTools/1.0 (Linux)"
TIMEOUT = 30.0
KEY_PREFIX = "smm"


class HubcapError(RuntimeError):
    def __init__(self, message: str, status: int | None = None) -> None:
        super().__init__(message)
        self.status = status


def _request(path: str, key: str, params: dict | None = None, timeout: float = TIMEOUT) -> bytes:
    if not key:
        raise HubcapError("no Hubcap API key set")
    url = BASE + path
    if params:
        url += "?" + urllib.parse.urlencode(params)
    request = urllib.request.Request(
        url,
        headers={
            "Authorization": f"Bearer {key.strip()}",
            "User-Agent": USER_AGENT,
            "Accept": "application/json, application/zip, text/plain, */*",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        detail = ""
        try:
            detail = error.read(600).decode("utf-8", "replace")
        except OSError:
            pass
        if error.code == 401:
            raise HubcapError("Hubcap rejected the API key", 401) from error
        if error.code == 429:
            raise HubcapError("Hubcap daily limit reached", 429) from error
        message = f"Hubcap returned HTTP {error.code}"
        if detail:
            message += f": {detail.strip()[:200]}"
        raise HubcapError(message, error.code) from error
    except urllib.error.URLError as error:
        raise HubcapError(f"cannot reach Hubcap: {error.reason}") from error
    except TimeoutError as error:
        raise HubcapError("Hubcap timed out") from error


def _json(path: str, key: str, params: dict | None = None):
    raw = _request(path, key, params)
    try:
        return json.loads(raw.decode("utf-8", "replace"))
    except json.JSONDecodeError as error:
        raise HubcapError("Hubcap returned a non-JSON payload") from error


def valid_key(value: str) -> str | None:
    text = (value or "").strip()
    if not text.startswith(KEY_PREFIX) or len(text) < 16:
        return None
    if any(ch.isspace() for ch in text):
        return None
    return text


# -- account --------------------------------------------------------------


def stats(key: str) -> dict:
    payload = _json("/user/stats", key)
    if not isinstance(payload, dict):
        raise HubcapError("unexpected /user/stats payload")
    return {
        "usage": payload.get("daily_usage"),
        "limit": payload.get("daily_limit"),
        "can_request": payload.get("can_make_requests"),
    }


def validate(key: str) -> dict:
    info = stats(key)
    return {"ok": bool(info.get("can_request")), **info}


# -- discovery ------------------------------------------------------------


def search(key: str, term: str, limit: int = 25) -> list[dict]:
    payload = _json("/search", key, {"q": term, "limit": max(1, min(limit, 100))})
    items = payload.get("results") if isinstance(payload, dict) else payload
    out = []
    for item in items or []:
        appid = item.get("game_id", item.get("appid"))
        if str(appid or "").isdigit():
            out.append(
                {
                    "appid": int(appid),
                    "name": item.get("game_name", item.get("name", "")),
                    "updated": item.get("uploaded_date", item.get("last_updated", "")),
                    "size": item.get("manifest_size", 0),
                }
            )
    return out


def library(key: str, limit: int = 100, offset: int = 0, term: str | None = None) -> dict:
    params = {"limit": max(1, min(limit, 500)), "offset": max(0, offset), "sort_by": "updated"}
    if term:
        params["search"] = term
    payload = _json("/library", key, params)
    games = []
    for item in payload.get("games", []) if isinstance(payload, dict) else []:
        appid = item.get("game_id", item.get("appid"))
        if str(appid or "").isdigit():
            games.append(
                {
                    "appid": int(appid),
                    "name": item.get("game_name", item.get("name", "")),
                    "updated": item.get("uploaded_date", item.get("last_updated", "")),
                }
            )
    return {
        "games": games,
        "total": payload.get("total_count", len(games)) if isinstance(payload, dict) else len(games),
        "offset": offset,
    }


# -- fetch and parse ------------------------------------------------------


def fetch(key: str, appid: int) -> dict:
    """Pull one app's archive: the lua text plus any bundled manifest blobs."""
    raw = _request(f"/manifest/{appid}", key)
    if raw[:2] == b"PK":
        return _from_zip(raw, appid)
    text = raw.decode("utf-8", "replace")
    stripped = text.lstrip()
    if stripped.startswith("<"):
        raise HubcapError(f"app {appid} is not in the Hubcap database", 404)
    if stripped.startswith("{"):
        raise HubcapError(f"Hubcap answered with JSON for {appid}: {stripped[:160]}")
    return {"appid": appid, "lua": text, "manifests": [], "source": "lua"}


def _from_zip(raw: bytes, appid: int) -> dict:
    try:
        archive = zipfile.ZipFile(io.BytesIO(raw))
    except zipfile.BadZipFile as error:
        raise HubcapError("Hubcap archive is not a valid zip") from error
    lua = None
    manifests = []
    with archive:
        for info in archive.infolist():
            name = info.filename
            lower = name.lower()
            if lower.endswith(".lua") and lua is None:
                lua = archive.read(info).decode("utf-8", "replace")
            elif lower.endswith(".manifest"):
                manifests.append({"name": name.rsplit("/", 1)[-1], "bytes": archive.read(info)})
    if lua is None:
        raise HubcapError(f"Hubcap archive for {appid} has no .lua inside")
    return {"appid": appid, "lua": lua, "manifests": manifests, "source": "zip"}


ADDAPPID = re.compile(r"^\s*addappid\s*\(([^)]*)\)", re.IGNORECASE)
SETMANIFEST = re.compile(r"^\s*setmanifestid\s*\(([^)]*)\)", re.IGNORECASE)
ARG = re.compile(r'"([^"]*)"|\'([^\']*)\'|([^,\s]+)')


def _args(raw: str) -> list[str]:
    out = []
    for match in ARG.finditer(raw):
        value = match.group(1) or match.group(2) or match.group(3) or ""
        out.append(value.strip())
    return out


def parse_lua(text: str, appid: int | None = None) -> dict:
    """Read a SteamTools/Hubcap lua into apps, depots with keys, and pins."""
    apps: list[int] = []
    depots: dict[int, dict] = {}
    pins: dict[int, dict] = {}
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("--"):
            continue
        match = ADDAPPID.match(stripped)
        if match:
            args = _args(match.group(1))
            if not args or not args[0].isdigit():
                continue
            target = int(args[0])
            key = args[2].lower() if len(args) >= 3 else ""
            if key and re.fullmatch(r"[0-9a-f]{64}", key):
                depots.setdefault(target, {"depot_id": target, "key": key, "gid": None, "size": None})
            else:
                apps.append(target)
            continue
        match = SETMANIFEST.match(stripped)
        if not match:
            continue
        args = _args(match.group(1))
        if len(args) < 2 or not args[0].isdigit() or not args[1].isdigit():
            continue
        size = int(args[2]) if len(args) > 2 and args[2].isdigit() else None
        pins[int(args[0])] = {"gid": args[1], "size": size}
    for depot_id, pin in pins.items():
        depot = depots.setdefault(depot_id, {"depot_id": depot_id, "key": "", "gid": None, "size": None})
        depot["gid"] = pin["gid"]
        depot["size"] = pin["size"]
    root = appid if appid is not None else (apps[0] if apps else None)
    dlc = [value for value in apps if root is None or value != root]
    return {
        "appid": root,
        "apps": apps,
        "dlc": dlc,
        "depots": sorted(depots.values(), key=lambda item: item["depot_id"]),
    }


def empty_key_depots(parsed: dict) -> list[int]:
    return [depot["depot_id"] for depot in parsed["depots"] if not depot["key"]]


# -- import ---------------------------------------------------------------


def apply_to_state(
    state: dict,
    parsed: dict,
    stamp: str,
    name: str | None = None,
    pin: bool = False,
    create: bool = False,
) -> dict:
    """Write Hubcap's keys (and optionally its pins) into the state dict.

    Keys always land: a key is a key, whatever branch you plan to install.
    Manifests move only with ``pin`` because Hubcap snapshots a build, and that
    build is not necessarily the branch you selected. A plan is created only
    when asked, so a fetch cannot silently replace a hand-tuned depot list.
    """
    appid = parsed.get("appid")
    if appid is None:
        raise HubcapError("that lua has no app id")
    imported = 0
    for depot in parsed["depots"]:
        key = depot.get("key")
        if key and len(key) == 64:
            state["depot_keys"][str(depot["depot_id"])] = key
            imported += 1

    record = state["apps"].get(str(appid))
    created = False
    if record is None and create:
        record = {
            "appid": appid,
            "name": name or str(appid),
            "branch": "public",
            "depots": [
                {
                    "depot_id": depot["depot_id"],
                    "gid": depot.get("gid") or "",
                    "size": depot.get("size"),
                }
                for depot in parsed["depots"]
            ],
            "dlc": parsed.get("dlc") or [],
            "source": "hubcap",
            "fixes": {"ticket": "forge", "steam_id": "", "avatar": 0, "purchase_time": 0, "online_fix": False},
            "updated": stamp,
        }
        state["apps"][str(appid)] = record
        created = True

    pinned = 0
    if record is not None and pin:
        planned = {int(depot["depot_id"]): depot for depot in record.get("depots") or []}
        for depot in parsed["depots"]:
            entry = planned.get(depot["depot_id"])
            if entry is None or not depot.get("gid"):
                continue
            entry["gid"] = depot["gid"]
            if depot.get("size"):
                entry["size"] = depot["size"]
            pinned += 1
        if parsed.get("dlc"):
            known = {int(value) for value in record.get("dlc") or []}
            record["dlc"] = sorted(known | {value for value in parsed["dlc"]})
        record["updated"] = stamp

    return {
        "appid": appid,
        "keys": imported,
        "depots": len(parsed["depots"]),
        "pinned": pinned,
        "created_plan": created,
        "missing_keys": empty_key_depots(parsed),
        "dlc": parsed.get("dlc") or [],
    }