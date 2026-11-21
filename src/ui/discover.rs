//! Discover is a deliberate browsing surface, not the library with different data.

use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib, Box as GtkBox, Orientation};

use crate::ui::art::Art;
use crate::ui::model::{parse_updated, Game};
use crate::ui::shell::Ctx;
use crate::ui::worker::Command;

const CARD_WIDTH: i32 = 232;
const COVER_WIDTH: i32 = 212;
const COVER_HEIGHT: i32 = 100;
/// Cells stretch past the minimum card width, so art decodes a little wider
/// than the cover's minimum to stay sharp in a three-column window.
const COVER_DECODE_WIDTH: i32 = 256;

/// One Discover cell: the game plus its library membership at render time.
struct Card {
    game: Game,
    in_library: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DiscoverFilter {
    #[default]
    Popular,
    InLibrary,
    RecentlyUpdated,
}

#[derive(Default)]
struct DiscoverState {
    games: RefCell<Vec<Game>>,
    library: RefCell<BTreeSet<u64>>,
    library_games: RefCell<Vec<Game>>,
    filter: Cell<DiscoverFilter>,
    search_mode: Cell<bool>,
}

impl DiscoverState {
    fn reconcile_library(&self, library: &BTreeMap<u64, Option<u64>>) {
        self.library.replace(library.keys().copied().collect());
    }

    fn set_library(&self, games: &[Game]) {
        self.library
            .replace(games.iter().map(|game| game.appid).collect());
        self.library_games.replace(games.to_vec());
    }
}

pub struct DiscoverPage {
    pub root: gtk::Widget,
    stack: gtk::Stack,
    cards: gio::ListStore,
    error: adw::StatusPage,
    loading: adw::StatusPage,
    empty: adw::StatusPage,
    subtitle: gtk::Label,
    filter_bar: GtkBox,
    state: Rc<DiscoverState>,
}

pub fn build(ctx: &Ctx) -> DiscoverPage {
    install_css();

    let title = gtk::Label::new(Some("Discover"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);

    let subtitle = gtk::Label::new(Some("Popular on Steam right now"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);

    let headings = GtkBox::new(Orientation::Vertical, 2);
    headings.set_hexpand(true);
    headings.append(&title);
    headings.append(&subtitle);

    // Refresh belongs to the single global header. Discover only keeps retry in
    // its actual error state, so the global controls never move between pages.
    let header = GtkBox::new(Orientation::Horizontal, 12);
    header.set_margin_top(22);
    header.set_margin_bottom(8);
    header.set_margin_start(24);
    header.set_margin_end(24);
    header.append(&headings);

    let filter_bar = GtkBox::new(Orientation::Horizontal, 0);
    filter_bar.add_css_class("segmented");
    filter_bar.set_halign(gtk::Align::Start);
    filter_bar.set_margin_top(6);
    filter_bar.set_margin_start(24);
    filter_bar.set_margin_end(24);
    filter_bar.set_sensitive(false);

    let popular = filter_button("Popular");
    let in_library = filter_button("In Library");
    let recently_updated = filter_button("Recently Updated");
    in_library.set_group(Some(&popular));
    recently_updated.set_group(Some(&popular));
    popular.set_active(true);
    filter_bar.append(&popular);
    filter_bar.append(&in_library);
    filter_bar.append(&recently_updated);

    let state = Rc::new(DiscoverState::default());
    let cards = gio::ListStore::new::<glib::BoxedAnyObject>();
    let grid = gtk::GridView::new(
        Some(gtk::NoSelection::new(Some(cards.clone()))),
        Some(card_factory(ctx, &state)),
    );
    grid.add_css_class("discover-grid");
    grid.set_min_columns(1);
    grid.set_max_columns(4);
    grid.set_valign(gtk::Align::Start);

    let flow_scroll = gtk::ScrolledWindow::new();
    flow_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    flow_scroll.set_child(Some(&grid));
    flow_scroll.set_vexpand(true);

    let loading_spinner = gtk::Spinner::new();
    loading_spinner.set_size_request(28, 28);
    loading_spinner.start();
    let loading = adw::StatusPage::builder()
        .icon_name("system-search-symbolic")
        .title("Looking for something good")
        .description("Reading Steam's current popular titles.")
        .vexpand(true)
        .build();
    loading.set_child(Some(&loading_spinner));

    let error = adw::StatusPage::builder()
        .icon_name("network-error-symbolic")
        .title("Discover is unavailable")
        .description("Steam did not answer. Check the connection and try again.")
        .vexpand(true)
        .build();
    let retry = gtk::Button::with_label("Try again");
    retry.add_css_class("suggested-action");
    retry.set_cursor_from_name(Some("pointer"));
    retry.set_halign(gtk::Align::Center);
    let commands = ctx.commands.clone();
    retry.connect_clicked(move |_| commands.send(Command::LoadDiscover));
    error.set_child(Some(&retry));

    let empty = adw::StatusPage::builder()
        .icon_name("system-search-symbolic")
        .title("Nothing in this view")
        .description("No loaded games match this view.")
        .vexpand(true)
        .build();

    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&flow_scroll, Some("flow"));
    stack.add_named(&empty, Some("empty"));
    stack.add_named(&error, Some("error"));
    stack.set_visible_child_name("loading");

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&header);
    root.append(&filter_bar);
    root.append(&stack);

    for (button, filter) in [
        (popular, DiscoverFilter::Popular),
        (in_library, DiscoverFilter::InLibrary),
        (recently_updated, DiscoverFilter::RecentlyUpdated),
    ] {
        let state = state.clone();
        let cards = cards.clone();
        let stack = stack.clone();
        let empty = empty.clone();
        let subtitle = subtitle.clone();
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            state.filter.set(filter);
            render(&state, &cards, &stack, &empty, &subtitle);
        });
    }

    DiscoverPage {
        root: root.upcast(),
        stack,
        cards,
        error,
        loading,
        empty,
        subtitle,
        filter_bar,
        state,
    }
}

impl DiscoverPage {
    pub fn show_loading(&self) {
        self.filter_bar.set_sensitive(false);
        self.loading
            .set_description(Some("Reading Steam's current popular titles."));
        self.stack.set_visible_child_name("loading");
    }

    pub fn show_error(&self, message: &str) {
        self.update_filter_sensitivity();
        self.error.set_description(Some(message));
        self.stack.set_visible_child_name("error");
    }

    pub fn set_games(&self, games: &[Game]) {
        let mut games = games.to_vec();
        if let Some(library) = loaded_library_metadata() {
            self.state.reconcile_library(&library);
            merge_library_metadata(&mut games, &library);
        }
        self.state.search_mode.set(false);
        self.state.games.replace(games);
        self.update_filter_sensitivity();
        self.render();
    }

    /// Show global-search results on the Discover page (Enter in the search box
    /// while Discover is active), instead of navigating away to Library.
    pub fn set_search(&self, games: &[Game]) {
        let mut games = games.to_vec();
        if let Some(library) = loaded_library_metadata() {
            merge_library_metadata(&mut games, &library);
        }
        self.state.search_mode.set(true);
        self.state.filter.set(DiscoverFilter::Popular);
        self.state.games.replace(games);
        self.update_filter_sensitivity();
        self.render();
    }

    pub fn set_library(&self, games: &[Game]) {
        self.state.set_library(games);
        self.update_filter_sensitivity();
        if self.state.filter.get() == DiscoverFilter::InLibrary {
            self.render();
        }
    }

    fn update_filter_sensitivity(&self) {
        let has_games =
            !self.state.games.borrow().is_empty() || !self.state.library_games.borrow().is_empty();
        self.filter_bar.set_sensitive(has_games);
    }

    /// Re-read the worker-persisted library after RefreshLibrary and update the
    /// existing Discover cards without another Discover network request.
    pub fn reconcile_library(&self) {
        let Some(library) = loaded_library_metadata() else {
            return;
        };
        self.state.reconcile_library(&library);
        merge_library_metadata(&mut self.state.games.borrow_mut(), &library);

        // A global Discover load may still be in flight; its eventual set_games
        // call owns that loading/error transition and reconciles the same state.
        let visible = self.stack.visible_child_name();
        if matches!(visible.as_deref(), Some("flow") | Some("empty")) {
            self.render();
        }
    }

    fn render(&self) {
        render(
            &self.state,
            &self.cards,
            &self.stack,
            &self.empty,
            &self.subtitle,
        );
    }
}

fn filter_button(label: &str) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::with_label(label);
    button.set_cursor_from_name(Some("pointer"));
    button
}

fn render(
    state: &Rc<DiscoverState>,
    cards: &gio::ListStore,
    stack: &gtk::Stack,
    empty: &adw::StatusPage,
    subtitle: &gtk::Label,
) {
    let filter = state.filter.get();
    let total =
        games_for_filter(&state.games.borrow(), &state.library_games.borrow(), filter).len();
    let visible = {
        let popular = state.games.borrow();
        let library = state.library_games.borrow();
        let added = state.library.borrow();
        visible_games(games_for_filter(&popular, &library, filter), filter, &added)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
    };

    let searching = state.search_mode.get();
    if searching {
        subtitle.set_text(&if visible.is_empty() {
            "Nothing found — try another name or app id".to_string()
        } else {
            format!(
                "{} search result{}",
                visible.len(),
                if visible.len() == 1 { "" } else { "s" }
            )
        });
    } else {
        match filter {
            DiscoverFilter::Popular => subtitle.set_text("Popular on Steam right now"),
            DiscoverFilter::InLibrary => subtitle.set_text(&format!(
                "{} {} in your library",
                visible.len(),
                if visible.len() == 1 { "game" } else { "games" }
            )),
            DiscoverFilter::RecentlyUpdated => {
                subtitle.set_text("Loaded titles, newest manifest updates first")
            }
        }
    }

    if total == 0 && filter != DiscoverFilter::InLibrary && !searching {
        cards.remove_all();
        stack.set_visible_child_name("error");
        return;
    }
    if visible.is_empty() {
        empty.set_description(Some(if searching {
            "Nothing found on Steam for that search."
        } else {
            match filter {
                DiscoverFilter::InLibrary => {
                    "Your library is empty. Add a game from Popular or search Steam above."
                }
                _ => "No loaded games match this view.",
            }
        }));
        cards.remove_all();
        stack.set_visible_child_name("empty");
        return;
    }

    let library = state.library.borrow();
    let items = visible
        .into_iter()
        .map(|game| {
            let in_library = library.contains(&game.appid);
            glib::BoxedAnyObject::new(Card { game, in_library })
        })
        .collect::<Vec<_>>();
    cards.splice(0, cards.n_items(), &items);
    stack.set_visible_child_name("flow");
}

/// In Library is a view of the actual library, not only the popular titles
/// that Steam happened to return for Discover.
fn games_for_filter<'a>(
    popular: &'a [Game],
    library: &'a [Game],
    filter: DiscoverFilter,
) -> &'a [Game] {
    if filter == DiscoverFilter::InLibrary {
        library
    } else {
        popular
    }
}

/// Select only from games already delivered by the worker. Popular preserves
/// Steam's order; Recently Updated is a stable newest-first sort.
fn visible_games<'a>(
    games: &'a [Game],
    filter: DiscoverFilter,
    added: &BTreeSet<u64>,
) -> Vec<&'a Game> {
    let mut visible = games
        .iter()
        .filter(|game| filter != DiscoverFilter::InLibrary || added.contains(&game.appid))
        .collect::<Vec<_>>();
    if filter == DiscoverFilter::RecentlyUpdated {
        visible.sort_by_key(|item| Reverse(item.updated));
    }
    visible
}

fn loaded_library_metadata() -> Option<BTreeMap<u64, Option<u64>>> {
    crate::state::load()
        .ok()
        .map(|state| library_metadata(&state))
}

fn library_metadata(state: &crate::state::State) -> BTreeMap<u64, Option<u64>> {
    state
        .apps
        .iter()
        .filter_map(|(key, record)| {
            let appid = record
                .get("appid")
                .and_then(serde_json::Value::as_u64)
                .or_else(|| key.parse().ok())?;
            Some((appid, parse_updated(record.get("updated"))))
        })
        .collect()
}

fn merge_library_metadata(games: &mut [Game], library: &BTreeMap<u64, Option<u64>>) {
    for game in games {
        if let Some(Some(updated)) = library.get(&game.appid) {
            game.updated = Some(*updated);
        }
    }
}

/// Cells are recycled while scrolling, so per-game state lives in the cell's
/// closures and is rewritten whenever the list item is bound to a new game.
fn card_factory(ctx: &Ctx, state: &Rc<DiscoverState>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    let ctx = ctx.clone();
    let state = state.clone();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        item.set_activatable(false);

        let art = Art::new(COVER_WIDTH, COVER_HEIGHT, "discover-cover");

        let title = gtk::Label::new(None);
        title.add_css_class("discover-title");
        title.set_halign(gtk::Align::Start);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(24);

        let action = gtk::Button::new();
        action.add_css_class("circular");
        action.add_css_class("discover-add");
        action.set_halign(gtk::Align::End);
        action.set_valign(gtk::Align::End);
        action.set_margin_end(10);
        action.set_margin_bottom(10);

        let current: Rc<RefCell<Option<(u64, String)>>> = Rc::default();
        {
            let ctx = ctx.clone();
            let state = state.clone();
            let current = current.clone();
            action.connect_clicked(move |_| {
                if let Some((appid, name)) = current.borrow().clone() {
                    confirm_add(&ctx, state.clone(), appid, &name);
                }
            });
        }

        let frame = gtk::Overlay::new();
        frame.add_css_class("discover-frame");
        frame.set_child(Some(&art.root));
        frame.add_overlay(&action);

        let card = GtkBox::new(Orientation::Vertical, 0);
        card.add_css_class("discover-card");
        card.set_size_request(CARD_WIDTH, -1);
        card.set_valign(gtk::Align::Start);
        card.append(&frame);
        card.append(&title);
        item.set_child(Some(&card));

        item.connect_item_notify(move |item| {
            let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
                current.replace(None);
                art.set(None, "", None);
                return;
            };
            let card = object.borrow::<Card>();
            let game = &card.game;
            art.set(game.cover.as_deref(), &game.initial(), Some(COVER_DECODE_WIDTH));
            title.set_text(&game.name);
            title.set_tooltip_text(Some(&game.name));
            if card.in_library {
                current.replace(None);
                show_in_library(&action);
            } else {
                current.replace(Some((game.appid, game.name.clone())));
                show_add(&action, &game.name);
            }
        });
    });
    factory
}

fn show_add(action: &gtk::Button, name: &str) {
    action.remove_css_class("discover-added");
    action.set_icon_name("list-add-symbolic");
    action.set_sensitive(true);
    action.set_cursor_from_name(Some("pointer"));
    action.set_tooltip_text(Some(&format!("Add {name} to your library")));
}

fn show_in_library(action: &gtk::Button) {
    action.add_css_class("discover-added");
    action.set_icon_name("object-select-symbolic");
    action.set_sensitive(false);
    action.set_cursor_from_name(None);
    action.set_tooltip_text(Some("Already in your library"));
}

fn add_response(_state: &DiscoverState, response: &str, appid: u64, name: &str) -> Option<Command> {
    if response != "add" {
        return None;
    }
    // Sending Add is only a request. RefreshLibrary reconciliation is the sole
    // membership transition, so a failed worker operation cannot enable this.
    Some(Command::Add {
        appid,
        name: name.to_string(),
    })
}

fn confirm_add(ctx: &Ctx, state: Rc<DiscoverState>, appid: u64, name: &str) {
    let dialog = adw::MessageDialog::new(
        Some(&ctx.window),
        Some("Add to library?"),
        Some(&format!(
            "ExxaTools will build a plan for {name} and pin its manifests so Steam can install it."
        )),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("add", "Add to library");
    dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
    let commands = ctx.commands.clone();
    let name = name.to_string();
    dialog.connect_response(None, move |dialog, response| {
        dialog.close();
        if let Some(command) = add_response(&state, response, appid, &name) {
            commands.send(command);
        }
    });
    dialog.present();
}

fn install_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_data(STYLE);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
    );
}

const STYLE: &str = r#"
gridview.discover-grid {
    background-color: transparent;
    padding: 12px 16px 28px;
}
gridview.discover-grid > child {
    padding: 10px;
    border-radius: 16px;
    background-color: transparent;
}
.discover-card { background-color: transparent; padding: 0; }
.discover-cover {
    border-radius: 12px;
    box-shadow: 0 10px 24px -14px rgba(0, 0, 0, 0.7), inset 0 0 0 1px @borders;
    transition: box-shadow 200ms ease-out;
}
.discover-cover > picture { transition: transform 260ms ease-out, opacity 220ms ease-out; }
.discover-card:hover .discover-cover {
    box-shadow: 0 16px 32px -14px rgba(0, 0, 0, 0.85), inset 0 0 0 1px alpha(@window_fg_color, 0.18);
}
.discover-card:hover .discover-cover > picture { transform: scale(1.05); }
.discover-cover .art-letter { font-size: 28px; }
.discover-title { margin: 10px 4px 0; font-weight: 600; }

button.discover-add {
    min-width: 34px;
    min-height: 34px;
    padding: 0;
    background-color: rgba(10, 10, 14, 0.62);
    color: #FFFFFF;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
    transition: background-color 160ms ease-out, box-shadow 160ms ease-out;
}
.discover-card:hover button.discover-add:not(.discover-added),
button.discover-add:focus-visible {
    background-color: @accent_bg_color;
    color: @accent_fg_color;
    box-shadow: 0 6px 16px -4px alpha(@accent_bg_color, 0.8);
}
button.discover-add.discover-added,
button.discover-add.discover-added:disabled {
    background-color: @success_color;
    color: #FFFFFF;
    opacity: 1;
    box-shadow: none;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn game(appid: u64, name: &str, updated: Option<u64>) -> Game {
        let mut game = Game::from_search(&json!({"appid": appid, "name": name})).expect("game");
        game.updated = updated;
        game
    }

    fn appids(games: Vec<&Game>) -> Vec<u64> {
        games.into_iter().map(|game| game.appid).collect()
    }

    #[test]
    fn in_library_reads_the_whole_library_instead_of_the_popular_overlap() {
        let popular = vec![game(30, "Popular only", None)];
        let library = vec![
            game(108600, "Project Zomboid", None),
            game(2605790, "Deep Rock Galactic: Rogue Core", None),
        ];

        assert_eq!(
            appids(
                games_for_filter(&popular, &library, DiscoverFilter::InLibrary)
                    .iter()
                    .collect()
            ),
            vec![108600, 2605790]
        );
        assert_eq!(
            appids(
                games_for_filter(&popular, &library, DiscoverFilter::Popular)
                    .iter()
                    .collect()
            ),
            vec![30]
        );
    }

    #[test]
    fn popular_preserves_the_loaded_steam_order() {
        let games = vec![game(30, "Third", None), game(10, "First", None)];
        let added = BTreeSet::from([10]);
        assert_eq!(
            appids(visible_games(&games, DiscoverFilter::Popular, &added)),
            vec![30, 10]
        );
    }

    #[test]
    fn in_library_filters_loaded_games_without_changing_order() {
        let games = vec![
            game(30, "Third", None),
            game(10, "First", None),
            game(20, "Second", None),
        ];
        let added = BTreeSet::from([20, 30, 999]);
        assert_eq!(
            appids(visible_games(&games, DiscoverFilter::InLibrary, &added)),
            vec![30, 20]
        );
    }

    #[test]
    fn recently_updated_is_newest_first_and_stable_for_unknown_dates() {
        let games = vec![
            game(1, "Old", Some(100)),
            game(2, "Unknown A", None),
            game(3, "New", Some(300)),
            game(4, "Unknown B", None),
            game(5, "Middle", Some(200)),
        ];
        let added = BTreeSet::new();
        assert_eq!(
            appids(visible_games(
                &games,
                DiscoverFilter::RecentlyUpdated,
                &added
            )),
            vec![3, 5, 1, 2, 4]
        );
    }

    #[test]
    fn sent_add_waits_for_authoritative_library_membership() {
        let state = DiscoverState::default();
        let games = vec![game(620, "Portal 2", None)];

        let command = add_response(&state, "add", 620, "Portal 2").expect("Add command");
        assert!(matches!(
            command,
            Command::Add { appid: 620, name } if name == "Portal 2"
        ));
        assert!(
            visible_games(&games, DiscoverFilter::InLibrary, &state.library.borrow()).is_empty(),
            "sending Add must not mark the title In Library before the worker succeeds"
        );

        let mut authoritative = crate::state::State::default();
        authoritative
            .apps
            .insert("620".into(), json!({"appid": 620, "name": "Portal 2"}));
        state.reconcile_library(&library_metadata(&authoritative));
        assert_eq!(
            appids(visible_games(
                &games,
                DiscoverFilter::InLibrary,
                &state.library.borrow()
            )),
            vec![620]
        );
    }

    #[test]
    fn authoritative_reconciliation_replaces_stale_membership() {
        let state = DiscoverState::default();
        let games = vec![game(10, "First", None), game(20, "Second", None)];
        let mut initial = crate::state::State::default();
        initial.apps.insert("10".into(), json!({"appid": 10}));
        initial.apps.insert("20".into(), json!({"appid": 20}));
        state.reconcile_library(&library_metadata(&initial));

        let mut current = crate::state::State::default();
        current.apps.insert("20".into(), json!({"appid": 20}));
        state.reconcile_library(&library_metadata(&current));

        assert_eq!(
            appids(visible_games(
                &games,
                DiscoverFilter::InLibrary,
                &state.library.borrow()
            )),
            vec![20]
        );
    }

    #[test]
    fn local_library_records_supply_membership_and_update_metadata() {
        let mut state = crate::state::State::default();
        state.apps.insert(
            "620".into(),
            json!({"appid": 620, "name": "Portal 2", "updated": "1700000000"}),
        );
        state
            .apps
            .insert("730".into(), json!({"name": "Counter-Strike 2"}));

        let metadata = library_metadata(&state);
        assert_eq!(metadata.get(&620), Some(&Some(1_700_000_000)));
        assert_eq!(metadata.get(&730), Some(&None));

        let mut games = vec![game(620, "Portal 2", None), game(10, "Other", Some(50))];
        merge_library_metadata(&mut games, &metadata);
        assert_eq!(games[0].updated, Some(1_700_000_000));
        assert_eq!(games[1].updated, Some(50));
    }
}
