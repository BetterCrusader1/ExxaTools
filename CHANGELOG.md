# Changelog

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

