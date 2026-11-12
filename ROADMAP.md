# ExxaTools — roadmap

Three things define the product:

1. the game shows up in the Steam library and actually downloads;
2. no "no internet connection" and no update-required loop after a restart;
3. the person always knows which step is stuck.

Everything below serves those three points.

## Where we are (0.3.0)

| Area | State |
| --- | --- |
| Language | Rust; `src/lib.rs` exports every module, `src/main.rs` only dispatches |
| Vapor Forge engine | installs, updates, `LD_AUDIT` verified against `/proc` |
| Plans | lua plan, manifest-code provider, manual and Hubcap keys, Ryuu and custom sources |
| Steam | menu and panel injected over CDP; the panel carries Install, Rebuild and `-onlinefix` per plan |
| UI | Library / Discover / Game / System in a GTK window, `.desktop` entry |
| Diagnostics | `exxa diagnose`, `exxa doctor`, `exxa verify`, `exxa diagnostics` with keys masked |
| Durability | state backups, offline refresh from the catalog snapshot carried by a plan |
| Tests | 74 unit tests and 9 integration tests against a temporary Steam tree, CI on push |
| Packaging | `packaging/PKGBUILD`, `v0.3.0` tag, `.desktop`, icon |
| Not done | older-build picker, snapshot diff, auto-finish after download, tray icon, Millennium plugin |

## Locked decisions

- Rust owns the product; Python lives in `legacy/` and is no longer wired to `./exxa`.
- Steam UI integration stays on CDP with a panel rendered inside the client, because
  a separate always-on-top window was rejected. Millennium stays a possible future
  loader, not a plan.
- Plan sources are pluggable: Hubcap, Ryuu, custom URL. Keys import; pins are explicit.
- UI copy, docs and identifiers are English-only.

## 0.3.0 — what shipped

- library target plus a test-owned Steam tree (`EXXATOOLS_STEAM_ROOT`,
  `EXXATOOLS_VF_HOME`, `EXXATOOLS_OFFLINE`);
- end-to-end tests: plan source → key → pinned depot → installed script → verify;
  appmanifest repair with a backup that stays stable on a second run; offline refresh;
- refresh saves before rewriting scripts (it used to pin the previous manifest);
- writes need a token from `/api/session`; foreign origins refused; no CORS headers;
- static serving confined to the web root, with a symlink test;
- depot key coverage and `-onlinefix` in the drawer; Install, Rebuild and the flag in
  the panel;
- `diagnostics.json` with every 64-hex run masked, including the engine digest;
- `scripts/docs-check.py` (with a `.sh` wrapper) fails when a documented command is not
  in the binary.

## Next — 0.4 candidates

Ordered by what the three points above need next, not by novelty.

1. **Auto-finish after a download.** Watch the appmanifest and, once the flags settle,
   repair them and raise a toast, so a finished download is playable without a second
   Steam restart.
2. **Older build picker.** Build history exists; a picker that pins an older gid per
   depot closes the "the new build broke my mods" case.
3. **Snapshot diff.** What changed between two catalog snapshots, per depot, with size
   and gid. Turns "why is it downloading again" into a sentence.
4. **Tray icon.** The app needs its window today; a tray item would make `exxa serve`
   a real background service.
5. **Millennium evaluation.** Only if CDP injection breaks on a Steam update; it
   competes with `LD_AUDIT`, so it needs a measured spike, not a switch.

## Risks and countermeasures

| Risk | Countermeasure |
| --- | --- |
| Steam ships a UI update and CDP injection breaks | find the menu by the "Help" label, not by class names; panel actions call the same Rust functions as the dashboard |
| Vapor Forge changes its Lua API | record the API version, refuse to run on a mismatch |
| Hubcap rate limits or outages | key cache, multiple sources, manual entry |
| appmanifest race with Steam | backup, validate after write, refuse while Steam is running |
| Keys leaking into support artefacts | every string in the bundle passes `redact`; a test scans for 64-hex runs |