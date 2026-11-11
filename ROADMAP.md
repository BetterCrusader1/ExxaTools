# ExxaTools — roadmap

Three things define the product:

1. the game shows up in the Steam library and actually downloads;
2. no "no internet connection" and no update-required loop after a restart;
3. the person always knows which step is stuck.

Everything below serves those three points.

## Where we are

| Area | State |
| --- | --- |
| Vapor Forge engine | installs, updates, `LD_AUDIT` verified against `/proc` |
| Plans | lua plan, manifest-code provider, manual keys plus Hubcap |
| Steam | icon and menu injected in the client over CDP, appmanifest fixes |
| UI | Library / Discover / Game / System, `.desktop` entry |
| Language | Rust is the product; `legacy/` keeps the Python build for reference |
| Tests | 22 unit tests over lua, appmanifest, hubcap, providers, vdf, launch options |
| Missing | named DLC, importing foreign lua, older build picker, auto-finish after download, self-diagnostics, app updates, tests |

## Locked decisions

- Rust owns the product; Python lives in `legacy/` and is no longer wired to `./exxa`.
- Steam protocol comes from `steamroom` behind a feature flag, not from Hubcap alone.
- Steam UI integration moves from CDP DOM patching to a Millennium plugin.
- UI copy, docs and identifiers are English-only.

## Phase 0 — hygiene (~half a day)

- [ ] Origin check and a token for POST. The API binds loopback, but any browser
      tab can still call `steam/restart`.
- [ ] Tests: `unittest` over lua rendering and parsing, appmanifest patching,
      Hubcap parsing, the provider script. Pure functions, no Steam.
- [ ] CI: `.github/workflows/ci.yml` — Python 3.12, `ruff`, `unittest`.
- [ ] Self-update: `./exxa update` pulls a release, prints the version.
- [ ] Version surfaced in `exxatools/__init__.py` and in the menu.

**Done when:** `python3 -m unittest` is green and CI passes on push.

## Phase 1 — "Add game" as one flow (2–3 days)

The important one. Right now there are five manual steps between finding a game
and Steam downloading it.

1. **Four-step wizard**: Game → Branch & DLC → Depots & keys → Install. Every
   step keeps its state and can be revisited.
2. **Named DLC**: `store.steampowered.com/api/appdetails` returns DLC titles.
   Checkboxes with names instead of bare ids.
3. **Sensible preselection**: platform (linux/windows), architecture, language,
   derived from `oslist`/`osarch`, with a short reason on each selected depot.
4. **Keys in one move**: Hubcap, drag-and-drop of `.lua` / `.vdf` / `steam.keys`,
   and a manual field for the rest. Show coverage as `18 of 20 depots keyed`.
5. **Install**: plan → script → `Config.toml` → Steam restart, as a checkbox.
6. **Auto-finish**: a background watcher reads the appmanifest and, once
   `StateFlags` becomes 4, runs the flag fix and raises a toast.
7. **Game state in Library**: `planned` / `downloading` / `needs key` /
   `installed` as a badge on the card, not a line in a log.

**Done when:** someone with no context walks the flow and ends up with a running
game without opening a terminal.

## Phase 2 — self-diagnostics (1–2 days)

- [ ] **Doctor**: twelve checks with human hints — engine, `LD_AUDIT`, debug
      port, script permissions, pattern freshness, key coverage, manifest
      request codes, appmanifest flags, free disk, Steam state, selected branch,
      depotcache.
- [ ] **"Why is it broken"** for a specific game: an ordered list of what failed
      and which button fixes it.
- [ ] **Status board** instead of a raw log: grouped, with states, log behind a
      "details" toggle.

**Done when:** every symptom in the table below has its own check and action.

| Symptom | Check chain |
| --- | --- |
| game never appears in Steam | engine → ld_audit → scripts dir → plan installed |
| "no internet connection" | keys → request codes → appmanifest |
| update-required loop | StateFlags → BytesDownloaded → AutoUpdateBehavior |
| wrong build downloads | branch → pinned manifest → platform |
| stuck at "Preallocating" | free disk → depotcache |

## Phase 3 — versions and builds (2 days)

- [x] **Build history**: observed branch gids per depot, newest first (Hubcap snapshots still to add)
- [x] **Pin UI**: pin a specific manifest per depot, with the consequence spelled out.
- [x] **Export/import** plans as JSON, so they move between machines.
- [ ] **Diff** two snapshots: what changed per depot (size, gid).

## Phase 4 — Steam integration (2 days)

- [x] Steam menu panel: saved plans with state, dashboard, restart, provider.
- [x] Toasts inside the client (injected, not the bridge).
- [x] Hotkey for the panel (Ctrl+Shift+E).
- [ ] Launch options: an `-onlinefix` toggle per game.
- [x] Post-update check: a watcher re-injects the menu every 45 seconds.

## Phase 5 — product (2 days)

- [x] Autostart entry (`exxa autostart`); tray icon still open.
- [x] Packaging: `packaging/PKGBUILD`, `.desktop`, icon, screenshot in the README.
- [ ] `diagnostics.json`: last app errors, an export button.

## Risks and countermeasures

| Risk | Countermeasure |
| --- | --- |
| Steam ships a UI update and CDP injection breaks | find the menu by the "Help" label, not by class names; Millennium plugin as the primary path; tray panel as last resort |
| Vapor Forge changes its Lua API | record the API version, refuse to run on a mismatch |
| Hubcap rate limits or outages | key cache, alternative providers, manual entry |
| appmanifest race with Steam | backup, validate after write, refuse while Steam is writing |
| User keys | never log them whole, only `ab12…` |

## Order I'd take

1. Phase 1, steps 1–4 — this is what makes it a LuaTools-grade tool.
2. Phase 2 Doctor — removes most "why doesn't it work" questions.
3. Phase 0 tests and CI — so the rest stops breaking.
4. Then 3, 4, 5.

Estimates assume one focused working day per phase, without polish.