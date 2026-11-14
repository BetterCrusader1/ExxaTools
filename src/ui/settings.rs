//! Settings is quiet: four values, one save, no modal dialog in the way.

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::shell::Ctx;
use crate::ui::worker::{Command, SettingsDraft};

pub struct SettingsPage {
    pub root: gtk::Widget,
    country: adw::EntryRow,
    hubcap: adw::PasswordEntryRow,
    ryuu: adw::PasswordEntryRow,
    source: adw::EntryRow,
    ctx: Ctx,
}

pub fn build(ctx: &Ctx) -> SettingsPage {
    let loaded = crate::state::load().unwrap_or_default();
    let page = adw::PreferencesPage::new();

    let region = adw::PreferencesGroup::builder()
        .title("Store")
        .description("Used for prices and Store search results.")
        .build();
    let country = adw::EntryRow::builder()
        .title("Country code")
        .text(&loaded.settings.country)
        .build();
    region.add(&country);
    page.add(&region);

    let sources = adw::PreferencesGroup::builder()
        .title("Plan sources")
        .description("Keys stay on this machine and are never shown back by the API.")
        .build();
    let hubcap = adw::PasswordEntryRow::builder()
        .title("Hubcap key")
        .text(&loaded.settings.hubcap_key)
        .build();
    sources.add(&hubcap);
    let ryuu = adw::PasswordEntryRow::builder()
        .title("Ryuu key")
        .text(&loaded.settings.ryuu_key)
        .build();
    sources.add(&ryuu);
    page.add(&sources);

    let defaults = adw::PreferencesGroup::builder()
        .title("Plan defaults")
        .description("Which source builds a plan when no key is chosen.")
        .build();
    let source = adw::EntryRow::builder()
        .title("Default source")
        .text(&loaded.settings.default_source)
        .build();
    defaults.add(&source);
    page.add(&defaults);

    let save = gtk::Button::with_label("Save changes");
    save.add_css_class("suggested-action");
    save.set_cursor_from_name(Some("pointer"));
    save.set_valign(gtk::Align::Center);
    let save_row = adw::ActionRow::builder()
        .title("Save to disk")
        .subtitle("Writes ~/.config/exxatools/state.json")
        .build();
    save_row.add_suffix(&save);
    let apply = adw::PreferencesGroup::new();
    apply.add(&save_row);
    page.add(&apply);

    let title = gtk::Label::new(Some("Settings"));
    title.add_css_class("page-title");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some("Sources and region used to build plans"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    let headings = GtkBox::new(Orientation::Vertical, 2);
    headings.set_margin_top(22);
    headings.set_margin_bottom(6);
    headings.set_margin_start(0);
    headings.set_margin_end(0);
    headings.append(&title);
    headings.append(&subtitle);
    let header_clamp = adw::Clamp::builder().maximum_size(580).build();
    header_clamp.set_child(Some(&headings));

    page.set_vexpand(true);
    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&header_clamp);
    root.append(&page);

    let page_ = SettingsPage {
        root: root.upcast(),
        country,
        hubcap,
        ryuu,
        source,
        ctx: ctx.clone(),
    };

    let ctx = ctx.clone();
    let form = page_.form();
    save.connect_clicked(move |_| {
        ctx.commands.send(Command::SaveSettings(form.clone()));
    });

    page_
}

impl SettingsPage {
    fn form(&self) -> SettingsDraft {
        SettingsDraft {
            country: self.country.text().to_string(),
            hubcap_key: self.hubcap.text().to_string(),
            ryuu_key: self.ryuu.text().to_string(),
            default_source: self.source.text().to_string(),
        }
    }

    pub fn reload(&self) {
        if let Ok(state) = crate::state::load() {
            self.country.set_text(&state.settings.country);
            self.hubcap.set_text(&state.settings.hubcap_key);
            self.ryuu.set_text(&state.settings.ryuu_key);
            self.source.set_text(&state.settings.default_source);
        }
        let _ = &self.ctx;
    }
}
