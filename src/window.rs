//! Native dashboard on GTK4 + libadwaita: the Adwaita design system, not a
//! hand-styled surface. One application window with a navigation split view.

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};
use serde_json::Value;

use crate::state;

const APP_ID: &str = "dev.exxatools.ExxaTools";

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
    window: adw::ApplicationWindow,
    toast: adw::ToastOverlay,
    stack: gtk::Stack,
    library_list: gtk::ListBox,
    library_title: gtk::Label,
    library_sub: gtk::Label,
    discover: gtk::FlowBox,
    doctor_list: gtk::ListBox,
    sender: std::sync::mpsc::Sender<Msg>,
    country: std::rc::Rc<std::cell::RefCell<String>>,
}

pub fn run(_url: &str) -> Result<(), String> {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        if let Some(display) = gtk::gdk::Display::default() {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(
                ".cover { border-radius: 6px; }
                 .mono { font-family: \"Noto Sans Mono\"; font-size: 11px; }
                 .status-ready { color: #7ed6a5; }
                 .status-planned { color: #d9b36c; }
                 .status-keys_missing { color: #e08a80; }
                 .status-disabled { color: #8a94a2; }
                 .steam-online { color: #7ed6a5; }",
            );
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
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

fn build(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("ExxaTools")
        .default_width(1240)
        .default_height(820)
        .build();

    let (sender, receiver) = std::sync::mpsc::channel::<Msg>();
    let country = std::rc::Rc::new(std::cell::RefCell::new(
        state::load()
            .map(|value| value.settings.country)
            .unwrap_or_else(|_| "us".into()),
    ));

    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.set_hexpand(true);

    let library_title = gtk::Label::new(Some("Library"));
    library_title.add_css_class("title-1");
    library_title.set_halign(gtk::Align::Start);
    let library_sub = gtk::Label::new(Some(""));
    library_sub.add_css_class("dim-label");
    library_sub.set_halign(gtk::Align::Start);
    let library_list = gtk::ListBox::new();
    library_list.add_css_class("boxed-list");
    library_list.set_selection_mode(gtk::SelectionMode::None);
    library_list.set_margin_top(6);
    library_list.set_margin_bottom(18);
    library_list.set_margin_start(18);
    library_list.set_margin_end(18);
    let library_header = GtkBox::new(Orientation::Vertical, 2);
    library_header.set_margin_top(18);
    library_header.set_margin_start(18);
    library_header.set_margin_end(18);
    library_header.append(&library_title);
    library_header.append(&library_sub);
    let library_page = GtkBox::new(Orientation::Vertical, 0);
    library_page.append(&library_header);
    library_page.append(&library_list);
    let library_scroll = gtk::ScrolledWindow::new();
    library_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    library_scroll.set_child(Some(&library_page));
    stack.add_named(&library_scroll, Some("library"));

    let discover = gtk::FlowBox::new();
    discover.set_column_spacing(14);
    discover.set_row_spacing(14);
    discover.set_selection_mode(gtk::SelectionMode::None);
    discover.set_valign(gtk::Align::Start);
    discover.set_margin_top(18);
    discover.set_margin_bottom(18);
    discover.set_margin_start(18);
    discover.set_margin_end(18);
    let discover_scroll = gtk::ScrolledWindow::new();
    discover_scroll.set_child(Some(&discover));
    discover_scroll.set_vexpand(true);
    let discover_page = GtkBox::new(Orientation::Vertical, 0);
    let discover_title = gtk::Label::new(Some("Discover"));
    discover_title.add_css_class("title-1");
    discover_title.set_halign(gtk::Align::Start);
    discover_title.set_margin_top(18);
    discover_title.set_margin_start(18);
    let discover_sub = gtk::Label::new(Some("Popular on Steam right now"));
    discover_sub.add_css_class("dim-label");
    discover_sub.set_halign(gtk::Align::Start);
    discover_sub.set_margin_start(18);
    discover_page.append(&discover_title);
    discover_page.append(&discover_sub);
    discover_page.append(&discover_scroll);
    stack.add_named(&discover_page, Some("discover"));

    let doctor_list = gtk::ListBox::new();
    doctor_list.add_css_class("boxed-list");
    doctor_list.set_selection_mode(gtk::SelectionMode::None);
    doctor_list.set_margin_top(6);
    doctor_list.set_margin_bottom(18);
    doctor_list.set_margin_start(18);
    doctor_list.set_margin_end(18);
    let doctor_page = GtkBox::new(Orientation::Vertical, 0);
    let doctor_head = GtkBox::new(Orientation::Horizontal, 8);
    doctor_head.set_margin_top(18);
    doctor_head.set_margin_start(18);
    doctor_head.set_margin_end(18);
    let doctor_title = gtk::Label::new(Some("System"));
    doctor_title.add_css_class("title-1");
    doctor_title.set_halign(gtk::Align::Start);
    doctor_title.set_hexpand(true);
    let copy = gtk::Button::with_label("Copy report");
    copy.add_css_class("flat");
    doctor_head.append(&doctor_title);
    doctor_head.append(&copy);
    doctor_page.append(&doctor_head);
    doctor_page.append(&doctor_list);
    let doctor_scroll = gtk::ScrolledWindow::new();
    doctor_scroll.set_child(Some(&doctor_page));
    stack.add_named(&doctor_scroll, Some("system"));

    let settings_page = build_settings(&window, &sender);
    stack.add_named(&settings_page, Some("settings"));

    let toast = adw::ToastOverlay::new();

    let sidebar_list = gtk::ListBox::new();
    sidebar_list.add_css_class("navigation-sidebar");
    sidebar_list.set_selection_mode(gtk::SelectionMode::Single);
    for (id, title) in [
        ("library", "Library"),
        ("discover", "Discover"),
        ("system", "System"),
        ("settings", "Settings"),
    ] {
        let row = gtk::ListBoxRow::new();
        let label = gtk::Label::new(Some(title));
        label.set_margin_top(8);
        label.set_margin_bottom(8);
        label.set_margin_start(8);
        label.set_margin_end(8);
        label.set_halign(gtk::Align::Start);
        row.set_child(Some(&label));
        unsafe { row.set_data("page", id.to_string()) };
        sidebar_list.append(&row);
    }

    let brand = gtk::Label::new(Some("ExxaTools"));
    brand.add_css_class("title-2");
    brand.set_halign(gtk::Align::Start);
    brand.set_margin_start(16);
    brand.set_margin_top(16);
    let tagline = gtk::Label::new(Some("Steam depot console"));
    tagline.add_css_class("dim-label");
    tagline.set_halign(gtk::Align::Start);
    tagline.set_margin_start(16);
    let steam = gtk::Label::new(Some(if crate::steamctl::is_running() {
        "Steam connected"
    } else {
        "Steam offline"
    }));
    steam.add_css_class("dim-label");
    if crate::steamctl::is_running() {
        steam.add_css_class("steam-online");
    }
    steam.set_halign(gtk::Align::Start);
    steam.set_margin_start(16);
    steam.set_margin_bottom(6);
    let version = gtk::Label::new(Some(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    version.add_css_class("dim-label");
    version.set_halign(gtk::Align::Start);
    version.set_margin_start(16);
    version.set_margin_bottom(16);

    let sidebar_box = GtkBox::new(Orientation::Vertical, 10);
    sidebar_box.append(&brand);
    sidebar_box.append(&tagline);
    sidebar_box.append(&sidebar_list);
    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    sidebar_box.append(&spacer);
    sidebar_box.append(&steam);
    sidebar_box.append(&version);
    sidebar_box.set_width_request(210);

    let sidebar_scroll = gtk::ScrolledWindow::new();
    sidebar_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sidebar_scroll.set_child(Some(&sidebar_box));
    let sidebar_toolbar = adw::ToolbarView::new();
    sidebar_toolbar.set_content(Some(&sidebar_scroll));
    let sidebar_page = adw::NavigationPage::builder()
        .title("ExxaTools")
        .child(&sidebar_toolbar)
        .build();

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Find a game — name or app id"));
    search.set_hexpand(true);
    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some("Refresh library and system checks"));
    let content_header = adw::HeaderBar::new();
    content_header.set_title_widget(Some(&search));
    content_header.pack_end(&refresh);
    let content_toolbar = adw::ToolbarView::new();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.set_content(Some(&stack));
    let content_page = adw::NavigationPage::builder()
        .title("ExxaTools")
        .child(&content_toolbar)
        .build();

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&sidebar_page));
    split.set_content(Some(&content_page));
    split.set_min_sidebar_width(210.0);
    split.set_max_sidebar_width(260.0);

    toast.set_child(Some(&split));
    window.set_content(Some(&toast));

    let ui = std::rc::Rc::new(Ui {
        window: window.clone(),
        toast: toast.clone(),
        stack: stack.clone(),
        library_list: library_list.clone(),
        library_title: library_title.clone(),
        library_sub: library_sub.clone(),
        discover: discover.clone(),
        doctor_list: doctor_list.clone(),
        sender: sender.clone(),
        country: country.clone(),
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
                    Msg::Info(text) => toast_text(&ui, &text),
                    Msg::Error(text) => {
                        toast_text(&ui, &text);
                        ui.window.present();
                    }
                }
            }
            gtk::glib::ControlFlow::Continue
        });
    }
    {
        let ui = ui.clone();
        let sender = ui.sender.clone();
        let country = ui.country.clone();
        sidebar_list.connect_row_selected(move |_, row| {
            let Some(row) = row else { return };
            let pages = ["library", "discover", "system", "settings"];
            let index = row.index();
            if index < 0 {
                return;
            }
            let Some(page) = pages.get(index as usize) else { return };
            ui.stack.set_visible_child_name(page);
            match *page {
                "discover" => {
                    let sender = sender.clone();
                    let country = country.borrow().clone();
                    std::thread::spawn(move || {
                        let mut items = crate::steam::discover(&country, 24);
                        attach_covers(&mut items);
                        sender.send(Msg::Discover(items)).ok();
                    });
                }
                "system" => load_doctor(sender.clone()),
                _ => {}
            }
        });
    }
    {
        let ui = ui.clone();
        search.connect_activate(move |entry| {
            let term = entry.text().trim().to_string();
            if term.is_empty() {
                return;
            }
            let sender = ui.sender.clone();
            let country = ui.country.borrow().clone();
            ui.stack.set_visible_child_name("library");
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
        let ui = ui.clone();
        let sender = ui.sender.clone();
        refresh.connect_clicked(move |_| {
            load_library(sender.clone());
            load_doctor(sender.clone());
        });
    }
    {
        let sender = sender.clone();
        sidebar_list.select_row(sidebar_list.row_at_index(0).as_ref());
        load_library(sender.clone());
        load_doctor(sender);
    }

    window.present();
}

fn toast_text(ui: &Ui, text: &str) {
    ui.toast.add_toast(adw::Toast::new(text));
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn image_at(path: &str, width: i32, height: i32) -> Option<gtk::Picture> {
    let picture = gtk::Picture::for_filename(path);
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(width, height);
    picture.add_css_class("cover");
    Some(picture)
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

fn render_library(ui: &Ui, items: Vec<Value>, from_search: bool) {
    clear_list(&ui.library_list);
    ui.library_title.set_text(if from_search { "Search results" } else { "Library" });
    let ready = items
        .iter()
        .filter(|item| item.get("state").and_then(Value::as_str) == Some("ready"))
        .count();
    let summary = if items.is_empty() {
        "No games yet".to_string()
    } else if from_search {
        format!("{} found", items.len())
    } else {
        format!("{} {} · {} ready", items.len(), if items.len() == 1 { "game" } else { "games" }, ready)
    };
    ui.library_sub.set_text(&summary);

    if items.is_empty() {
        let status = adw::StatusPage::builder()
            .icon_name("folder-open-symbolic")
            .title(if from_search { "Nothing found" } else { "Your library is empty" })
            .description(if from_search {
                "Try another name or a numeric app id."
            } else {
                "Search for a game above, then press Add."
            })
            .build();
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.set_child(Some(&status));
        ui.library_list.append(&row);
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
        let depots = item
            .get("depot_count")
            .and_then(Value::as_u64)
            .map(|count| format!("{count} depots"))
            .unwrap_or_else(|| "no depots".into());
        let row = adw::ActionRow::builder()
            .title(&name)
            .subtitle(format!(
                "{} · {} branch",
                depots,
                item.get("branch").and_then(Value::as_str).unwrap_or("public")
            ))
            .build();
        if let Some(cover) = item
            .get("cover")
            .and_then(Value::as_str)
            .and_then(|path| image_at(path, 96, 45))
        {
            row.add_prefix(&cover);
        }
        let status = item.get("state").and_then(Value::as_str).unwrap_or("planned");
        let state_label = gtk::Label::new(Some(status));
        state_label.add_css_class("mono");
        state_label.add_css_class(&format!("status-{status}"));
        row.add_suffix(&state_label);

        let installed = item.get("installed").and_then(Value::as_bool).unwrap_or(false);
        if from_search || !installed {
            let add = gtk::Button::with_label("Add");
            add.add_css_class("suggested-action");
            let sender = ui.sender.clone();
            let name_hint = name.clone();
            add.connect_clicked(move |_| {
                let sender = sender.clone();
                let hint = name_hint.clone();
                std::thread::spawn(move || match crate::plans::fetch_plan(appid, Some(&hint)) {
                    Ok(_) => {
                        sender.send(Msg::Info(format!("{hint} added"))).ok();
                        load_library(sender.clone());
                    }
                    Err(error) => {
                        sender.send(Msg::Error(error)).ok();
                    }
                });
            });
            row.add_suffix(&add);
        } else {
            let details = gtk::Button::with_label("Details");
            details.add_css_class("flat");
            details.set_valign(gtk::Align::Center);
            let sender = ui.sender.clone();
            let window = ui.window.clone();
            let detail_name = name.clone();
            details.connect_clicked(move |_| {
                open_details(&window, appid, &detail_name, sender.clone());
            });
            row.add_suffix(&details);
            let launch = gtk::Button::with_label("Launch");
            launch.add_css_class("suggested-action");
            launch.set_valign(gtk::Align::Center);
            launch.connect_clicked(move |_| {
                let _ = std::process::Command::new("steam")
                    .arg(format!("steam://rungameid/{appid}"))
                    .spawn();
            });
            row.add_suffix(&launch);
        }

        row.set_activatable(false);
        row.set_selectable(false);
        ui.library_list.append(&row);
    }
}

fn render_discover(ui: &Ui, items: Vec<Value>) {
    while let Some(child) = ui.discover.first_child() {
        ui.discover.remove(&child);
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
        tile.add_css_class("flat");
        let inner = GtkBox::new(Orientation::Vertical, 6);
        if let Some(cover) = item
            .get("cover")
            .and_then(Value::as_str)
            .and_then(|path| image_at(path, 210, 98))
        {
            inner.append(&cover);
        }
        let caption = gtk::Label::new(Some(&name));
        caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
        caption.set_max_width_chars(26);
        caption.set_halign(gtk::Align::Start);
        inner.append(&caption);
        tile.set_child(Some(&inner));
        let ui_window = ui.window.clone();
        let sender = ui.sender.clone();
        let hint = name.clone();
        tile.connect_clicked(move |_| {
            let dialog = adw::MessageDialog::new(
                Some(&ui_window),
                Some("Add to library"),
                Some(&format!("Add {hint}?")),
            );
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("add", "Add");
            dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
            let sender = sender.clone();
            let hint = hint.clone();
            dialog.connect_response(None, move |dialog, response| {
                dialog.close();
                if response != "add" {
                    return;
                }
                let sender = sender.clone();
                let hint = hint.clone();
                std::thread::spawn(move || match crate::plans::fetch_plan(appid, Some(&hint)) {
                    Ok(_) => {
                        sender.send(Msg::Info(format!("{hint} added"))).ok();
                        load_library(sender.clone());
                    }
                    Err(error) => {
                        sender.send(Msg::Error(error)).ok();
                    }
                });
            });
            dialog.present();
        });
        ui.discover.append(&tile);
    }
}

fn render_doctor(ui: &Ui, value: Value) {
    clear_list(&ui.doctor_list);
    let items = value
        .get("checks")
        .or_else(|| value.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for item in items {
        let status = item.get("status").and_then(Value::as_str).unwrap_or("warn");
        let icon_name = match status {
            "ok" => "emblem-ok-symbolic",
            "bad" => "dialog-error-symbolic",
            _ => "dialog-warning-symbolic",
        };
        let row = adw::ActionRow::builder()
            .title(item.get("label").and_then(Value::as_str).unwrap_or(""))
            .subtitle(item.get("detail").and_then(Value::as_str).unwrap_or(""))
            .build();
        let icon = gtk::Image::from_icon_name(icon_name);
        row.add_prefix(&icon);
        if let Some(hint) = item.get("hint").and_then(Value::as_str) {
            if !hint.is_empty() {
                let label = gtk::Label::new(Some(hint));
                label.add_css_class("dim-label");
                row.add_suffix(&label);
            }
        }
        row.set_activatable(false);
        row.set_selectable(false);
        ui.doctor_list.append(&row);
    }
}

fn build_settings(
    window: &adw::ApplicationWindow,
    sender: &std::sync::mpsc::Sender<Msg>,
) -> adw::PreferencesPage {
    let loaded = state::load().unwrap_or_default();
    let page = adw::PreferencesPage::new();

    let region = adw::PreferencesGroup::builder().title("Store &amp; region").build();
    let country = adw::EntryRow::builder().title("Store country").build();
    country.set_text(&loaded.settings.country);
    region.add(&country);
    page.add(&region);

    let sources = adw::PreferencesGroup::builder().title("Sources &amp; API keys").build();
    let key = adw::EntryRow::builder().title("Hubcap key").build();
    key.set_text(&loaded.settings.hubcap_key);
    sources.add(&key);
    let ryuu = adw::EntryRow::builder().title("Ryuu key").build();
    ryuu.set_text(&loaded.settings.ryuu_key);
    sources.add(&ryuu);
    let source = adw::EntryRow::builder().title("Default source").build();
    source.set_text(&loaded.settings.default_source);
    sources.add(&source);
    page.add(&sources);

    let save = gtk::Button::with_label("Save settings");
    save.add_css_class("suggested-action");
    save.set_valign(gtk::Align::Center);
    let save_row = adw::ActionRow::builder()
        .title("Apply changes")
        .subtitle("Writes ~/.config/exxatools/state.json")
        .build();
    save_row.add_suffix(&save);
    let save_group = adw::PreferencesGroup::new();
    save_group.add(&save_row);
    page.add(&save_group);

    let window = window.clone();
    let sender = sender.clone();
    save.connect_clicked(move |_| {
        let result = state::update(|state| {
            state.settings.country = country.text().to_string().trim().to_lowercase();
            state.settings.hubcap_key = key.text().to_string().trim().to_string();
            state.settings.ryuu_key = ryuu.text().to_string().trim().to_string();
            state.settings.default_source = source.text().to_string().trim().to_string();
            Ok(())
        });
        match result {
            Ok(()) => {
                sender.send(Msg::Info("Settings saved".into())).ok();
            }
            Err(error) => {
                sender.send(Msg::Error(error)).ok();
                let _ = &window;
            }
        }
    });
    page
}

fn open_details(
    window: &adw::ApplicationWindow,
    appid: u64,
    name: &str,
    sender: std::sync::mpsc::Sender<Msg>,
) {
    let dialog = adw::MessageDialog::new(
        Some(window),
        Some(&format!("{name} — {appid}")),
        None,
    );
    dialog.add_response("close", "Close");
    let loaded = state::load().ok();
    let record = loaded
        .as_ref()
        .and_then(|state| state.apps.get(&appid.to_string()))
        .cloned();
    let body = GtkBox::new(Orientation::Vertical, 6);
    if let Some(record) = record {
        body.append(&gtk::Label::new(Some(&format!(
            "Branch {} · {} depots",
            record.get("branch").and_then(Value::as_str).unwrap_or("public"),
            record.get("depots").and_then(Value::as_array).map(|list| list.len()).unwrap_or(0)
        ))));
        for depot in record.get("depots").and_then(Value::as_array).cloned().unwrap_or_default() {
            let id = depot.get("depot_id").and_then(Value::as_u64).unwrap_or(0);
            let gid = depot.get("gid").and_then(Value::as_str).unwrap_or("");
            let line = gtk::Label::new(Some(&format!("{id} · {gid}")));
            line.add_css_class("mono");
            line.set_halign(gtk::Align::Start);
            body.append(&line);
        }
    }
    let actions = GtkBox::new(Orientation::Horizontal, 8);
    let sync = gtk::Button::with_label("Sync manifests");
    let sender_sync = sender.clone();
    let sync_name = name.to_string();
    sync.connect_clicked(move |_| {
        match crate::plans::refresh_plan(appid) {
            Ok(_) => sender_sync.send(Msg::Info(format!("{sync_name} synced"))).ok(),
            Err(error) => sender_sync.send(Msg::Error(error)).ok(),
        };
    });
    actions.append(&sync);
    let remove = gtk::Button::with_label("Remove");
    remove.add_css_class("destructive-action");
    let sender_remove = sender.clone();
    let remove_name = name.to_string();
    remove.connect_clicked(move |_| {
        let _ = crate::vfsys::remove_script(appid);
        let result = state::update(|state| {
            state.apps.remove(&appid.to_string());
            Ok(())
        });
        if result.is_ok() {
            sender_remove.send(Msg::Info(format!("{remove_name} removed"))).ok();
            load_library(sender_remove.clone());
        } else if let Err(error) = result {
            sender_remove.send(Msg::Error(error)).ok();
        }
    });
    actions.append(&remove);
    body.append(&actions);
    dialog.set_extra_child(Some(&body));
    dialog.connect_response(None, |dialog, _| dialog.close());
    dialog.present();
}