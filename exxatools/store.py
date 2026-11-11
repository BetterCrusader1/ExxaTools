"""On-disk state: depot keys, saved plans, manifest codes, and settings."""

from __future__ import annotations

import json
import os
import threading
from pathlib import Path

CONFIG_HOME = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
STATE_DIR = Path(os.environ.get("EXXATOOLS_HOME") or CONFIG_HOME / "exxatools")
STATE_PATH = STATE_DIR / "state.json"

_LOCK = threading.RLock()

DEFAULT_SETTINGS = {
    "manifest_providers": ["steamrun", "20770407", "wudrm", "manifestdex", "opensteamtool"],
    "key_source_url": "",
    "country": "us",
    "default_os": "linux",
    "default_source": "auto",
    "sources": {},
    "hubcap_key": "",
}

_DEFAULT = {
    "version": 1,
    "depot_keys": {},
    "apps": {},
    "codes": {},
    "settings": DEFAULT_SETTINGS,
}


class StateError(RuntimeError):
    pass


def _merge_defaults(state: dict) -> dict:
    merged = json.loads(json.dumps(_DEFAULT))
    merged.update(state)
    settings = dict(DEFAULT_SETTINGS)
    settings.update(state.get("settings") or {})
    merged["settings"] = settings
    return merged


def load() -> dict:
    with _LOCK:
        if not STATE_PATH.exists():
            legacy = STATE_PATH.parent.parent / "vapordepot" / "state.json"
            if legacy.is_file():
                try:
                    STATE_DIR.mkdir(parents=True, exist_ok=True)
                    STATE_PATH.write_text(legacy.read_text("utf-8"), "utf-8")
                except OSError:
                    pass
        if not STATE_PATH.exists():
            return _merge_defaults({})
        try:
            raw = json.loads(STATE_PATH.read_text("utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise StateError(f"cannot read {STATE_PATH}: {error}") from error
        if not isinstance(raw, dict):
            raise StateError(f"{STATE_PATH} is not a JSON object")
        return _merge_defaults(raw)


def save(state: dict) -> None:
    with _LOCK:
        STATE_DIR.mkdir(parents=True, exist_ok=True)
        temp = STATE_PATH.with_suffix(".tmp")
        temp.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", "utf-8")
        os.replace(temp, STATE_PATH)


def update(mutator):
    """Read-modify-write under one lock; returns whatever mutator returns."""
    with _LOCK:
        state = load()
        result = mutator(state)
        save(state)
        return result


def valid_key(value: str) -> str | None:
    text = (value or "").strip().lower().replace(" ", "")
    if len(text) != 64 or any(ch not in "0123456789abcdef" for ch in text):
        return None
    return text


def key_for(state: dict, depot_id: int) -> str | None:
    return state["depot_keys"].get(str(depot_id))


def set_key(state: dict, depot_id: int, key: str | None) -> None:
    if key is None:
        state["depot_keys"].pop(str(depot_id), None)
    else:
        state["depot_keys"][str(depot_id)] = key


def cached_code(state: dict, gid: str) -> int | None:
    return state["codes"].get(str(gid))


def remember_code(state: dict, gid: str, code: int) -> None:
    state["codes"][str(gid)] = int(code)


def app_record(state: dict, appid: int | str) -> dict | None:
    return state["apps"].get(str(appid))
