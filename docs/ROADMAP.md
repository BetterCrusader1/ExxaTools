# ExxaTools — roadmap

Three things define the product:

1. the game shows up in the Steam library and actually downloads;
2. no "no internet connection" and no update-required loop after a restart;
3. the person always knows which step is stuck.

Everything below serves those three points.

## Where we are (post-0.3, on main)

| Area | State |
| --- | --- |
| Language | Rust; `src/lib.rs` exports every module, `src/main.rs` only dispatches |
| Steam start | bare `steam` raises the companion server; the container wrapper and the client patch carry the engine |
| Engine | installs, updates; the Doctor reads live process maps, not env guesses |
| Client patch | [`patches/vapor-forge/`](../patches/vapor-forge/) keeps Vapor Forge v0.1.2 working on Steam build `1794536281` |
| Plans | lua plan, request-code provider chain editable in Settings, manual and Hubcap keys, Ryuu and custom sources |
| Depotcache | Hubcap's manifest blobs land in Steam's `depotcache`, so a pinned build mounts without a CM request code |
| Steam UI | menu and panel injected over CDP; panel carries Install, Rebuild and `-onlinefix` per plan; store-page card |
| UI | native GTK4/libadwaita: Library / Discover / System / Settings, Nacre themes, single instance |
| Diagnostics | `exxa diagnose`, `exxa doctor`, `exxa verify`, `exxa diagnostics` with keys masked |
| State | transactional saves with rollback, backups (five newest), offline refresh |
| Tests | 112 passing; integration tests against a temporary Steam tree; CI on push |
| Packaging | `packaging/PKGBUILD`, `v0.3.0` tag, `.desktop`, icon |
| Not done | older-build picker, snapshot diff, auto-finish after download, tray icon |

## Locked decisions

- Rust owns the product; Python lives in `legacy/` and is no longer wired to
  `./exxa`.
- The tool starts with Steam itself: the wrapper plus the companion copy, no
  autostart files, no supervision loops.
- Steam UI integration stays on CDP with a panel rendered inside the client,
  because a separate always-on-top window was rejected.
- Plan sources are pluggable: Hubcap, Ryuu, custom URL. Keys import; pins are
  explicit.
- UI copy, docs and identifiers are English-only.

## Next — 0.4 candidates

Ordered by what the three points above need next, not by novelty.

1. **Auto-finish after a download.** Watch the appmanifest and, once the flags
   settle, repair them and raise a toast, so a finished download is playable
   without a second Steam restart.
2. **Older build picker.** Build history exists; a picker that pins an older
   gid per depot closes the "the new build broke my mods" case.
3. **Snapshot diff.** What changed between two catalog snapshots, per depot,
   with size and gid. Turns "why is it downloading again" into a sentence.
4. **Tray icon.** The app needs its window today; a tray item would make
   `exxa serve` a real background service.
5. **Wrapper state in doctor.** The container wrapper is the one moving part
  Steam can replace between launches; a doctor row should say whether it is
  ours, stock, or stale, and repair it.

## Risks and countermeasures

| Risk | Countermeasure |
| --- | --- |
| Steam ships a UI update and CDP injection breaks | find the menu by the "Help" label, not by class names; panel actions call the same Rust functions as the dashboard |
| Steam re-extracts the container launcher over the wrapper | the wrapper pads itself to the byte size Steam verifies; every `exxa steam` reinstalls it |
| Vapor Forge changes its Lua API | record the API version, refuse to run on a mismatch |
| A Steam client update breaks the patch | the patch is one file with a build id in its name; rebuild against the new binary, verify with `git apply --check` |
| Hubcap rate limits or outages | key cache, multiple sources, manual entry |
| appmanifest race with Steam | backup, validate after write, refuse while Steam is running |
| Keys leaking into support artefacts | every string in the bundle passes `redact`; a test scans for 64-hex runs |
