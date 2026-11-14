# Changelog

## Unreleased

The desktop window is now a native GTK4/libadwaita depot desk instead of one
hand-styled list.

- Three-pane Library: navigation rail, game shelf, and a selected-game inspector
  with a compact depot table and collapsed plan details.
- `src/ui` is split into model, covers, worker, shell, shelf, inspector,
  discover, system and settings modules; `src/window.rs` is only a launcher.
- All network and state work runs on a worker channel; the GTK thread never
  blocks on Steam, a plan source or a cover download.
- Discover is a consistent four-column cover grid; System lifts warnings into a
  Needs attention group and hides hints on healthy checks; Settings is an
  Adwaita preferences page with masked keys and a save toast.
- Deep links open a view directly: `?page=discover`, `?page=system`,
  `?page=settings`.
- The old hand-written web dashboard files and the WebKit dependency are gone.
- The **Nacre** visual system: pure-black dark violet and light lilac pearl
  themes, a frosted hero panel over the game art, a deep pearl primary action,
  a live theme switch in the title bar, and a crossfade between views. The
  choice persists with the rest of the settings.
- Steam's wide `library_hero.jpg` is cached for the inspector; header art stays
  as the shelf fallback.

## 0.3.0

The release where the work can be proven without a live Steam session.

- `exxa` is a library plus a thin binary, so integration tests call the product.
- A temporary Steam tree and Vapor Forge home (`EXXATOOLS_STEAM_ROOT`,
  `EXXATOOLS_VF_HOME`, `EXXATOOLS_OFFLINE`) let tests own every path.
- End-to-end tests: a plan source becomes an installed script that verify accepts;
  an interrupted appmanifest is repaired with a backup and stays stable; a refresh
  with the API unreachable rebuilds from the catalog snapshot carried by the plan.
- Refresh rewrote scripts from the state on disk before saving, so it pinned the
  old manifest; it now saves first and rewrites after (found by the new test).
- Writes need the token from `/api/session`; a foreign Origin is refused; the
  wildcard CORS header is gone.
- Static serving resolves through `canonicalize` and refuses `..`, absolute paths
  and symlinks that leave the web root.
- The drawer shows depot key coverage and toggles `-onlinefix`.
- The Steam panel carries Install and Rebuild per plan through the CDP channel.
- `exxa diagnostics` and Settings produce a JSON bundle with every key masked.
- `scripts/docs-check.py` fails when a documented command no longer exists.

