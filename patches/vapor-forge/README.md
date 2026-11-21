# Vapor Forge on Steam build 1794536281

This directory carries the local compatibility patch used by ExxaTools for
Steam build `1794536281` and Vapor Forge `v0.1.2`.

## Vapor Forge v0.2.0 (current)

Official `v0.2.0` supports the steamrt64 client natively and fixes the
license/depot-selection half of the "0 B" bug, so it supersedes most of the
`v0.1.2` patch below (ownership latch, CM fail-open, capability gates and the
build-specific patterns are all upstream now). Install it with
`exxa engine install v0.2.0` and delete the stale
`~/.local/share/vapor-forge/x86_64/.exxatools-pattern-overrides` marker.

One gap remains: stock v0.2.0 only patches depots Steam already selected, so an
unowned app whose `BuildDepotDependency` returns an empty vector still mounts
nothing and shows **0 B**. `v0.2.0-depot-injection.patch` ports
`inject_missing_depots` (append the plan's depots with `manifest_gid` and
`manifest_size`, scoped to the declaring app via `-- @appid`) onto v0.2.0.

The size must also reach the plan: `api.steamcmd.net` returns depot `size` and
`download` as decimal strings, which ExxaTools now parses (`steam::value_to_u64`)
so `setmanifestid(depot, "gid", size)` carries a real size. Without it the
injected `manifest_size` is 0 and Steam still shows 0 B.

### Build (must match the official CI recipe)

```sh
git clone --depth 1 --branch v0.2.0 https://github.com/Hintay/vapor-forge /tmp/vapor-forge
cd /tmp/vapor-forge
git apply /path/to/ExxaTools/patches/vapor-forge/v0.2.0-depot-injection.patch
source "$HOME/.cargo/env"
CFLAGS_x86_64_unknown_linux_gnu=-DLUAJIT_NO_UNWIND \
  cargo build --release -p vapor-forge-audit-loader --target x86_64-unknown-linux-gnu
install -Dm755 target/x86_64-unknown-linux-gnu/release/libvapor_forge.so \
  "$HOME/.local/share/vapor-forge/x86_64/libvapor_forge.so"
```

`-DLUAJIT_NO_UNWIND` is mandatory: without it vendored LuaJIT throws through
libgcc unwinding into mlua's allocator and Steam aborts during hook init. The
official release builds with `cargo zigbuild` (zig cc = clang); a plain GCC 16
build also miscompiles LuaJIT, so prefer clang or the unwind flag. Restart Steam
after installing; verify `manifest: injected … manifest_size=<non-zero>` in the
console log and a real size in the install dialog.

## What the patch changes

- `CUser::CheckAppOwnership` and `CUser::GetSubscribedApps` are resolved from
  the live binary and installed even when the old semantic checks reject the
  new instruction layout. License sync is latched from the first NULL/0
  sizing call with a real count, because this build never makes the second
  fill call the upstream gate waits for.
- `BuildDepotDependency` now appends script-pinned depots to the dependency
  vector. Steam returns an empty vector for an app it does not own, so the
  previous implementation could only rewrite entries that already existed.
  Injection is limited to apps named in the active plan. Every manifest and
  decryption-key lookup is also scoped to the script that declared the depot,
  so one app's plan cannot inject depot IDs belonging to another app.
- A Lua script can declare `-- @appid <app-id>`; ExxaTools writes this
  automatically. Otherwise Vapor Forge discovers the app from the file's
  `-- @filename` identity. `setmanifestid` remains available for explicit
  ownership, and unloading a script removes its file-to-app mapping.
- WebSocket CM frames pass through unchanged when the packet layout is not
  available. This keeps Steam's login and UI alive on the new build.
- Depot injection no longer requires `cm-interception`; ownership no longer
  requires it either.
- `VAPOR_FORGE_TRUST_PATTERNS=1` bypasses semantic entry-point validation for
  this build. ExxaTools sets it when
  `~/.local/share/vapor-forge/x86_64/.exxatools-pattern-overrides` exists.

## Build

```sh
git clone --depth 1 https://github.com/Hintay/vapor-forge /tmp/vapor-forge
cd /tmp/vapor-forge
git apply /path/to/ExxaTools/patches/vapor-forge/steam-1794536281.patch
source "$HOME/.cargo/env"
cargo --config 'build.rustc-wrapper=""' build --release \
  -p vapor-forge-audit-loader --target x86_64-unknown-linux-gnu
install -Dm755 \
  target/x86_64-unknown-linux-gnu/release/libvapor_forge.so \
  "$HOME/.local/share/vapor-forge/x86_64/libvapor_forge.so"
touch "$HOME/.local/share/vapor-forge/x86_64/.exxatools-pattern-overrides"
```

## Manifest cache

Steam needs the real depot manifest file before it can stage chunks. Hubcap's
per-app archive contains those files as protobuf data (`.manifest`, not a zip
wrapper). ExxaTools now extracts them into Steam's cache automatically when a
Hubcap plan is fetched; the file must still be present at:

```text
~/.local/share/Steam/depotcache/<depot>_<manifest-gid>.manifest
```

`ExxaTools` can then point the app at the pinned depot:

```lua
setmanifestid(108603, "7642676132386718371", 10337371365)
```

The app manifest must also list the depot once, with `StateFlags` set to
`2` while files are missing. Steam's updater reads the cached manifest and
stages the files without a CM manifest request code.

## Stability notes

Steam's bootstrapper re-verifies package file sizes on every launch, so the
container launcher wrapper (`srt-client-container`) is padded to the original
byte size and re-checked on every `exxa steam`. The wrapper passes the 64-bit
shim alone in `--ld-audit`: `pv-adverb` rejects a colon-separated list. The
session Xauthority cookie is picked by newest mtime, not by name, because the
login session rotates it.

## Companion server

The shim's `la_preinit` forks once from the Steam client process and execs
`~/.local/share/exxatools/bin/exxa serve`, so the tool starts with Steam with
no scripts, no autostart entry, and no supervision loop. The child calls
`setsid`, rebinds stdio to `/dev/null`, and passes a NULL-terminated `execv`
argv (an unterminated argv was what silently killed the child on the first
attempt). `exxa steam` and `exxa serve` keep the companion copy current and
executable (0755); a missing or stale copy makes the shim log
`exxa companion: ...` and continue without it. The server is single-instance
on its port, so a Steam restart reuses the running one.

## Patch verification

```sh
cargo test -p vapor-forge-scripting -p vapor-forge-features
cargo check -p vapor-forge-audit-loader --target x86_64-unknown-linux-gnu
git apply --check --reverse patches/vapor-forge/steam-1794536281.patch
```

The scripting tests cover two plans whose keys and manifests must retain their
declared app IDs. Feature tests verify that both manifest patches and depot
keys are refused when a different app requests them.

## Verified

Steam build `1794536281`, played through the containerised client with the shim
active inside it: `SteamAPI_Init` succeeds against the real `steam.pipe`,
Project Zomboid runs with full Steam API (not `-nosteam`), and the library
shows the installed build at its full size.