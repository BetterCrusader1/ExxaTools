"""Steam catalog access: store search, store metadata, and appinfo depots."""

from __future__ import annotations

from . import http

STORE_SEARCH = "https://store.steampowered.com/api/storesearch/?term={term}&l=english&cc={cc}"
STORE_DETAILS = "https://store.steampowered.com/api/appdetails?appids={appid}&l=english&cc={cc}"
APPINFO = "https://api.steamcmd.net/v1/info/{appid}"

BRANCH_FALLBACKS = ("public",)


class SteamError(RuntimeError):
    pass


def search(term: str, cc: str = "us") -> list[dict]:
    term = term.strip()
    if not term:
        return []
    if term.isdigit():
        try:
            info = store_details(int(term), cc)
        except SteamError:
            info = None
        if info:
            return [
                {
                    "appid": int(term),
                    "name": info.get("name", str(term)),
                    "image": info.get("header_image", ""),
                    "price": _price(info),
                }
            ]
        # Not sold in the store (runtimes, tools, delisted apps) - appinfo still
        # knows the name, and an app id is enough to open it.
        try:
            app = app_info(int(term))
        except SteamError:
            app = None
        if app:
            common = app.get("common") or {}
            return [
                {
                    "appid": int(term),
                    "name": common.get("name") or str(term),
                    "image": "",
                    "price": None,
                }
            ]
    payload = http.get_json(STORE_SEARCH.format(term=http.quote(term), cc=cc))
    items = []
    for item in payload.get("items", []):
        if item.get("type") != "app":
            continue
        items.append(
            {
                "appid": int(item["id"]),
                "name": item.get("name", ""),
                "image": item.get("tiny_image", ""),
                "price": _price(item),
            }
        )
    return items


def _price(item: dict) -> str | None:
    price = item.get("price")
    if not isinstance(price, dict):
        return None
    final = price.get("final")
    if final in (None, 0):
        return "free"
    return f"{final / 100:.2f} {price.get('currency', '')}".strip()


def store_details(appid: int, cc: str = "us") -> dict | None:
    payload = http.get_json(STORE_DETAILS.format(appid=appid, cc=cc))
    entry = payload.get(str(appid)) or {}
    if not entry.get("success"):
        return None
    return entry.get("data") or None


def app_info(appid: int) -> dict:
    payload = http.get_json(APPINFO.format(appid=appid))
    if payload.get("status") != "success":
        raise SteamError(f"appinfo for {appid} is unavailable")
    data = payload.get("data") or {}
    info = data.get(str(appid))
    if not info:
        raise SteamError(f"appinfo for {appid} has no entry")
    return info


def catalog(appid: int) -> dict:
    """Normalised description of an app: name, branches, depots, dlc."""
    info = app_info(appid)
    common = info.get("common") or {}
    extended = info.get("extended") or {}
    branches = _branches(info.get("branches") or {})
    depots = _depots(info.get("depots") or {}, branches)
    if not branches:
        branches = _branches_from_depots(depots)
    return {
        "appid": appid,
        "name": (common.get("name") or str(appid)).strip(),
        "type": common.get("type", "unknown"),
        "developers": extended.get("developer") or "",
        "dlc": _dlc_ids(extended),
        "branches": branches,
        "depots": depots,
    }


def _branches(raw: dict) -> list[dict]:
    out = []
    for name, value in raw.items():
        if not isinstance(value, dict):
            continue
        if "buildid" not in value:
            continue
        out.append(
            {
                "name": name,
                "buildid": str(value.get("buildid", "")),
                "description": value.get("description", ""),
                "timeupdated": int(value.get("timeupdated") or 0),
            }
        )
    out.sort(key=lambda item: (item["name"] != "public", -item["timeupdated"]))
    return out


def _branches_from_depots(depots: list[dict]) -> list[dict]:
    names = sorted({name for depot in depots for name in depot["manifests"]})
    ordered = sorted(names, key=lambda name: (name != "public", name))
    return [{"name": name, "buildid": "", "description": "", "timeupdated": 0} for name in ordered]


def _dlc_ids(extended: dict) -> list[int]:
    raw = extended.get("listofdlc") or ""
    ids = []
    for chunk in str(raw).replace(",", " ").split():
        if chunk.isdigit():
            ids.append(int(chunk))
    return ids


def _depots(raw: dict, branches: list[dict]) -> list[dict]:
    branch_names = [branch["name"] for branch in branches] or list(BRANCH_FALLBACKS)
    out = []
    for key, value in raw.items():
        if not key.isdigit() or not isinstance(value, dict):
            continue
        if "manifests" not in value:
            continue
        config = value.get("config") or {}
        manifests = {}
        for branch, entry in (value.get("manifests") or {}).items():
            if not isinstance(entry, dict):
                continue
            gid = entry.get("gid")
            if not gid:
                continue
            manifests[branch] = {
                "gid": str(gid),
                "size": _int(entry.get("size")),
                "download": _int(entry.get("download")),
            }
        if not manifests:
            continue
        oslist = [
            part.strip()
            for part in str(config.get("oslist", "")).split(",")
            if part.strip()
        ]
        out.append(
            {
                "depot_id": int(key),
                "oslist": oslist,
                "osarch": str(config.get("osarch", "")),
                "shared_install": bool(value.get("sharedinstall")),
                "dlc_appid": _int(value.get("dlcappid")),
                "depot_from_app": _int(value.get("depotfromapp")),
                "manifests": manifests,
                "branches": branch_names,
            }
        )
    out.sort(key=lambda item: item["depot_id"])
    return out


def _int(value) -> int | None:
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def pick_manifest(depot: dict, branch: str) -> dict | None:
    manifests = depot["manifests"]
    if branch in manifests:
        return manifests[branch]
    for fallback in BRANCH_FALLBACKS:
        if fallback in manifests:
            return manifests[fallback]
    return None


def selectable(depot: dict, branch: str) -> bool:
    return pick_manifest(depot, branch) is not None
