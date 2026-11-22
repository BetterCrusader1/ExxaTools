//! Window shell: navigation rail, three-pane library, and the one event loop.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::model::Game;
use crate::ui::theme::{self, Theme};
use crate::ui::worker::{Command, Event, Worker};
use crate::ui::backdrop::Backdrop;
use crate::ui::{discover, inspector, settings, shelf, system};

const APP_ID: &str = "dev.exxatools.ExxaTools";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Library,
    Discover,
    System,
    Settings,
}

impl Page {
    fn name(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Discover => "discover",
            Self::System => "system",
            Self::Settings => "settings",
        }
    }

    fn index(self) -> i32 {
        match self {
            Self::Library => 0,
            Self::Discover => 1,
            Self::System => 2,
            Self::Settings => 3,
        }
    }
}

/// Search and library loads run on parallel worker threads, so an answer can
/// land after a newer request. Only the latest search may fill the shelf, and
/// only while the search box still holds a term.
fn accept_search(latest: u64, searching: bool, id: u64) -> bool {
    searching && id == latest
}

/// A library reload (startup, Add, Sync, watcher) must not replace visible
/// search results; it only refreshes the shelf when no search is active.
fn library_fills_shelf(searching: bool) -> bool {
    !searching
}

type NavigateHook = Box<dyn Fn(Page)>;
type SelectHook = Box<dyn Fn(Option<u64>)>;

pub struct Hooks {
    navigate: RefCell<Option<NavigateHook>>,
    select: RefCell<Option<SelectHook>>,
}

#[derive(Clone)]
pub struct Ctx {
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub commands: Worker,
    hooks: Rc<Hooks>,
}

impl Ctx {
    pub fn navigate(&self, page: Page) {
        if let Some(navigate) = self.hooks.navigate.borrow().as_ref() {
            navigate(page);
        }
    }

    pub fn select(&self, appid: Option<u64>) {
        if let Some(select) = self.hooks.select.borrow().as_ref() {
            select(appid);
        }
    }

    pub fn toast_text(&self, text: &str) {
        self.toast.add_toast(adw::Toast::new(text));
    }

    pub fn clipboard(&self, text: &str) {
        self.window.clipboard().set_text(text);
    }
}

struct View {
    games: Vec<Game>,
    selected: Option<u64>,
    search: bool,
}

struct NavRail {
    root: gtk::Widget,
    list: gtk::ListBox,
    /// Hidden in the icon-only rail of a narrow window.
    labels: Vec<gtk::Label>,
    rows: Vec<GtkBox>,
}

thread_local! {
    /// Set when a new build is installed: the page to reopen after exec.
    static RESTART: Cell<Option<Page>> = const { Cell::new(None) };
}

pub fn run(url: &str) -> Result<(), String> {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.set_flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE);
    app.connect_startup(|_| {
        if let Some(display) = gtk::gdk::Display::default() {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(&format!("{STYLE}{}", crate::ui::art::STYLE));
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
    // command_line runs in the primary process for later invocations too. That
    // means `exxa window --url …?page=discover` really changes the open window
    // instead of merely raising it on whatever page it had before.
    let current: Rc<RefCell<Option<Ctx>>> = Rc::new(RefCell::new(None));
    app.connect_command_line(move |app, command_line| {
        let page = command_line
            .arguments()
            .get(1)
            .and_then(|value| value.to_str())
            .map(page_from_url)
            .unwrap_or(Page::Library);
        let existing = current.borrow().as_ref().cloned();
        if let Some(ctx) = existing {
            ctx.navigate(page);
            ctx.window.present();
        } else {
            let ctx = build(app, page);
            *current.borrow_mut() = Some(ctx);
        }
        0.into()
    });
    app.run_with_args(&["exxa", url]);
    if let Some(page) = RESTART.with(Cell::take) {
        // The GTK app has released its D-Bus name and display connection, so
        // the new build starts as the primary instance on the same page.
        let url = format!(
            "http://127.0.0.1:{}/?page={}",
            crate::server::DEFAULT_PORT,
            page.name()
        );
        use std::os::unix::process::CommandExt;
        let error = std::process::Command::new(crate::paths::companion_bin())
            .args(["window", "--url", &url])
            .env(crate::live::REEXEC_ENV, "1")
            .exec();
        return Err(format!("cannot restart into the new build: {error}"));
    }
    Ok(())
}

/// Keeps the dashboard in step with Steam and with new builds: restart into
/// a freshly installed companion, close when Steam exits, report builds.
fn follow_live(ctx: &Ctx, current_page: Rc<Cell<Page>>) {
    let mut updates = crate::live::UpdateWatch::new();
    let mut steam_seen = crate::steamctl::is_running();
    let started = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let mut reported = started;
    let ctx = ctx.clone();
    gtk::glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
        let status = crate::live::status();
        let at = status.get("at").and_then(serde_json::Value::as_u64).unwrap_or(0);
        if at > reported {
            reported = at;
            let message = status.get("message").and_then(serde_json::Value::as_str).unwrap_or("");
            match status.get("state").and_then(serde_json::Value::as_str) {
                Some("building") => ctx.toast_text("Building the latest changes…"),
                Some("failed") => ctx.toast_text(&format!("Build failed: {message}")),
                _ => {}
            }
        }
        if updates.ready() {
            RESTART.with(|slot| slot.set(Some(current_page.get())));
            ctx.toast_text("Updating to the latest build…");
            let window = ctx.window.clone();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
                if let Some(app) = window.application() {
                    app.quit();
                }
            });
            return gtk::glib::ControlFlow::Break;
        }
        let running = crate::steamctl::is_running();
        if steam_seen && !running && crate::live::close_with_steam() {
            if let Some(app) = ctx.window.application() {
                app.quit();
            }
            return gtk::glib::ControlFlow::Break;
        }
        steam_seen |= running;
        gtk::glib::ControlFlow::Continue
    });
}

fn build(app: &adw::Application, initial: Page) -> Ctx {
    let (width, height, maximized) = window_size::load();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("ExxaTools")
        .icon_name("exxatools")
        .default_width(width)
        .default_height(height)
        .maximized(maximized)
        .build();
    // Breakpoints need a minimum size; below this the layout cannot fold.
    window.set_size_request(MIN_WIDTH, MIN_HEIGHT);
    window.connect_close_request(|window| {
        window_size::save(window);
        gtk::glib::Propagation::Proceed
    });

    let overlay = adw::ToastOverlay::new();
    let initial_theme = crate::state::load()
        .map(|state| Theme::from_setting(&state.settings.theme))
        .unwrap_or(Theme::Dark);
    let current_theme = Rc::new(Cell::new(initial_theme));
    let theme_slot: Rc<RefCell<Option<gtk::CssProvider>>> = Rc::new(RefCell::new(None));
    apply_theme(&theme_slot, initial_theme);

    let (worker, events) = Worker::start();
    let hooks = Rc::new(Hooks {
        navigate: RefCell::new(None),
        select: RefCell::new(None),
    });
    let ctx = Ctx {
        window: window.clone(),
        toast: overlay.clone(),
        commands: worker.clone(),
        hooks: hooks.clone(),
    };

    let shelf = shelf::build();
    let inspector = Rc::new(inspector::build());
    let discover = Rc::new(discover::build(&ctx));
    let system = Rc::new(system::build(&ctx));
    let settings = Rc::new(settings::build(&ctx));

    // Side by side when there is room; on a narrow window the shelf slides
    // over the game instead, from the header's sidebar button.
    let split = adw::OverlaySplitView::new();
    split.add_css_class("library-split");
    split.set_sidebar(Some(&shelf.root));
    split.set_content(Some(&inspector.root));
    split.set_min_sidebar_width(280.0);
    split.set_max_sidebar_width(340.0);
    split.set_sidebar_width_fraction(0.28);

    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(180);
    // Only the visible page may set the window's minimum width.
    stack.set_hhomogeneous(false);
    stack.add_named(&split, Some(Page::Library.name()));
    stack.add_named(&discover.root, Some(Page::Discover.name()));
    stack.add_named(&system.root, Some(Page::System.name()));
    stack.add_named(&settings.root, Some(Page::Settings.name()));

    let nav = build_nav();
    let body = GtkBox::new(Orientation::Horizontal, 0);
    body.append(&nav.root);
    body.append(&stack);

    let search = gtk::SearchEntry::new();
    search.add_css_class("global-search");
    search.set_placeholder_text(Some("Search Steam by name or app id"));
    search.set_width_chars(4);
    search.set_max_width_chars(46);
    search.set_hexpand(true);

    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.add_css_class("flat");
    refresh.add_css_class("global-control");
    refresh.set_tooltip_text(Some("Refresh the library, manifests, and system checks"));
    refresh.set_cursor_from_name(Some("pointer"));

    let theme_button = gtk::Button::from_icon_name(initial_theme.icon());
    theme_button.add_css_class("flat");
    theme_button.add_css_class("global-control");
    theme_button.set_tooltip_text(Some(initial_theme.tooltip()));
    theme_button.set_cursor_from_name(Some("pointer"));
    {
        let current = current_theme.clone();
        let commands = worker.clone();
        theme_button.connect_clicked(move |_| {
            commands.send(Command::SetTheme(current.get().next().as_str().into()));
        });
    }

    let shelf_toggle = gtk::ToggleButton::new();
    shelf_toggle.set_icon_name("sidebar-show-symbolic");
    shelf_toggle.add_css_class("flat");
    shelf_toggle.add_css_class("global-control");
    shelf_toggle.set_tooltip_text(Some("Show the library (F9)"));
    shelf_toggle.set_visible(false);
    split
        .bind_property("show-sidebar", &shelf_toggle, "active")
        .bidirectional()
        .sync_create()
        .build();

    let header = adw::HeaderBar::new();
    header.add_css_class("global-header");
    header.set_title_widget(Some(&search));
    header.pack_start(&shelf_toggle);
    header.pack_end(&theme_button);
    header.pack_end(&refresh);

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&header);
    root.append(&body);

    // The blurred game art sits under everything; the content measures the
    // window so the tiny backdrop texture never drives its size.
    let backdrop = Backdrop::new(root.upcast_ref());
    let layers = gtk::Overlay::new();
    layers.set_child(Some(&backdrop.root));
    layers.add_overlay(&root);
    layers.set_measure_overlay(&root, true);
    overlay.set_child(Some(&layers));
    window.set_content(Some(&overlay));

    let view = Rc::new(RefCell::new(View {
        games: Vec::new(),
        selected: None,
        search: false,
    }));
    let syncing = Rc::new(Cell::new(false));
    let search_seq = Rc::new(Cell::new(0u64));
    let current_page = Rc::new(Cell::new(Page::Library));

    {
        let ctx = ctx.clone();
        let stack = stack.clone();
        let nav_list = nav.list.clone();
        let discover = discover.clone();
        let system = system.clone();
        let settings = settings.clone();
        let syncing = syncing.clone();
        let worker = worker.clone();
        let discover_loaded = Rc::new(Cell::new(false));
        let system_loaded = Rc::new(Cell::new(false));
        let current_page = current_page.clone();
        let backdrop = backdrop.clone();
        *hooks.navigate.borrow_mut() = Some(Box::new(move |page| {
            current_page.set(page);
            backdrop.set_active(page == Page::Library);
            stack.set_visible_child_name(page.name());
            syncing.set(true);
            if let Some(row) = nav_list.row_at_index(page.index()) {
                nav_list.select_row(Some(&row));
            }
            syncing.set(false);
            match page {
                Page::Library => {}
                Page::Discover if !discover_loaded.get() => {
                    discover_loaded.set(true);
                    discover.show_loading();
                    worker.send(Command::LoadDiscover);
                }
                Page::System if !system_loaded.get() => {
                    system_loaded.set(true);
                    system.show_loading();
                    worker.send(Command::LoadDoctor);
                }
                Page::Settings => settings.reload(),
                _ => {}
            }
            let _ = &ctx;
        }));
    }

    {
        let ctx = ctx.clone();
        let view = view.clone();
        let inspector = inspector.clone();
        let backdrop = backdrop.clone();
        *hooks.select.borrow_mut() = Some(Box::new(move |appid| {
            view.borrow_mut().selected = appid;
            let game = appid.and_then(|appid| {
                view.borrow()
                    .games
                    .iter()
                    .find(|game| game.appid == appid)
                    .cloned()
            });
            if let Some(id) = appid {
                ctx.commands.send(Command::LoadHero(id));
            }
            // A known hero fades in now; otherwise the previous backdrop stays
            // until LoadHero answers, so selection never flashes to plain.
            match &game {
                Some(game) if game.hero.is_some() => backdrop.set_art(game.hero.as_deref()),
                Some(_) => {}
                None => backdrop.set_art(None),
            }
            inspector.show(&ctx, game.as_ref());
        }));
    }

    {
        let appids = shelf.appids.clone();
        let ctx = ctx.clone();
        shelf.list.connect_row_selected(move |_, row| {
            let appid = row.and_then(|row| {
                let index = row.index();
                if index < 0 {
                    None
                } else {
                    appids.borrow().get(index as usize).copied()
                }
            });
            ctx.select(appid);
        });
    }

    {
        let ctx = ctx.clone();
        let syncing = syncing.clone();
        nav.list.connect_row_selected(move |_, row| {
            if syncing.get() {
                return;
            }
            let Some(row) = row else { return };
            let page = match row.index() {
                1 => Page::Discover,
                2 => Page::System,
                3 => Page::Settings,
                _ => Page::Library,
            };
            ctx.navigate(page);
        });
    }

    {
        let ctx = ctx.clone();
        let shelf = shelf.clone();
        let worker = worker.clone();
        let view = view.clone();
        let search_seq = search_seq.clone();
        let current_page = current_page.clone();
        search.connect_activate(move |entry| {
            let term = entry.text().trim().to_string();
            if term.is_empty() {
                view.borrow_mut().search = false;
                worker.send(Command::LoadLibrary);
                return;
            }
            let id = search_seq.get() + 1;
            search_seq.set(id);
            view.borrow_mut().search = true;
            if current_page.get() != Page::Discover {
                ctx.navigate(Page::Library);
            }
            shelf.set_title("Search results", "Searching Steam…");
            worker.send(Command::Search { id, term });
        });
    }
    {
        let worker = worker.clone();
        let shelf = shelf.clone();
        let view = view.clone();
        let current_page = current_page.clone();
        search.connect_search_changed(move |entry| {
            if entry.text().trim().is_empty() && view.borrow().search {
                view.borrow_mut().search = false;
                if current_page.get() == Page::Discover {
                    worker.send(Command::LoadDiscover);
                } else {
                    shelf.set_title("Library", "Reloading…");
                    worker.send(Command::LoadLibrary);
                }
            }
        });
    }
    {
        let worker = worker.clone();
        let discover = discover.clone();
        let system = system.clone();
        refresh.connect_clicked(move |_| {
            discover.show_loading();
            worker.send(Command::LoadDiscover);
            system.show_loading();
            worker.send(Command::LoadDoctor);
            worker.send(Command::SyncAll);
        });
    }

    {
        let view = view.clone();
        let shelf = shelf.clone();
        let inspector = inspector.clone();
        let backdrop = backdrop.clone();
        let discover = discover.clone();
        let system = system.clone();
        let ctx = ctx.clone();
        let search_seq = search_seq.clone();
        let current_page = current_page.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
            while let Ok(event) = events.try_recv() {
                match event {
                    Event::Library(games) => {
                        discover.set_library(&games);
                        if !library_fills_shelf(view.borrow().search) {
                            continue;
                        }
                        let selected = view.borrow().selected;
                        view.borrow_mut().games = games.clone();
                        shelf.set_games(&games);
                        let ready = games
                            .iter()
                            .filter(|game| game.state == crate::ui::model::GameState::Ready)
                            .count();
                        let needs = games.len().saturating_sub(ready);
                        let summary = if games.is_empty() {
                            "Add your first game from Discover".to_string()
                        } else if needs == 0 {
                            format!("{ready} games ready to play")
                        } else {
                            format!("{ready} ready to play, {needs} need setup")
                        };
                        shelf.set_title("Library", &summary);
                        let still_there =
                            selected.filter(|id| games.iter().any(|game| game.appid == *id));
                        if let Some(id) = still_there {
                            shelf.select_appid(id);
                            let game = games.iter().find(|game| game.appid == id).cloned();
                            inspector.show(&ctx, game.as_ref());
                        } else if !games.is_empty() {
                            shelf.select_first();
                        } else {
                            view.borrow_mut().selected = None;
                            inspector.show_empty(Some(&ctx));
                        }
                    }
                    Event::Search { id, games } => {
                        if !accept_search(search_seq.get(), view.borrow().search, id) {
                            continue;
                        }
                        if current_page.get() == Page::Discover {
                            discover.set_search(&games);
                            continue;
                        }
                        view.borrow_mut().games = games.clone();
                        shelf.set_games(&games);
                        shelf.set_title(
                            "Search results",
                            &if games.is_empty() {
                                "Nothing found — try another name or app id".to_string()
                            } else {
                                format!("{} found", games.len())
                            },
                        );
                        if games.is_empty() {
                            view.borrow_mut().selected = None;
                            inspector.show_empty(Some(&ctx));
                        } else {
                            shelf.select_first();
                        }
                    }
                    Event::Discover(games) => {
                        discover.set_games(&games);
                        if games.is_empty() {
                            discover.show_error("Steam returned no popular titles.");
                        }
                    }
                    Event::Doctor(report) => system.set_report(&report),
                    Event::Toast { text, error } => {
                        if error {
                            ctx.toast_text(&format!("Could not complete: {text}"));
                        } else {
                            ctx.toast_text(&text);
                        }
                    }
                    Event::Clipboard(text) => {
                        ctx.clipboard(&text);
                        ctx.toast_text("Diagnostics copied to the clipboard");
                    }
                    Event::SettingsReload => settings.reload(),
                    Event::Hero { appid, path, logo } => {
                        if view.borrow().selected == Some(appid) {
                            if let Some(game) = view
                                .borrow_mut()
                                .games
                                .iter_mut()
                                .find(|game| game.appid == appid)
                            {
                                game.hero = path;
                                game.logo = logo;
                            }
                            let game = view
                                .borrow()
                                .games
                                .iter()
                                .find(|game| game.appid == appid)
                                .cloned();
                            if let Some(game) = &game {
                                backdrop.set_art(game.hero.as_deref().or(game.cover.as_deref()));
                            }
                            inspector.show(&ctx, game.as_ref());
                        }
                    }
                    Event::Theme(name) => {
                        let selected = Theme::from_setting(&name);
                        current_theme.set(selected);
                        apply_theme(&theme_slot, selected);
                        theme_button.set_icon_name(selected.icon());
                        theme_button.set_tooltip_text(Some(selected.tooltip()));
                    }
                    Event::RefreshLibrary => {
                        discover.reconcile_library();
                        ctx.commands.send(Command::LoadLibrary);
                    }
                }
            }
            gtk::glib::ControlFlow::Continue
        });
    }

    install_breakpoints(&window, &nav, &split, &shelf_toggle, &inspector);
    install_shortcuts(&window, &ctx, &search, &refresh, &split);
    {
        // Picking a game in the folded layout returns to it.
        let split = split.clone();
        shelf.list.connect_row_activated(move |_, _| {
            if split.is_collapsed() {
                split.set_show_sidebar(false);
            }
        });
    }
    {
        let ctx = ctx.clone();
        shelf_toggle.connect_clicked(move |_| ctx.navigate(Page::Library));
    }
    window.present();
    follow_live(&ctx, current_page.clone());
    snapshot_on_request(&window);
    nav.list
        .select_row(nav.list.row_at_index(initial.index()).as_ref());
    worker.send(Command::LoadLibrary);
    ctx
}

fn apply_theme(slot: &Rc<RefCell<Option<gtk::CssProvider>>>, selected: Theme) {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    adw::StyleManager::default().set_color_scheme(selected.color_scheme());
    if let Some(old) = slot.borrow_mut().take() {
        gtk::style_context_remove_provider_for_display(&display, &old);
    }
    let provider = gtk::CssProvider::new();
    provider.load_from_data(&theme::css(selected));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    *slot.borrow_mut() = Some(provider);
}

fn page_from_url(url: &str) -> Page {
    if url.contains("page=discover") {
        Page::Discover
    } else if url.contains("page=system") {
        Page::System
    } else if url.contains("page=settings") {
        Page::Settings
    } else {
        Page::Library
    }
}

/// `EXXA_SNAPSHOT=/path.png` renders the window to a PNG after
/// `EXXA_SNAPSHOT_DELAY` seconds (default 10) and quits: screenshots for docs
/// and layout checks that do not depend on which window has focus.
fn snapshot_on_request(window: &adw::ApplicationWindow) {
    let Some(path) = std::env::var_os("EXXA_SNAPSHOT") else {
        return;
    };
    let delay = std::env::var("EXXA_SNAPSHOT_DELAY")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);
    let window = window.clone();
    gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(delay), move || {
        let paintable = gtk::WidgetPaintable::new(Some(&window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
        let texture = snapshot.to_node().and_then(|node| {
            window
                .renderer()
                .map(|renderer| renderer.render_texture(node, None))
        });
        match texture.map(|texture| texture.save_to_png(&path)) {
            Some(Ok(())) => eprintln!("snapshot saved"),
            Some(Err(error)) => eprintln!("snapshot failed: {error}"),
            None => eprintln!("snapshot failed: nothing rendered"),
        }
        if let Some(app) = window.application() {
            app.quit();
        }
    });
}

const MIN_WIDTH: i32 = 420;
const MIN_HEIGHT: i32 = 480;

/// Below this the rail shows icons only.
const RAIL_BREAKPOINT: &str = "max-width: 1000sp";
/// Below this the shelf folds over the game.
const FOLD_BREAKPOINT: &str = "max-width: 760sp";

fn install_breakpoints(
    window: &adw::ApplicationWindow,
    nav: &NavRail,
    split: &adw::OverlaySplitView,
    shelf_toggle: &gtk::ToggleButton,
    inspector: &Rc<inspector::Inspector>,
) {
    // libadwaita applies only the last matching breakpoint, so the folded
    // layout repeats the icon-only rail setters.
    let compact_rail = |breakpoint: &adw::Breakpoint| {
        breakpoint.add_setter(&nav.root, "width-request", Some(&64.to_value()));
        for label in &nav.labels {
            breakpoint.add_setter(label, "visible", Some(&false.to_value()));
        }
        for row in &nav.rows {
            breakpoint.add_setter(row, "halign", Some(&gtk::Align::Center.to_value()));
        }
    };
    if let Ok(condition) = adw::BreakpointCondition::parse(RAIL_BREAKPOINT) {
        let rail = adw::Breakpoint::new(condition);
        compact_rail(&rail);
        window.add_breakpoint(rail);
    }
    if let Ok(condition) = adw::BreakpointCondition::parse(FOLD_BREAKPOINT) {
        let fold = adw::Breakpoint::new(condition);
        compact_rail(&fold);
        fold.add_setter(split, "collapsed", Some(&true.to_value()));
        fold.add_setter(split, "show-sidebar", Some(&false.to_value()));
        fold.add_setter(shelf_toggle, "visible", Some(&true.to_value()));
        let classes = vec!["library-split".to_string(), "compact".to_string()];
        fold.add_setter(split, "css-classes", Some(&classes.to_value()));
        {
            let inspector = inspector.clone();
            fold.connect_apply(move |_| inspector.set_compact(true));
        }
        {
            let inspector = inspector.clone();
            fold.connect_unapply(move |_| inspector.set_compact(false));
        }
        window.add_breakpoint(fold);
    }
}

/// Keyboard: Ctrl+K or Ctrl+F search, Ctrl+1..4 pages, Ctrl+R or F5 refresh,
/// F9 the folded library.
fn install_shortcuts(
    window: &adw::ApplicationWindow,
    ctx: &Ctx,
    search: &gtk::SearchEntry,
    refresh: &gtk::Button,
    split: &adw::OverlaySplitView,
) {
    let controller = gtk::ShortcutController::new();
    controller.set_scope(gtk::ShortcutScope::Managed);
    let add = |triggers: &str, action: Box<dyn Fn() + 'static>| {
        let Some(trigger) = gtk::ShortcutTrigger::parse_string(triggers) else {
            return;
        };
        let callback = gtk::CallbackAction::new(move |_, _| {
            action();
            gtk::glib::Propagation::Stop
        });
        controller.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(callback)));
    };
    {
        let search = search.clone();
        add(
            "<Control>k|<Control>f",
            Box::new(move || {
                search.grab_focus();
                search.select_region(0, -1);
            }),
        );
    }
    for (index, page) in [Page::Library, Page::Discover, Page::System, Page::Settings]
        .into_iter()
        .enumerate()
    {
        let ctx = ctx.clone();
        add(
            &format!("<Control>{}", index + 1),
            Box::new(move || ctx.navigate(page)),
        );
    }
    {
        let refresh = refresh.clone();
        add("<Control>r|F5", Box::new(move || refresh.emit_clicked()));
    }
    {
        let split = split.clone();
        let ctx = ctx.clone();
        add(
            "F9",
            Box::new(move || {
                if split.is_collapsed() {
                    ctx.navigate(Page::Library);
                    split.set_show_sidebar(!split.shows_sidebar());
                }
            }),
        );
    }
    window.add_controller(controller);
}

/// The window keeps its size across restarts, including live updates.
mod window_size {
    use gtk::prelude::*;

    const DEFAULT: (i32, i32) = (1280, 820);

    fn path() -> std::path::PathBuf {
        crate::paths::state_dir().join("window-size.json")
    }

    pub fn load() -> (i32, i32, bool) {
        let value: serde_json::Value = std::fs::read(path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let read = |key: &str, fallback: i32, min: i32| {
            value
                .get(key)
                .and_then(serde_json::Value::as_i64)
                .map(|number| (number as i32).max(min))
                .unwrap_or(fallback)
        };
        (
            read("width", DEFAULT.0, super::MIN_WIDTH),
            read("height", DEFAULT.1, super::MIN_HEIGHT),
            value
                .get("maximized")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        )
    }

    pub fn save(window: &adw::ApplicationWindow) {
        let (width, height) = window.default_size();
        let body = serde_json::json!({
            "width": width,
            "height": height,
            "maximized": window.is_maximized(),
        });
        let _ = std::fs::write(path(), body.to_string());
    }
}

fn build_nav() -> NavRail {
    let brand = gtk::Label::new(Some("ExxaTools"));
    brand.add_css_class("title-3");
    brand.set_halign(gtk::Align::Start);
    brand.set_valign(gtk::Align::Center);
    let mark = gtk::Picture::for_filename(crate::paths::web_dir().join("logo-48.png"));
    mark.set_size_request(34, 34);
    mark.set_valign(gtk::Align::Center);
    mark.set_can_shrink(true);

    let brand_row = GtkBox::new(Orientation::Horizontal, 11);
    brand_row.set_margin_top(20);
    brand_row.set_margin_bottom(20);
    brand_row.set_margin_start(16);
    brand_row.set_margin_end(12);
    brand_row.append(&mark);
    brand_row.append(&brand);

    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    list.set_selection_mode(gtk::SelectionMode::Single);

    let mut labels = vec![brand.clone()];
    let mut rows = Vec::new();
    for (index, (icon, label)) in [
        ("applications-games-symbolic", "Library"),
        ("system-search-symbolic", "Discover"),
        ("applications-utilities-symbolic", "System"),
        ("emblem-system-symbolic", "Settings"),
    ]
    .iter()
    .enumerate()
    {
        let row = gtk::ListBoxRow::new();
        row.add_css_class("nav-row");
        row.set_cursor_from_name(Some("pointer"));
        row.set_tooltip_text(Some(&format!("{label} (Ctrl+{})", index + 1)));
        let image = gtk::Image::from_icon_name(icon);
        image.set_pixel_size(16);
        image.add_css_class("nav-icon");
        let text = gtk::Label::new(Some(label));
        text.add_css_class("nav-label");
        text.set_halign(gtk::Align::Start);
        let content = GtkBox::new(Orientation::Horizontal, 10);
        content.set_margin_top(7);
        content.set_margin_bottom(7);
        content.set_margin_start(10);
        content.set_margin_end(10);
        content.append(&image);
        content.append(&text);
        row.set_child(Some(&content));
        list.append(&row);
        labels.push(text);
        rows.push(content);
    }
    rows.push(brand_row.clone());

    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("nav-rail");
    root.set_width_request(200);
    root.append(&brand_row);
    root.append(&list);
    root.append(&spacer);

    NavRail {
        root: root.upcast(),
        list,
        labels,
        rows,
    }
}

const STYLE: &str = r#"
.nav-rail {
    background-color: @sidebar_bg_color;
    border-right: 1px solid @borders;
}
.nav-rail list { background: none; }
.nav-row {
    border-radius: 10px;
    margin: 1px 10px;
    transition: background-color 140ms ease-out;
}
.nav-row:hover { background-color: @surface_raise; }
.nav-row:selected,
.nav-row:selected:hover {
    background-color: @surface_hover;
}
.nav-icon { color: alpha(@window_fg_color, 0.6); }
.nav-label { font-weight: 600; color: alpha(@window_fg_color, 0.78); }
.nav-row:selected .nav-icon { color: @accent_color; }
.nav-row:selected .nav-label { color: @window_fg_color; }

.shelf {
    background-color: @window_bg_color;
    border-right: 1px solid @borders;
}
.shelf-list { background-color: transparent; }
.game-row {
    border-radius: 12px;
    margin-bottom: 2px;
    transition: background-color 140ms ease-out;
}
.game-row:hover { background-color: @surface_raise; }
.game-row:selected,
.game-row:selected:hover { background-color: @surface_hover; }
.game-name { font-weight: 600; }

.status-dot {
    min-width: 7px;
    min-height: 7px;
    border-radius: 50%;
    background-color: @insensitive_fg_color;
}
.dot-ready { background-color: @success_color; }
.dot-planned { background-color: @warning_color; }
.dot-warning { background-color: @error_color; }
.dot-disabled { background-color: @insensitive_fg_color; }

.shelf-cover { border-radius: 7px; }
.shelf-cover .art-letter { font-size: 13px; }

.hero { border-radius: 16px; }
.hero .art-letter { font-size: 64px; }

.depot-table { padding: 3px 0; }
.depot-table separator { min-height: 1px; }
.library-split > widget.sidebar-pane { background: none; }
/* Folded, the shelf floats over the game and must be opaque. */
.library-split.compact .shelf,
.has-backdrop .library-split.compact .shelf {
    background-color: @window_bg_color;
    box-shadow: 0 0 24px rgba(0, 0, 0, 0.45);
}
"#;

#[cfg(test)]
mod search_race_tests {
    use super::{accept_search, library_fills_shelf};

    #[test]
    fn only_the_latest_active_search_fills_the_shelf() {
        assert!(accept_search(3, true, 3));
        assert!(!accept_search(3, true, 2));
        assert!(!accept_search(3, false, 3));
    }

    #[test]
    fn library_reload_does_not_replace_search_results() {
        assert!(!library_fills_shelf(true));
        assert!(library_fills_shelf(false));
    }
}
