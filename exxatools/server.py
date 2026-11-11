"""Local web API and static front end for ExxaTools."""

from __future__ import annotations

import json
import mimetypes
import threading
import traceback
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

from . import (
    appmanifest,
    configfile,
    desktop,
    engine,
    http,
    hubcap,
    lua,
    providers,
    sources,
    steam,
    steamctl,
    steamui,
    store,
    verify,
    vfsys,
)

WEB_DIR = Path(__file__).resolve().parent.parent / "web"
MAX_BODY = 1 << 20


class ApiError(RuntimeError):
    def __init__(self, message: str, status: int = 400) -> None:
        super().__init__(message)
        self.status = status


def _int_param(value, field: str) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        raise ApiError(f"{field} must be an integer") from None


def _fixes(raw) -> dict:
    raw = raw if isinstance(raw, dict) else {}
    ticket = str(raw.get("ticket") or "forge")
    if ticket not in ("forge", "delegate"):
        raise ApiError("ticket must be forge or delegate")
    steam_id = str(raw.get("steam_id") or "").strip()
    if steam_id:
        if not steam_id.isdigit() or not (76561197960265728 < int(steam_id) < 2**63):
            raise ApiError("steam_id must be a SteamID64")
    avatar = raw.get("avatar") or 0
    try:
        avatar = int(avatar)
    except (TypeError, ValueError):
        raise ApiError("avatar must be an app id") from None
    try:
        purchase_time = int(raw.get("purchase_time") or 0)
    except (TypeError, ValueError):
        raise ApiError("purchase_time must be an integer timestamp") from None
    return {
        "ticket": ticket,
        "steam_id": steam_id,
        "avatar": avatar,
        "purchase_time": purchase_time,
        "online_fix": bool(raw.get("online_fix")),
    }


def _plan_depots(raw) -> list[dict]:
    if not isinstance(raw, list):
        raise ApiError("depots must be a list")
    depots = []
    for item in raw:
        if not isinstance(item, dict):
            raise ApiError("each depot must be an object")
        depot_id = _int_param(item.get("depot_id"), "depot_id")
        gid = str(item.get("gid") or "").strip()
        if gid and not gid.isdigit():
            raise ApiError(f"depot {depot_id}: gid must be decimal")
        size = item.get("size")
        depots.append(
            {
                "depot_id": depot_id,
                "gid": gid,
                "size": int(size) if size not in (None, "", 0) else None,
            }
        )
    return depots


class Handler(BaseHTTPRequestHandler):
    server_version = "ExxaTools/1.0"
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt: str, *args) -> None:
        if self.server.verbose:  # type: ignore[attr-defined]
            print(f"[http] {self.address_string()} {fmt % args}")

    # -- plumbing ---------------------------------------------------------

    def _send(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def _json(self, payload, status: int = 200) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self._send(status, body, "application/json; charset=utf-8")

    def _read_json(self) -> dict:
        length = int(self.headers.get("Content-Length") or 0)
        if length > MAX_BODY:
            raise ApiError("request body too large", 413)
        if not length:
            return {}
        try:
            payload = json.loads(self.rfile.read(length))
        except json.JSONDecodeError:
            raise ApiError("body must be JSON") from None
        if not isinstance(payload, dict):
            raise ApiError("body must be a JSON object")
        return payload

    def do_GET(self) -> None:
        self._dispatch("GET")

    def do_POST(self) -> None:
        self._dispatch("POST")

    def do_DELETE(self) -> None:
        self._dispatch("DELETE")

    def do_OPTIONS(self) -> None:
        self.send_response(204)
        self.send_header("Content-Length", "0")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")
        self.end_headers()

    def _dispatch(self, method: str) -> None:
        parsed = urlparse(self.path)
        query = {key: values[0] for key, values in parse_qs(parsed.query).items()}
        try:
            if parsed.path.startswith("/api/"):
                result = self._route(method, parsed.path[len("/api/"):], query)
                if result is not None:
                    self._json(result)
            elif method == "GET":
                self._static(parsed.path)
            else:
                raise ApiError("not found", 404)
        except ApiError as error:
            self._json({"error": str(error)}, error.status)
        except (engine.EngineError, appmanifest.ManifestError, ValueError) as error:
            self._json({"error": str(error)}, 400)
        except hubcap.HubcapError as error:
            self._json({"error": str(error)}, 401 if error.status == 401 else 502)
        except (http.HttpError, steam.SteamError, providers.ProviderError, store.StateError) as error:
            self._json({"error": str(error)}, 502)
        except OSError as error:
            self._json({"error": str(error)}, 500)
        except Exception:  # noqa: BLE001 - surface unexpected failures as JSON
            traceback.print_exc()
            self._json({"error": "internal error"}, 500)

    # -- routes -----------------------------------------------------------

    def _route(self, method: str, path: str, query: dict):
        if path == "status" and method == "GET":
            return vfsys.status()

        if path == "engine" and method == "GET":
            return engine.detect()

        if path == "engine/releases" and method == "GET":
            return {"releases": engine.releases(5)}

        if path == "engine/install" and method == "POST":
            body = self._read_json()
            tag = str(body.get("tag") or "").strip() or None
            return engine.install(tag)

        if path == "engine/uninstall" and method == "POST":
            return engine.uninstall()

        if path == "log" and method == "GET":
            lines = _int_param(query.get("lines") or 150, "lines")
            return engine.log(lines)

        if path == "script/check" and method == "POST":
            body = self._read_json()
            text = body.get("text")
            hint = "inline"
            if not isinstance(text, str) or not text.strip():
                appid = _int_param(body.get("appid"), "appid")
                state = store.load()
                record = store.app_record(state, appid)
                if record is None:
                    raise ApiError("save a plan first", 409)
                text = lua.render_from_record(record, state["depot_keys"])
                hint = f"{appid}.lua"
            return engine.check_script(text, hint)

        if path == "provider":
            state = store.load()
            if method == "GET":
                text = lua.render_provider(
                    state["settings"]["manifest_providers"], vfsys.stamp()
                )
                return {"text": text, "state": vfsys.provider_state()}
            if method == "POST":
                body = self._read_json()
                order = body.get("providers") or state["settings"]["manifest_providers"]
                if not isinstance(order, list):
                    raise ApiError("providers must be a list")
                text = lua.render_provider(order, vfsys.stamp())
                check = engine.check_script(text, lua.PROVIDER_FILE, plan=False)
                if not check["ok"]:
                    raise ApiError(f"provider script failed to parse: {check['output']}", 500)
                target = vfsys.write_provider(text)
                return {
                    "path": str(target),
                    "state": vfsys.provider_state(),
                    "check": check,
                    "reload": "Vapor Forge hot-reloads its script directory; no restart needed",
                }
            if method == "DELETE":
                return {"removed": vfsys.remove_provider(), "state": vfsys.provider_state()}

        if path == "open" and method == "POST":
            body = self._read_json()
            return desktop.open_target(str(body.get("target") or ""))

        if path == "app/launch" and method == "POST":
            return desktop.launch_app()

        if path == "steamui":
            if method == "GET":
                return steamui.status()
            body = self._read_json()
            action = str(body.get("action") or "inject")
            if action == "inject":
                return steamui.inject()
            if action == "remove":
                return steamui.remove()
            if action == "enable":
                return steamui.enable_debug()
            raise ApiError("action must be inject, remove, or enable")

        if path == "hubcap":
            state = store.load()
            key = str(state["settings"].get("hubcap_key") or "")
            if method == "GET":
                info = {
                    "configured": bool(key),
                    "hint": f"{key[:8]}…" if key else "",
                    "providers": [spec["name"] for spec in (providers.resolve(entry) for entry in state["settings"]["manifest_providers"]) if spec],
                }
                if key:
                    try:
                        info["stats"] = hubcap.validate(key)
                    except hubcap.HubcapError as error:
                        info["error"] = str(error)
                return info
            if method == "POST":
                body = self._read_json()
                action = str(body.get("action") or "save")
                if action == "save":
                    raw = str(body.get("key") or "").strip()
                    if not raw:
                        store.update(lambda current: current["settings"].__setitem__("hubcap_key", ""))
                        return {"configured": False}
                    valid = hubcap.valid_key(raw)
                    if valid is None:
                        raise ApiError("a Hubcap key starts with smm and has no spaces")
                    info = hubcap.validate(valid)
                    store.update(lambda current: current["settings"].__setitem__("hubcap_key", valid))
                    return {"configured": True, "stats": info}
                if action == "search":
                    if not key:
                        raise ApiError("set a Hubcap API key first", 409)
                    term = str(body.get("query") or "").strip()
                    if not term:
                        raise ApiError("query is required")
                    return {"items": hubcap.search(key, term)}
                if action == "fetch":
                    if not key:
                        raise ApiError("set a Hubcap API key first", 409)
                    appid = _int_param(body.get("appid"), "appid")
                    fetched = hubcap.fetch(key, appid)
                    parsed = hubcap.parse_lua(fetched["lua"], appid)
                    summary = store.update(
                        lambda current: hubcap.apply_to_state(
                            current,
                            parsed,
                            vfsys.stamp(),
                            name=str(body.get("name") or "") or None,
                            pin=bool(body.get("pin")),
                            create=bool(body.get("create")),
                        )
                    )
                    return {
                        "summary": summary,
                        "manifests": [item["name"] for item in fetched["manifests"]],
                        "source": fetched["source"],
                    }
                raise ApiError("action must be save, search, or fetch")

        if path == "appmanifest":
            if method == "GET":
                appid = _int_param(query.get("appid"), "appid")
                state = store.load()
                return appmanifest.report(appid, store.app_record(state, appid))
            if method == "POST":
                body = self._read_json()
                appid = _int_param(body.get("appid"), "appid")
                mode = str(body.get("mode") or "fix")
                force = bool(body.get("force"))
                restart = bool(body.get("restart"))
                if restart and steamctl.is_running():
                    busy = appmanifest.running_apps()
                    if busy:
                        raise ApiError(
                            "a game is running ("
                            + ", ".join(str(app["name"] or app["appid"]) for app in busy)
                            + "): close it before restarting Steam"
                        )
                    steamctl.shutdown()
                    result = appmanifest.patch(appid, mode, steam_check=False)
                    steamctl.start()
                    result["restarted"] = True
                    return result
                return appmanifest.patch(appid, mode, steam_check=not force)

        if path == "search" and method == "GET":
            term = query.get("q", "")
            cc = query.get("cc") or store.load()["settings"]["country"]
            return {"items": steam.search(term, cc)}

        if path == "app" and method == "GET":
            appid = _int_param(query.get("appid"), "appid")
            branch = query.get("branch") or "public"
            return self._app_view(appid, branch)

        if path == "key":
            if method == "GET":
                depot_id = _int_param(query.get("depot_id"), "depot_id")
                return {"depot_id": depot_id, "key": store.load()["depot_keys"].get(str(depot_id))}
            if method == "POST":
                body = self._read_json()
                depot_id = _int_param(body.get("depot_id"), "depot_id")
                key = store.valid_key(str(body.get("key") or ""))
                if key is None:
                    raise ApiError("a depot key is 64 hex characters")
                store.update(lambda state: store.set_key(state, depot_id, key))
                return {"depot_id": depot_id, "key": key}
            if method == "DELETE":
                depot_id = _int_param(query.get("depot_id"), "depot_id")
                store.update(lambda state: store.set_key(state, depot_id, None))
                return {"depot_id": depot_id, "key": None}

        if path == "key/fetch" and method == "POST":
            body = self._read_json()
            depot_id = _int_param(body.get("depot_id"), "depot_id")
            appid = body.get("appid")
            return self._fetch_key(depot_id, _int_param(appid, "appid") if appid else None)

        if path == "sources" and method == "GET":
            state = store.load()
            return {
                "sources": sources.catalog(state["settings"]),
                "default": state["settings"].get("default_source", "auto"),
            }

        if path == "sources" and method == "POST":
            return self._save_source(self._read_json())

        if path == "sources" and method == "DELETE":
            name = str(query.get("name") or "")
            return self._delete_source(name)

        if path == "fixes" and method == "POST":
            body = self._read_json()
            appid = _int_param(body.get("appid"), "appid")
            fixes = _fixes(body.get("fixes"))
            source_name = body.get("source")
            dlc = body.get("dlc")

            def mutate(state):
                record = state["apps"].get(str(appid))
                if record is None:
                    raise ApiError("save a plan first", 409)
                record["fixes"] = fixes
                if source_name is not None:
                    record["source"] = str(source_name)
                if dlc is not None:
                    record["dlc"] = [_int_param(value, "dlc") for value in dlc]
                record["updated"] = vfsys.stamp()
                return record

            return store.update(mutate)

        if path == "launchoptions" and method == "GET":
            appid = _int_param(query.get("appid"), "appid")
            return steamctl.read_launch_options(appid)

        if path == "launchoptions" and method == "POST":
            body = self._read_json()
            appid = _int_param(body.get("appid"), "appid")
            options = str(body.get("options") or "")
            return steamctl.set_launch_options(appid, options)

        if path == "config/preview" and method == "GET":
            state = store.load()
            plans = {int(key): value for key, value in state["apps"].items()}
            return configfile.preview(vfsys.VF_CONFIG, plans)

        if path == "apply" and method == "POST":
            body = self._read_json()
            appid = _int_param(body.get("appid"), "appid")
            return self._apply(appid)

        if path == "verify" and method == "GET":
            appid = _int_param(query.get("appid"), "appid")
            return verify.run(appid)

        if path == "steam" and method == "GET":
            appid = query.get("appid")
            account = steamctl.active_account()
            return {
                "root": str(steamctl.root()) if steamctl.root() else None,
                "running": steamctl.is_running(),
                "account": account[1] if account else None,
                "localconfig": str(steamctl.localconfig_path() or ""),
                "launch_options": steamctl.read_launch_options(int(appid)).get("options")
                if appid and str(appid).isdigit()
                else None,
            }

        if path == "steam/restart" and method == "POST":
            return steamctl.restart()

        if path == "steam/shutdown" and method == "POST":
            return {"stopped": steamctl.shutdown()}

        if path == "desktop" and method == "POST":
            body = self._read_json()
            action = str(body.get("action") or "install")
            if action == "install":
                return desktop.install_desktop_entry()
            if action == "uninstall":
                return desktop.uninstall_desktop_entry()
            raise ApiError("action must be install or uninstall")

        if path == "code" and method == "POST":
            body = self._read_json()
            appid = _int_param(body.get("appid"), "appid")
            depot_id = _int_param(body.get("depot_id"), "depot_id")
            gid = str(body.get("gid") or "").strip()
            if not gid.isdigit():
                raise ApiError("gid must be decimal")
            return self._code(appid, depot_id, gid)

        if path == "codes" and method == "POST":
            body = self._read_json()
            appid = _int_param(body.get("appid"), "appid")
            depots = _plan_depots(body.get("depots"))
            return self._codes(appid, depots)

        if path == "plan":
            if method == "GET":
                appid = _int_param(query.get("appid"), "appid")
                state = store.load()
                record = store.app_record(state, appid)
                if record is None:
                    raise ApiError("no saved plan", 404)
                return record
            if method == "POST":
                body = self._read_json()
                appid = _int_param(body.get("appid"), "appid")
                depots = _plan_depots(body.get("depots"))
                record = {
                    "appid": appid,
                    "name": str(body.get("name") or appid).strip(),
                    "branch": str(body.get("branch") or "public"),
                    "depots": depots,
                    "dlc": [_int_param(value, "dlc") for value in (body.get("dlc") or [])],
                    "source": str(body.get("source") or "auto"),
                    "fixes": _fixes(body.get("fixes")),
                    "updated": vfsys.stamp(),
                }

                def mutate(state):
                    state["apps"][str(appid)] = record
                    return record

                return store.update(mutate)
            if method == "DELETE":
                appid = _int_param(query.get("appid"), "appid")
                store.update(lambda state: state["apps"].pop(str(appid), None))
                vfsys.remove_script(appid)
                return {"removed": appid}

        if path == "script" and method == "GET":
            appid = _int_param(query.get("appid"), "appid")
            state = store.load()
            record = store.app_record(state, appid)
            if record is None:
                raise ApiError("no saved plan", 404)
            return {
                "appid": appid,
                "script": lua.render_from_record(record, state["depot_keys"]),
                "installed": vfsys.script_path(appid).exists(),
            }

        if path == "install" and method == "POST":
            body = self._read_json()
            appid = _int_param(body.get("appid"), "appid")
            enabled = bool(body.get("enabled", True))
            state = store.load()
            record = store.app_record(state, appid)
            if record is None:
                raise ApiError("save a plan first", 409)
            text = lua.render_from_record(record, state["depot_keys"])
            target = vfsys.write_script(appid, text, enabled=enabled)
            return {
                "path": str(target),
                "script": text,
                "missing_keys": lua.missing_keys(record["depots"], state["depot_keys"]),
            }

        if path == "toggle" and method == "POST":
            body = self._read_json()
            appid = _int_param(body.get("appid"), "appid")
            target = vfsys.toggle_script(appid, bool(body.get("enabled", True)))
            if target is None:
                raise ApiError("script not installed", 404)
            return {"path": str(target), "enabled": target.name.endswith(".lua")}

        if path == "script" and method == "DELETE":
            appid = _int_param(query.get("appid"), "appid")
            return {"removed": vfsys.remove_script(appid)}

        if path == "settings":
            if method == "GET":
                return store.load()["settings"]
            if method == "POST":
                return self._save_settings(self._read_json())

        raise ApiError(f"unknown endpoint {method} /api/{path}", 404)

    # -- handlers ---------------------------------------------------------

    def _app_view(self, appid: int, branch: str) -> dict:
        catalog = steam.catalog(appid)
        state = store.load()
        keys = state["depot_keys"]
        depots = []
        for depot in catalog["depots"]:
            depot_id = depot["depot_id"]
            manifest = steam.pick_manifest(depot, branch)
            key = keys.get(str(depot_id))
            depots.append(
                {
                    "depot_id": depot_id,
                    "oslist": depot["oslist"],
                    "osarch": depot["osarch"],
                    "shared_install": depot["shared_install"],
                    "dlc_appid": depot["dlc_appid"],
                    "gid": manifest["gid"] if manifest else "",
                    "size": manifest["size"] if manifest else None,
                    "download": manifest["download"] if manifest else None,
                    "branches": sorted(depot["manifests"]),
                    "has_key": bool(key),
                    "key": key,
                    "code": store.cached_code(state, manifest["gid"]) if manifest else None,
                }
            )
        record = store.app_record(state, appid) or None
        try:
            launch = steamctl.read_launch_options(appid)
        except steamctl.SteamError as error:
            launch = {"found": False, "options": None, "error": str(error)}
        return {
            "appid": appid,
            "name": catalog["name"],
            "type": catalog["type"],
            "developers": catalog["developers"],
            "branch": branch,
            "branches": catalog["branches"],
            "dlc": catalog["dlc"],
            "depots": depots,
            "plan": record,
            "installed": vfsys.script_path(appid).exists(),
            "source": (record or {}).get("source") or state["settings"].get("default_source", "auto"),
            "sources": sources.catalog(state["settings"]),
            "launch_options": launch,
        }

    def _code(self, appid: int, depot_id: int, gid: str, source_name: str | None = None) -> dict:
        state = store.load()
        cached = store.cached_code(state, gid)
        if cached is not None:
            return {"gid": gid, "depot_id": depot_id, "code": cached, "provider": "cache"}
        plan = store.app_record(state, appid) or {}
        chosen = source_name or plan.get("source")
        source = sources.resolve(state["settings"], chosen)
        result = providers.fetch_code(
            appid, depot_id, gid, order=source["providers"]
        )

        def mutate(current):
            store.remember_code(current, gid, result["code"])

        store.update(mutate)
        return {
            "gid": gid,
            "depot_id": depot_id,
            "code": result["code"],
            "provider": result["provider"],
            "source": source["name"],
            "failures": result["failures"],
        }

    def _codes(self, appid: int, depots: list[dict]) -> dict:
        targets = [depot for depot in depots if depot.get("gid")]
        if not targets:
            raise ApiError("no depots with a manifest id")
        results: dict[str, dict] = {}
        errors: dict[int, str] = {}

        def work(depot: dict) -> None:
            try:
                results[depot["gid"]] = self._code(appid, depot["depot_id"], depot["gid"])
            except (providers.ProviderError, http.HttpError, ApiError) as error:
                errors[depot["depot_id"]] = str(error)

        with ThreadPoolExecutor(max_workers=3) as pool:
            list(pool.map(work, targets))
        return {"results": list(results.values()), "errors": errors}

    def _fetch_key(self, depot_id: int, appid: int | None = None) -> dict:
        state = store.load()
        plan = store.app_record(state, appid) if appid else {}
        template = sources.key_source(state["settings"], (plan or {}).get("source")).strip()
        if not template:
            raise ApiError(
                "no key source configured; paste the depot key or set a key source URL in settings",
                409,
            )
        if "{depotId}" not in template:
            raise ApiError("key source URL must contain {depotId}")
        url = template.replace("{depotId}", str(depot_id))
        if not url.startswith(("http://", "https://")):
            raise ApiError("key source URL must be http(s)")
        text = http.get_text(url, timeout=15.0).strip()
        key = store.valid_key(text)
        if key is None:
            try:
                payload = json.loads(text)
            except json.JSONDecodeError:
                payload = None
            if isinstance(payload, dict):
                for field in ("key", "depot_key", "content", "data"):
                    candidate = payload.get(field)
                    if isinstance(candidate, str):
                        key = store.valid_key(candidate)
                        if key:
                            break
        if key is None:
            raise ApiError("key source did not return a 64-character hex key", 502)
        store.update(lambda current: store.set_key(current, depot_id, key))
        return {"depot_id": depot_id, "key": key}

    def _save_settings(self, body: dict) -> dict:
        allowed = set(store.DEFAULT_SETTINGS)

        def mutate(state):
            for key, value in body.items():
                if key not in allowed:
                    continue
                if key == "manifest_providers":
                    if not isinstance(value, list):
                        raise ApiError("manifest_providers must be a list")
                    state["settings"][key] = [entry for entry in value if isinstance(entry, (str, dict))]
                elif key == "key_source_url":
                    url = str(value or "").strip()
                    if url and not url.startswith(("http://", "https://")):
                        raise ApiError("key_source_url must be http(s)")
                    state["settings"][key] = url
                elif key == "hubcap_key":
                    raw = str(value or "").strip()
                    if not raw:
                        state["settings"][key] = ""
                        continue
                    valid = hubcap.valid_key(raw)
                    if valid is None:
                        raise ApiError("a Hubcap key starts with smm and has no spaces")
                    state["settings"][key] = valid
                else:
                    state["settings"][key] = str(value)
            return dict(state["settings"])

        return store.update(mutate)

    def _apply(self, appid: int) -> dict:
        state = store.load()
        plan = store.app_record(state, appid)
        if plan is None:
            raise ApiError("save a plan first", 409)
        text = lua.render_from_record(plan, state["depot_keys"])
        script = vfsys.write_script(appid, text, enabled=True)
        plans = {int(key): value for key, value in state["apps"].items()}
        try:
            config = configfile.apply(vfsys.VF_CONFIG, plans)
        except configfile.ConfigError as error:
            raise ApiError(f"config.toml: {error}", 500) from error
        launch = None
        warning = None
        if (plan.get("fixes") or {}).get("online_fix"):
            try:
                launch = steamctl.add_launch_option(appid, "-onlinefix")
            except steamctl.SteamError as error:
                warning = str(error)
        return {
            "appid": appid,
            "script": str(script),
            "config": config,
            "launch_options": launch,
            "warning": warning,
            "missing_keys": lua.missing_keys(plan["depots"], state["depot_keys"]),
        }

    def _save_source(self, body: dict) -> dict:
        name = str(body.get("name") or "").strip()
        if not name or not name.replace("-", "").replace("_", "").isalnum():
            raise ApiError("source name must be alphanumeric (dashes allowed)")
        providers_list = body.get("providers")
        if not isinstance(providers_list, list) or not providers_list:
            raise ApiError("providers must be a non-empty list")
        entry = {
            "label": str(body.get("label") or name),
            "providers": [item for item in providers_list if isinstance(item, (str, dict))],
            "key_source_url": str(body.get("key_source_url") or "").strip(),
        }
        if entry["key_source_url"] and not entry["key_source_url"].startswith(("http://", "https://")):
            raise ApiError("key_source_url must be http(s)")
        usable = [spec for spec in (providers.resolve(item) for item in entry["providers"]) if spec]
        if not usable:
            raise ApiError("no provider in that list is valid")

        def mutate(state):
            state["settings"].setdefault("sources", {})
            state["settings"]["sources"][name] = entry
            if body.get("default"):
                state["settings"]["default_source"] = name
            return {"name": name, **entry}

        return store.update(mutate)

    def _delete_source(self, name: str) -> dict:
        if not name:
            raise ApiError("name is required")

        def mutate(state):
            removed = state["settings"].get("sources", {}).pop(name, None)
            if removed is None:
                raise ApiError("unknown custom source", 404)
            if state["settings"].get("default_source") == name:
                state["settings"]["default_source"] = "auto"
            return {"removed": name}

        return store.update(mutate)

    # -- static -----------------------------------------------------------

    def _static(self, path: str) -> None:
        relative = path.lstrip("/") or "index.html"
        target = (WEB_DIR / relative).resolve()
        if not str(target).startswith(str(WEB_DIR.resolve())) or not target.is_file():
            target = WEB_DIR / "index.html"
            if not target.is_file():
                raise ApiError("front end missing", 500)
        content_type = mimetypes.guess_type(target.name)[0] or "application/octet-stream"
        if content_type.startswith("text/") or content_type in ("application/javascript", "application/json"):
            content_type += "; charset=utf-8"
        self._send(200, target.read_bytes(), content_type)


class Server(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def serve(host: str = "127.0.0.1", port: int = 8765, verbose: bool = False) -> None:
    httpd = Server((host, port), Handler)
    httpd.verbose = verbose  # type: ignore[attr-defined]
    url = f"http://{host}:{port}/"
    print(f"ExxaTools listening on {url}")
    print(f"state: {store.STATE_PATH}")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nstopped")
    finally:
        httpd.server_close()


def serve_background(port: int) -> tuple[ThreadingHTTPServer, threading.Thread]:
    httpd = Server(("127.0.0.1", port), Handler)
    httpd.verbose = False  # type: ignore[attr-defined]
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    return httpd, thread
