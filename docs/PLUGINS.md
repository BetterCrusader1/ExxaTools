# Plugins

ExxaTools runs small JavaScript plugins inside the Steam client. A plugin can add a
section to the ExxaTools panel in Steam and can ask the Rust host for a few scoped
things (its own config, a Steam status snapshot). It cannot reach the network, the
filesystem or other plugins' state.

This is not Millennium and does not load Millennium plugins. The host uses the same
public CEF debugging surface Steam exposes with `.cef-enable-remote-debugging`, and
runs each plugin in a CDP isolated world: the DOM is shared with Steam, the JavaScript
globals are not.

## Where plugins live

```
~/.config/exxatools/plugins/<id>/
  exxa-plugin.json      # manifest, required
  frontend.js           # entry named by the manifest
```

A folder without a valid manifest is listed as invalid and never injected. The folder
name does not have to match the id, but keeping them equal is easier to read.

## Manifest

```json
{
  "schema": 1,
  "id": "hello",
  "name": "Hello plugin",
  "version": "1.0.0",
  "author": "ExxaTools",
  "description": "Shows a small section in the Steam panel.",
  "entry": "frontend.js",
  "permissions": ["ui", "config", "steam"]
}
```

- `schema` must be `1`.
- `id` is lowercase letters, digits, `.`, `_` and `-`, at most 64 characters, and has
  to start with a letter or digit. It identifies the plugin in state and in the API.
- `entry` is a relative path inside the plugin folder. Absolute paths, `..` and
  backslashes are refused. The file must exist, stay inside the folder, and be at most
  512 KiB.
- `permissions` may contain `ui`, `config`, `steam`, `plugin-data`. Unknown values make
  the plugin invalid.

At most 20 enabled plugins are injected. The rest are listed but not loaded.

## The `exxa` object

Inside the isolated world the host provides:

```js
exxa.version                 // "1"
exxa.plugin                  // { id, name, version }
exxa.log(...args)            // console.log prefixed with the plugin id
exxa.ui.section(title, build) // build returns a DOM node; returns true when mounted
exxa.ui.toast(text)          // small toast in the Steam window

await exxa.config.get(key)              // permission: config
await exxa.config.set(key, value)       // permission: config, JSON value ≤ 4 KiB
await exxa.steam.status()               // permission: steam
                                        // { steam_running, engine, plans }
```

`exxa.ui.section` appends into the Plugins block of the panel. Sections are DOM nodes,
so build them with `document.createElement`; the section survives panel rebuilds until
the window reloads.

Calls are not direct function calls into Rust: the plugin writes into a DOM queue, the
host drains it on a short tick, checks the manifest permission, answers, and the plugin
resolves a promise. A denied call rejects with `permission denied: <name>`.

`plugin-data` is reserved for a future per-plugin file area; nothing reads it yet.

## Lifecycle

- The host injects on start and after Steam reloads; it reconnects when the client
  restarts, provided Steam was started with the debugging flag ExxaTools writes into
  the launcher.
- Disabling a plugin removes its folder from the next injection; the current page keeps
  the section until the window reloads.
- A plugin that throws while loading is logged and skipped; other plugins and the page
  continue.

## Management

```
exxa plugins list
exxa plugins enable hello
exxa plugins disable hello
exxa plugins remove hello
```

HTTP, with the session token:

```
GET  /api/plugins
POST /api/plugins/toggle   {"id": "hello", "enabled": false}
POST /api/plugins/remove   {"id": "hello"}
POST /api/plugins/inject   # re-inject without restarting Steam
```

The dashboard's Settings page lists every plugin with its state and a toggle.

## Security model

- Plugin JavaScript runs in an isolated world: separate globals, shared DOM. It cannot
  read Steam's JavaScript objects or other plugins' variables.
- Every host call is checked against the manifest in Rust. The manifest is read from
  disk, not from the plugin's own claims at runtime.
- Plugins are unsigned, arbitrary JavaScript by design. Only install plugins you trust;
  a plugin can still change the DOM of the Steam window it runs in.
- The debug port is loopback-only. Treat any local process able to reach it as able to
  change the Steam UI.
- Do not run ExxaTools plugins and Millennium injection in the same client; both rewrite
  the same panel area and neither coordinates with the other.

## Sample

`examples/plugins/hello` is a complete plugin: it reads the Steam status, stores a
greeting through `exxa.config`, and renders a section with a button.

```
mkdir -p ~/.config/exxatools/plugins
cp -r examples/plugins/hello ~/.config/exxatools/plugins/
./target/release/exxa plugins list
./target/release/exxa steamui inject
```