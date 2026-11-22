//! The right pane: the selected game, its one primary action, then the evidence.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::art::{self, Art};
use crate::ui::model::{Game, GameState, Origin};
use crate::ui::shell::Ctx;
use crate::ui::worker::Command;

const HERO_HEIGHT: i32 = 300;
/// The technical names live here, one hover away from the plain labels.
const CHECK_TOOLTIP: &str = "Sync manifests: fetch the newest manifests and depot keys from your sources";
const REPAIR_TOOLTIP: &str = "Fix the update loop: clear Steam's update-required flags for this game and restart Steam";
/// Steam library heroes are up to 3840 px wide; the full-bleed hero rarely
/// shows more than this logical width, so decoding more only wastes memory.
const HERO_DECODE_WIDTH: i32 = 1280;
/// The title logo fits inside this logical box, keeping its aspect.
const LOGO_MAX_WIDTH: i32 = 380;
const LOGO_MAX_HEIGHT: i32 = 120;

pub struct Inspector {
    pub root: gtk::Widget,
    page: gtk::Box,
    hero: gtk::Box,
    body: gtk::Box,
    /// Narrow windows move the secondary actions into the overflow menu.
    compact: Cell<bool>,
    last: std::cell::RefCell<Option<(Ctx, Game)>>,
}

pub fn build() -> Inspector {
    // The hero and the details below share the pane's full width so their
    // left edges line up.
    let hero = GtkBox::new(Orientation::Vertical, 0);
    hero.set_margin_top(20);
    hero.set_margin_start(24);
    hero.set_margin_end(24);

    let body = GtkBox::new(Orientation::Vertical, 0);
    body.set_margin_top(16);
    body.set_margin_bottom(28);
    body.set_margin_start(24);
    body.set_margin_end(24);
    body.set_hexpand(true);
    body.set_vexpand(true);

    let page = GtkBox::new(Orientation::Vertical, 0);
    page.append(&hero);
    page.append(&body);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_overlay_scrolling(false);
    scroll.set_child(Some(&page));

    let inspector = Inspector {
        root: scroll.upcast(),
        page,
        hero,
        body,
        compact: Cell::new(false),
        last: std::cell::RefCell::new(None),
    };
    inspector.show_empty(None);
    inspector
}

impl Inspector {
    pub fn show_empty(&self, ctx: Option<&Ctx>) {
        clear(&self.hero);
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

    /// Content that scrolls with the spotlight: it sits right under the
    /// hero, so the collection leads and the technical details close the page.
    pub fn append_below(&self, widget: &impl IsA<gtk::Widget>) {
        self.page.insert_child_after(widget, Some(&self.hero));
    }

    /// Re-renders the current game when the layout switches between wide and
    /// narrow, so the action row always fits.
    pub fn set_compact(&self, compact: bool) {
        if self.compact.replace(compact) == compact {
            return;
        }
        let last = self.last.borrow().clone();
        if let Some((ctx, game)) = last {
            self.show(&ctx, Some(&game));
        }
    }

    pub fn show(&self, ctx: &Ctx, game: Option<&Game>) {
        let Some(game) = game else {
            self.last.replace(None);
            self.show_empty(Some(ctx));
            return;
        };
        self.last.replace(Some((ctx.clone(), game.clone())));
        let compact = self.compact.get();
        clear(&self.hero);
        clear(&self.body);

        let can_launch =
            game.origin != Origin::Search && game.installed && game.state != GameState::KeysMissing;
        let actions = GtkBox::new(Orientation::Horizontal, 8);
        actions.add_css_class("hero-actions");
        actions.set_margin_top(14);
        actions.append(&primary_button(ctx, game));

        // (label, icon, tooltip, command): beside the primary action when
        // there is room, in the overflow menu on narrow windows.
        let mut secondary: Vec<(&str, &str, &str, Command)> = Vec::new();
        if game.origin != Origin::Search {
            if can_launch {
                secondary.push(("Check for updates", "emblem-synchronizing-symbolic", CHECK_TOOLTIP, Command::Sync(game.appid)));
            }
            secondary.push(("Repair", "applications-engineering-symbolic", REPAIR_TOOLTIP, Command::Fix(game.appid)));
        }
        if !compact {
            for (label, _, tooltip, command) in &secondary {
                let button = action_button(label);
                button.set_tooltip_text(Some(tooltip));
                let commands = ctx.commands.clone();
                let command = command.clone();
                button.connect_clicked(move |_| commands.send(command.clone()));
                actions.append(&button);
            }
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

        if compact {
            for (label, icon, tooltip, command) in secondary {
                let button = menu_button(label, icon);
                button.set_tooltip_text(Some(tooltip));
                let commands = ctx.commands.clone();
                let popover = popover.clone();
                button.connect_clicked(move |_| {
                    popover.popdown();
                    commands.send(command.clone());
                });
                menu.append(&button);
            }
        }

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
                let _ = crate::desktop::spawn_reaped(
                    std::process::Command::new("xdg-open")
                        .arg(format!("https://store.steampowered.com/app/{appid}/")),
                );
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

        self.hero.append(&hero(game, &actions));

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
        let launch = gtk::Button::with_label("Play");
        launch.add_css_class("suggested-action");
        launch.add_css_class("play");
        launch.set_cursor_from_name(Some("pointer"));
        let commands = ctx.commands.clone();
        let appid = game.appid;
        launch.connect_clicked(move |_| commands.send(Command::Launch(appid)));
        return launch;
    }
    let sync = gtk::Button::with_label("Check for updates");
    sync.set_tooltip_text(Some(CHECK_TOOLTIP));
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
    let art = game.hero.as_deref().or(game.cover.as_deref());
    let base = Art::new(-1, HERO_HEIGHT, "hero");
    base.set(art, &game.initial(), Some(HERO_DECODE_WIDTH));
    base.root.set_hexpand(true);

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

    let title = hero_title(game);

    let meta = gtk::Label::new(Some(&game.hero_meta()));
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
    overlay.set_size_request(-1, HERO_HEIGHT);
    overlay.set_hexpand(true);
    overlay.set_child(Some(&base.root));
    overlay.add_overlay(&scrim);
    overlay.add_overlay(&content);

    // Wide windows would otherwise grow the hero to the art's natural aspect
    // height; the vertical clamp holds the cinematic band at a fixed height.
    let clamp = adw::Clamp::builder()
        .orientation(Orientation::Vertical)
        .maximum_size(HERO_HEIGHT)
        .tightening_threshold(HERO_HEIGHT)
        .child(&overlay)
        .build();
    clamp.set_hexpand(true);
    clamp.upcast()
}

/// The game's own logo when Steam has one, otherwise the name as text. The
/// slot stays empty for the frame it takes a cached logo to arrive.
pub(crate) fn hero_title(game: &Game) -> gtk::Widget {
    let slot = GtkBox::new(Orientation::Vertical, 0);
    slot.set_halign(gtk::Align::Start);
    let text = || {
        let title = gtk::Label::new(Some(&game.name));
        title.add_css_class("title-1");
        title.add_css_class("hero-title");
        title.set_halign(gtk::Align::Start);
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        title.set_max_width_chars(42);
        title.set_lines(2);
        title
    };
    let Some(path) = game.logo.as_deref() else {
        slot.append(&text());
        return slot.upcast();
    };
    let fallback = text();
    let name = game.name.clone();
    let target = slot.clone();
    art::logo(path, LOGO_MAX_WIDTH * art::scale(&slot), move |loaded| {
        let Some(loaded) = loaded else {
            target.append(&fallback);
            return;
        };
        let (width, height) = logo_size(loaded.texture.width(), loaded.texture.height());
        let picture = gtk::Picture::for_paintable(&loaded.texture);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_can_shrink(true);
        picture.set_alternative_text(Some(&name));
        picture.set_tooltip_text(Some(&name));
        picture.add_css_class("hero-logo");
        // Clamps keep the texture's device-pixel natural size from growing
        // the logo past its logical box on HiDPI screens.
        let tall = adw::Clamp::builder()
            .orientation(Orientation::Vertical)
            .maximum_size(height)
            .tightening_threshold(height)
            .child(&picture)
            .build();
        let wide = adw::Clamp::builder()
            .maximum_size(width)
            .tightening_threshold(width)
            .child(&tall)
            .build();
        // Height only: a fixed width would set the window's minimum width.
        wide.set_size_request(-1, height);
        wide.set_halign(gtk::Align::Start);
        wide.set_margin_bottom(6);
        target.append(&wide);
    });
    slot.upcast()
}

/// Fits a logo into `LOGO_MAX_WIDTH` x `LOGO_MAX_HEIGHT`, keeping its aspect.
fn logo_size(texture_width: i32, texture_height: i32) -> (i32, i32) {
    if texture_width <= 0 || texture_height <= 0 {
        return (LOGO_MAX_WIDTH, LOGO_MAX_HEIGHT);
    }
    let aspect = texture_width as f64 / texture_height as f64;
    let height = (LOGO_MAX_WIDTH as f64 / aspect).min(LOGO_MAX_HEIGHT as f64);
    ((height * aspect).round() as i32, height.round() as i32)
}

fn depots_section(ctx: &Ctx, game: &Game) -> gtk::Widget {
    let loaded = game.depots.iter().filter(|depot| depot.has_key).count();
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

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.set_margin_top(6);
    content.append(&note);
    content.append(&table);

    let expander = gtk::Expander::new(Some("Depots and manifests"));
    expander.set_margin_top(18);
    expander.set_child(Some(&content));
    expander.upcast()
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
        .subtitle(format!("{source}, {} branch", game.branch))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_logos_are_limited_by_width_and_tall_ones_by_height() {
        assert_eq!(logo_size(640, 191), (380, 113));
        assert_eq!(logo_size(501, 335), (179, 120));
        assert_eq!(logo_size(0, 10), (LOGO_MAX_WIDTH, LOGO_MAX_HEIGHT));
    }
}
