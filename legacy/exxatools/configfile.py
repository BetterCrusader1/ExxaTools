"""Write ExxaTools's share of Vapor Forge's `config.toml`.

Every write replaces one delimited block, so hand-written configuration keeps
its place and formatting. Sections that already exist outside the block are not
duplicated: their keys are updated in place instead, because a second
`[steam_id]` header would make the whole file unparseable.
"""

from __future__ import annotations

import re
import shutil
import time
from pathlib import Path

try:  # Python 3.11+
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - older interpreters
    tomllib = None  # type: ignore[assignment]

BEGIN = "# >>> exxatools managed block (rewritten on apply)"
END = "# <<< exxatools managed block"
BACKUP_SUFFIX = ".exxatools.bak"

STEAM_ID64_BASE = 76561197960265728


class ConfigError(RuntimeError):
    pass


def _valid_steam_id(value: str) -> int | None:
    text = str(value or "").strip()
    if not text:
        return None
    if not text.isdigit():
        return None
    number = int(text)
    if not (STEAM_ID64_BASE < number < 2**63):
        return None
    return number


def _block_body(plans: dict[int, dict], include_tables: set[str]) -> str:
    lines = [BEGIN, f"# generated {time.strftime('%Y-%m-%d %H:%M:%S')}"]
    for appid in sorted(plans):
        plan = plans[appid]
        fixes = plan.get("fixes") or {}
        dlc = [int(value) for value in plan.get("dlc") or []]
        ticket = str(fixes.get("ticket") or "forge")
        if ticket not in ("forge", "delegate"):
            ticket = "forge"
        purchase = int(fixes.get("purchase_time") or 0)
        lines.append("")
        lines.append("[[apps.inject]]")
        lines.append(f"id = {int(appid)}")
        if dlc:
            lines.append("dlc = [" + ", ".join(str(value) for value in dlc) + "]")
        else:
            lines.append("dlc = []")
        lines.append(f'ticket = "{ticket}"')
        lines.append(f"purchase_time = {purchase}")

    if "steam_id" in include_tables:
        rows = []
        for appid in sorted(plans):
            value = _valid_steam_id((plans[appid].get("fixes") or {}).get("steam_id"))
            if value is not None:
                rows.append(f"{int(appid)} = {value}")
        if rows:
            lines.append("")
            lines.append("[steam_id]")
            lines.extend(rows)

    if "app_avatar" in include_tables:
        rows = []
        for appid in sorted(plans):
            avatar = int((plans[appid].get("fixes") or {}).get("avatar") or 0)
            if avatar:
                rows.append(f"{int(appid)} = {avatar}")
        if rows:
            lines.append("")
            lines.append("[app_avatar]")
            lines.extend(rows)

    lines.append(END)
    return "\n".join(lines)


def _strip_block(text: str) -> str:
    begin = text.find(BEGIN)
    if begin == -1:
        return text
    end = text.find(END, begin)
    if end == -1:
        raise ConfigError("managed block start marker has no matching end marker")
    end += len(END)
    while end < len(text) and text[end] == "\n":
        end += 1
    return text[:begin] + text[end:]


def _parse(text: str, label: str) -> dict:
    if tomllib is None:
        return {}
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:  # type: ignore[union-attr]
        raise ConfigError(f"{label} is not valid TOML: {error}") from error


def _table_span(text: str, table: str) -> tuple[int, int] | None:
    """Line span of a `[table]` block, excluding the header line."""
    header = re.compile(rf"^\[{re.escape(table)}\]\s*$", re.MULTILINE)
    match = header.search(text)
    if not match:
        return None
    start = match.end() + 1
    rest = text[start:]
    next_header = re.search(r"^\[", rest, re.MULTILINE)
    end = start + (next_header.start() if next_header else len(rest))
    return start, end


def _set_table_keys(text: str, table: str, values: dict[int, int]) -> str:
    span = _table_span(text, table)
    if span is None or not values:
        return text
    start, end = span
    body = text[start:end]
    for key, value in values.items():
        pattern = re.compile(rf"^(\s*){key}\s*=.*$", re.MULTILINE)
        if pattern.search(body):
            body = pattern.sub(lambda m, v=value: f"{m.group(1)}{key} = {v}", body, count=1)
        else:
            if body and not body.endswith("\n"):
                body += "\n"
            body += f"{key} = {value}\n"
    return text[:start] + body + text[end:]


def _existing_app_injects(parsed: dict) -> set[int]:
    ids = set()
    for entry in ((parsed.get("apps") or {}).get("inject") or []):
        if isinstance(entry, dict) and isinstance(entry.get("id"), int):
            ids.add(entry["id"])
    return ids


def compose(text: str, plans: dict[int, dict]) -> tuple[str, dict]:
    """Return the new file text plus a report of what happened."""
    existing = _strip_block(text)
    parsed = _parse(existing, "config.toml")
    report: dict = {"skipped": {}, "updated_in_place": []}

    hand_managed = _existing_app_injects(parsed)
    inject_plans = {}
    for appid, plan in plans.items():
        if appid in hand_managed:
            report["skipped"][appid] = "already declared in config.toml by hand"
            continue
        inject_plans[appid] = plan

    include_tables = set()
    for table, field in (("steam_id", "steam_id"), ("app_avatar", "avatar")):
        present = table in parsed
        if field == "steam_id":
            wanted = any(_valid_steam_id((plan.get("fixes") or {}).get("steam_id")) for plan in plans.values())
        else:
            wanted = any((plan.get("fixes") or {}).get("avatar") for plan in plans.values())
        if wanted and not present:
            include_tables.add(table)
        elif wanted:
            report["updated_in_place"].append(table)

    body = _block_body(inject_plans, include_tables) if inject_plans else ""
    joined = existing.rstrip("\n")
    if joined:
        joined += "\n\n"
    joined += body
    if body:
        joined += "\n"

    if "steam_id" in report["updated_in_place"]:
        values = {
            appid: _valid_steam_id((plan.get("fixes") or {}).get("steam_id"))
            for appid, plan in plans.items()
        }
        joined = _set_table_keys(joined, "steam_id", {k: v for k, v in values.items() if v})
    if "app_avatar" in report["updated_in_place"]:
        values = {
            appid: int((plan.get("fixes") or {}).get("avatar") or 0)
            for appid, plan in plans.items()
        }
        joined = _set_table_keys(joined, "app_avatar", {k: v for k, v in values.items() if v})

    report["apps"] = sorted(inject_plans)
    report["block"] = body
    final = _parse(joined, "the merged config")
    for appid in inject_plans:
        if appid not in _existing_app_injects(final):
            raise ConfigError(f"app {appid} did not land in apps.inject")
    return joined, report


def preview(config_path: Path, plans: dict[int, dict]) -> dict:
    text = config_path.read_text("utf-8") if config_path.exists() else ""
    composed, report = compose(text, plans)
    return {
        "path": str(config_path),
        "exists": config_path.exists(),
        "changed": composed != text,
        "block": report["block"],
        "apps": [str(value) for value in report["apps"]],
        "skipped": {str(key): value for key, value in report["skipped"].items()},
        "updated_in_place": report["updated_in_place"],
    }


def apply(config_path: Path, plans: dict[int, dict]) -> dict:
    text = config_path.read_text("utf-8") if config_path.exists() else ""
    composed, report = compose(text, plans)
    if composed == text:
        return {"path": str(config_path), "written": False, "apps": report["apps"], "block": report["block"]}
    config_path.parent.mkdir(parents=True, exist_ok=True)
    if config_path.exists():
        shutil.copy2(config_path, config_path.with_suffix(config_path.suffix + BACKUP_SUFFIX))
    temp = config_path.with_suffix(".exxatools.tmp")
    temp.write_text(composed, "utf-8")
    _parse(temp.read_text("utf-8"), "the merged config")
    temp.replace(config_path)
    return {
        "path": str(config_path),
        "written": True,
        "backup": str(config_path) + BACKUP_SUFFIX,
        "apps": report["apps"],
        "block": report["block"],
    }


def managed_app_ids(config_path: Path) -> list[int]:
    if not config_path.exists():
        return []
    text = config_path.read_text("utf-8", "replace")
    begin = text.find(BEGIN)
    if begin == -1:
        return []
    end = text.find(END, begin)
    block = text[begin:] if end == -1 else text[begin:end]
    return [int(match) for match in re.findall(r"^id = (\d+)$", block, re.MULTILINE)]
