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
}

#[derive(Clone, Debug)]
pub enum Command {
    LoadLibrary,
    Search(String),
    LoadDiscover,
    LoadDoctor,
    Add { appid: u64, name: String },
    Sync(u64),
    Remove(u64),
    Launch(u64),
    SaveSettings(SettingsDraft),
    CopyDiagnostics,
    SyncAll,
    LoadHero(u64),
    SetTheme(String),
}

#[derive(Debug)]
pub enum Event {
    Library(Vec<Game>),
    Search(Vec<Game>),
    Discover(Vec<Game>),
    Doctor(DoctorReport),
    Toast {
        text: String,
        error: bool,
    },
    Clipboard(String),
    RefreshLibrary,
    Hero {
        appid: u64,
        path: Option<std::path::PathBuf>,
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
        std::thread::spawn(move || {
            while let Ok(command) = command_rx.recv() {
                let events = event_tx.clone();
                std::thread::spawn(move || run(command, events));
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

fn run(command: Command, events: Sender<Event>) {
    match command {
        Command::LoadLibrary => {
            let mut games = library();
            decorate(&mut games);
            let _ = events.send(Event::Library(games));
        }
        Command::Search(term) => {
            let mut games = search(&term);
            decorate(&mut games);
            let _ = events.send(Event::Search(games));
        }
        Command::LoadDiscover => {
            let country = crate::state::load()
                .map(|state| state.settings.country)
                .unwrap_or_else(|_| "us".into());
            let mut games: Vec<Game> = crate::steam::discover(&country, 60)
                .iter()
                .filter_map(Game::from_discover)
                .collect();
            decorate(&mut games);
            games.retain(|game| game.cover.is_some());
            games.truncate(30);
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
            if let Err(error) = std::process::Command::new("steam")
                .arg(format!("steam://rungameid/{appid}"))
                .spawn()
            {
                toast(&events, format!("Steam did not open: {error}"), true);
            }
        }
        Command::SaveSettings(draft) => {
            let result = crate::state::update(|state| {
                state.settings.country = draft.country.trim().to_lowercase();
                state.settings.hubcap_key = draft.hubcap_key.trim().to_string();
                state.settings.ryuu_key = draft.ryuu_key.trim().to_string();
                state.settings.default_source = draft.default_source.trim().to_string();
                let providers: Vec<String> = draft
                    .manifest_providers
                    .iter()
                    .map(|entry| entry.trim().to_string())
                    .filter(|entry| !entry.is_empty())
                    .collect();
                if !providers.is_empty() {
                    // A chain entry is either a builtin name or a JSON object
                    // describing a custom mirror; the settings file keeps the
                    // raw strings and resolve() interprets them on read.
                    state.settings.manifest_providers = providers;
                }
                Ok(())
            });
            // Ordering may have changed: republish the provider script so
            // Vapor Forge's hot-reload picks the new chain up at once.
            if let Ok(state) = crate::state::load() {
                let order = crate::lua::provider_order(&state);
                if !order.is_empty() {
                    let _ = crate::lua::republish_provider(&order);
                }
            }
            match result {
                Ok(()) => toast(&events, "Settings saved".into(), false),
                Err(error) => toast(&events, error, true),
            }
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
            let _ = events.send(Event::Hero { appid, path });
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

fn search(term: &str) -> Vec<Game> {
    let term = term.trim();
    if term.is_empty() {
        return Vec::new();
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
                return vec![game];
            }
        }
    }
    crate::steam::search(term, &country)
        .unwrap_or_default()
        .iter()
        .filter_map(Game::from_search)
        .collect()
}

fn decorate(games: &mut [Game]) {
    std::thread::scope(|scope| {
        for chunk in games.chunks_mut(6) {
            scope.spawn(move || {
                for game in chunk {
                    game.cover = game
                        .cover_url
                        .as_deref()
                        .and_then(|url| covers::ensure_cover_url(game.appid, url))
                        .or_else(|| covers::ensure_cover(game.appid));
                }
            });
        }
    });
}

fn toast(events: &Sender<Event>, text: String, error: bool) {
    let _ = events.send(Event::Toast { text, error });
}
