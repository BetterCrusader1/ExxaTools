//! The native face of ExxaTools: GTK3, no WebKit and no browser tab.
//!
//! It is a GtkApplication, so a second launch from the Steam panel, the desktop
//! entry or a shell raises the window that already exists. Views read the same
//! library functions as the HTTP API; nothing here talks to localhost.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GtkBox, Orientation};
use serde_json::Value;

use crate::state;

const APP_ID: &str = "dev.exxatools.ExxaTools";

const STYLE: &str = r#"
    window, .background { background: #0b0d10; color: #eef2f7; }
    headerbar { background: #101318; border-bottom: 1px solid #20252d; box-shadow: none; }
    headerbar .title { color: #eef2f7; font-weight: 600; }
    .nav button { background: transparent; border: none; box-shadow: none;
                  color: #9aa4b2; padding: 6px 12px; border-radius: 6px; }
    .nav button:hover { background: #171b21; color: #eef2f7; }
    .nav button:checked { background: #1b2027; color: #ffffff; }
    .sidebar { background: #0e1115; border-right: 1px solid #1e242b; padding: 14px 10px; }
    .sidebar button { background: transparent; border: none; box-shadow: none; color: #9aa4b2;
                      padding: 8px 10px; border-radius: 7px; }
    .sidebar button:hover { background: #171b21; color: #eef2f7; }
    .sidebar button:checked { background: #1b2027; color: #ffffff;
                              border-left: 2px solid #e8edf4; border-radius: 0 7px 7px 0; }
    .brand { font-size: 18px; font-weight: 700; color: #f2f5f8; padding: 2px 2px 0 2px; }
    .topbar { padding: 12px 16px; border-bottom: 1px solid #1e242b; background: #0d1014; }
    entry { background: #12161b; border: 1px solid #242b34;
            border-radius: 7px; color: #eef2f7; padding: 7px 9px; }
    entry:focus { border-color: #3b434f; }
    button.action { background-image: none; background: #171c22; border: 1px solid #29313b;
                    box-shadow: none; color: #e6ebf1; border-radius: 7px; padding: 5px 10px; }
    button.action:hover { background: #1e242c; }
    button.primary { background-image: none; background: #f2f3f5; border: 1px solid #f2f3f5;
                     box-shadow: none; color: #0b0d10; border-radius: 7px; padding: 5px 10px; }
    button.primary:hover { background: #ffffff; }
    button.primary label, button.primary image { color: #0b0d10; }
    button.action label { color: #e6ebf1; }
    .card { background-image: none; background: #141a21; border: 1px solid #2a333e;
            border-radius: 10px; padding: 12px; }
    .game-name { font-size: 15px; font-weight: 600; color: #f2f5f8; }
    .muted { color: #9aa4b2; font-size: 12px; }
    .mono { font-family: monospace; font-size: 11px; color: #8d97a4; }
    .online { color: #63c894; font-size: 11px; }
    .faint { color: #6f7883; font-size: 11px; }
    .section-title { font-size: 18px; font-weight: 600; margin: 4px 0 10px 0; }
    button.badge { background-image: none; border: none; border-radius: 99px;
                   padding: 0 8px; font-size: 9px; font-weight: 700; min-height: 20px; }
    .badge.ready { background: #16301f; color: #74d29a; }
    .badge.planned { background: #2a2317; color: #e8bd72; }
    .badge.keys_missing { background: #321d1d; color: #e88a80; }
    .badge.disabled { background: #20242b; color: #9aa4b2; }
    .dot { border-radius: 99px; min-width: 8px; min-height: 8px; }
    .dot.ok { background: #63c894; }
    .dot.warn { background: #edb45f; }
    .dot.bad { background: #e07a6f; }
    .check-row { padding: 8px 2px; border-bottom: 1px solid #1b2027; }
    .empty { color: #8d97a4; padding: 40px; }
    .empty-title { font-size: 17px; font-weight: 600; color: #dfe5ec; }
    .cover { border-radius: 8px; background: #171c22; }
    button.tile { background-image: none; background: #11151a; border: 1px solid #222933; border-radius: 10px;
                  box-shadow: none; padding: 8px; }
    button.tile:hover { background: #171c23; border-color: #303945; }
    .content-title { font-size: 20px; font-weight: 600; color: #f2f5f8; }
    .content-sub { font-size: 12px; color: #8d97a4; margin-bottom: 6px; }
    statusbar { background: #0e1115; border-top: 1px solid #1e242b; color: #8d97a4;
                font-size: 11px; padding: 6px 12px; }
"#;

#[derive(Clone)]
enum Msg {
    Library(Vec<Value>),
    Search(Vec<Value>),
    Discover(Vec<Value>),
    Doctor(Value),
    Info(String),
    Error(String),
}

struct Ui {
    window: gtk::ApplicationWindow,
    library_head: GtkBox,
    library_grid: gtk::FlowBox,
    discover: gtk::FlowBox,
    doctor: GtkBox,
    status: gtk::Label,
    sender: std::sync::mpsc::Sender<Msg>,
}

pub fn run(_url: &str) -> Result<(), String> {
    let app = gtk::Application::new(Some(APP_ID), gtk::gio::ApplicationFlags::empty());
    app.connect_startup(|_| {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_application_prefer_dark_theme(true);
        }
    });
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        build(app);
    });
    app.run_with_args(&["exxa"]);
    Ok(())
}

fn label(text: &str, class: &str) -> gtk::Label {
    let widget = gtk::Label::new(Some(text));
    widget.set_xalign(0.0);
    widget.style_context().add_class(class);
    widget
}

fn button(text: &str, class: &str) -> gtk::Button {
    let widget = gtk::Button::with_label(text);
    widget.style_context().add_class(class);
    widget
}

fn badge(status: &str) -> gtk::Button {
    let text = match status {
        "ready" => "ready",
        "planned" => "planned",
        "keys_missing" => "keys missing",
        "disabled" => "disabled",
        other => other,
    };
    let widget = gtk::Button::with_label(text);
    widget.set_relief(gtk::ReliefStyle::None);
    widget.set_valign(gtk::Align::Center);
    widget.set_can_focus(false);
    widget.set_size_request(-1, 22);
    widget.style_context().add_class("badge");
    widget.style_context().add_class(status);
    widget
}

fn message(parent: &gtk::ApplicationWindow, text: &str) {
    let dialog = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Info,
        gtk::ButtonsType::Ok,
        text,
    );
    dialog.run();
    dialog.close();
}

fn build(app: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(app);
    if let Some(screen) = gtk::prelude::GtkWindowExt::screen(&window) {
        let provider = gtk::CssProvider::new();
        if let Err(error) = provider.load_from_data(STYLE.as_bytes()) {
            eprintln!("window css: {error}");
        }
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    window.set_title("ExxaTools");
    window.set_default_size(1240, 820);
    window.set_position(gtk::WindowPosition::Center);
    let icon = crate::paths::web_dir().join("logo-128.png");
    if icon.is_file() {
        let _ = window.set_icon_from_file(&icon);
    }

    let header = gtk::HeaderBar::new();
    header.set_show_close_button(true);
    header.set_title(Some("ExxaTools"));
    window.set_titlebar(Some(&header));

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(120);
    stack.set_hexpand(true);
    stack.set_vexpand(true);

    let sidebar = GtkBox::new(Orientation::Vertical, 4);
    sidebar.set_size_request(196, -1);
    sidebar.style_context().add_class("sidebar");
    let brand = gtk::Label::new(Some("ExxaTools"));
    brand.set_xalign(0.0);
    brand.style_context().add_class("brand");
    sidebar.pack_start(&brand, false, false, 0);
    let tagline = label("Vapor Forge", "faint");
    sidebar.pack_start(&tagline, false, false, 10);

    let views = [
        ("library", "Library"),
        ("discover", "Discover"),
        ("system", "System"),
        ("settings", "Settings"),
    ];
    let mut nav_buttons = Vec::new();
    for (name, title) in views {
        let item = gtk::ToggleButton::with_label(title);
        item.set_relief(gtk::ReliefStyle::None);
        if let Some(child) = item.child() {
            if let Some(label) = child.downcast_ref::<gtk::Label>() {
                label.set_xalign(0.0);
            }
        }
        sidebar.pack_start(&item, false, false, 0);
        let stack = stack.clone();
        item.connect_clicked(move |active| {
            if !active.is_active() {
                active.set_active(true);
            }
            stack.set_visible_child_name(name);
        });
        nav_buttons.push(item);
    }
    let steam = label(
        if crate::steamctl::is_running() { "● Steam connected" } else { "○ Steam offline" },
        if crate::steamctl::is_running() { "online" } else { "faint" },
    );
    steam.set_valign(gtk::Align::End);
    steam.set_vexpand(true);
    sidebar.pack_end(&steam, false, false, 4);
    let version = label(&format!("v{}", env!("CARGO_PKG_VERSION")), "faint");
    sidebar.pack_end(&version, false, false, 0);

    let topbar = GtkBox::new(Orientation::Horizontal, 8);
    topbar.style_context().add_class("topbar");
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Find a game — name or app id"));
    search.set_hexpand(true);
    let refresh = gtk::Button::from_icon_name(Some("view-refresh-symbolic"), gtk::IconSize::Button);
    refresh.set_tooltip_text(Some("Refresh library and doctor"));
    refresh.style_context().add_class("action");
    topbar.pack_start(&search, true, true, 0);
    topbar.pack_end(&refresh, false, false, 0);

    let library = GtkBox::new(Orientation::Vertical, 10);
    library.set_margin_top(16);
    library.set_margin_bottom(16);
    library.set_margin_start(20);
    library.set_margin_end(20);
    let library_head = GtkBox::new(Orientation::Vertical, 2);
    let library_grid = gtk::FlowBox::new();
    library_grid.set_column_spacing(12);
    library_grid.set_row_spacing(12);
    library_grid.set_selection_mode(gtk::SelectionMode::None);
    library_grid.set_valign(gtk::Align::Start);
    library_grid.set_max_children_per_line(4);
    library_grid.set_min_children_per_line(1);
    library.pack_start(&library_head, false, false, 0);
    library.pack_start(&library_grid, false, false, 0);
    let library_scroll = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    library_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    library_scroll.add(&library);

    let discover = gtk::FlowBox::new();
    discover.set_column_spacing(10);
    discover.set_row_spacing(10);
    discover.set_selection_mode(gtk::SelectionMode::None);
    discover.set_valign(gtk::Align::Start);
    discover.set_margin_top(12);
    discover.set_margin_bottom(12);
    discover.set_margin_start(14);
    discover.set_margin_end(14);
    let discover_scroll = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    discover_scroll.add(&discover);
    let discover_page = GtkBox::new(Orientation::Vertical, 0);
    let discover_head = GtkBox::new(Orientation::Vertical, 2);
    discover_head.set_margin_top(16);
    discover_head.set_margin_start(20);
    discover_head.set_margin_end(20);
    discover_head.pack_start(&label("Discover", "content-title"), false, false, 0);
    discover_head.pack_start(&label("Popular on Steam right now", "content-sub"), false, false, 0);
    discover_page.pack_start(&discover_head, false, false, 0);
    discover_page.pack_start(&discover_scroll, true, true, 0);

    let doctor = GtkBox::new(Orientation::Vertical, 0);
    doctor.set_margin_top(14);
    doctor.set_margin_bottom(14);
    doctor.set_margin_start(20);
    doctor.set_margin_end(20);
    let doctor_scroll = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    doctor_scroll.add(&doctor);

    let settings = build_settings(&window);
    let settings_scroll = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    settings_scroll.add(&settings);

    stack.add_named(&library_scroll, "library");
    stack.add_named(&discover_page, "discover");
    stack.add_named(&doctor_scroll, "system");
    stack.add_named(&settings_scroll, "settings");
    nav_buttons[0].set_active(true);

    let status = gtk::Label::new(Some(""));
    status.set_xalign(0.0);
    status.style_context().add_class("muted");
    let statusbar = gtk::Box::new(Orientation::Horizontal, 8);
    statusbar.style_context().add_class("statusbar");
    statusbar.pack_start(&status, true, true, 0);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.pack_start(&topbar, false, false, 0);
    content.pack_start(&stack, true, true, 0);
    content.pack_start(&statusbar, false, false, 0);

    let body = GtkBox::new(Orientation::Horizontal, 0);
    body.pack_start(&sidebar, false, false, 0);
    body.pack_start(&content, true, true, 0);
    window.add(&body);

    let (sender, receiver) = std::sync::mpsc::channel::<Msg>();
    let country = std::rc::Rc::new(std::cell::RefCell::new(
        state::load()
            .map(|value| value.settings.country)
            .unwrap_or_else(|_| "us".into()),
    ));
    let ui = Rc::new(Ui {
        window: window.clone(),
        library_head: library_head.clone(),
        library_grid: library_grid.clone(),
        discover: discover.clone(),
        doctor: doctor.clone(),
        status: status.clone(),
        sender: sender.clone(),
    });

    {
        let ui = ui.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    Msg::Library(items) => render_library(&ui, items, false),
                    Msg::Search(items) => render_library(&ui, items, true),
                    Msg::Discover(items) => render_discover(&ui, items),
                    Msg::Doctor(value) => render_doctor(&ui, value),
                    Msg::Info(text) => ui.status.set_text(&text),
                    Msg::Error(text) => {
                        ui.status.set_text(&text);
                        message(&ui.window, &text);
                    }
                }
            }
            gtk::glib::ControlFlow::Continue
        });
    }

    {
        let sender = sender.clone();
        let country = country.clone();
        let stack = stack.clone();
        search.connect_activate(move |entry| {
            let term = entry.text().trim().to_string();
            if term.is_empty() {
                return;
            }
            let sender = sender.clone();
            let country = country.borrow().clone();
            stack.set_visible_child_name("library");
            sender.send(Msg::Info("Searching Steam…".into())).ok();
            std::thread::spawn(move || {
                let mut result = term
                    .parse::<u64>()
                    .ok()
                    .and_then(|appid| crate::steam::app_info(appid).ok())
                    .map(|info| vec![info])
                    .or_else(|| crate::steam::search(&term, &country).ok())
                    .unwrap_or_default();
                attach_covers(&mut result);
                sender.send(Msg::Search(result)).ok();
            });
        });
    }
    {
        let sender = sender.clone();
        let stack = stack.clone();
        refresh.connect_clicked(move |_| {
            stack.set_visible_child_name("library");
            load_library(sender.clone());
            load_doctor(sender.clone());
        });
    }
    {
        let sender = sender.clone();
        let country = country.clone();
        nav_buttons[1].connect_clicked(move |_| {
            let sender = sender.clone();
            let country = country.borrow().clone();
            std::thread::spawn(move || {
                let mut items = crate::steam::discover(&country, 24);
                attach_covers(&mut items);
                sender.send(Msg::Discover(items)).ok();
            });
        });
    }
    {
        let sender = sender.clone();
        nav_buttons[2].connect_clicked(move |_| load_doctor(sender.clone()));
    }
    {
        let sender = sender.clone();
        nav_buttons[3].connect_clicked(move |_| {
            sender.send(Msg::Info("Settings save to ~/.config/exxatools/state.json".into())).ok();
        });
    }

    window.show_all();
    window.present();
    // Focus stealing prevention can leave a freshly spawned app under the
    // browser; sit above for a moment so the window that was asked for is seen.
    window.set_keep_above(true);
    let raised = window.clone();
    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
        raised.set_keep_above(false);
    });
    load_library(sender.clone());
    load_doctor(sender);
}

fn ensure_cover(appid: u64) -> Option<String> {
    let dir = crate::paths::state_dir().join("covers");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{appid}.jpg"));
    if path.is_file() {
        return Some(path.display().to_string());
    }
    let url = format!("https://cdn.akamai.steamstatic.com/steam/apps/{appid}/header.jpg");
    let body = crate::http::get_with(&url, None, Some(std::time::Duration::from_secs(8))).ok()?;
    if body.len() < 1024 {
        return None;
    }
    let temp = dir.join(format!("{appid}.jpg.tmp"));
    std::fs::write(&temp, &body).ok()?;
    std::fs::rename(&temp, &path).ok()?;
    Some(path.display().to_string())
}

fn attach_covers(items: &mut [Value]) {
    for item in items {
        let Some(appid) = item.get("appid").and_then(Value::as_u64) else { continue };
        if let Some(path) = ensure_cover(appid) {
            item["cover"] = Value::String(path);
        }
    }
}

fn load_library(sender: std::sync::mpsc::Sender<Msg>) {
    std::thread::spawn(move || {
        let mut items = state::load()
            .map(|state| {
                state
                    .apps
                    .values()
                    .map(|record| state::app_summary(&state, record))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        attach_covers(&mut items);
        sender.send(Msg::Library(items)).ok();
    });
}

fn load_doctor(sender: std::sync::mpsc::Sender<Msg>) {
    std::thread::spawn(move || {
        sender.send(Msg::Doctor(crate::doctor::run())).ok();
    });
}

fn clear(container: &GtkBox) {
    for child in container.children() {
        container.remove(&child);
    }
}

fn clear_flow(flow: &gtk::FlowBox) {
    for child in flow.children() {
        flow.remove(&child);
    }
}

fn render_library(ui: &Ui, items: Vec<Value>, from_search: bool) {
    clear(&ui.library_head);
    clear_flow(&ui.library_grid);
    let ready = items
        .iter()
        .filter(|item| item.get("state").and_then(Value::as_str) == Some("ready"))
        .count();
    ui.library_head.pack_start(
        &label(if from_search { "Search results" } else { "Library" }, "content-title"),
        false,
        false,
        0,
    );
    let summary = if items.is_empty() {
        "No games yet".to_string()
    } else if from_search {
        format!("{} found", items.len())
    } else {
        format!("{} {} · {} ready", items.len(), if items.len() == 1 { "game" } else { "games" }, ready)
    };
    ui.library_head.pack_start(&label(&summary, "content-sub"), false, false, 0);

    if items.is_empty() {
        let empty = GtkBox::new(Orientation::Vertical, 6);
        empty.set_valign(gtk::Align::Center);
        empty.set_vexpand(true);
        let title = label(
            if from_search { "Nothing found" } else { "Your library is empty" },
            "empty-title",
        );
        title.set_xalign(0.5);
        let hint = label(
            if from_search {
                "Try another name or a numeric app id."
            } else {
                "Search for a game above, then press Add."
            },
            "muted",
        );
        hint.set_xalign(0.5);
        empty.pack_start(&title, false, false, 0);
        empty.pack_start(&hint, false, false, 0);
        ui.library_head.pack_start(&empty, true, true, 24);
        ui.library_head.show_all();
        return;
    }

    for item in items {
        let appid = item.get("appid").and_then(Value::as_u64).unwrap_or(0);
        if appid == 0 {
            continue;
        }
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unknown app")
            .to_string();
        let meta = item
            .get("depot_count")
            .and_then(Value::as_u64)
            .map(|count| {
                format!(
                    "{} {} · {}",
                    count,
                    if count == 1 { "depot" } else { "depots" },
                    item.get("branch").and_then(Value::as_str).unwrap_or("public")
                )
            })
            .unwrap_or_else(|| {
                item.get("price")
                    .map(|value| format!("{value}"))
                    .unwrap_or_else(|| "Steam".into())
            });

        let card = GtkBox::new(Orientation::Vertical, 8);
        card.style_context().add_class("card");
        card.set_size_request(264, -1);
        card.set_tooltip_text(Some(&format!("App {appid} · {meta}")));

        if let Some(path) = item.get("cover").and_then(Value::as_str) {
            let cover = gtk::Image::from_file(path);
            cover.set_size_request(238, 111);
            cover.style_context().add_class("cover");
            card.pack_start(&cover, false, false, 0);
        }

        let title = label(&name, "game-name");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(26);
        card.pack_start(&title, false, false, 0);

        if let Some(status) = item.get("state").and_then(Value::as_str) {
            let row = GtkBox::new(Orientation::Horizontal, 0);
            row.pack_start(&badge(status), false, false, 0);
            card.pack_start(&row, false, false, 0);
        }

        let actions = GtkBox::new(Orientation::Horizontal, 6);
        let installed = item.get("installed").and_then(Value::as_bool).unwrap_or(false);
        if from_search || !installed {
            let add = button("Add", "primary");
            let sender = ui.sender.clone();
            let name_hint = name.clone();
            add.connect_clicked(move |_| {
                let sender = sender.clone();
                let name_hint = name_hint.clone();
                sender.send(Msg::Info(format!("Fetching a plan for {name_hint}…"))).ok();
                std::thread::spawn(move || match crate::plans::fetch_plan(appid, Some(&name_hint)) {
                    Ok(_) => {
                        sender.send(Msg::Info(format!("{name_hint} added"))).ok();
                        load_library(sender.clone());
                    }
                    Err(error) => {
                        sender.send(Msg::Error(error)).ok();
                    }
                });
            });
            actions.pack_start(&add, false, false, 0);
        } else {
            let details = button("Details", "action");
            let sender = ui.sender.clone();
            let window = ui.window.clone();
            let detail_name = name.clone();
            details.connect_clicked(move |_| {
                open_details(&window, appid, &detail_name, sender.clone());
            });
            actions.pack_start(&details, false, false, 0);

            let launch = button("Launch", "primary");
            launch.connect_clicked(move |_| {
                let _ = std::process::Command::new("steam")
                    .arg(format!("steam://rungameid/{appid}"))
                    .spawn();
            });
            actions.pack_start(&launch, false, false, 0);
        }
        card.pack_start(&actions, false, false, 2);
        ui.library_grid.add(&card);
    }
    ui.library_head.show_all();
    ui.library_grid.show_all();
}

fn render_discover(ui: &Ui, items: Vec<Value>) {
    for child in ui.discover.children() {
        ui.discover.remove(&child);
    }
    if items.is_empty() {
        let empty = label("Steam did not answer with suggestions.", "empty");
        ui.discover.add(&empty);
        ui.discover.show_all();
        return;
    }
    for item in items {
        let appid = item.get("appid").and_then(Value::as_u64).unwrap_or(0);
        if appid == 0 {
            continue;
        }
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_string();
        let tile = gtk::Button::new();
        tile.style_context().add_class("tile");
        let inner = GtkBox::new(Orientation::Vertical, 6);
        if let Some(path) = item.get("cover").and_then(Value::as_str) {
            let cover = gtk::Image::from_file(path);
            cover.set_size_request(210, 98);
            inner.pack_start(&cover, false, false, 0);
        }
        let caption = label(&name, "game-name");
        caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
        caption.set_max_width_chars(26);
        inner.pack_start(&caption, false, false, 0);
        tile.add(&inner);
        let window = ui.window.clone();
        let sender = ui.sender.clone();
        let hint = name.clone();
        tile.connect_clicked(move |_| {
            let question = format!("Add {hint} to the library?");
            let dialog = gtk::MessageDialog::new(
                Some(&window),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Question,
                gtk::ButtonsType::YesNo,
                &question,
            );
            let response = dialog.run();
            dialog.close();
            if response == gtk::ResponseType::Yes {
                let sender = sender.clone();
                let hint = hint.clone();
                std::thread::spawn(move || {
                    match crate::plans::fetch_plan(appid, Some(&hint)) {
                        Ok(_) => {
                            sender.send(Msg::Info(format!("{hint} added"))).ok();
                            load_library(sender.clone());
                        }
                        Err(error) => {
                            sender.send(Msg::Error(error)).ok();
                        }
                    }
                });
            }
        });
        ui.discover.add(&tile);
    }
    ui.discover.show_all();
}

fn render_doctor(ui: &Ui, value: Value) {
    clear(&ui.doctor);
    let head = GtkBox::new(Orientation::Horizontal, 10);
    let titles = GtkBox::new(Orientation::Vertical, 2);
    titles.pack_start(&label("System", "content-title"), false, false, 0);
    let items = value
        .get("checks")
        .or_else(|| value.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let problems = items
        .iter()
        .filter(|item| item.get("status").and_then(Value::as_str) != Some("ok"))
        .count();
    titles.pack_start(
        &label(
            &format!("{} checks · {} need attention", items.len(), problems),
            "content-sub",
        ),
        false,
        false,
        0,
    );
    head.pack_start(&titles, true, true, 0);
    let copy = button("Copy report", "action");
    let report = serde_json::to_string_pretty(&value).unwrap_or_default();
    copy.connect_clicked(move |_| {
        gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_text(&report);
    });
    head.pack_end(&copy, false, false, 0);
    ui.doctor.pack_start(&head, false, false, 8);

    let card = GtkBox::new(Orientation::Vertical, 0);
    card.style_context().add_class("card");
    if items.is_empty() {
        card.pack_start(&label("Doctor returned nothing.", "empty"), false, false, 0);
    }
    for item in items {
        let row = GtkBox::new(Orientation::Horizontal, 10);
        row.style_context().add_class("check-row");
        let status = item.get("status").and_then(Value::as_str).unwrap_or("warn");
        let dot = gtk::Label::new(Some(""));
        dot.set_size_request(8, 8);
        dot.set_valign(gtk::Align::Start);
        dot.style_context().add_class("dot");
        dot.style_context().add_class(status);
        row.pack_start(&dot, false, false, 0);

        let text = GtkBox::new(Orientation::Vertical, 2);
        text.pack_start(
            &label(item.get("label").and_then(Value::as_str).unwrap_or(""), "game-name"),
            false,
            false,
            0,
        );
        let detail = label(item.get("detail").and_then(Value::as_str).unwrap_or(""), "mono");
        text.pack_start(&detail, false, false, 0);
        row.pack_start(&text, true, true, 0);
        let hint = item.get("hint").and_then(Value::as_str).unwrap_or("");
        if !hint.is_empty() {
            row.pack_start(&label(hint, "faint"), false, false, 0);
        }
        card.pack_start(&row, false, false, 0);
    }
    ui.doctor.pack_start(&card, false, false, 0);
    ui.doctor.show_all();
}

fn build_settings(window: &gtk::ApplicationWindow) -> GtkBox {
    let loaded = state::load().unwrap_or_default();
    let page = GtkBox::new(Orientation::Vertical, 14);
    page.set_margin_top(18);
    page.set_margin_bottom(18);
    page.set_margin_start(24);
    page.set_margin_end(24);
    page.pack_start(&label("Settings", "content-title"), false, false, 0);

    let form = GtkBox::new(Orientation::Vertical, 14);
    form.set_size_request(540, -1);
    form.set_halign(gtk::Align::Start);

    let region = GtkBox::new(Orientation::Vertical, 8);
    region.style_context().add_class("card");
    region.pack_start(&label("Store & region", "game-name"), false, false, 0);
    let country = gtk::Entry::new();
    country.set_text(&loaded.settings.country);
    country.set_max_width_chars(12);
    region.pack_start(&label("Store country", "muted"), false, false, 0);
    region.pack_start(&country, false, false, 0);
    form.pack_start(&region, false, false, 0);

    let sources = GtkBox::new(Orientation::Vertical, 8);
    sources.style_context().add_class("card");
    sources.pack_start(&label("Sources & API keys", "game-name"), false, false, 0);
    let key = gtk::Entry::new();
    key.set_visibility(false);
    key.set_placeholder_text(Some("smm_…"));
    key.set_text(&loaded.settings.hubcap_key);
    sources.pack_start(&label("Hubcap key", "muted"), false, false, 0);
    sources.pack_start(&key, false, false, 0);
    let ryuu = gtk::Entry::new();
    ryuu.set_visibility(false);
    ryuu.set_text(&loaded.settings.ryuu_key);
    sources.pack_start(&label("Ryuu key", "muted"), false, false, 0);
    sources.pack_start(&ryuu, false, false, 0);
    let source = gtk::Entry::new();
    source.set_text(&loaded.settings.default_source);
    sources.pack_start(&label("Default source", "muted"), false, false, 0);
    sources.pack_start(&source, false, false, 0);
    form.pack_start(&sources, false, false, 0);

    let note = label("", "muted");
    let save = button("Save settings", "primary");
    let note_saved = note.clone();
    let window = window.clone();
    save.connect_clicked(move |_| {
        let result = state::update(|state| {
            state.settings.country = country.text().to_string().trim().to_lowercase();
            state.settings.hubcap_key = key.text().to_string().trim().to_string();
            state.settings.ryuu_key = ryuu.text().to_string().trim().to_string();
            state.settings.default_source = source.text().to_string().trim().to_string();
            Ok(())
        });
        match result {
            Ok(()) => note_saved.set_text("Saved to ~/.config/exxatools/state.json"),
            Err(error) => {
                note_saved.set_text(&error);
                message(&window, &error);
            }
        }
    });
    let row = GtkBox::new(Orientation::Horizontal, 10);
    row.pack_start(&save, false, false, 0);
    row.pack_start(&note, true, true, 0);
    form.pack_start(&row, false, false, 0);
    form.pack_start(&label("State: ~/.config/exxatools/state.json", "faint"), false, false, 0);
    page.pack_start(&form, false, false, 0);
    page
}

fn open_details(window: &gtk::ApplicationWindow, appid: u64, name: &str, sender: std::sync::mpsc::Sender<Msg>) {
    let dialog = gtk::Dialog::with_buttons(
        Some(&format!("{name} — {appid}")),
        Some(window),
        gtk::DialogFlags::MODAL,
        &[("Close", gtk::ResponseType::Close)],
    );
    dialog.set_default_size(560, 420);
    let content = dialog.content_area();
    let loaded = state::load().ok();
    let record = loaded
        .as_ref()
        .and_then(|state| state.apps.get(&appid.to_string()))
        .cloned();

    let text = GtkBox::new(Orientation::Vertical, 6);
    text.set_margin_top(12);
    text.set_margin_bottom(12);
    text.set_margin_start(14);
    text.set_margin_end(14);
    if let Some(record) = record {
        let summary = gtk::Label::new(Some(&format!(
            "Branch {} · {} depot(s)",
            record.get("branch").and_then(Value::as_str).unwrap_or("public"),
            record
                .get("depots")
                .and_then(Value::as_array)
                .map(|list| list.len())
                .unwrap_or(0)
        )));
        summary.set_xalign(0.0);
        text.pack_start(&summary, false, false, 0);
        for depot in record
            .get("depots")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let id = depot.get("depot_id").and_then(Value::as_u64).unwrap_or(0);
            let gid = depot.get("gid").and_then(Value::as_str).unwrap_or("");
            let enabled = depot.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            let line = gtk::Label::new(Some(&format!(
                "{} {} · {}",
                id,
                if enabled { "" } else { "(disabled)" },
                gid
            )));
            line.set_xalign(0.0);
            line.style_context().add_class("muted");
            text.pack_start(&line, false, false, 0);
        }
    } else {
        text.pack_start(&label("No saved plan for this app yet.", "muted"), false, false, 0);
    }

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    let rebuild = button("Sync manifests", "action");
    let sender_rebuild = sender.clone();
    let sync_name = name.to_string();
    rebuild.connect_clicked(move |_| {
        match crate::plans::refresh_plan(appid) {
            Ok(_) => sender_rebuild.send(Msg::Info(format!("{sync_name} synced"))).ok(),
            Err(error) => sender_rebuild.send(Msg::Error(error)).ok(),
        };
    });
    actions.pack_start(&rebuild, false, false, 0);

    let launch = button("Launch", "action");
    launch.connect_clicked(move |_| {
        let _ = std::process::Command::new("steam")
            .arg(format!("steam://rungameid/{appid}"))
            .spawn();
    });
    actions.pack_start(&launch, false, false, 0);

    let remove = button("Remove", "action");
    let sender_remove = sender.clone();
    let remove_name = name.to_string();
    remove.connect_clicked(move |_| {
        let _ = crate::vfsys::remove_script(appid);
        let result = state::update(|state| {
            state.apps.remove(&appid.to_string());
            Ok(())
        });
        match result {
            Ok(()) => {
                sender_remove.send(Msg::Info(format!("{remove_name} removed"))).ok();
                load_library(sender_remove.clone());
            }
            Err(error) => {
                sender_remove.send(Msg::Error(error)).ok();
            }
        };
    });
    actions.pack_start(&remove, false, false, 0);

    text.pack_start(&actions, false, false, 6);
    content.pack_start(&text, true, true, 0);
    dialog.show_all();
    dialog.run();
    dialog.close();
}