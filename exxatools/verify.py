"""Post-apply checks: would this plan actually load in Steam?"""

from __future__ import annotations

import re

from . import configfile, lua, steamctl, store, vfsys

CALL = re.compile(r"^(addappid|setmanifestid|setappticket|seteticket|setstat|setavatar|addtoken)\b")


def _item(key: str, label: str, status: str, detail: str = "") -> dict:
    return {"id": key, "label": label, "status": status, "detail": detail}


def lint_lua(text: str) -> list[str]:
    problems = []
    for number, raw in enumerate(text.splitlines(), start=1):
        line = raw.split("--", 1)[0].strip()
        if not line:
            continue
        if not CALL.match(line):
            problems.append(f"line {number}: unexpected statement")
    for depot, gid in re.findall(r"setmanifestid\((\d+),\s*\"(\d+)\"", text):
        if not gid.isdigit():
            problems.append(f"depot {depot}: manifest id is not decimal")
    for depot, key in re.findall(r"addappid\(\d+,\s*1,\s*\"([0-9a-f]*)\"", text):
        if len(key) != 64:
            problems.append(f"depot key for {depot} is {len(key)} characters")
    return problems


def run(appid: int) -> dict:
    state = store.load()
    forge = vfsys.status()["vapor_forge"]
    plan = store.app_record(state, appid)
    items = []

    items.append(
        _item(
            "vapor_forge",
            "Vapor Forge libraries",
            "ok" if forge["installed"] else "bad",
            ", ".join(
                name for arch in forge["libs"].values() for name in arch
            ) or "not installed",
        )
    )
    items.append(
        _item(
            "ld_audit",
            "LD_AUDIT wired for Steam",
            "ok" if forge["ld_audit"]["configured"] else "bad",
            " | ".join(forge["ld_audit"]["evidence"]) or "no drop-in, .env.steam, or launcher entry",
        )
    )

    if plan is None:
        items.append(_item("plan", "Saved plan", "bad", "no plan for this app yet"))
        return {"appid": appid, "items": items, "ok": False}

    script = vfsys.script_path(appid)
    disabled = vfsys.VF_SCRIPTS / f"{appid}.lua.disabled"
    if script.exists():
        status, detail = "ok", str(script)
    elif disabled.exists():
        status, detail = "warn", f"disabled: {disabled}"
    else:
        status, detail = "bad", "not installed into Vapor Forge"
    items.append(_item("script", "Lua plan installed", status, detail))

    text = lua.render_from_record(plan, state["depot_keys"])
    problems = lint_lua(text)
    items.append(
        _item(
            "lua",
            "Lua plan parses",
            "ok" if not problems else "bad",
            "; ".join(problems[:4]) or f"{len(text.splitlines())} lines",
        )
    )

    missing = lua.missing_keys(plan["depots"], state["depot_keys"])
    items.append(
        _item(
            "keys",
            "Depot keys",
            "ok" if not missing else "warn",
            "all depots keyed" if not missing else f"missing: {', '.join(str(v) for v in missing)}",
        )
    )

    cached = sum(1 for depot in plan["depots"] if store.cached_code(state, depot.get("gid", "")))
    items.append(
        _item(
            "codes",
            "Manifest request codes",
            "ok" if cached == len(plan["depots"]) else "warn",
            f"{cached}/{len(plan['depots'])} resolved (Vapor Forge can also fetch them itself)",
        )
    )

    fixes = plan.get("fixes") or {}
    config_path = vfsys.VF_CONFIG
    if fixes or plan.get("dlc"):
        preview = configfile.preview(config_path, {appid: plan})
        in_block = str(appid) in preview["apps"]
        items.append(
            _item(
                "config",
                "config.toml injection",
                "ok" if in_block else "warn",
                "ticket=%s, dlc=%d" % (fixes.get("ticket", "forge"), len(plan.get("dlc") or []))
                if in_block
                else "not written yet - press Apply",
            )
        )
    else:
        items.append(_item("config", "config.toml injection", "ok", "not needed"))

    if fixes.get("online_fix"):
        try:
            options = steamctl.read_launch_options(appid)
            current = options.get("options") or ""
            present = "-onlinefix" in current.split()
        except steamctl.SteamError as error:
            present = False
            current = str(error)
        items.append(
            _item(
                "launch_options",
                "Launch option -onlinefix",
                "ok" if present else "warn",
                current or "not set",
            )
        )

    items.append(
        _item(
            "steam",
            "Steam client",
            "warn" if steamctl.is_running() else "ok",
            "running - restart it to load the plan" if steamctl.is_running() else "not running",
        )
    )

    return {
        "appid": appid,
        "items": items,
        "ok": all(item["status"] != "bad" for item in items),
    }
