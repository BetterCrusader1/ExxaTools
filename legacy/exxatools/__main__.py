"""Command line entry point: `python3 -m exxatools ...`."""

from __future__ import annotations

import argparse
import sys
import time
import webbrowser

from . import appmanifest, desktop, engine, hubcap, lua, providers, server, steam, steamui, store, vfsys


def _fmt_size(value: int | None) -> str:
    if not value:
        return "-"
    size = float(value)
    for unit in ("B", "KiB", "MiB", "GiB", "TiB"):
        if size < 1024 or unit == "TiB":
            return f"{size:.1f} {unit}"
        size /= 1024
    return "-"


def cmd_serve(args: argparse.Namespace) -> int:
    url = f"http://{args.host}:{args.port}/"
    if args.window:
        httpd, _thread = server.serve_background(args.port)
        print(f"ExxaTools listening on {url}")
        window = desktop.open_window(url, prefer=args.window_mode)
        print(f"window: {window['mode']}")
        if window.get("pid"):
            desktop.wait_for_window(window["pid"])
        else:
            print("no app window available; serving until Ctrl-C")
            try:
                while True:
                    time.sleep(1)
            except KeyboardInterrupt:
                pass
        httpd.shutdown()
        httpd.server_close()
        return 0
    if not args.no_browser:
        webbrowser.open(url)
    server.serve(args.host, args.port, verbose=args.verbose)
    return 0


def cmd_search(args: argparse.Namespace) -> int:
    items = steam.search(args.term, args.cc)
    if not items:
        print("no results")
        return 1
    for item in items:
        price = f"  {item['price']}" if item.get("price") else ""
        print(f"{item['appid']:>8}  {item['name']}{price}")
    return 0


def cmd_app(args: argparse.Namespace) -> int:
    catalog = steam.catalog(args.appid)
    state = store.load()
    keys = state["depot_keys"]
    print(f"{catalog['name']}  ({catalog['appid']}, {catalog['type']})")
    print(f"branches: {', '.join(branch['name'] for branch in catalog['branches']) or '-'}")
    print()
    print(f"{'depot':>10}  {'os':<14} {'gid':>20} {'size':>10}  key")
    for depot in catalog["depots"]:
        manifest = steam.pick_manifest(depot, args.branch)
        if manifest is None:
            continue
        os_label = ",".join(depot["oslist"]) or "all"
        has_key = "yes" if keys.get(str(depot["depot_id"])) else "-"
        print(
            f"{depot['depot_id']:>10}  {os_label:<14} {manifest['gid']:>20} "
            f"{_fmt_size(manifest['size']):>10}  {has_key}"
        )
    if catalog["dlc"]:
        print(f"\nDLC: {' '.join(str(dlc) for dlc in catalog['dlc'])}")
    return 0


def cmd_key(args: argparse.Namespace) -> int:
    if args.value in ("-", "none", "clear"):
        store.update(lambda state: store.set_key(state, args.depot_id, None))
        print(f"cleared key for {args.depot_id}")
        return 0
    key = store.valid_key(args.value)
    if key is None:
        print("a depot key is 64 hex characters", file=sys.stderr)
        return 2
    store.update(lambda state: store.set_key(state, args.depot_id, key))
    print(f"stored key for {args.depot_id}")
    return 0


def cmd_code(args: argparse.Namespace) -> int:
    state = store.load()
    cached = store.cached_code(state, args.gid)
    if cached is not None and not args.refresh:
        print(f"{cached}  (cache)")
        return 0
    result = providers.fetch_code(
        args.appid, args.depot_id, args.gid, order=state["settings"]["manifest_providers"]
    )
    store.update(lambda current: store.remember_code(current, args.gid, result["code"]))
    print(f"{result['code']}  ({result['provider']})")
    return 0


def cmd_lua(args: argparse.Namespace) -> int:
    state = store.load()
    record = store.app_record(state, args.appid)
    if record is None:
        print("no saved plan for that app", file=sys.stderr)
        return 1
    text = lua.render_from_record(record, state["depot_keys"])
    if args.write:
        path = vfsys.write_script(args.appid, text)
        print(path)
    else:
        sys.stdout.write(text)
    return 0


def cmd_install(args: argparse.Namespace) -> int:
    state = store.load()
    record = store.app_record(state, args.appid)
    if record is None:
        print("no saved plan for that app", file=sys.stderr)
        return 1
    text = lua.render_from_record(record, state["depot_keys"])
    path = vfsys.write_script(args.appid, text, enabled=not args.disabled)
    print(path)
    missing = lua.missing_keys(record["depots"], state["depot_keys"])
    if missing:
        print("depots without a key: " + ", ".join(str(depot) for depot in missing))
    return 0


def cmd_status(args: argparse.Namespace) -> int:
    data = vfsys.status()
    forge = data["vapor_forge"]
    print(f"vapor forge installed: {'yes' if forge['installed'] else 'no'}")
    for arch, libs in forge["libs"].items():
        print(f"  {arch}: {', '.join(libs) if libs else '-'}")
    print(f"ld_audit configured:  {'yes' if forge['ld_audit']['configured'] else 'no'}")
    for entry in forge["ld_audit"]["evidence"]:
        print(f"  {entry}")
    print(f"scripts dir:          {forge['scripts_dir']}")
    print(f"script dirs scanned:  {', '.join(forge['script_dirs'])}")
    print(f"installed scripts:    {len(data['scripts'])}")
    for script in data["scripts"]:
        flag = "on " if script["enabled"] else "off"
        print(f"  [{flag}] {script['file']}  {script['name']}")
    print(f"saved plans:          {len(data['apps'])}")
    print(f"state:                {data['state_path']}")
    return 0


def cmd_engine(args: argparse.Namespace) -> int:
    if args.action == "status":
        info = engine.detect()
        print(f"engine installed:  {'yes' if info['installed'] else 'no'}")
        print(f"version:           {info['version'] or '-'}")
        print(f"libs:              {sum(len(v) for v in info['libs'].values())}/{len(engine.ARCHES) * len(engine.LIBS)}")
        if info["missing"]:
            print(f"missing:           {', '.join(info['missing'])}")
        print(f"ld_audit wired:    {'yes' if info['ld_audit']['configured'] else 'no'}")
        for entry in info["ld_audit"]["evidence"]:
            print(f"  {entry}")
        print(f"config:            {info['config_path']}{'' if info['config_exists'] else ' (not created yet)'}")
        print(f"scripts:           {info['scripts_dir']}")
        print(f"steam running:     {'yes' if info['steam_running'] else 'no'}")
        if info["steam_running"]:
            print(f"engine in steam:   {'loaded' if info['steam_engine_loaded'] else 'not loaded (restart Steam)'}")
        if info["needs_restart"]:
            print("restart Steam to load the freshly installed engine")
        return 0 if info["installed"] else 1

    if args.action == "releases":
        for release in engine.releases(5):
            print(f"{release['tag']:<10} {release['published'][:10]}  {_fmt_size(release['size'])}  {release['asset']}")
        return 0

    if args.action == "install":
        info = engine.install(args.tag)
        print(f"installed {info['version']}  sha256 {info['sha256'][:16]}...")
        for line in info["installer"]["output"]:
            print(f"  {line}")
        if info["status"]["needs_restart"]:
            print("Steam is running: restart it so LD_AUDIT picks the engine up")
        return 0

    if args.action == "uninstall":
        info = engine.uninstall()
        for line in info["output"]:
            print(f"  {line}")
        print("engine removed; restart Steam")
        return 0
    return 2


def cmd_check(args: argparse.Namespace) -> int:
    state = store.load()
    record = store.app_record(state, args.appid)
    if record is None:
        print("no saved plan for that app", file=sys.stderr)
        return 1
    text = lua.render_from_record(record, state["depot_keys"])
    report = engine.check_script(text, f"{args.appid}.lua")
    print(f"tool:   {report['tool'] or 'none (install luac for a real parse)'}")
    print(f"result: {'ok' if report['ok'] else 'failed'}")
    if report["output"]:
        print(report["output"])
    for problem in report["lint"]:
        print(f"lint: {problem}")
    return 0 if report["ok"] else 1


def cmd_fix(args: argparse.Namespace) -> int:
    result = appmanifest.patch(args.appid, args.mode, steam_check=not args.force)
    print(f"{result['path']}")
    for change in result["changes"]:
        state = "set" if change["updated"] else "added"
        print(f"  {change['key']} = {change['value']}  ({state})")
    for problem in result["after"]["problems"]:
        print(f"still: {problem}")
    print(f"backup: {result['backup']}")
    return 0


def cmd_provider(args: argparse.Namespace) -> int:
    state = store.load()
    order = args.providers.split(",") if args.providers else state["settings"]["manifest_providers"]
    order = [item.strip() for item in order if item.strip()]
    text = lua.render_provider(order, vfsys.stamp())
    if args.write:
        path = vfsys.write_provider(text)
        print(path)
        report = engine.check_script(text, lua.PROVIDER_FILE, plan=False)
        print(f"parse: {'ok' if report['ok'] else 'failed'}")
        if not report["ok"] and report["output"]:
            print(report["output"])
            return 1
    else:
        sys.stdout.write(text)
    return 0


def cmd_hubcap(args: argparse.Namespace) -> int:
    state = store.load()
    key = args.api_key or str(state["settings"].get("hubcap_key") or "")

    if args.action == "key":
        valid = hubcap.valid_key(args.value or "")
        if valid is None:
            print("a Hubcap key starts with smm and has no spaces", file=sys.stderr)
            return 2
        info = hubcap.validate(valid)
        store.update(lambda current: current["settings"].__setitem__("hubcap_key", valid))
        print(f"key stored  usage {info.get('usage')}/{info.get('limit')}")
        return 0

    if args.action == "clear":
        store.update(lambda current: current["settings"].__setitem__("hubcap_key", ""))
        print("key cleared")
        return 0

    if not key:
        print("no Hubcap key: ./vd hubcap key smm_...", file=sys.stderr)
        return 1

    if args.action == "stats":
        info = hubcap.validate(key)
        print(f"usage: {info.get('usage')}/{info.get('limit')}  can_request={info.get('can_request')}")
        return 0

    if args.action == "search":
        term = (args.value or "").strip()
        if not term:
            print("search needs a term", file=sys.stderr)
            return 2
        for item in hubcap.search(key, term):
            print(f"{item['appid']:>8}  {item['name']}  {item['updated']}")
        return 0

    if args.action == "fetch":
        if not (args.value or "").isdigit():
            print("fetch needs an appid", file=sys.stderr)
            return 2
        appid = int(args.value)
        fetched = hubcap.fetch(key, appid)
        parsed = hubcap.parse_lua(fetched["lua"], appid)
        record = store.app_record(state, appid)
        summary = store.update(
            lambda current: hubcap.apply_to_state(
                current,
                parsed,
                vfsys.stamp(),
                name=(record or {}).get("name") or args.name,
                pin=args.pin,
                create=args.plan,
            )
        )
        print(f"app {appid}: {summary['keys']}/{summary['depots']} depot keys, "
              f"{len(summary['dlc'])} dlc, pinned {summary['pinned']}")
        if summary["created_plan"]:
            print("plan created from the Hubcap snapshot")
        if summary["missing_keys"]:
            print("no key in Hubcap for: " + ", ".join(str(value) for value in summary["missing_keys"]))
        if fetched["manifests"]:
            print(f"archive also carried {len(fetched['manifests'])} manifest blobs (unused)")
        return 0
    return 2


def cmd_steamui(args: argparse.Namespace) -> int:
    if args.action == "status":
        info = steamui.status()
        print(f"steam running:  {'yes' if info['steam_running'] else 'no'}")
        print(f"debug port:     {info['debug_port']} ({'ready' if info['debug_ready'] else 'closed'})")
        print(f"menu injected:  {'yes' if info['injected'] else 'no'}")
        if info.get("error"):
            print(f"error:          {info['error']}")
        return 0
    if args.action == "inject":
        info = steamui.inject()
        print(f"injected ({info['result']})")
        return 0
    if args.action == "remove":
        info = steamui.remove()
        print(f"removed: {info.get('removed')}")
        return 0
    if args.action == "enable":
        info = steamui.enable_debug()
        print(f"flag: {info['flag']}")
        for path in info["patched"]:
            print(f"  patched {path}")
        print("Steam restarted with the debugging port")
        return 0
    return 2


def cmd_shortcut(args: argparse.Namespace) -> int:
    if args.action == "install":
        info = desktop.install_desktop_entry()
        print(f"menu entry: {info['path']}")
        print(f"exec:       {desktop.LAUNCHER} serve --window")
        return 0
    info = desktop.uninstall_desktop_entry()
    print(f"removed: {info['path']}" if info["removed"] else "no menu entry installed")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="exxatools", description="Steam depot/manifest manager for Vapor Forge")
    sub = parser.add_subparsers(dest="command", required=True)

    serve = sub.add_parser("serve", help="start the local web UI")
    serve.add_argument("--host", default="127.0.0.1")
    serve.add_argument("--port", type=int, default=8765)
    serve.add_argument("--no-browser", action="store_true")
    serve.add_argument("--verbose", action="store_true")
    serve.add_argument("--window", action="store_true", help="run as a desktop window instead of a browser tab")
    serve.add_argument(
        "--window-mode",
        choices=["auto", "webkit", "chromium", "firefox", "browser"],
        default="auto",
        help="which shell to render the UI in",
    )
    serve.set_defaults(func=cmd_serve)

    eng = sub.add_parser("engine", help="install, inspect, or remove the Vapor Forge engine")
    eng.add_argument("action", choices=["status", "install", "uninstall", "releases"])
    eng.add_argument("--tag", help="pin a specific release tag")
    eng.set_defaults(func=cmd_engine)

    check = sub.add_parser("check", help="parse a saved plan the way the runtime will")
    check.add_argument("appid", type=int)
    check.set_defaults(func=cmd_check)

    fix = sub.add_parser("fix", help="patch the appmanifest so Steam stops updating the app")
    fix.add_argument("appid", type=int)
    fix.add_argument("--mode", choices=["fix", "block", "unblock"], default="fix")
    fix.add_argument("--force", action="store_true", help="patch even while Steam is running")
    fix.set_defaults(func=cmd_fix)

    prov = sub.add_parser("provider", help="render the manifest request-code provider script")
    prov.add_argument("--providers", help="comma separated provider names or URLs")
    prov.add_argument("--write", action="store_true", help="install it into Vapor Forge's script dir")
    prov.set_defaults(func=cmd_provider)

    shortcut = sub.add_parser("shortcut", help="add or remove the desktop menu entry")
    shortcut.add_argument("action", nargs="?", choices=["install", "uninstall"], default="install")
    shortcut.set_defaults(func=cmd_shortcut)

    hub = sub.add_parser("hubcap", help="Hubcap Manifest: depot keys and snapshots per app")
    hub.add_argument("action", choices=["key", "clear", "stats", "search", "fetch"])
    hub.add_argument("value", nargs="?", help="key, search term, or appid")
    hub.add_argument("--key", dest="api_key", help="override the stored Hubcap key")
    hub.add_argument("--pin", action="store_true", help="also pin Hubcap's manifest ids in the saved plan")
    hub.add_argument("--plan", action="store_true", help="create a plan from the Hubcap snapshot if none exists")
    hub.add_argument("--name", help="name for a plan created by --plan")
    hub.set_defaults(func=cmd_hubcap)

    ui = sub.add_parser("steamui", help="put our icon next to Help inside the Steam client")
    ui.add_argument("action", nargs="?", choices=["status", "inject", "remove", "enable"], default="status")
    ui.set_defaults(func=cmd_steamui)

    search = sub.add_parser("search", help="search the Steam store")
    search.add_argument("term")
    search.add_argument("--cc", default=store.DEFAULT_SETTINGS["country"])
    search.set_defaults(func=cmd_search)

    app = sub.add_parser("app", help="list depots and manifests for an app")
    app.add_argument("appid", type=int)
    app.add_argument("--branch", default="public")
    app.set_defaults(func=cmd_app)

    key = sub.add_parser("key", help="store or clear a depot key")
    key.add_argument("depot_id", type=int)
    key.add_argument("value", help="64 hex characters, or '-' to clear")
    key.set_defaults(func=cmd_key)

    code = sub.add_parser("code", help="fetch a manifest request code")
    code.add_argument("appid", type=int)
    code.add_argument("depot_id", type=int)
    code.add_argument("gid")
    code.add_argument("--refresh", action="store_true")
    code.set_defaults(func=cmd_code)

    render = sub.add_parser("lua", help="render the Lua plan for a saved app")
    render.add_argument("appid", type=int)
    render.add_argument("--write", action="store_true", help="write it into the script directory")
    render.set_defaults(func=cmd_lua)

    install = sub.add_parser("install", help="write a saved plan into Vapor Forge")
    install.add_argument("appid", type=int)
    install.add_argument("--disabled", action="store_true")
    install.set_defaults(func=cmd_install)

    status = sub.add_parser("status", help="show Vapor Forge install and script state")
    status.set_defaults(func=cmd_status)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return int(args.func(args))
    except KeyboardInterrupt:
        return 130
    except Exception as error:  # noqa: BLE001 - CLI boundary
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
