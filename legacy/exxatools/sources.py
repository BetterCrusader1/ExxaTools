"""Named manifest sources: where request codes come from for a plan."""

from __future__ import annotations

from . import providers

BUILTIN_SOURCES = {
    "auto": {
        "label": "Auto (all providers, in order)",
        "providers": list(providers.DEFAULT_ORDER),
        "key_source_url": "",
    },
    "steamrun": {
        "label": "steam.run",
        "providers": ["steamrun"],
        "key_source_url": "",
    },
    "20770407": {
        "label": "20770407.xyz",
        "providers": ["20770407"],
        "key_source_url": "",
    },
    "wudrm": {
        "label": "Wudrm",
        "providers": ["wudrm"],
        "key_source_url": "",
    },
    "manifestdex": {
        "label": "ManifestDeX",
        "providers": ["manifestdex"],
        "key_source_url": "",
    },
    "opensteamtool": {
        "label": "OpenSteamTool",
        "providers": ["opensteamtool"],
        "key_source_url": "",
    },
}


def catalog(settings: dict) -> list[dict]:
    """Built-in sources plus any custom ones defined in settings."""
    sources = {name: dict(spec, name=name) for name, spec in BUILTIN_SOURCES.items()}
    for name, spec in (settings.get("sources") or {}).items():
        if not isinstance(spec, dict):
            continue
        entry = {
            "name": name,
            "label": str(spec.get("label") or name),
            "providers": [item for item in spec.get("providers") or [] if isinstance(item, (str, dict))],
            "key_source_url": str(spec.get("key_source_url") or ""),
        }
        if entry["providers"]:
            sources[name] = entry
    return list(sources.values())


def resolve(settings: dict, name: str | None) -> dict:
    chosen = name or settings.get("default_source") or "auto"
    for entry in catalog(settings):
        if entry["name"] == chosen:
            return entry
    for entry in catalog(settings):
        if entry["name"] == "auto":
            return entry
    return {
        "name": "auto",
        "label": "Auto (all providers, in order)",
        "providers": list(providers.DEFAULT_ORDER),
        "key_source_url": "",
    }


def key_source(settings: dict, name: str | None) -> str:
    source = resolve(settings, name)
    return source["key_source_url"] or (settings.get("key_source_url") or "")
