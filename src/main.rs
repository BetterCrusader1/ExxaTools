//! ExxaTools: Steam depots and manifests for the Linux client, on Vapor Forge.

use exxa::{
    appmanifest,
    steamui,
    window,
    desktop,
    diagnose,
    diagnostics,
    doctor,
    engine,
    lua,
    paths,
    pins,
    plans,
    plugins,
    providers,
    selftest,
    server,
    sources,
    verify,
    state,
    storage,
    steam,
    steamctl,
    vfsys,
    watch,
};

use serde_json::{json, Value};

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|arg| arg == flag).and_then(|index| args.get(index + 1)).cloned()
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn print_json(value: &Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into()));
}

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("error: {}", message.as_ref());
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        print_json(&json!({
            "app": "exxa",
            "version": env!("CARGO_PKG_VERSION"),
            "usage": [
                "exxa status",
                "exxa engine status|install [tag]|uninstall|releases",
                "exxa search <term>",
                "exxa app <appid>",
                "exxa plan <appid> [--branch public] [--write]",
                "exxa fix <appid> [--mode fix|block|unblock]",
                "exxa check <appid>",
                "exxa key <depot> <64-hex-key>",
                "exxa code <app> <depot> <gid>",
                "exxa log [lines]",
                "exxa discover [limit]",
                "exxa provider [--write]",
                "exxa serve [--port 8765] [--window]",
                "exxa steam [steam args...]",
                "exxa steamui status|inject|remove|install|uninstall|enable",
                "exxa shortcut [install|remove]",
                "exxa pins <appid> [--branch]",
                "exxa autostart [install|remove]",
                "exxa export <appid> > plan.json",
                "exxa import-plan plan.json",
                "exxa pin <appid> <depot> [gid|clear]",
                "exxa doctor",
                "exxa diagnose",
                "exxa selftest",
                "exxa diagnostics",
                "exxa storage",
                "exxa plugins [list|enable <id>|disable <id>|remove <id>]",
                "exxa window",
                "exxa update",
                "exxa version",
                "exxa verify <appid>",
                "exxa launchoptions <appid> [options]",
                "exxa sources",
                "exxa watch [--once] [--apply]",
            ],
        }));
        return;
    };
    let rest = &args[1..];

    match command.as_str() {
        "status" => {
            let st = state::load().unwrap_or_else(|error| fail(error));
            print_json(&json!({
                "version": env!("CARGO_PKG_VERSION"),
                "steam_root": steamctl::root().map(|path| path.display().to_string()),
                "steam_running": steamctl::is_running(),
                "engine": engine::detect(),
                "keys": st.depot_keys.len(),
                "apps": st.apps.len(),
                "codes": st.codes.len(),
                "state": state::state_path().display().to_string(),
            }));
        }
        "engine" => {
            let action = rest.first().cloned().unwrap_or_else(|| "status".into());
            match action.as_str() {
                "status" => print_json(&engine::detect()),
                "releases" => {
                    let list = engine::releases(5).unwrap_or_else(|error| fail(error));
                    print_json(&Value::Array(list));
                }
                "install" => {
                    let tag = rest.get(1).cloned();
                    print_json(&engine::install(tag.as_deref()).unwrap_or_else(|error| fail(error)));
                }
                "uninstall" => print_json(&engine::uninstall().unwrap_or_else(|error| fail(error))),
                other => fail(format!("unknown engine action {other}; use status, install, uninstall, releases")),
            }
        }
        "search" => {
            let term = rest.join(" ");
            if term.trim().is_empty() {
                fail("search needs a term");
            }
            let st = state::load().unwrap_or_default();
            let items = steam::search(&term, &st.settings.country).unwrap_or_else(|error| fail(error));
            print_json(&Value::Array(items));
        }
        "discover" => {
            let limit = rest.first().and_then(|value| value.parse::<usize>().ok()).unwrap_or(18);
            let st = state::load().unwrap_or_default();
            print_json(&Value::Array(steam::discover(&st.settings.country, limit)));
        }
        "app" => {
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("app needs a numeric app id");
            };
            print_json(&steam::catalog(appid).unwrap_or_else(|error| fail(error)));
        }
        "plan" => {
            if rest.first().map(String::as_str) == Some("fetch") {
                let Some(appid) = rest.get(1).and_then(|value| value.parse::<u64>().ok()) else {
                    fail("plan fetch needs a numeric app id");
                };
                let name = arg_value(rest, "--name");
                print_json(&plans::fetch_plan(appid, name.as_deref()).unwrap_or_else(|error| fail(error)));
                return;
            }
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("plan needs a numeric app id");
            };
            let st = state::load().unwrap_or_else(|error| fail(error));
            let branch = arg_value(rest, "--branch").unwrap_or_else(|| "public".into());
            let catalog = steam::catalog(appid).unwrap_or_else(|error| fail(error));
            let name = catalog.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let mut depots: Vec<Value> = Vec::new();
            if let Some(list) = catalog.get("depots").and_then(Value::as_array) {
                for depot in list {
                    let oslist = depot.get("oslist").and_then(Value::as_array).cloned().unwrap_or_default();
                    let platform_ok = oslist.is_empty()
                        || oslist.iter().any(|value| value.as_str() == Some(st.settings.default_os.as_str()));
                    if !platform_ok {
                        continue;
                    }
                    let Some(manifest) = steam::pick_manifest(depot, &branch) else { continue };
                    let mut entry = depot.clone();
                    entry["gid"] = manifest.get("gid").cloned().unwrap_or(Value::Null);
                    entry["size"] = manifest.get("size").cloned().unwrap_or(Value::Null);
                    depots.push(entry);
                }
            }
            let dlc: Vec<u64> = catalog
                .get("dlc")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default();
            let text = lua::render_plan(appid, &name, &branch, &depots, &st.depot_keys, &dlc, &crate::vfsys::stamp(), "auto");
            if has_flag(rest, "--write") {
                std::fs::create_dir_all(paths::vf_scripts()).unwrap_or_else(|error| fail(error.to_string()));
                let target = paths::vf_scripts().join(format!("exxatools-{appid}.lua"));
                std::fs::write(&target, &text).unwrap_or_else(|error| fail(error.to_string()));
                print_json(&json!({ "written": target.display().to_string(), "bytes": text.len() }));
            } else {
                println!("{text}");
                let check = engine::check_script(&text, "plan.lua");
                if !check.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                    eprintln!("{}", check.get("output").and_then(Value::as_str).unwrap_or("luac failed"));
                    std::process::exit(2);
                }
            }
        }
        "fix" => {
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("fix needs a numeric app id");
            };
            let mode = arg_value(rest, "--mode").unwrap_or_else(|| "fix".into());
            let mode = appmanifest::Mode::parse(&mode).unwrap_or_else(|error| fail(error));
            print_json(&appmanifest::patch(appid, &mode, true).unwrap_or_else(|error| fail(error)));
        }
        "check" => {
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("check needs a numeric app id");
            };
            let st = state::load().unwrap_or_default();
            let catalog = steam::catalog(appid).unwrap_or_else(|error| fail(error));
            let name = catalog.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let depots: Vec<Value> = catalog.get("depots").and_then(Value::as_array).cloned().unwrap_or_default();
            let text = lua::render_plan(appid, &name, "public", &depots, &st.depot_keys, &[], &crate::vfsys::stamp(), "auto");
            print_json(&engine::check_script(&text, "plan.lua"));
        }
        "key" => {
            let Some(depot) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("key needs a depot id");
            };
            let Some(raw) = rest.get(1) else { fail("key needs a value") };
            let Some(key) = state::valid_key(raw) else {
                fail("that is not a depot key: 64 hex characters expected")
            };
            let mut st = state::load().unwrap_or_else(|error| fail(error));
            state::set_key(&mut st, depot, Some(&key));
            state::save(&st).unwrap_or_else(|error| fail(error));
            print_json(&json!({ "depot": depot, "key": format!("{}...", &key[..8]) }));
        }
        "code" => {
            let Some(app) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("code needs an app id");
            };
            let Some(depot) = rest.get(1).and_then(|value| value.parse::<u64>().ok()) else {
                fail("code needs a depot id");
            };
            let Some(gid) = rest.get(2) else { fail("code needs a manifest id") };
            let st = state::load().unwrap_or_default();
            let order: Vec<Value> = st
                .settings
                .manifest_providers
                .iter()
                .map(|name| Value::String(name.clone()))
                .collect();
            print_json(&providers::fetch_code(app, depot, gid, &order).unwrap_or_else(|error| fail(error)));
        }
        "steam" => {
            let mut args: Vec<String> = rest.to_vec();
            if !args.iter().any(|value| value == steamui::FLAG) {
                args.insert(0, steamui::FLAG.to_string());
            }
            if let Err(error) = steamctl::ensure_server(8765) {
                fail(error);
            }
            if let Err(error) = steamctl::launch(&args) {
                fail(error);
            }
        }
        "serve" => {
            let port = arg_value(rest, "--port").and_then(|value| value.parse().ok()).unwrap_or(8765);
            if has_flag(rest, "--window") {
                let url = format!("http://127.0.0.1:{port}/");
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = desktop::open_window(&url);
                });
            }
            crate::steamui::ensure_started();
            if let Err(error) = server::serve(server::Config { port }) {
                fail(error);
            }
        }
        "window" => {
            let port = arg_value(rest, "--port").unwrap_or_else(|| "8765".into());
            let url = arg_value(rest, "--url").unwrap_or_else(|| format!("http://127.0.0.1:{port}/"));
            if std::net::TcpStream::connect(("127.0.0.1", port.parse().unwrap_or(8765))).is_err() {
                let exe = std::env::current_exe().unwrap_or_default();
                let _ = std::process::Command::new(exe)
                    .args(["serve", "--port", &port])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                std::thread::sleep(std::time::Duration::from_millis(800));
            }
            if let Err(error) = window::run(&url) {
                fail(error);
            }
        }
        "diagnostics" => print_json(&diagnostics::build()),
        "diagnose" => print_json(&diagnose::from_files()),
        "plugins" => {
            let action = rest.first().map(String::as_str).unwrap_or("list");
            let result = match action {
                "list" | "status" => Ok(plugins::list_json()),
                "enable" | "disable" => {
                    let id = rest.get(1).cloned().unwrap_or_default();
                    plugins::set_enabled(&id, action == "enable")
                }
                "remove" => {
                    let id = rest.get(1).cloned().unwrap_or_default();
                    plugins::remove(&id)
                }
                other => Err(format!("unknown plugins action: {other}")),
            };
            print_json(&result.unwrap_or_else(|error| fail(error)));
        }
        "storage" => print_json(&storage::report()),
        "selftest" => {
            let report = selftest::run();
            print_json(&report);
            if !report.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                std::process::exit(1);
            }
        }
        "steamui" => {
            let action = rest.first().cloned().unwrap_or_else(|| "status".into());
            match action.as_str() {
                "status" => print_json(&steamui::status()),
                "inject" => print_json(&steamui::inject().unwrap_or_else(|error| fail(error))),
                "remove" => print_json(&steamui::remove()),
                "enable" => print_json(&steamui::enable_debug().unwrap_or_else(|error| fail(error))),
                "install" => print_json(&steamui::install_launcher().unwrap_or_else(|error| fail(error))),
                "uninstall" => print_json(&steamui::uninstall_launcher()),
                other => fail(format!("unknown steamui action {other}; use status, inject, remove, install, uninstall, enable")),
            }
        }
        "shortcut" => {
            let action = rest.first().cloned().unwrap_or_else(|| "install".into());
            match action.as_str() {
                "install" => print_json(&desktop::install_desktop_entry().unwrap_or_else(|error| fail(error))),
                "remove" => print_json(&desktop::uninstall_desktop_entry()),
                other => fail(format!("unknown shortcut action {other}; use install or remove")),
            }
        }
        "version" => print_json(&json!({
            "version": env!("CARGO_PKG_VERSION"),
            "project": paths::project_dir().display().to_string(),
        })),
        "update" => {
            let project = paths::project_dir();
            let git = project.join(".git");
            if !git.exists() {
                fail("this checkout has no .git; update it the way it was installed");
            }
            let pull = std::process::Command::new("git")
                .args(["-C", &project.display().to_string(), "pull", "--ff-only", "--quiet"])
                .output();
            match pull {
                Ok(output) if output.status.success() => {
                    let before = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let build = std::process::Command::new("cargo")
                        .args(["build", "--release", "--quiet"])
                        .current_dir(&project)
                        .status();
                    let built = build.map(|status| status.success()).unwrap_or(false);
                    print_json(&json!({ "updated": true, "git": before, "rebuilt": built }));
                }
                Ok(output) => fail(String::from_utf8_lossy(&output.stderr).trim()),
                Err(error) => fail(error.to_string()),
            }
        }
        "pins" => {
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("pins needs a numeric app id");
            };
            let mut st = state::load().unwrap_or_else(|error| fail(error));
            let branch = arg_value(rest, "--branch").unwrap_or_else(|| "public".into());
            let result = pins::compare(&mut st, appid, &branch).unwrap_or_else(|error| fail(error));
            if result.get("changed").and_then(Value::as_bool).unwrap_or(false) {
                state::save(&st).unwrap_or_else(|error| fail(error));
            }
            print_json(&result);
        }
        "pin" => {
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("pin needs an app id");
            };
            let Some(depot) = rest.get(1).and_then(|value| value.parse::<u64>().ok()) else {
                fail("pin needs a depot id");
            };
            let gid = rest.get(2).cloned().filter(|value| value != "clear");
            let mut st = state::load().unwrap_or_else(|error| fail(error));
            let result = pins::set_pin(&mut st, appid, depot, gid.as_deref()).unwrap_or_else(|error| fail(error));
            state::save(&st).unwrap_or_else(|error| fail(error));
            print_json(&result);
        }
        "export" => {
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("export needs a numeric app id");
            };
            let st = state::load().unwrap_or_else(|error| fail(error));
            let record = st.apps.get(&appid.to_string()).cloned().unwrap_or_else(|| fail("no saved plan"));
            print_json(&record);
        }
        "import-plan" => {
            let Some(path) = rest.first() else { fail("import-plan needs a file path") };
            let text = std::fs::read_to_string(path).unwrap_or_else(|error| fail(error.to_string()));
            let record: Value = serde_json::from_str(&text).unwrap_or_else(|error| fail(format!("that is not a plan: {error}")));
            let appid = record.get("appid").and_then(Value::as_u64).unwrap_or_else(|| fail("plan has no appid"));
            if record.get("depots").and_then(Value::as_array).is_none() {
                fail("plan has no depots");
            }
            let mut st = state::load().unwrap_or_else(|error| fail(error));
            st.apps.insert(appid.to_string(), record);
            state::save(&st).unwrap_or_else(|error| fail(error));
            print_json(&json!({ "imported": appid }));
        }
        "autostart" => {
            let action = rest.first().cloned().unwrap_or_else(|| "install".into());
            match action.as_str() {
                "install" | "on" => print_json(&desktop::install_autostart().unwrap_or_else(|error| fail(error))),
                "remove" | "off" => print_json(&desktop::remove_autostart()),
                other => fail(format!("unknown autostart action {other}; use install or remove")),
            }
        }
        "doctor" => print_json(&doctor::run()),
        "verify" => {
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("verify needs a numeric app id");
            };
            print_json(&verify::run(appid));
        }
        "launchoptions" => {
            let Some(appid) = rest.first().and_then(|value| value.parse::<u64>().ok()) else {
                fail("launchoptions needs a numeric app id");
            };
            if let Some(flag) = arg_value(rest, "--add") {
                print_json(&steamctl::add_launch_option(appid, &flag).unwrap_or_else(|error| fail(error)));
            } else if let Some(options) = rest.get(1).filter(|value| !value.starts_with("--")) {
                print_json(&steamctl::set_launch_options(appid, options).unwrap_or_else(|error| fail(error)));
            } else {
                print_json(&json!({
                    "path": steamctl::localconfig_path().map(|path| path.display().to_string()),
                    "options": steamctl::read_launch_options(appid),
                }));
            }
        }
        "sources" => {
            let st = state::load().unwrap_or_else(|error| fail(error));
            print_json(&json!({
                "sources": sources::catalog(&st.settings),
                "default": st.settings.default_source,
            }));
        }
        "watch" => {
            if has_flag(rest, "--once") {
                let apply = has_flag(rest, "--apply");
                print_json(&watch::sweep(apply));
            } else {
                watch::run().unwrap_or_else(|error| fail(error));
            }
        }
        "log" => {
            let lines = rest.first().and_then(|value| value.parse::<usize>().ok()).unwrap_or(200);
            print_json(&engine::log(lines));
        }
        "provider" => {
            let st = state::load().unwrap_or_else(|error| fail(error));
            let order: Vec<Value> = st
                .settings
                .manifest_providers
                .iter()
                .map(|name| Value::String(name.clone()))
                .collect();
            let text = lua::render_provider(&order).unwrap_or_else(|error| fail(error));
            if has_flag(rest, "--write") {
                std::fs::create_dir_all(paths::vf_scripts()).unwrap_or_else(|error| fail(error.to_string()));
                let target = paths::vf_scripts().join(lua::PROVIDER_FILE);
                std::fs::write(&target, &text).unwrap_or_else(|error| fail(error.to_string()));
                print_json(&json!({ "written": target.display().to_string() }));
            } else {
                println!("{text}");
            }
        }
        other => fail(format!("unknown command {other}")),
    }
}