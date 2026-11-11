//! ExxaTools: Steam depots and manifests for the Linux client, on Vapor Forge.

mod appmanifest;
mod steamui;
mod desktop;
mod cdp;
mod engine;
mod http;
mod lua;
mod paths;
mod providers;
mod server;
mod state;
mod steam;
mod hubcap;
mod steamctl;
mod vfsys;
mod vdf;

use serde_json::{json, Value};

pub fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

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
                "exxa steamui status|inject|remove|enable",
                "exxa shortcut [install|remove]",
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
            let text = lua::render_plan(appid, &name, &branch, &depots, &st.depot_keys, &dlc, "auto");
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
            let text = lua::render_plan(appid, &name, "public", &depots, &st.depot_keys, &[], "auto");
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
        "serve" => {
            let port = arg_value(rest, "--port").and_then(|value| value.parse().ok()).unwrap_or(8765);
            if has_flag(rest, "--window") {
                let url = format!("http://127.0.0.1:{port}/");
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = desktop::open_window(&url);
                });
            }
            if let Err(error) = server::serve(server::Config { port }) {
                fail(error);
            }
        }
        "steamui" => {
            let action = rest.first().cloned().unwrap_or_else(|| "status".into());
            match action.as_str() {
                "status" => print_json(&steamui::status()),
                "inject" => print_json(&steamui::inject().unwrap_or_else(|error| fail(error))),
                "remove" => print_json(&steamui::remove()),
                "enable" => print_json(&steamui::enable_debug().unwrap_or_else(|error| fail(error))),
                other => fail(format!("unknown steamui action {other}; use status, inject, remove, enable")),
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