//! The GTK thread never waits on the network. Every command runs on a plain
//! thread and reports back through one channel the shell drains on the main loop.

use std::sync::mpsc::{channel, Receiver, Sender};

use crate::ui::covers;
use crate::ui::model::{DoctorReport, Game};

#[derive(Clone, Debug)]
pub struct SettingsDraft {
    pub country: String,
    pub hubcap_key: String,
    pub ryuu_key: String,
    pub default_source: String,
    pub manifest_providers: Vec<String>,
    pub auto_update_plans: bool,
}

#[derive(Clone, Debug)]
pub enum Command {
    LoadLibrary,
    Search { id: u64, term: String },
    LoadDiscover,
    LoadDoctor,
    Add { appid: u64, name: String },
    Sync(u64),
    Remove(u64),
    Launch(u64),
    Fix(u64),
    RestartSteam,
    SaveSettings(SettingsDraft),
    CopyDiagnostics,
    SyncAll,
    LoadHero(u64),
    SetTheme(String),
}

#[derive(Debug)]
pub enum Event {
    Library(Vec<Game>),
    Search {
        id: u64,
        games: Vec<Game>,
    },
    Discover(Vec<Game>),
    Doctor(DoctorReport),
    Toast {
        text: String,
        error: bool,
    },
    Clipboard(String),
    RefreshLibrary,
    SettingsReload,
    Hero {
        appid: u64,
        path: Option<std::path::PathBuf>,
        logo: Option<std::path::PathBuf>,
    },
    Theme(String),
}

#[derive(Clone)]
pub struct Worker {
    commands: Sender<Command>,
}

impl Worker {
    pub fn start() -> (Self, Receiver<Event>) {
        let (command_tx, command_rx) = channel::<Command>();
        let (event_tx, event_rx) = channel::<Event>();
        heap::limit_arenas();
        std::thread::spawn(move || {
            while let Ok(command) = command_rx.recv() {
                let events = event_tx.clone();
                std::thread::spawn(move || {
                    run(command, events);
                    heap::trim();
                });
            }
        });
        (
            Self {
                commands: command_tx,
            },
            event_rx,
        )
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
}

/// glibc gives each thread its own malloc arena and keeps freed memory in
/// it. Every command runs on a fresh thread and downloads art in parallel,
/// so without a cap the freed HTTP and JSON buffers stayed resident.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod heap {
    extern "C" {
        fn mallopt(param: i32, value: i32) -> i32;
        fn malloc_trim(pad: usize) -> i32;
    }

    const M_ARENA_MAX: i32 = -8;

    pub fn limit_arenas() {
        // SAFETY: plain glibc tunables; no pointers cross the boundary.
        unsafe {
            mallopt(M_ARENA_MAX, 2);
        }
    }

    pub fn trim() {
        // SAFETY: returns free heap pages to the OS; never touches live data.
        unsafe {
            malloc_trim(0);
        }
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
mod heap {
    pub fn limit_arenas() {}
    pub fn trim() {}
}

fn run(command: Command, events: Sender<Event>) {
    match command {
        Command::LoadLibrary => {
            let mut games = library();
            decorate(&mut games);
            let _ = events.send(Event::Library(games));
        }
        Command::Search { id, term } => {
            let games = match search(&term) {
                Ok(mut games) => {
                    decorate(&mut games);
                    games
                }
                Err(error) => {
                    toast(&events, format!("Steam search failed: {error}"), true);
                    Vec::new()
                }
            };
            let _ = events.send(Event::Search { id, games });
        }
        Command::LoadDiscover => {
            let country = crate::state::load()
                .map(|state| state.settings.country)
                .unwrap_or_else(|_| "us".into());
            let mut games: Vec<Game> = crate::steam::discover(&country, 60)
                .iter()
                .filter_map(Game::from_discover)
                .collect();
            // A cold art cache costs ~7 s of downloads. Show what is already
            // on disk first, then replace it once every poster has arrived.
            let mut quick = games.clone();
            decorate_cached(&mut quick);
            quick.retain(|game| game.cover.is_some());
            if quick.len() >= QUICK_DISCOVER_MIN {
                quick.truncate(30);
                let _ = events.send(Event::Discover(quick));
            }
            decorate(&mut games);
            games.retain(|game| game.cover.is_some());
            games.truncate(30);
            feature(&mut games);
            let _ = events.send(Event::Discover(games));
        }
        Command::LoadDoctor => {
            let report = DoctorReport::from_value(&crate::doctor::run());
            let _ = events.send(Event::Doctor(report));
        }
        Command::Add { appid, name } => match crate::plans::fetch_plan(appid, Some(&name)) {
            Ok(_) => {
                toast(&events, format!("{name} added to the library"), false);
                let _ = events.send(Event::RefreshLibrary);
            }
            Err(error) => toast(&events, error, true),
        },
        Command::Sync(appid) => match crate::plans::refresh_plan(appid) {
            Ok(value) => {
                let updated = value
                    .get("updated")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                toast(
                    &events,
                    if updated > 0 {
                        format!(
                            "Synced {updated} manifest{}",
                            if updated == 1 { "" } else { "s" }
                        )
                    } else {
                        "Manifests are already current".into()
                    },
                    false,
                );
                let _ = events.send(Event::RefreshLibrary);
            }
            Err(error) => toast(&events, error, true),
        },
        Command::Remove(appid) => {
            let _ = crate::vfsys::remove_script(appid);
            match crate::state::update(|state| {
                state.apps.remove(&appid.to_string());
                Ok(())
            }) {
                Ok(()) => {
                    toast(&events, "Removed from the library".into(), false);
                    let _ = events.send(Event::RefreshLibrary);
                }
                Err(error) => toast(&events, error, true),
            }
        }
        Command::Launch(appid) => {
            if let Err(error) = crate::desktop::spawn_reaped(
                std::process::Command::new("steam").arg(format!("steam://rungameid/{appid}")),
            ) {
                toast(&events, format!("Steam did not open: {error}"), true);
            }
        }
        Command::Fix(appid) => {
            match crate::appmanifest::patch_with_restart(appid, &crate::appmanifest::Mode::Fix) {
                Ok(_) => toast(
                    &events,
                    "Update flags repaired; Steam is back with the fixed manifest".into(),
                    false,
                ),
                Err(error) => toast(&events, error, true),
            }
        }
        Command::RestartSteam => match crate::steamctl::restart(&[]) {
            Ok(_) => toast(&events, "Steam is restarting".into(), false),
            Err(error) => toast(&events, error, true),
        },
        Command::SaveSettings(draft) => {
            let result = crate::state::update(|state| {
                state.settings.country = draft.country.trim().to_lowercase();
                state.settings.hubcap_key = draft.hubcap_key.trim().to_string();
                state.settings.ryuu_key = draft.ryuu_key.trim().to_string();
                state.settings.default_source = draft.default_source.trim().to_string();
                state.settings.auto_update_plans = draft.auto_update_plans;
                let providers: Vec<String> = draft
                    .manifest_providers
                    .iter()
                    .map(|entry| entry.trim().to_string())
                    .filter(|entry| !entry.is_empty())
                    .collect();
                // Empty is a real choice too: every mirror was switched off.
                state.settings.manifest_providers = providers;
                Ok(())
            });
            // Ordering may have changed: republish the provider script so
            // Vapor Forge's hot-reload picks the new chain up at once.
            if let Ok(state) = crate::state::load() {
                let order = crate::lua::provider_order(&state);
                if order.is_empty() {
                    let _ = crate::vfsys::remove_provider();
                } else {
                    let _ = crate::lua::republish_provider(&order);
                }
            }
            match result {
                Ok(()) => toast(&events, "Settings saved".into(), false),
                Err(error) => toast(&events, error, true),
            }
            // Saving is a tracked UI state, not just a toast. Reload after both
            // outcomes so success becomes Saved and failure restores the draft.
            let _ = events.send(Event::SettingsReload);
        }
        Command::SyncAll => match crate::plans::refresh_manifests() {
            Ok(value) => {
                let updated = value
                    .get("updated")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                toast(
                    &events,
                    format!(
                        "Synced {updated} manifest{}",
                        if updated == 1 { "" } else { "s" }
                    ),
                    false,
                );
                let _ = events.send(Event::RefreshLibrary);
            }
            Err(error) => toast(&events, error, true),
        },
        Command::LoadHero(appid) => {
            let path = covers::ensure_hero(appid);
            let logo = covers::ensure_asset(appid, covers::Asset::Logo);
            let _ = events.send(Event::Hero { appid, path, logo });
        }
        Command::SetTheme(value) => {
            let result = crate::state::update(|state| {
                state.settings.theme = value.clone();
                Ok(())
            });
            match result {
                Ok(()) => {
                    let _ = events.send(Event::Theme(value));
                }
                Err(error) => toast(&events, error, true),
            }
        }
        Command::CopyDiagnostics => {
            let text = serde_json::to_string_pretty(&crate::diagnostics::build())
                .unwrap_or_else(|error| format!("could not render diagnostics: {error}"));
            let _ = events.send(Event::Clipboard(text));
        }
    }
}

fn library() -> Vec<Game> {
    let state = match crate::state::load() {
        Ok(state) => state,
        Err(_) => return Vec::new(),
    };
    let mut games: Vec<Game> = state
        .apps
        .values()
        .filter_map(|record| Game::from_record(&state, record))
        .collect();
    games.sort_by_key(|game| game.name.to_lowercase());
    games
}

fn search(term: &str) -> Result<Vec<Game>, String> {
    let term = term.trim();
    if term.is_empty() {
        return Ok(Vec::new());
    }
    let country = crate::state::load()
        .map(|state| state.settings.country)
        .unwrap_or_else(|_| "us".into());
    if let Ok(appid) = term.parse::<u64>() {
        if let Ok(info) = crate::steam::app_info(appid) {
            let name = info
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(term);
            if let Some(game) = Game::from_search(&serde_json::json!({
                "appid": appid,
                "name": name,
            })) {
                return Ok(vec![game]);
            }
        }
    }
    Ok(crate::steam::search(term, &country)?
        .iter()
        .filter_map(Game::from_search)
        .collect())
}

/// Enough cached titles to fill the first screen of Discover.
const QUICK_DISCOVER_MIN: usize = 10;

/// Art that is already on disk, without touching the network.
fn decorate_cached(games: &mut [Game]) {
    for (index, game) in games.iter_mut().enumerate() {
        game.cover = covers::cached_cover(game.appid);
        game.capsule = covers::cached_asset(game.appid, covers::Asset::Capsule);
        if index < 8 {
            game.hero = covers::cached_asset(game.appid, covers::Asset::Hero);
            game.logo = covers::cached_asset(game.appid, covers::Asset::Logo);
        }
    }
}

fn decorate(games: &mut [Game]) {
    std::thread::scope(|scope| {
        for chunk in games.chunks_mut(3) {
            scope.spawn(move || {
                for game in chunk {
                    game.cover = game
                        .cover_url
                        .as_deref()
                        .and_then(|url| covers::ensure_cover_url(game.appid, url))
                        .or_else(|| covers::ensure_cover(game.appid));
                    game.capsule = covers::ensure_asset(game.appid, covers::Asset::Capsule);
                }
            });
        }
    });
}

/// The first popular titles also get hero art and a logo for Discover's
/// featured banner; Discover picks the first five that have a hero.
fn feature(games: &mut [Game]) {
    let count = games.len().min(8);
    std::thread::scope(|scope| {
        for game in &mut games[..count] {
            scope.spawn(move || {
                game.hero = covers::ensure_hero(game.appid);
                game.logo = covers::ensure_asset(game.appid, covers::Asset::Logo);
            });
        }
    });
}

fn toast(events: &Sender<Event>, text: String, error: bool) {
    let _ = events.send(Event::Toast { text, error });
}
