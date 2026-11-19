//! The middle pane: a quiet shelf of games, with artwork doing the visual work.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::model::Game;

#[derive(Clone)]
pub struct Shelf {
    pub root: gtk::Widget,
    pub list: gtk::ListBox,
    title: gtk::Label,
    subtitle: gtk::Label,
    scroll: gtk::ScrolledWindow,
    empty: GtkBox,
    pub appids: Rc<RefCell<Vec<u64>>>,
}

pub fn build() -> Shelf {
    let title = gtk::Label::new(Some("Library"));
    title.add_css_class("title-3");
    title.set_halign(gtk::Align::Start);
    title.set_hexpand(true);

    let heading = GtkBox::new(Orientation::Horizontal, 8);
    heading.set_margin_start(16);
    heading.set_margin_end(16);
    heading.set_margin_top(18);
    heading.append(&title);

    let subtitle = gtk::Label::new(Some("No games yet"));
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("caption");
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_margin_start(16);
    subtitle.set_margin_end(16);
    subtitle.set_margin_top(2);
    subtitle.set_margin_bottom(12);

    let list = gtk::ListBox::new();
    list.add_css_class("shelf-list");
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(true);
    list.set_margin_start(6);
    list.set_margin_end(6);
    list.set_margin_bottom(12);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&list));
    scroll.set_vexpand(true);
    scroll.set_visible(false);

    let empty = empty_shelf();

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("shelf");
    root.set_width_request(310);
    root.append(&heading);
    root.append(&subtitle);
    root.append(&scroll);
    root.append(&empty);

    Shelf {
        root: root.upcast(),
        list,
        title,
        subtitle,
        scroll,
        empty,
        appids: Rc::new(RefCell::new(Vec::new())),
    }
}

impl Shelf {
    pub fn set_games(&self, games: &[Game]) {
        clear(&self.list);
        let mut appids = self.appids.borrow_mut();
        appids.clear();
        for game in games {
            appids.push(game.appid);
            self.list.append(&row(game));
        }
        drop(appids);

        self.subtitle.set_text(if games.is_empty() {
            "Nothing here yet"
        } else {
            "Sorted by name"
        });
        self.scroll.set_visible(!games.is_empty());
        self.empty.set_visible(games.is_empty());
    }

    pub fn set_title(&self, title: &str, subtitle: &str) {
        self.title.set_text(title);
        self.subtitle.set_text(subtitle);
    }

    pub fn select_appid(&self, appid: u64) {
        let index = self
            .appids
            .borrow()
            .iter()
            .position(|value| *value == appid);
        if let Some(index) = index {
            if let Some(row) = self.list.row_at_index(index as i32) {
                self.list.select_row(Some(&row));
            }
        }
    }

    pub fn select_first(&self) {
        if let Some(row) = self.list.row_at_index(0) {
            self.list.select_row(Some(&row));
        } else {
            self.list.select_row(None::<&gtk::ListBoxRow>);
        }
    }

    pub fn selected_appid(&self) -> Option<u64> {
        let row = self.list.selected_row()?;
        let index = row.index();
        if index < 0 {
            return None;
        }
        self.appids.borrow().get(index as usize).copied()
    }
}

fn empty_shelf() -> GtkBox {
    let icon = gtk::Image::from_icon_name("applications-games-symbolic");
    icon.set_pixel_size(32);
    icon.add_css_class("dim-label");

    let title = gtk::Label::new(Some("Your library is empty"));
    title.add_css_class("heading");

    let detail = gtk::Label::new(Some("Add a game from Discover or search Steam above."));
    detail.add_css_class("dim-label");
    detail.add_css_class("caption");
    detail.set_wrap(true);
    detail.set_max_width_chars(28);
    detail.set_justify(gtk::Justification::Center);

    let empty = GtkBox::new(Orientation::Vertical, 8);
    empty.set_valign(gtk::Align::Center);
    empty.set_vexpand(true);
    empty.set_margin_top(32);
    empty.set_margin_bottom(32);
    empty.set_margin_start(24);
    empty.set_margin_end(24);
    empty.append(&icon);
    empty.append(&title);
    empty.append(&detail);
    empty
}

fn row(game: &Game) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("game-row");
    row.set_cursor_from_name(Some("pointer"));
    row.set_tooltip_text(Some(&format!("{} — {}", game.name, game.state.label())));

    let cover = match &game.cover {
        Some(path) => thumb(path, 64, 30),
        None => monogram(game, 64, 30),
    };
    cover.set_valign(gtk::Align::Center);

    let name = gtk::Label::new(Some(&game.name));
    name.add_css_class("game-name");
    name.set_halign(gtk::Align::Start);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.set_xalign(0.0);
    name.set_max_width_chars(24);

    let meta = gtk::Label::new(Some(&game.subtitle()));
    meta.add_css_class("dim-label");
    meta.add_css_class("caption");
    meta.set_halign(gtk::Align::Start);
    meta.set_ellipsize(gtk::pango::EllipsizeMode::End);
    meta.set_xalign(0.0);

    let text = GtkBox::new(Orientation::Vertical, 1);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    text.append(&name);
    text.append(&meta);

    let dot = gtk::Box::new(Orientation::Horizontal, 0);
    dot.add_css_class("status-dot");
    dot.add_css_class(game.state.dot_class());
    dot.set_valign(gtk::Align::Center);
    dot.set_tooltip_text(Some(game.state.label()));

    let content = GtkBox::new(Orientation::Horizontal, 10);
    content.set_margin_top(6);
    content.set_margin_bottom(6);
    content.set_margin_start(8);
    content.set_margin_end(10);
    content.append(&cover);
    content.append(&text);
    content.append(&dot);
    row.set_child(Some(&content));
    row
}

pub fn monogram(game: &Game, width: i32, height: i32) -> gtk::Widget {
    let letter = gtk::Label::new(Some(&game.initial()));
    letter.add_css_class("monogram-letter");
    let box_ = GtkBox::new(Orientation::Vertical, 0);
    box_.add_css_class("monogram");
    box_.set_size_request(width, height);
    box_.set_halign(gtk::Align::Start);
    box_.set_valign(gtk::Align::Center);
    box_.append(&letter);
    box_.upcast()
}

fn clear(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn thumb(path: &std::path::Path, width: i32, height: i32) -> gtk::Widget {
    match gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, width, height, false) {
        Ok(pixbuf) => {
            let image = gtk::Image::from_pixbuf(Some(&pixbuf));
            image.add_css_class("shelf-cover");
            image.upcast()
        }
        Err(_) => {
            let letter = gtk::Label::new(Some("?"));
            letter.add_css_class("monogram-letter");
            let box_ = GtkBox::new(Orientation::Vertical, 0);
            box_.add_css_class("monogram");
            box_.set_size_request(width, height);
            box_.append(&letter);
            box_.upcast()
        }
    }
}
