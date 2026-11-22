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

const FEATURED_COUNT: usize = 5;
const FEATURED_HEIGHT: i32 = 320;
const FEATURED_DECODE_WIDTH: i32 = 1280;
/// Long enough to read a title and reach the Add button.
const FEATURED_INTERVAL: std::time::Duration = std::time::Duration::from_secs(7);

/// Steam's library_600x900 portrait at a 2:3 logical size.
const POSTER_WIDTH: i32 = 180;
const POSTER_HEIGHT: i32 = 270;

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
    /// Adds sent to the worker and not answered yet.
    pending: RefCell<BTreeSet<u64>>,
    /// Titles that just joined the library; their card flashes foil once.
    flash: RefCell<BTreeSet<u64>>,
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
    featured: Rc<Featured>,
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
    filter_bar.set_sensitive(false);

    // On a narrow window the segmented control scrolls instead of setting the
    // window's minimum width.
    let filter_scroll = gtk::ScrolledWindow::new();
    filter_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    filter_scroll.set_propagate_natural_width(true);
    filter_scroll.set_propagate_natural_height(true);
    filter_scroll.set_halign(gtk::Align::Start);
    filter_scroll.set_margin_top(6);
    filter_scroll.set_margin_start(24);
    filter_scroll.set_margin_end(24);
    filter_scroll.set_child(Some(&filter_bar));

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
    grid.set_max_columns(12);
    grid.set_valign(gtk::Align::Start);

    let featured = build_featured(ctx, &state);

    // The banner scrolls away with the grid. Thirty posters are cheap enough
    // that giving up the grid's own viewport recycling is not noticeable.
    let flow = GtkBox::new(Orientation::Vertical, 0);
    flow.append(&featured.root);
    flow.append(&grid);

    let flow_scroll = gtk::ScrolledWindow::new();
    flow_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    flow_scroll.set_child(Some(&flow));
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
    root.append(&filter_scroll);
    root.append(&stack);

    for (button, filter) in [
        (popular, DiscoverFilter::Popular),
        (in_library, DiscoverFilter::InLibrary),
        (recently_updated, DiscoverFilter::RecentlyUpdated),
    ] {
        let state = state.clone();
        let cards = cards.clone();
        let featured = featured.clone();
        let stack = stack.clone();
        let empty = empty.clone();
        let subtitle = subtitle.clone();
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            state.filter.set(filter);
            render(&state, &cards, &featured, &stack, &empty, &subtitle);
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
        featured,
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

    /// The worker answered an Add. Success joins the library now (the
    /// RefreshLibrary that follows confirms it) and flashes the card once.
    pub fn finish_add(&self, appid: u64, added: bool) {
        self.state.pending.borrow_mut().remove(&appid);
        if added {
            self.state.library.borrow_mut().insert(appid);
            self.state.flash.borrow_mut().insert(appid);
        }
        let visible = self.stack.visible_child_name();
        if matches!(visible.as_deref(), Some("flow") | Some("empty")) {
            self.render();
        }
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
            &self.featured,
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
    featured: &Featured,
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
    let featured_ids = if searching || filter != DiscoverFilter::Popular {
        featured.set_games(&[], &library);
        BTreeSet::new()
    } else {
        let picks = visible
            .iter()
            .filter(|game| game.hero.is_some())
            .take(FEATURED_COUNT)
            .cloned()
            .collect::<Vec<_>>();
        featured.set_games(&picks, &library);
        picks.iter().map(|game| game.appid).collect()
    };
    // The banner already shows its titles; the grid continues after them.
    let items = visible
        .into_iter()
        .filter(|game| !featured_ids.contains(&game.appid))
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

        let art = Art::new(POSTER_WIDTH, POSTER_HEIGHT, "discover-cover");
        art.root.set_hexpand(false);

        // Behind a header-only title: its own blurred art, so the contained
        // wide image sits in a matching poster instead of an empty frame.
        let fill = gtk::Picture::new();
        fill.set_content_fit(gtk::ContentFit::Cover);
        fill.set_can_shrink(true);
        fill.add_css_class("discover-fill");
        let fill_generation = Rc::new(Cell::new(0u64));

        let title = gtk::Label::new(None);
        title.add_css_class("discover-title");
        title.set_halign(gtk::Align::Start);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(18);
        title.set_width_request(POSTER_WIDTH - 8);

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
            action.connect_clicked(move |action| {
                if let Some((appid, name)) = current.borrow().clone() {
                    start_add(&ctx, &state, appid, &name);
                    show_adding(action, &name);
                }
            });
        }

        let frame = gtk::Overlay::new();
        frame.add_css_class("discover-frame");
        frame.set_overflow(gtk::Overflow::Hidden);
        frame.set_child(Some(&fill));
        frame.add_overlay(&art.root);
        frame.add_overlay(&action);
        frame.set_size_request(POSTER_WIDTH, POSTER_HEIGHT);

        let card = GtkBox::new(Orientation::Vertical, 0);
        card.add_css_class("discover-card");
        card.set_halign(gtk::Align::Center);
        card.set_valign(gtk::Align::Start);
        card.append(&frame);
        card.append(&title);
        item.set_child(Some(&card));

        let state = state.clone();
        item.connect_item_notify(move |item| {
            let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
                current.replace(None);
                art.set(None, "", None);
                return;
            };
            let card = object.borrow::<Card>();
            let game = &card.game;
            let generation = fill_generation.get().wrapping_add(1);
            fill_generation.set(generation);
            fill.set_paintable(None::<&gtk::gdk::Paintable>);
            match (&game.capsule, &game.cover) {
                (Some(poster), _) => {
                    art.set_content_fit(gtk::ContentFit::Cover);
                    art.set(Some(poster), &game.initial(), None);
                }
                (None, Some(header)) => {
                    art.set_content_fit(gtk::ContentFit::Contain);
                    art.set(Some(header), &game.initial(), Some(POSTER_WIDTH));
                    let fill = fill.clone();
                    let fill_generation = fill_generation.clone();
                    crate::ui::art::backdrop(header, move |loaded| {
                        if fill_generation.get() == generation {
                            if let Some(loaded) = loaded {
                                fill.set_paintable(Some(&loaded.texture));
                            }
                        }
                    });
                }
                (None, None) => art.set(None, &game.initial(), None),
            }
            title.set_text(&game.name);
            title.set_tooltip_text(Some(&game.name));
            if card.in_library {
                current.replace(None);
                show_in_library(&action);
                if state.flash.borrow_mut().remove(&game.appid) {
                    frame.add_css_class("foil-flash");
                    let frame = frame.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(1400), move || {
                        frame.remove_css_class("foil-flash");
                    });
                }
            } else if state.pending.borrow().contains(&game.appid) {
                current.replace(None);
                show_adding(&action, &game.name);
            } else {
                current.replace(Some((game.appid, game.name.clone())));
                show_add(&action, &game.name);
            }
        });
    });
    factory
}

/// Discover's banner: a few popular titles with their hero art and logo,
/// advancing on its own unless the pointer or focus is on it.
struct Featured {
    root: GtkBox,
    carousel: adw::Carousel,
    shown: RefCell<Vec<(u64, bool, bool, bool)>>,
    ctx: Ctx,
    state: Rc<DiscoverState>,
}

fn build_featured(ctx: &Ctx, state: &Rc<DiscoverState>) -> Rc<Featured> {
    let carousel = adw::Carousel::new();
    carousel.add_css_class("featured");
    carousel.set_hexpand(true);
    carousel.set_spacing(16);
    carousel.set_allow_scroll_wheel(false);

    let dots = adw::CarouselIndicatorLines::new();
    dots.set_carousel(Some(&carousel));
    dots.set_halign(gtk::Align::Center);

    let root = GtkBox::new(Orientation::Vertical, 6);
    root.set_margin_top(14);
    root.set_margin_start(24);
    root.set_margin_end(24);
    root.append(&carousel);
    root.append(&dots);
    root.set_visible(false);

    let paused = Rc::new(Cell::new(false));
    let motion = gtk::EventControllerMotion::new();
    {
        let paused = paused.clone();
        motion.connect_enter(move |_, _, _| paused.set(true));
    }
    {
        let paused = paused.clone();
        motion.connect_leave(move |_| paused.set(false));
    }
    root.add_controller(motion);
    let focus = gtk::EventControllerFocus::new();
    {
        let paused = paused.clone();
        focus.connect_enter(move |_| paused.set(true));
    }
    {
        let paused = paused.clone();
        focus.connect_leave(move |_| paused.set(false));
    }
    root.add_controller(focus);

    let weak = carousel.downgrade();
    glib::timeout_add_local(FEATURED_INTERVAL, move || {
        let Some(carousel) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let animations = gtk::Settings::default()
            .map(|settings| settings.is_gtk_enable_animations())
            .unwrap_or(true);
        let pages = carousel.n_pages();
        if animations && !paused.get() && carousel.is_mapped() && pages > 1 {
            let next = (carousel.position().round() as u32 + 1) % pages;
            carousel.scroll_to(&carousel.nth_page(next), true);
        }
        glib::ControlFlow::Continue
    });

    Rc::new(Featured {
        root,
        carousel,
        shown: RefCell::new(Vec::new()),
        ctx: ctx.clone(),
        state: state.clone(),
    })
}

impl Featured {
    /// Rebuilds only when the titles or their library membership changed,
    /// so a re-render does not jump the banner back to its first page.
    fn set_games(&self, games: &[Game], library: &BTreeSet<u64>) {
        let wanted = games
            .iter()
            .map(|game| {
                (
                    game.appid,
                    library.contains(&game.appid),
                    game.hero.is_some(),
                    game.logo.is_some(),
                )
            })
            .collect::<Vec<_>>();
        if *self.shown.borrow() == wanted {
            return;
        }
        while self.carousel.n_pages() > 0 {
            self.carousel.remove(&self.carousel.nth_page(0));
        }
        for (game, (_, in_library, _, _)) in games.iter().zip(&wanted) {
            self.carousel
                .append(&featured_page(&self.ctx, &self.state, game, *in_library));
        }
        self.root.set_visible(!games.is_empty());
        self.shown.replace(wanted);
    }
}

fn featured_page(ctx: &Ctx, state: &Rc<DiscoverState>, game: &Game, in_library: bool) -> gtk::Widget {
    let art = Art::new(-1, FEATURED_HEIGHT, "featured-art");
    art.set(game.hero.as_deref(), &game.initial(), Some(FEATURED_DECODE_WIDTH));
    art.root.set_hexpand(true);

    let scrim = GtkBox::new(Orientation::Vertical, 0);
    scrim.add_css_class("hero-scrim");
    scrim.set_can_target(false);

    let adding = state.pending.borrow().contains(&game.appid);
    let action = if adding {
        let busy = gtk::Button::with_label("Adding…");
        busy.add_css_class("suggested-action");
        busy.set_sensitive(false);
        busy
    } else if in_library {
        let added = gtk::Button::with_label("In your library");
        added.add_css_class("flat");
        added.set_sensitive(false);
        added
    } else {
        let add = gtk::Button::with_label("Add to library");
        add.add_css_class("suggested-action");
        add.set_cursor_from_name(Some("pointer"));
        let ctx = ctx.clone();
        let state = state.clone();
        let appid = game.appid;
        let name = game.name.clone();
        add.connect_clicked(move |add| {
            start_add(&ctx, &state, appid, &name);
            add.set_label("Adding…");
            add.set_sensitive(false);
        });
        add
    };
    action.set_halign(gtk::Align::Start);
    action.set_margin_top(14);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.add_css_class("hero-panel");
    content.set_valign(gtk::Align::End);
    content.set_halign(gtk::Align::Start);
    content.append(&crate::ui::inspector::hero_title(game));
    content.append(&action);

    let overlay = gtk::Overlay::new();
    overlay.add_css_class("hero-wrap");
    overlay.set_overflow(gtk::Overflow::Hidden);
    overlay.set_hexpand(true);
    overlay.set_child(Some(&art.root));
    overlay.add_overlay(&scrim);
    overlay.add_overlay(&content);

    adw::Clamp::builder()
        .orientation(Orientation::Vertical)
        .maximum_size(FEATURED_HEIGHT)
        .tightening_threshold(FEATURED_HEIGHT)
        .child(&overlay)
        .hexpand(true)
        .build()
        .upcast()
}

fn show_add(action: &gtk::Button, name: &str) {
    action.remove_css_class("discover-added");
    action.remove_css_class("discover-adding");
    action.set_icon_name("list-add-symbolic");
    action.set_sensitive(true);
    action.set_cursor_from_name(Some("pointer"));
    action.set_tooltip_text(Some(&format!("Add {name} to your library")));
}

fn show_adding(action: &gtk::Button, name: &str) {
    action.remove_css_class("discover-added");
    action.add_css_class("discover-adding");
    let spinner = gtk::Spinner::new();
    spinner.start();
    action.set_child(Some(&spinner));
    action.set_sensitive(false);
    action.set_tooltip_text(Some(&format!("Adding {name}…")));
}

fn show_in_library(action: &gtk::Button) {
    action.remove_css_class("discover-adding");
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
    // Sending Add is only a request: membership changes when the worker
    // answers (finish_add on success, then RefreshLibrary), never on send.
    Some(Command::Add {
        appid,
        name: name.to_string(),
    })
}

/// Adding is reversible (Remove is one menu away), so it starts at once: no
/// confirmation, the card shows progress, and the worker's answer settles it.
fn start_add(ctx: &Ctx, state: &Rc<DiscoverState>, appid: u64, name: &str) {
    if !state.pending.borrow_mut().insert(appid) {
        return;
    }
    if let Some(command) = add_response(state, "add", appid, name) {
        ctx.commands.send(command);
    }
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
    padding: 10px 8px 14px;
    border-radius: 8px;
    background-color: transparent;
}
.featured-art { border-radius: 4px; }
.featured-art .art-letter { font-size: 64px; }
.discover-card { background-color: transparent; padding: 0; }
.discover-frame { border-radius: 3px; }
.discover-cover.ready { background-color: transparent; }
.discover-cover {
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px @borders;
    transition: box-shadow 120ms ease-out;
}
.discover-card:hover .discover-cover { box-shadow: inset 0 0 0 2px @pearl_light; }
.discover-cover .art-letter { font-size: 28px; }
.discover-title {
    margin: 9px 2px 0;
    font-weight: 700;
    font-size: 13px;
}

button.discover-add {
    min-width: 32px;
    min-height: 32px;
    padding: 0;
    background-color: rgba(0, 0, 0, 0.72);
    color: #ffffff;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.24);
}
.discover-card:hover button.discover-add:not(.discover-added),
button.discover-add:focus-visible {
    background: linear-gradient(120deg, @pearl_deep, @pearl 55%, @pearl_light);
    color: #ffffff;
    box-shadow: none;
}
button.discover-add.discover-adding,
button.discover-add.discover-adding:disabled {
    background-color: rgba(0, 0, 0, 0.72);
    color: #ffffff;
    opacity: 1;
}
.discover-frame.foil-flash {
    background-image: linear-gradient(135deg, #4a2fb8 0%, #b7a4ff 30%, #f1ecff 48%, #9d86f7 62%, #4a2fb8 100%);
    box-shadow: 0 14px 28px -10px rgba(123, 92, 240, 0.6);
}
.discover-frame { padding: 2px; transition: box-shadow 400ms ease-out; }
button.discover-add.discover-added,
button.discover-add.discover-added:disabled {
    background-color: rgba(0, 0, 0, 0.72);
    color: #8ee6b0;
    opacity: 1;
    box-shadow: inset 0 0 0 1px rgba(142, 230, 176, 0.5);
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
