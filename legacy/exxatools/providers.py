"""Manifest request-code providers.

These are the same small shared services the Vapor Forge runtime is configured
to use. Requests are serialised per provider with a minimum interval so one
client cannot burst a service the whole community depends on.
"""

from __future__ import annotations

import json
import threading
import time

from . import http

MIN_INTERVAL_SECONDS = 1.0
COOLDOWN_SECONDS = 30.0

BUILTIN = {
    "steamrun": {
        "url": "https://manifest.steam.run/api/manifest/{gid}",
        "format": "json",
        "json_field": "content",
        "user_agent": "ExxaTools/1.0",
    },
    "20770407": {
        "url": "https://20770407.xyz/manifest/{depotId}/{gid}",
        "format": "plain",
        "json_field": None,
        "user_agent": "ExxaTools/1.0",
    },
    "wudrm": {
        "url": "http://gmrc.wudrm.com/manifest/{gid}",
        "format": "plain",
        "json_field": None,
        "user_agent": None,
    },
    "manifestdex": {
        "url": "https://manifest.manifestdex.com/{gid}",
        "format": "plain",
        "json_field": None,
        "user_agent": "ManifestDeX/1.0",
    },
    "opensteamtool": {
        "url": "https://manifest.opensteamtool.com/{gid}",
        "format": "plain",
        "json_field": None,
        "user_agent": "OpenSteamTool/1.0",
    },
}

DEFAULT_ORDER = ["steamrun", "20770407", "wudrm", "manifestdex", "opensteamtool"]


class ProviderError(RuntimeError):
    pass


class _Limiter:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._last: dict[str, float] = {}
        self._cooldown: dict[str, float] = {}

    def wait(self, name: str) -> None:
        with self._lock:
            until = self._cooldown.get(name, 0.0)
            now = time.monotonic()
            if until > now:
                raise ProviderError(f"{name} is cooling down for {until - now:.0f}s")
            last = self._last.get(name, 0.0)
            delay = MIN_INTERVAL_SECONDS - (now - last)
            self._last[name] = max(now, last + MIN_INTERVAL_SECONDS)
        if delay > 0:
            time.sleep(delay)

    def cool(self, name: str) -> None:
        with self._lock:
            self._cooldown[name] = time.monotonic() + COOLDOWN_SECONDS

    def reset(self) -> None:
        with self._lock:
            self._last.clear()
            self._cooldown.clear()


LIMITER = _Limiter()


def resolve(entry: dict | str) -> dict | None:
    if isinstance(entry, str):
        spec = BUILTIN.get(entry)
        if spec is None:
            return None
        return {"name": entry, **spec}
    if not isinstance(entry, dict):
        return None
    name = str(entry.get("name") or "").strip()
    url = str(entry.get("url") or "").strip()
    if not name or "{gid}" not in url:
        return None
    if not url.startswith(("http://", "https://")):
        return None
    fmt = str(entry.get("format") or "plain")
    field = entry.get("json_field")
    if fmt == "json" and not field:
        return None
    return {
        "name": name,
        "url": url,
        "format": fmt,
        "json_field": field,
        "user_agent": entry.get("user_agent"),
    }


def build_url(spec: dict, app_id: int, depot_id: int, gid: str) -> str:
    return (
        spec["url"]
        .replace("{gid}", gid)
        .replace("{depotId}", str(depot_id))
        .replace("{appId}", str(app_id))
    )


def _parse(body: str, spec: dict) -> int:
    text = body.strip()
    if not text:
        raise ProviderError("empty response")
    if text.lstrip().startswith("<"):
        raise ProviderError("HTML response (challenge or outage)")
    if spec["format"] == "json" or text.startswith("{"):
        try:
            payload = json.loads(text)
        except json.JSONDecodeError as error:
            raise ProviderError("malformed JSON") from error
        if isinstance(payload, dict):
            value = payload.get(spec["json_field"] or "content")
        else:
            value = payload
        text = str(value).strip() if value is not None else ""
    if not text.isdigit():
        raise ProviderError(f"not a manifest code: {text[:60]!r}")
    code = int(text)
    if not 0 < code < 2**64:
        raise ProviderError("manifest code out of range")
    return code


def fetch_code(
    app_id: int,
    depot_id: int,
    gid: str,
    order: list | None = None,
    timeout: float = 15.0,
) -> dict:
    """Try each provider in order. Returns code, provider name, and failures."""
    failures: dict[str, str] = {}
    specs = [spec for spec in (resolve(entry) for entry in (order or DEFAULT_ORDER)) if spec]
    if not specs:
        raise ProviderError("no usable manifest providers configured")
    for spec in specs:
        name = spec["name"]
        try:
            LIMITER.wait(name)
            headers = {"User-Agent": spec["user_agent"]} if spec["user_agent"] else None
            body = http.get_text(
                build_url(spec, app_id, depot_id, gid), headers=headers, timeout=timeout
            )
            code = _parse(body, spec)
        except (ProviderError, http.HttpError) as error:
            failures[name] = str(error)
            LIMITER.cool(name)
            continue
        return {"code": code, "provider": name, "failures": failures}
    raise ProviderError(
        "every provider failed: "
        + "; ".join(f"{name}: {reason}" for name, reason in failures.items())
    )
