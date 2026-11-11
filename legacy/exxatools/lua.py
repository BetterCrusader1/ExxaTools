"""Render Vapor Forge Lua scripts from a saved plan."""

from __future__ import annotations

from . import vfsys


def _quote(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render(
    appid: int,
    name: str,
    branch: str,
    depots: list[dict],
    keys: dict[str, str],
    dlc: list[int] | None = None,
    source: str = "auto",
) -> str:
    dlc = dlc or []
    lines: list[str] = [
        "-- ExxaTools plan",
        f"-- @appid {appid}",
        f"-- @name {name}".rstrip(),
        f"-- @branch {branch}",
        f"-- @source {source}",
        f"-- @depots {','.join(str(depot['depot_id']) for depot in depots)}".rstrip(),
        f"-- generated {vfsys.stamp()}",
        "",
        f"addappid({appid})",
    ]

    keyed = [depot for depot in depots if keys.get(str(depot["depot_id"]))]
    if keyed:
        lines.append("")
        lines.append("-- depot decryption keys")
        for depot in keyed:
            depot_id = depot["depot_id"]
            lines.append(f"addappid({depot_id}, 1, {_quote(keys[str(depot_id)])})")

    pinned = [depot for depot in depots if depot.get("gid")]
    if pinned:
        lines.append("")
        lines.append("-- manifest pinning")
        for depot in pinned:
            depot_id = depot["depot_id"]
            size = depot.get("size")
            if size:
                lines.append(f"setmanifestid({depot_id}, {_quote(str(depot['gid']))}, {int(size)})")
            else:
                lines.append(f"setmanifestid({depot_id}, {_quote(str(depot['gid']))})")

    if dlc:
        lines.append("")
        lines.append("-- DLC")
        for dlc_id in dlc:
            lines.append(f"addappid({int(dlc_id)})")

    return "\n".join(lines) + "\n"


def missing_keys(depots: list[dict], keys: dict[str, str]) -> list[int]:
    return [depot["depot_id"] for depot in depots if not keys.get(str(depot["depot_id"]))]


def render_from_record(record: dict, keys: dict[str, str]) -> str:
    return render(
        int(record["appid"]),
        record.get("name", ""),
        record.get("branch", "public"),
        record.get("depots", []),
        keys,
        record.get("dlc", []),
        record.get("source") or "auto",
    )


PROVIDER_FILE = "zz-exxatools-provider.lua"

PROVIDER_LUA = """-- ExxaTools manifest provider (generated __STAMP__)
-- Vapor Forge calls fetch_manifest_code_ex(app_id, depot_id, gid) whenever Steam
-- asks for a manifest request code the client cannot obtain itself, so plans
-- keep resolving after a Steam update instead of holding stale codes.

local SOURCES = __SOURCES__

local function trim(text)
  if type(text) ~= "string" then
    return nil
  end
  return (string.gsub(text, "^%s*(.-)%s*$", "%1"))
end

-- Manifest ids exceed 2^53, so the runtime hands big ones over as decimal
-- strings and small ones as numbers. Normalise both back to digits.
local function decimal(value)
  if type(value) == "string" then
    return value
  end
  if type(value) == "number" then
    if value ~= value or value < 0 then
      return nil
    end
    return string.format("%.0f", value)
  end
  return nil
end

local function fill(template, app_id, depot_id, gid)
  local url = string.gsub(template, "{gid}", gid)
  url = string.gsub(url, "{depotId}", depot_id)
  url = string.gsub(url, "{appId}", app_id)
  return url
end

local function code_from(body, status, source)
  if type(body) ~= "string" or status ~= 200 then
    return nil
  end
  local text = trim(body)
  if text == nil or text == "" or string.sub(text, 1, 1) == "<" then
    return nil
  end
  if source.kind == "json" or string.sub(text, 1, 1) == "{" then
    local field = source.field or "content"
    local value = string.match(text, '"' .. field .. '"%s*:%s*"([^"]*)"')
    if value == nil then
      value = string.match(text, '"' .. field .. '"%s*:%s*(%d+)')
    end
    if value == nil then
      return nil
    end
    text = trim(value)
  end
  if string.match(text, "^%d+$") == nil then
    return nil
  end
  return text
end

function fetch_manifest_code_ex(app_id, depot_id, gid)
  local gid_text = decimal(gid)
  local depot_text = decimal(depot_id)
  local app_text = decimal(app_id)
  if gid_text == nil or depot_text == nil or app_text == nil then
    return nil
  end
  local index = 1
  while index <= #SOURCES do
    local source = SOURCES[index]
    local headers = nil
    if source.agent ~= nil then
      headers = { ["User-Agent"] = source.agent }
    end
    local body, status = http_get(fill(source.url, app_text, depot_text, gid_text), headers)
    local code = code_from(body, status, source)
    if code ~= nil then
      return code
    end
    index = index + 1
  end
  return nil
end
"""


def _lua_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render_provider(order: list, stamp: str) -> str:
    from . import providers

    rows = []
    for entry in order or []:
        spec = providers.resolve(entry)
        if spec is None:
            continue
        parts = [
            f'name = {_lua_string(spec["name"])}',
            f'url = {_lua_string(spec["url"])}',
            f'kind = {_lua_string(spec["format"])}',
        ]
        if spec.get("json_field"):
            parts.append(f'field = {_lua_string(spec["json_field"])}')
        if spec.get("user_agent"):
            parts.append(f'agent = {_lua_string(spec["user_agent"])}')
        rows.append("  { " + ", ".join(parts) + " },")
    if not rows:
        raise ValueError("no usable manifest provider configured")
    block = "{\n" + "\n".join(rows) + "\n}"
    return PROVIDER_LUA.replace("__STAMP__", stamp).replace("__SOURCES__", block)
