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
- Discover keeps games only (featured DLC entries are filtered out), uses the
  store capsule URL when the generic CDN path is missing, and drops any tile
  that still has no artwork instead of showing a letter placeholder.
- The hand cursor is set on every clickable surface: navigation, shelf rows,
  Discover cards, buttons and the plan expander.
- The app now publishes its existing logo to the icon theme and sets it as the
  window icon, so the dock/taskbar shows ExxaTools instead of a fallback mark.
  `exxa shortcut install` now creates a visible dashboard launcher with
  `StartupWMClass=exxa`; the packaged desktop entry matches it.
- `exxa steam` detects a Steam client that was already running without
  `-cef-enable-debugging` and restarts it once, unless a game is running, so the
  panel is available even when Steam was opened from an old shortcut.
- The engine check no longer trusts `LD_AUDIT` alone. Steam's runtime starts the
  client inside pressure-vessel, which keeps the variable in the wrapper but
  never maps the shim into the process that owns `steamclient.so`; the Doctor
  now reads the live process maps, reports that case as a failure with its own
  action, and only reads `ok` when the shim is mapped where the hooks live.
- The store-page ExxaTools card had a missing brace in its injected stylesheet,
  which dropped every rule after the game name and left raw browser buttons.
  The block is closed, buttons get consistent 28 px height, 7 px radius, spacing
  and a separated close control.
- The sidebar no longer carries a separate "Steam running" dot: the brand row is
  now just the logo and a larger ExxaTools wordmark, and the Steam state stays
  on the games and in the runtime checks.
- The menu icon asks Rust to open the panel in the Steam window under the icon.
  The store and library are separate CEF windows layered above the shell, so the
  old page-local panel could be painted behind Steam's own chrome; it now opens
  in the active content window and stays fully visible. Closing from the panel
  also clears the host state, so the next click reopens instead of toggling a
  ghost.
- ExxaTools now starts with Steam itself: the container launcher wrapper raises
  the companion server (`~/.local/share/exxatools/bin/exxa serve`) before Steam
  enters its sandbox, so a bare `steam` start brings the tool up with no
  scripts, no autostart entry and no supervision loop. The wrapper recovers
  when Steam's bootstrap replaces or removes the launcher, passes only the
  64-bit shim to pv-adverb (it rejects colon-separated audit lists), and pads
  itself to the byte size Steam verifies.
- The session Xauthority cookie is picked by newest mtime instead of by name,
  because the login session rotates those files.
- Settings gains a manifest-provider chain editor: the order is one editable
  list, custom mirrors join it as JSON entries (name, URL with `{gid}`, optional
  JSON field), and saving republishes the Lua provider script through one
  shared code path so Vapor Forge's hot-reload picks it up at once.
- Hubcap's per-app archive also carries depot manifest blobs; ExxaTools stores
  them in Steam's `depotcache` so the updater can mount a pinned build without
  a CM manifest request code.

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

