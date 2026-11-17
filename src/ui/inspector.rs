//! The right pane: the selected game, its one primary action, then the evidence.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::model::{Game, GameState, Origin};
use crate::ui::shell::Ctx;
use crate::ui::worker::Command;

pub struct Inspector {
    pub root: gtk::Widget,
    body: gtk::Box,
}

pub fn build() -> Inspector {
    let body = GtkBox::new(Orientation::Vertical, 0);
    body.set_margin_top(20);
    body.set_margin_bottom(28);
    body.set_margin_start(24);
    body.set_margin_end(24);
    body.set_hexpand(true);
    body.set_vexpand(true);

    let clamp = adw::Clamp::builder().maximum_size(780).build();
    clamp.set_vexpand(true);
    clamp.set_child(Some(&body));

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_overlay_scrolling(false);
    scroll.set_child(Some(&clamp));

    let inspector = Inspector {
        root: scroll.upcast(),
        body,
    };
    inspector.show_empty(None);
    inspector
}

impl Inspector {
    pub fn show_empty(&self, ctx: Option<&Ctx>) {
        clear(&self.body);
        let page = adw::StatusPage::builder()
            .icon_name("applications-games-symbolic")
            .title("No game selected")
            .description("Choose a title from the shelf, or browse Discover to add a game.")
            .vexpand(true)
            .build();
        if let Some(ctx) = ctx {
            let discover = gtk::Button::with_label("Browse Discover");
            discover.add_css_class("suggested-action");
            discover.set_halign(gtk::Align::Center);
            let ctx = ctx.clone();
            discover.connect_clicked(move |_| ctx.navigate(crate::ui::shell::Page::Discover));
            page.set_child(Some(&discover));
        }
        self.body.append(&page);
    }

    pub fn show(&self, ctx: &Ctx, game: Option<&Game>) {
        let Some(game) = game else {
            self.show_empty(Some(ctx));
            return;
        };
        clear(&self.body);

        let can_launch =
            game.origin != Origin::Search && game.installed && game.state != GameState::KeysMissing;
        let actions = GtkBox::new(Orientation::Horizontal, 8);
        actions.add_css_class("hero-actions");
        actions.set_margin_top(14);
        actions.append(&primary_button(ctx, game));

        if game.origin != Origin::Search {
            if can_launch {
                let sync = action_button("Sync manifests");
                let commands = ctx.commands.clone();
                let appid = game.appid;
                sync.connect_clicked(move |_| commands.send(Command::Sync(appid)));
                actions.append(&sync);
            }

            let fix = action_button("Fix update loop");
            fix.set_tooltip_text(Some("Repair appmanifest flags and restart Steam"));
            let commands = ctx.commands.clone();
            let appid = game.appid;
            fix.connect_clicked(move |_| commands.send(Command::Fix(appid)));
            actions.append(&fix);
        }

        let menu_spacer = GtkBox::new(Orientation::Horizontal, 0);
        menu_spacer.set_hexpand(true);
        actions.append(&menu_spacer);

        let more = gtk::MenuButton::new();
        more.set_icon_name("view-more-symbolic");
        more.set_cursor_from_name(Some("pointer"));
        more.add_css_class("flat");
        more.set_tooltip_text(Some("More actions"));
        let popover = gtk::Popover::new();
        let menu = GtkBox::new(Orientation::Vertical, 2);
        menu.set_margin_top(5);
        menu.set_margin_bottom(5);
        menu.set_margin_start(5);
        menu.set_margin_end(5);

        let copy = menu_button("Copy app id", "edit-copy-symbolic");
        {
            let window = ctx.window.clone();
            let toast = ctx.toast.clone();
            let appid = game.appid;
            let popover = popover.clone();
            copy.connect_clicked(move |_| {
                window.clipboard().set_text(&appid.to_string());
                toast.add_toast(adw::Toast::new("App ID copied"));
                popover.popdown();
            });
        }
        menu.append(&copy);

        let store = menu_button("Open Steam page", "web-browser-symbolic");
        {
            let appid = game.appid;
            let popover = popover.clone();
            store.connect_clicked(move |_| {
                let _ = std::process::Command::new("xdg-open")
                    .arg(format!("https://store.steampowered.com/app/{appid}/"))
                    .spawn();
                popover.popdown();
            });
        }
        menu.append(&store);

        if game.origin != Origin::Search {
            menu.append(&gtk::Separator::new(Orientation::Horizontal));
            let remove = menu_button("Remove from library", "user-trash-symbolic");
            remove.add_css_class("destructive-action");
            let ctx = ctx.clone();
            let appid = game.appid;
            let name = game.name.clone();
            let popover = popover.clone();
            remove.connect_clicked(move |_| {
                popover.popdown();
                confirm_remove(&ctx, appid, &name);
            });
            menu.append(&remove);
        }

        popover.set_child(Some(&menu));
        more.set_popover(Some(&popover));
        actions.append(&more);

        self.body.append(&hero(game, &actions));

        if game.origin == Origin::Search {
            let note = gtk::Label::new(Some(
                "Found on Steam. Add it to build a plan and pin manifests.",
            ));
            note.add_css_class("dim-label");
            note.set_halign(gtk::Align::Start);
            note.set_margin_top(8);
            note.set_margin_bottom(4);
            self.body.append(&note);
        }

        if !game.depots.is_empty() {
            self.body.append(&depots_section(ctx, game));
        }

        if game.origin != Origin::Search {
            self.body.append(&plan_group(game));
        }
    }
}

fn primary_button(ctx: &Ctx, game: &Game) -> gtk::Button {
    if game.origin == Origin::Search {
        let add = gtk::Button::with_label("Add to library");
        add.add_css_class("suggested-action");
        add.set_cursor_from_name(Some("pointer"));
        let commands = ctx.commands.clone();
        let appid = game.appid;
        let name = game.name.clone();
        add.connect_clicked(move |_| {
            commands.send(Command::Add {
                appid,
                name: name.clone(),
            });
        });
        return add;
    }
    if game.installed && game.state != GameState::KeysMissing {
        let launch = gtk::Button::with_label("Launch");
        launch.add_css_class("suggested-action");
        launch.set_cursor_from_name(Some("pointer"));
        let commands = ctx.commands.clone();
        let appid = game.appid;
        launch.connect_clicked(move |_| commands.send(Command::Launch(appid)));
        return launch;
    }
    let sync = gtk::Button::with_label("Sync manifests");
    sync.add_css_class("suggested-action");
    sync.set_cursor_from_name(Some("pointer"));
    let commands = ctx.commands.clone();
    let appid = game.appid;
    sync.connect_clicked(move |_| commands.send(Command::Sync(appid)));
    sync
}

fn action_button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("flat");
    button.set_cursor_from_name(Some("pointer"));
    button
}

fn hero(game: &Game, actions: &GtkBox) -> gtk::Widget {
    let art = game.hero.as_ref().or(game.cover.as_ref());
    let base: gtk::Widget = match art {
        Some(path) => {
            let picture = gtk::Picture::for_filename(path);
            picture.set_content_fit(gtk::ContentFit::Cover);
            picture.set_size_request(-1, 260);
            picture.set_hexpand(true);
            picture.add_css_class("hero");
            picture.upcast()
        }
        None => {
            let letter = gtk::Label::new(Some(&game.initial()));
            letter.add_css_class("hero-letter");
            letter.set_halign(gtk::Align::Center);
            letter.set_valign(gtk::Align::Center);
            let fallback = gtk::Overlay::new();
            fallback.add_css_class("hero-fallback");
            fallback.set_size_request(-1, 260);
            fallback.set_hexpand(true);
            fallback.set_child(Some(&letter));
            fallback.upcast()
        }
    };

    let scrim = GtkBox::new(Orientation::Vertical, 0);
    scrim.add_css_class("hero-scrim");
    scrim.set_vexpand(true);
    scrim.set_hexpand(true);

    let pill = gtk::Label::new(Some(game.state.label()));
    pill.add_css_class("status-pill");
    pill.add_css_class(game.state.pill_class());
    let top = GtkBox::new(Orientation::Horizontal, 8);
    top.set_margin_bottom(8);
    top.append(&pill);
    if game.dlc > 0 {
        let dlc = gtk::Label::new(Some(&format!("{} DLC", game.dlc)));
        dlc.add_css_class("status-pill");
        dlc.add_css_class("pill-neutral");
        top.append(&dlc);
    }

    let title = gtk::Label::new(Some(&game.name));
    title.add_css_class("hero-title");
    title.set_halign(gtk::Align::Start);
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title.set_max_width_chars(42);
    title.set_lines(2);

    let meta = gtk::Label::new(Some(&game.meta()));
    meta.add_css_class("hero-meta");
    meta.set_halign(gtk::Align::Start);
    meta.set_xalign(0.0);
    meta.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.add_css_class("hero-panel");
    content.set_valign(gtk::Align::End);
    content.set_hexpand(true);
    content.set_margin_start(22);
    content.set_margin_end(22);
    content.set_margin_bottom(18);
    content.append(&top);
    content.append(&title);
    content.append(&meta);
    content.append(actions);

    let overlay = gtk::Overlay::new();
    overlay.add_css_class("hero-wrap");
    overlay.set_size_request(-1, 260);
    overlay.set_hexpand(true);
    overlay.set_child(Some(&base));
    overlay.add_overlay(&scrim);
    overlay.add_overlay(&content);
    overlay.upcast()
}

fn depots_section(ctx: &Ctx, game: &Game) -> gtk::Widget {
    let loaded = game.depots.iter().filter(|depot| depot.has_key).count();
    let title = gtk::Label::new(Some("Depots"));
    title.add_css_class("section-title");
    title.set_halign(gtk::Align::Start);
    title.set_margin_top(20);

    let note = gtk::Label::new(Some(&format!(
        "{loaded} of {} keys loaded. Select an ID to copy; missing manifests are supplied at launch.",
        game.depots.len()
    )));
    note.add_css_class("section-note");
    note.add_css_class("dim-label");
    note.set_halign(gtk::Align::Start);
    note.set_wrap(true);
    note.set_xalign(0.0);
    note.set_margin_top(2);
    note.set_margin_bottom(8);

    let show_size = game.depots.iter().any(|depot| depot.size.is_some());
    let show_key = game.depots.iter().any(|depot| !depot.has_key);
    let columns = 2 + i32::from(show_size) + i32::from(show_key);
    let table = gtk::Grid::new();
    table.add_css_class("depot-table");
    table.set_column_spacing(12);
    table.set_row_spacing(4);
    table.set_margin_top(4);
    table.set_margin_bottom(4);
    table.set_margin_start(10);
    table.set_margin_end(10);
    table.set_hexpand(true);

    table.attach(&depot_header("Depot ID", gtk::Align::Start), 0, 0, 1, 1);
    let manifest_header = depot_header("Manifest ID", gtk::Align::Start);
    manifest_header.set_hexpand(true);
    table.attach(&manifest_header, 1, 0, 1, 1);
    if show_size {
        table.attach(&depot_header("Size", gtk::Align::End), 2, 0, 1, 1);
    }
    if show_key {
        table.attach(&depot_header("Key", gtk::Align::End), columns - 1, 0, 1, 1);
    }

    for (index, depot) in game.depots.iter().enumerate() {
        let separator_row = index as i32 * 2 + 1;
        let content_row = separator_row + 1;
        let separator = gtk::Separator::new(Orientation::Horizontal);
        table.attach(&separator, 0, separator_row, columns, 1);

        let depot_id = copy_id_button(ctx, "Depot", &depot.id.to_string(), 100);
        table.attach(&depot_id, 0, content_row, 1, 1);

        let manifest: gtk::Widget = if depot.gid.is_empty() {
            let provided = gtk::Label::new(Some("At launch"));
            provided.add_css_class("caption");
            provided.add_css_class("dim-label");
            provided.set_halign(gtk::Align::Start);
            provided.upcast()
        } else {
            copy_id_button(ctx, "Manifest", &depot.gid, 176).upcast()
        };
        table.attach(&manifest, 1, content_row, 1, 1);

        if show_size {
            let size = depot
                .size
                .map(crate::ui::model::human_size)
                .unwrap_or_else(|| "—".into());
            let label = depot_value(&size);
            table.attach(&label, 2, content_row, 1, 1);
        }
        if show_key {
            let key = key_status(depot.has_key);
            table.attach(&key, columns - 1, content_row, 1, 1);
        }
    }

    let section = GtkBox::new(Orientation::Vertical, 0);
    section.append(&title);
    section.append(&note);
    section.append(&table);
    section.upcast()
}

fn depot_header(text: &str, align: gtk::Align) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("caption");
    label.add_css_class("dim-label");
    label.set_halign(align);
    label.set_xalign(if align == gtk::Align::End { 1.0 } else { 0.0 });
    label
}

fn depot_value(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("caption");
    label.set_halign(gtk::Align::End);
    label.set_xalign(1.0);
    label
}

fn key_status(loaded: bool) -> gtk::Label {
    let label = gtk::Label::new(Some(if loaded { "Loaded" } else { "Missing" }));
    label.add_css_class("status-pill");
    label.add_css_class(if loaded { "pill-ready" } else { "pill-warning" });
    label.set_halign(gtk::Align::End);
    label
}

fn copy_id_button(ctx: &Ctx, kind: &str, value: &str, width: i32) -> gtk::Button {
    let button = gtk::Button::with_label(value);
    button.add_css_class("flat");
    button.add_css_class("mono");
    button.set_size_request(width, -1);
    button.set_halign(gtk::Align::Start);
    button.set_cursor_from_name(Some("pointer"));
    button.set_tooltip_text(Some(&format!("Copy {kind} ID {value}")));

    let clipboard = ctx.window.clipboard();
    let copied_value = value.to_string();
    let original_value = value.to_string();
    let original_tooltip = format!("Copy {kind} ID {value}");
    let copied_tooltip = format!("{kind} ID copied");
    let revision = Rc::new(Cell::new(0_u64));
    button.connect_clicked(move |button| {
        clipboard.set_text(&copied_value);
        button.set_label("Copied");
        button.set_tooltip_text(Some(&copied_tooltip));

        let next_revision = revision.get() + 1;
        revision.set(next_revision);
        let revision = revision.clone();
        let button = button.downgrade();
        let original_value = original_value.clone();
        let original_tooltip = original_tooltip.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(1_400), move || {
            if revision.get() != next_revision {
                return;
            }
            if let Some(button) = button.upgrade() {
                button.set_label(&original_value);
                button.set_tooltip_text(Some(&original_tooltip));
            }
        });
    });
    button
}

fn plan_group(game: &Game) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Plan").build();
    group.set_margin_top(20);
    let source = if game.source.is_empty() {
        "local".to_string()
    } else {
        game.source.clone()
    };
    let expander = adw::ExpanderRow::builder()
        .title("Plan details")
        .subtitle(format!("{} · {}", source, game.branch))
        .build();
    expander.set_cursor_from_name(Some("pointer"));
    expander.add_row(&plan_row("Source", &source));
    expander.add_row(&plan_row("Branch", &game.branch));
    let pinned = game
        .depots
        .iter()
        .filter(|depot| !depot.gid.is_empty())
        .count();
    expander.add_row(&plan_row(
        "Manifests",
        &if pinned == 0 {
            "Supplied by the provider at launch".to_string()
        } else {
            format!("{pinned} pinned")
        },
    ));
    expander.add_row(&plan_row(
        "Last refresh",
        &game
            .updated
            .map(crate::ui::model::relative_time)
            .unwrap_or_else(|| "not yet refreshed".into()),
    ));
    group.add(&expander);
    group
}

fn plan_row(title: &str, value: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(value)
        .build();
    row.set_subtitle_selectable(true);
    row
}

fn menu_button(label: &str, icon: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("flat");
    button.set_cursor_from_name(Some("pointer"));
    button.set_halign(gtk::Align::Start);
    let image = gtk::Image::from_icon_name(icon);
    button.set_child(Some(&row_with_icon(&image, label)));
    button
}

fn row_with_icon(image: &gtk::Image, label: &str) -> GtkBox {
    let text = gtk::Label::new(Some(label));
    text.set_halign(gtk::Align::Start);
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.append(image);
    row.append(&text);
    row
}

fn confirm_remove(ctx: &Ctx, appid: u64, name: &str) {
    let dialog = adw::MessageDialog::new(
        Some(&ctx.window),
        Some("Remove from library?"),
        Some(&format!(
            "{name} will be removed from ExxaTools. The game's installed files and Steam library are not touched."
        )),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("remove", "Remove");
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    let commands = ctx.commands.clone();
    dialog.connect_response(None, move |dialog, response| {
        dialog.close();
        if response == "remove" {
            commands.send(Command::Remove(appid));
        }
    });
    dialog.present();
}

fn clear(container: &GtkBox) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
