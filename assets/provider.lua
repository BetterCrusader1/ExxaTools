-- ExxaTools manifest provider (generated __STAMP__)
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