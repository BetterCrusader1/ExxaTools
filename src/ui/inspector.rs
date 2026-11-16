//! The right pane: the selected game, its one primary action, then the evidence.

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

    let clamp = adw::Clamp::builder().maximum_size(780).build();
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
            .title("Pick a game")
            .description(
                "Choose a title from the shelf to see its plan, manifests and launch action.",
            )
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

        let actions = GtkBox::new(Orientation::Horizontal, 8);
        actions.set_margin_top(14);
        actions.append(&primary_button(ctx, game));

        let syncing = game.origin != Origin::Search;
        if syncing {
            let sync = gtk::Button::with_label("Sync manifests");
            sync.add_css_class("flat");
            sync.set_cursor_from_name(Some("pointer"));
            let commands = ctx.commands.clone();
            let appid = game.appid;
            sync.connect_clicked(move |_| commands.send(Command::Sync(appid)));
            actions.append(&sync);

            let fix = gtk::Button::with_label("Fix update loop");
            fix.add_css_class("flat");
            fix.set_cursor_from_name(Some("pointer"));
            fix.set_tooltip_text(Some("Repair appmanifest flags and restart Steam"));
            let commands = ctx.commands.clone();
            let appid = game.appid;
            fix.connect_clicked(move |_| commands.send(Command::Fix(appid)));
            actions.append(&fix);
        }

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
            let appid = game.appid;
            let popover = popover.clone();
            copy.connect_clicked(move |_| {
                window.clipboard().set_text(&appid.to_string());
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

        let menu_spacer = GtkBox::new(Orientation::Horizontal, 0);
        menu_spacer.set_hexpand(true);
        actions.append(&menu_spacer);

        self.body.append(&hero(game, &actions));

        if game.origin == Origin::Search {
            let note = gtk::Label::new(Some(
                "Found on Steam. Add it to build a plan and pin manifests.",
            ));
            note.add_css_class("dim-label");
            note.set_halign(gtk::Align::Start);
            note.set_margin_top(4);
            note.set_margin_bottom(4);
            self.body.append(&note);
        }

        if !game.depots.is_empty() {
            self.body.append(&depots_section(game));
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
    top.append(&pill);
    if game.dlc > 0 {
        let dlc = gtk::Label::new(Some(&format!("{} DLC", game.dlc)));
        dlc.add_css_class("hero-meta");
        top.append(&dlc);
    }

    let title = gtk::Label::new(Some(&game.name));
    title.add_css_class("hero-title");
    title.set_halign(gtk::Align::Start);
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title.set_max_width_chars(42);

    let meta = gtk::Label::new(Some(&game.meta()));
    meta.add_css_class("hero-meta");
    meta.set_halign(gtk::Align::Start);
    meta.set_xalign(0.0);

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

fn depots_section(game: &Game) -> gtk::Widget {
    let loaded = game.depots.iter().filter(|depot| depot.has_key).count();
    let title = gtk::Label::new(Some("Depots"));
    title.add_css_class("section-title");
    title.set_halign(gtk::Align::Start);
    title.set_margin_top(20);

    let note = gtk::Label::new(Some(&format!(
        "{loaded} of {} keys loaded · manifests without an id are supplied at launch",
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
    let table = GtkBox::new(Orientation::Vertical, 0);
    table.add_css_class("depot-table");
    let mut header = vec![("Depot", 64, 0.0, false), ("Manifest", 0, 0.0, true)];
    if show_size {
        header.push(("Size", 58, 1.0, false));
    }
    if show_key {
        header.push(("Key", 48, 1.0, false));
    }
    table.append(&depot_columns(&header));
    for depot in &game.depots {
        table.append(&gtk::Separator::new(Orientation::Horizontal));
        let manifest = if depot.gid.is_empty() {
            "at launch"
        } else {
            &depot.gid
        };
        let size = depot
            .size
            .map(crate::ui::model::human_size)
            .unwrap_or_default();
        let key = if depot.has_key { "" } else { "missing" };
        let id = depot.id.to_string();
        let mut cells = vec![(id.as_str(), 64, 0.0, false), (manifest, 0, 0.0, true)];
        if show_size {
            cells.push((size.as_str(), 58, 1.0, false));
        }
        if show_key {
            cells.push((key, 48, 1.0, false));
        }
        let row = depot_columns(&cells);
        row.add_css_class("depot-row");
        if !depot.has_key {
            row.add_css_class("depot-missing");
        }
        table.append(&row);
    }

    let section = GtkBox::new(Orientation::Vertical, 0);
    section.append(&title);
    section.append(&note);
    section.append(&table);
    section.upcast()
}

fn depot_columns(cells: &[(&str, i32, f32, bool)]) -> gtk::Grid {
    let grid = gtk::Grid::new();
    grid.set_column_spacing(12);
    grid.set_margin_top(3);
    grid.set_margin_bottom(3);
    grid.set_margin_start(10);
    grid.set_margin_end(10);
    for (index, (text, width, xalign, expand)) in cells.iter().enumerate() {
        let label = gtk::Label::new(Some(text));
        label.set_halign(if *xalign > 0.5 {
            gtk::Align::End
        } else {
            gtk::Align::Start
        });
        label.set_xalign(*xalign);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_max_width_chars(32);
        if *expand {
            label.set_hexpand(true);
        }
        if *width > 0 {
            label.set_size_request(*width, -1);
        }
        if index == 0 || index == 1 {
            label.add_css_class("mono");
        } else {
            label.add_css_class("caption");
        }
        if index == 3 {
            label.add_css_class("depot-key");
        }
        grid.attach(&label, index as i32, 0, 1, 1);
    }
    grid
}

fn plan_group(game: &Game) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Plan").build();
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
    expander.add_row(
        &adw::ActionRow::builder()
            .title("Source")
            .subtitle(source)
            .build(),
    );
    expander.add_row(
        &adw::ActionRow::builder()
            .title("Branch")
            .subtitle(game.branch.clone())
            .build(),
    );
    let pinned = game
        .depots
        .iter()
        .filter(|depot| !depot.gid.is_empty())
        .count();
    expander.add_row(
        &adw::ActionRow::builder()
            .title("Manifests")
            .subtitle(if pinned == 0 {
                "Supplied by the provider at launch".to_string()
            } else {
                format!("{pinned} pinned")
            })
            .build(),
    );
    expander.add_row(
        &adw::ActionRow::builder()
            .title("Last refresh")
            .subtitle(
                game.updated
                    .map(crate::ui::model::relative_time)
                    .unwrap_or_else(|| "not yet refreshed".into()),
            )
            .build(),
    );
    group.add(&expander);
    group
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
