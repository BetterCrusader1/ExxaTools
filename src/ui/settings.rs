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
    providers: adw::EntryRow,
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

    // The manifest provider chain, tried top to bottom when Steam needs a
    // request code. Names are comma-separated; built-ins resolve by name and
    // anything else with a URL goes through the custom-entry path below.
    let providers = adw::PreferencesGroup::builder()
        .title("Manifest providers")
        .description("Tried in order when a manifest request code is needed.")
        .build();
    let chain = adw::EntryRow::builder()
        .title("Provider order")
        .text(loaded.settings.manifest_providers.join(", "))
        .build();
    providers.add(&chain);

    let custom_name = adw::EntryRow::builder().title("Custom name").build();
    let custom_url = adw::EntryRow::builder()
        .title("Custom URL (must contain {gid})")
        .build();
    let custom_field = adw::EntryRow::builder()
        .title("JSON field (plain servers: leave empty)")
        .build();
    let add_custom = gtk::Button::with_label("Add");
    add_custom.add_css_class("suggested-action");
    add_custom.set_cursor_from_name(Some("pointer"));
    add_custom.set_valign(gtk::Align::Center);
    let add_row = adw::ActionRow::builder().title("Append to the chain").build();
    add_row.add_suffix(&add_custom);
    providers.add(&custom_name);
    providers.add(&custom_url);
    providers.add(&custom_field);
    providers.add(&add_row);
    page.add(&providers);

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

    // The Add button needs the three entry widgets and the toast overlay;
    // wire it before the struct consumes them.
    {
        let chain = chain.clone();
        let name = custom_name.clone();
        let url = custom_url.clone();
        let field = custom_field.clone();
        let toast = ctx.toast.clone();
        add_custom.connect_clicked(move |_| {
            let name_text = name.text().trim().to_string();
            let url_text = url.text().trim().to_string();
            let field_text = field.text().trim().to_string();
            if name_text.is_empty() || !url_text.contains("{gid}") {
                toast.add_toast(adw::Toast::new(
                    "A provider needs a name and a URL with {gid}",
                ));
                return;
            }
            let entry = if field_text.is_empty() {
                serde_json::json!({ "name": name_text, "url": url_text })
            } else {
                serde_json::json!({ "name": name_text, "url": url_text, "format": "json", "json_field": field_text })
            };
            let joined = format!("{}, {}", chain.text(), entry);
            chain.set_text(joined.trim_start_matches(','));
            name.set_text("");
            url.set_text("");
            field.set_text("");
        });
    }

    let page_ = SettingsPage {
        root: root.upcast(),
        country,
        hubcap,
        ryuu,
        source,
        providers: chain,
        ctx: ctx.clone(),
    };

    let ctx = ctx.clone();
    let country = page_.country.clone();
    let hubcap = page_.hubcap.clone();
    let ryuu = page_.ryuu.clone();
    let source = page_.source.clone();
    let chain = page_.providers.clone();
    // Capture the widgets, not their initial values: read the form on every click.
    let save_current = save_action(
        move || SettingsDraft {
            country: country.text().to_string(),
            hubcap_key: hubcap.text().to_string(),
            ryuu_key: ryuu.text().to_string(),
            default_source: source.text().to_string(),
            manifest_providers: parse_provider_chain(&chain.text()),
        },
        move |command| ctx.commands.send(command),
    );
    save.connect_clicked(move |_| save_current());

    page_
}

// Keep deferred form reads testable without constructing GTK widgets.
fn save_action(
    read_form: impl Fn() -> SettingsDraft,
    send: impl Fn(Command),
) -> impl Fn() {
    move || send(Command::SaveSettings(read_form()))
}

/// Split the provider-order field into raw entries. Built-ins are plain
/// names; a custom mirror appended by the UI is a JSON object and must
/// survive intact, so commas inside braces are not separators.
fn parse_provider_chain(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let entry = current.trim().to_string();
                if !entry.is_empty() {
                    out.push(entry);
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }
    let entry = current.trim().to_string();
    if !entry.is_empty() {
        out.push(entry);
    }
    out
}

impl SettingsPage {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn save_reads_current_form_on_every_click() {
        let form = RefCell::new(SettingsDraft {
            country: "us".into(),
            hubcap_key: String::new(),
            ryuu_key: String::new(),
            default_source: "auto".into(),
            manifest_providers: Vec::new(),
        });
        let sent = RefCell::new(Vec::new());
        let save = save_action(
            || form.borrow().clone(),
            |command| sent.borrow_mut().push(command),
        );
        assert!(sent.borrow().is_empty());

        // Edits after connecting the handler must replace the startup values.
        *form.borrow_mut() = SettingsDraft {
            country: "gb".into(),
            hubcap_key: "test-hubcap".into(),
            ryuu_key: "test-ryuu".into(),
            default_source: "hubcap".into(),
            manifest_providers: Vec::new(),
        };
        save();

        // Subsequent clicks must also read fresh values, including cleared keys.
        *form.borrow_mut() = SettingsDraft {
            country: "de".into(),
            hubcap_key: String::new(),
            ryuu_key: String::new(),
            default_source: "ryuu".into(),
            manifest_providers: Vec::new(),
        };
        save();

        let sent = sent.borrow();
        assert_eq!(sent.len(), 2);
        let Command::SaveSettings(first) = &sent[0] else {
            panic!("expected SaveSettings");
        };
        assert_eq!(first.country, "gb");
        assert_eq!(first.hubcap_key, "test-hubcap");
        assert_eq!(first.ryuu_key, "test-ryuu");
        assert_eq!(first.default_source, "hubcap");
        let Command::SaveSettings(second) = &sent[1] else {
            panic!("expected SaveSettings");
        };
        assert_eq!(second.country, "de");
        assert!(second.hubcap_key.is_empty());
        assert!(second.ryuu_key.is_empty());
        assert_eq!(second.default_source, "ryuu");
    }

    #[test]
    fn chain_parser_keeps_objects_intact() {
        let chain = parse_provider_chain(
            "steamrun, {\"name\":\"mirror\", \"url\":\"https://m.example/{gid}\"}, manifestdex",
        );
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], "steamrun");
        assert!(chain[1].starts_with('{') && chain[1].ends_with('}'));
        assert!(chain[1].contains("{gid}"));
        assert_eq!(chain[2], "manifestdex");
    }

    #[test]
    fn chain_parser_drops_empty_entries() {
        let chain = parse_provider_chain(" steamrun , , manifestdex ,");
        assert_eq!(chain, vec!["steamrun", "manifestdex"]);
    }
}
