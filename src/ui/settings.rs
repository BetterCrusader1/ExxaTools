//! Settings in task language: region, sources, mirrors. Raw provider ids and
//! JSON stay inside the custom-mirror editor instead of leading the page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::shell::Ctx;
use crate::ui::worker::{Command, SettingsDraft};

type ProviderToggle = (String, adw::SwitchRow);

const NOT_CONFIGURED: &str = "Not configured";
const SOURCE_LABELS: [&str; 4] = [
    "Automatic — first configured source",
    "Hubcap Manifest",
    "Ryuu",
    "Custom plan source",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceLabelLayout {
    width_chars: i32,
    max_width_chars: i32,
    wrap: bool,
    truncate: bool,
}

const SOURCE_LABEL_LAYOUT: SourceLabelLayout = SourceLabelLayout {
    width_chars: 22,
    max_width_chars: 36,
    wrap: true,
    truncate: false,
};

const COUNTRIES: [(&str, &str); 8] = [
    ("us", "United States"),
    ("ua", "Ukraine"),
    ("gb", "United Kingdom"),
    ("de", "Germany"),
    ("pl", "Poland"),
    ("fr", "France"),
    ("ca", "Canada"),
    ("br", "Brazil"),
];

const BUILTIN_PROVIDERS: [(&str, &str, &str); 5] = [
    ("steamrun", "SteamRun", "manifest.steam.run"),
    ("20770407", "20770407", "20770407.xyz"),
    ("wudrm", "Wudrm", "gmrc.wudrm.com"),
    ("manifestdex", "ManifestDeX", "manifest.manifestdex.com"),
    (
        "opensteamtool",
        "Open Steam Tool",
        "manifest.opensteamtool.com",
    ),
];

pub struct SettingsPage {
    pub root: gtk::Widget,
    country: adw::ComboRow,
    country_custom: adw::EntryRow,
    hubcap: adw::PasswordEntryRow,
    ryuu: adw::PasswordEntryRow,
    source: adw::ComboRow,
    auto_update: adw::SwitchRow,
    providers: Rc<Vec<ProviderToggle>>,
    custom_chain: adw::EntryRow,
    save: gtk::Button,
    tracker: Rc<RefCell<SaveTracker>>,
    read_form: Rc<dyn Fn() -> SettingsDraft>,
    reloading: Rc<Cell<bool>>,
    ctx: Ctx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SaveState {
    Clean,
    Dirty,
    Saving,
    Saved,
    Error,
}

struct SaveTracker {
    baseline: SettingsDraft,
    pending: Option<SettingsDraft>,
    post_submit: Option<SettingsDraft>,
    state: SaveState,
}

enum ReloadOutcome {
    Clean,
    Saved,
    Restore(SettingsDraft),
}

impl SaveTracker {
    fn new(baseline: SettingsDraft) -> Self {
        Self {
            baseline,
            pending: None,
            post_submit: None,
            state: SaveState::Clean,
        }
    }

    fn state(&self) -> SaveState {
        self.state
    }

    fn can_save(&self) -> bool {
        matches!(self.state, SaveState::Dirty | SaveState::Error)
    }

    fn sync(&mut self, current: &SettingsDraft) {
        if self.pending.is_some() {
            self.track_post_submit(current);
        }
        if self.state == SaveState::Saving {
            return;
        }
        if drafts_equal(&self.baseline, current) {
            self.pending = None;
            self.post_submit = None;
            self.state = SaveState::Clean;
        } else if self.state != SaveState::Error {
            self.state = SaveState::Dirty;
        }
    }

    fn track_post_submit(&mut self, current: &SettingsDraft) {
        self.post_submit = self.pending.as_ref().and_then(|pending| {
            if drafts_equal(pending, current) {
                None
            } else {
                Some(current.clone())
            }
        });
    }

    fn begin_save(&mut self, current: &SettingsDraft) -> bool {
        if self.state == SaveState::Saving || drafts_equal(&self.baseline, current) {
            return false;
        }
        self.pending = Some(current.clone());
        self.post_submit = None;
        self.state = SaveState::Saving;
        true
    }

    fn finish_reload(&mut self, loaded: SettingsDraft) -> ReloadOutcome {
        self.baseline = loaded.clone();
        if let Some(latest) = self.post_submit.take() {
            self.pending = None;
            self.state = SaveState::Dirty;
            return ReloadOutcome::Restore(latest);
        }
        match self.pending.take() {
            Some(pending) if drafts_equal(&pending, &loaded) => {
                self.state = SaveState::Saved;
                ReloadOutcome::Saved
            }
            Some(pending) => {
                self.state = SaveState::Error;
                ReloadOutcome::Restore(pending)
            }
            None => {
                self.state = SaveState::Clean;
                ReloadOutcome::Clean
            }
        }
    }

    fn fail_reload(&mut self, current: &SettingsDraft) {
        if self.pending.is_some() {
            if drafts_equal(&self.baseline, current) {
                self.pending = None;
                self.post_submit = None;
                self.state = SaveState::Clean;
            } else {
                self.state = SaveState::Error;
            }
        }
    }
}

fn drafts_equal(left: &SettingsDraft, right: &SettingsDraft) -> bool {
    left.country
        .trim()
        .eq_ignore_ascii_case(right.country.trim())
        && left.hubcap_key.trim() == right.hubcap_key.trim()
        && left.ryuu_key.trim() == right.ryuu_key.trim()
        && left.default_source.trim() == right.default_source.trim()
        && left
            .manifest_providers
            .iter()
            .map(|entry| entry.trim())
            .filter(|entry| !entry.is_empty())
            .eq(right
                .manifest_providers
                .iter()
                .map(|entry| entry.trim())
                .filter(|entry| !entry.is_empty()))
        && left.auto_update_plans == right.auto_update_plans
}

/// Steam coupling switches apply at once, like the theme: they change how
/// ExxaTools behaves around Steam, not a draft that needs saving.
fn steam_group(ctx: &Ctx, settings: &crate::state::Settings) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Steam")
        .description("ExxaTools starts with Steam; choose how closely the dashboard follows it.")
        .build();
    type Apply = fn(&mut crate::state::Settings, bool);
    // (title, subtitle, active, needs a source checkout, apply)
    let rows: [(&str, &str, bool, bool, Apply); 4] = [
        (
            "Update games with Steam",
            "Keep Steam's automatic updates on; manifests refresh when Steam starts and every 30 minutes",
            settings.follow_steam_updates,
            false,
            |settings, value| settings.follow_steam_updates = value,
        ),
        (
            "Open with Steam",
            "Show the dashboard as soon as Steam's window is up",
            settings.open_with_steam,
            false,
            |settings, value| settings.open_with_steam = value,
        ),
        (
            "Close with Steam",
            "Close the dashboard when Steam exits",
            settings.close_with_steam,
            false,
            |settings, value| settings.close_with_steam = value,
        ),
        (
            "Live updates from source",
            "Rebuild when the code changes and restart into the new build",
            settings.live_rebuild,
            true,
            |settings, value| settings.live_rebuild = value,
        ),
    ];
    for (title, subtitle, active, needs_checkout, apply) in rows {
        let row = adw::SwitchRow::builder()
            .title(title)
            .subtitle(subtitle)
            .active(active)
            .build();
        // A packaged build has no checkout to rebuild from.
        row.set_visible(!needs_checkout || crate::live::checkout().is_some());
        let ctx = ctx.clone();
        row.connect_active_notify(move |row| {
            let value = row.is_active();
            if let Err(error) = crate::state::update(|state| {
                apply(&mut state.settings, value);
                Ok(())
            }) {
                ctx.toast_text(&format!("Could not save: {error}"));
            }
        });
        group.add(&row);
    }
    group
}

pub fn build(ctx: &Ctx) -> SettingsPage {
    let loaded = crate::state::load().unwrap_or_default();
    let page = adw::PreferencesPage::new();

    let country_names: Vec<String> = COUNTRIES
        .iter()
        .map(|(code, name)| format!("{name} ({code})"))
        .chain(["Custom country code".to_string()])
        .collect();
    let country_refs: Vec<&str> = country_names.iter().map(String::as_str).collect();
    let country_model = gtk::StringList::new(&country_refs);
    let country = adw::ComboRow::builder()
        .title("Store country")
        .subtitle("Used for prices and Steam search results")
        .model(&country_model)
        .selected(country_index(&loaded.settings.country))
        .build();
    let country_custom = adw::EntryRow::builder()
        .title("Custom country code")
        .width_chars(12)
        .max_width_chars(24)
        .text(
            if country_index(&loaded.settings.country) as usize == COUNTRIES.len() {
                loaded.settings.country.clone()
            } else {
                String::new()
            },
        )
        .build();
    country_custom.set_not_configured_placeholder();
    country_custom.set_visible(country.selected() as usize == COUNTRIES.len());
    {
        let custom = country_custom.clone();
        country.connect_selected_notify(move |row| {
            custom.set_visible(row.selected() as usize == COUNTRIES.len());
        });
    }

    let auto_update = adw::SwitchRow::builder()
        .title("Automatic plan updates")
        .subtitle("Check every 30 minutes and when Steam starts ExxaTools")
        .active(loaded.settings.auto_update_plans)
        .build();
    let updates = adw::PreferencesGroup::builder()
        .title("Updates")
        .description("Every configured source is checked; the freshest manifest wins per depot.")
        .build();
    updates.add(&auto_update);
    page.add(&updates);

    page.add(&steam_group(ctx, &loaded.settings));

    let region = adw::PreferencesGroup::builder()
        .title("Store")
        .description("Choose the region once; searches and prices use it automatically.")
        .build();
    region.add(&country);
    region.add(&country_custom);
    page.add(&region);

    let source_factory = source_item_factory();
    let source_list_factory = source_item_factory();
    let source = adw::ComboRow::builder()
        .title("Preferred plan source")
        .subtitle("Where depot keys and pinned manifests come from")
        .model(&gtk::StringList::new(&SOURCE_LABELS))
        .factory(&source_factory)
        .list_factory(&source_list_factory)
        .selected(source_index(&loaded.settings.default_source))
        .hexpand(true)
        .build();
    let hubcap = adw::PasswordEntryRow::builder()
        .title("Hubcap API key")
        .text(&loaded.settings.hubcap_key)
        .width_chars(12)
        .max_width_chars(24)
        .build();
    hubcap.set_not_configured_placeholder();
    hubcap.set_tooltip_text(Some("Free smm_ key · hubcapmanifest.com"));
    let ryuu = adw::PasswordEntryRow::builder()
        .title("Ryuu API key")
        .text(&loaded.settings.ryuu_key)
        .width_chars(12)
        .max_width_chars(24)
        .build();
    ryuu.set_not_configured_placeholder();
    ryuu.set_tooltip_text(Some("X-Auth-Key · generator.ryuu.lol"));

    let sources = adw::PreferencesGroup::builder()
        .title("Keys and plan sources")
        .description(
            "Keys stay on this machine. Automatic uses the first source that is configured.",
        )
        .build();
    sources.add(&source);
    sources.add(&hubcap);
    sources.add(&ryuu);
    page.add(&sources);

    let current = loaded.settings.manifest_providers.clone();
    let toggles: Vec<ProviderToggle> = BUILTIN_PROVIDERS
        .iter()
        .map(|(id, name, domain)| {
            let row = adw::SwitchRow::builder()
                .title(*name)
                .subtitle(*domain)
                .active(current.iter().any(|entry| entry == id))
                .build();
            (id.to_string(), row)
        })
        .collect();
    let providers_group = adw::PreferencesGroup::builder()
        .title("Manifest request codes")
        .description(
            "Codes that let Steam download a manifest, tried in this order. They are not depot keys: keys come only from the plan sources above.",
        )
        .build();
    for (_, row) in &toggles {
        providers_group.add(row);
    }
    let providers = Rc::new(toggles);

    let custom_text = current
        .iter()
        .filter(|entry| !is_builtin(entry))
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let custom_chain = adw::EntryRow::builder()
        .title("Saved custom entries")
        .text(custom_text)
        .width_chars(12)
        .max_width_chars(36)
        .build();
    custom_chain.set_not_configured_placeholder();
    custom_chain.set_tooltip_text(Some(
        "JSON entries, separated by commas. Edit or remove them here.",
    ));

    let custom_name = adw::EntryRow::builder()
        .title("Mirror name")
        .width_chars(12)
        .max_width_chars(28)
        .build();
    custom_name.set_not_configured_placeholder();
    let custom_url = adw::EntryRow::builder()
        .title("URL with {gid}")
        .width_chars(12)
        .max_width_chars(36)
        .build();
    custom_url.set_not_configured_placeholder();
    custom_url.set_tooltip_text(Some("Example: https://mirror.example/manifest/{gid}"));
    let custom_field = adw::EntryRow::builder()
        .title("JSON response field")
        .width_chars(12)
        .max_width_chars(28)
        .build();
    custom_field.set_not_configured_placeholder();
    custom_field.set_tooltip_text(Some(
        "Leave empty when the server returns the code as plain text",
    ));
    let add_custom = gtk::Button::with_label("Add mirror");
    add_custom.add_css_class("suggested-action");
    add_custom.set_cursor_from_name(Some("pointer"));
    add_custom.set_valign(gtk::Align::Center);
    let add_row = adw::ActionRow::builder()
        .title("Add to the custom chain")
        .build();
    add_row.add_suffix(&add_custom);
    let custom = adw::ExpanderRow::builder()
        .title("Custom mirrors")
        .subtitle("Only needed for your own server; the built-in mirrors are above")
        .build();
    custom.add_row(&custom_name);
    custom.add_row(&custom_url);
    custom.add_row(&custom_field);
    custom.add_row(&add_row);
    custom.add_row(&custom_chain);
    providers_group.add(&custom);
    page.add(&providers_group);

    {
        let chain = custom_chain.clone();
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
                    "A mirror needs a name and a URL with {gid}",
                ));
                return;
            }
            let entry = if field_text.is_empty() {
                serde_json::json!({ "name": name_text, "url": url_text })
            } else {
                serde_json::json!({ "name": name_text, "url": url_text, "format": "json", "json_field": field_text })
            };
            let current = chain.text();
            let joined = if current.trim().is_empty() {
                entry.to_string()
            } else {
                format!("{current}, {entry}")
            };
            chain.set_text(&joined);
            name.set_text("");
            url.set_text("");
            field.set_text("");
        });
    }

    let save = gtk::Button::with_label("Save changes");
    save.add_css_class("suggested-action");
    save.set_cursor_from_name(Some("pointer"));
    save.set_valign(gtk::Align::Center);
    save.set_tooltip_text(Some("Save and apply every section"));

    let title = gtk::Label::new(Some("Settings"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some("Store region, download sources and manifest mirrors"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_xalign(0.0);
    subtitle.set_width_chars(24);
    subtitle.set_max_width_chars(56);
    subtitle.set_wrap(true);
    subtitle.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    let headings = GtkBox::new(Orientation::Vertical, 2);
    headings.set_hexpand(true);
    headings.append(&title);
    headings.append(&subtitle);
    let header = GtkBox::new(Orientation::Horizontal, 12);
    header.set_margin_top(22);
    header.set_margin_bottom(8);
    header.append(&headings);
    header.append(&save);
    let header_clamp = adw::Clamp::builder().maximum_size(680).build();
    header_clamp.set_child(Some(&header));

    page.set_vexpand(true);
    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&header_clamp);
    root.append(&page);

    let read_form: Rc<dyn Fn() -> SettingsDraft> = {
        let country = country.clone();
        let country_custom = country_custom.clone();
        let hubcap = hubcap.clone();
        let ryuu = ryuu.clone();
        let source = source.clone();
        let auto_update = auto_update.clone();
        let providers = providers.clone();
        let custom_chain = custom_chain.clone();
        // Read the widgets on every check and click, never their startup values.
        Rc::new(move || SettingsDraft {
            country: country_code(country.selected(), &country_custom.text()),
            hubcap_key: hubcap.text().to_string(),
            ryuu_key: ryuu.text().to_string(),
            default_source: source_name(source.selected()).to_string(),
            manifest_providers: compose_provider_chain(&providers, &custom_chain.text()),
            auto_update_plans: auto_update.is_active(),
        })
    };
    let tracker = Rc::new(RefCell::new(SaveTracker::new(read_form())));
    let reloading = Rc::new(Cell::new(false));
    let sync_save = {
        let tracker = tracker.clone();
        let read_form = read_form.clone();
        let reloading = reloading.clone();
        let save = save.clone();
        Rc::new(move || {
            if reloading.get() {
                return;
            }
            tracker.borrow_mut().sync(&read_form());
            update_save_button(&save, &tracker.borrow());
        })
    };

    for entry in [&country_custom, &custom_chain] {
        let sync_save = sync_save.clone();
        entry.connect_changed(move |_| sync_save());
    }
    for entry in [&hubcap, &ryuu] {
        let sync_save = sync_save.clone();
        entry.connect_changed(move |_| sync_save());
    }
    for combo in [&country, &source] {
        let sync_save = sync_save.clone();
        combo.connect_selected_notify(move |_| sync_save());
    }
    {
        let sync_save = sync_save.clone();
        auto_update.connect_active_notify(move |_| sync_save());
    }
    for (_, row) in providers.iter() {
        let sync_save = sync_save.clone();
        row.connect_active_notify(move |_| sync_save());
    }
    update_save_button(&save, &tracker.borrow());

    let page_ = SettingsPage {
        root: root.upcast(),
        country,
        country_custom,
        hubcap,
        ryuu,
        source,
        auto_update,
        providers,
        custom_chain,
        save: save.clone(),
        tracker: tracker.clone(),
        read_form: read_form.clone(),
        reloading,
        ctx: ctx.clone(),
    };

    let read_on_save = read_form.clone();
    let ctx = ctx.clone();
    let save_current = save_action(
        move || read_on_save(),
        move |command| ctx.commands.send(command),
    );
    let save_button = save.clone();
    save.connect_clicked(move |_| {
        let should_save = tracker.borrow_mut().begin_save(&read_form());
        if should_save {
            update_save_button(&save_button, &tracker.borrow());
            save_current();
        }
    });

    page_
}

trait EntryRowPlaceholderExt {
    fn set_not_configured_placeholder(&self);
}

impl<T: IsA<gtk::Widget>> EntryRowPlaceholderExt for T {
    fn set_not_configured_placeholder(&self) {
        let mut pending = vec![self.as_ref().clone()];
        let mut found_editor = false;
        while let Some(widget) = pending.pop() {
            if let Ok(editor) = widget.clone().downcast::<gtk::Text>() {
                editor.set_placeholder_text(Some(NOT_CONFIGURED));
                found_editor = true;
                break;
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                pending.push(widget.clone());
                child = widget.next_sibling();
            }
        }
        debug_assert!(found_editor, "entry rows contain a native text editor");
    }
}

fn source_item_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let item = item
            .downcast_ref::<gtk::ListItem>()
            .expect("source rows use ListItem");
        let layout = SOURCE_LABEL_LAYOUT;
        let label = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .width_chars(layout.width_chars)
            .max_width_chars(layout.max_width_chars)
            .wrap(layout.wrap)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .ellipsize(if layout.truncate {
                gtk::pango::EllipsizeMode::End
            } else {
                gtk::pango::EllipsizeMode::None
            })
            .build();
        item.set_child(Some(&label));
    });
    factory.connect_bind(|_, item| {
        let item = item
            .downcast_ref::<gtk::ListItem>()
            .expect("source rows use ListItem");
        let label = item
            .child()
            .and_downcast::<gtk::Label>()
            .expect("source row setup installs a Label");
        let value = item
            .item()
            .and_downcast::<gtk::StringObject>()
            .expect("source model contains StringObject values");
        label.set_text(&value.string());
    });
    factory
}

fn update_save_button(save: &gtk::Button, tracker: &SaveTracker) {
    let (label, tooltip) = match tracker.state() {
        SaveState::Clean => ("Save changes", "No changes to save"),
        SaveState::Dirty => ("Save changes", "Save and apply every section"),
        SaveState::Saving => ("Saving…", "Saving every settings section"),
        SaveState::Saved => ("Saved", "Settings saved and reloaded"),
        SaveState::Error => (
            "Try again",
            "The last save was not confirmed; your edits are still here",
        ),
    };
    save.set_label(label);
    save.set_sensitive(tracker.can_save());
    save.set_tooltip_text(Some(tooltip));
}

fn is_builtin(entry: &str) -> bool {
    for (id, _, _) in BUILTIN_PROVIDERS {
        if entry == id {
            return true;
        }
    }
    false
}

fn compose_provider_chain(toggles: &[ProviderToggle], custom_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (id, _, _) in BUILTIN_PROVIDERS {
        if toggles
            .iter()
            .any(|(toggle, row)| toggle.as_str() == id && row.is_active())
        {
            out.push(id.to_string());
        }
    }
    out.extend(
        parse_provider_chain(custom_text)
            .into_iter()
            .filter(|entry| !is_builtin(entry)),
    );
    out
}

fn country_index(code: &str) -> u32 {
    COUNTRIES
        .iter()
        .position(|(candidate, _)| candidate.eq_ignore_ascii_case(code))
        .map(|index| index as u32)
        .unwrap_or(COUNTRIES.len() as u32)
}

fn country_code(selected: u32, custom: &str) -> String {
    COUNTRIES
        .get(selected as usize)
        .map(|(code, _)| code.to_string())
        .unwrap_or_else(|| custom.trim().to_lowercase())
}

fn source_index(name: &str) -> u32 {
    match name {
        "hubcap" => 1,
        "ryuu" => 2,
        "custom" => 3,
        _ => 0,
    }
}

fn source_name(selected: u32) -> &'static str {
    match selected {
        1 => "hubcap",
        2 => "ryuu",
        3 => "custom",
        _ => "auto",
    }
}

// Keep deferred form reads testable without constructing GTK widgets.
fn save_action(read_form: impl Fn() -> SettingsDraft, send: impl Fn(Command)) -> impl Fn() {
    move || send(Command::SaveSettings(read_form()))
}

/// Split the custom-provider field into raw entries. JSON objects must survive
/// intact, so commas inside braces are not separators.
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
        match crate::state::load() {
            Ok(state) => {
                let loaded = SettingsDraft {
                    country: state.settings.country,
                    hubcap_key: state.settings.hubcap_key,
                    ryuu_key: state.settings.ryuu_key,
                    default_source: state.settings.default_source,
                    manifest_providers: state.settings.manifest_providers,
                    auto_update_plans: state.settings.auto_update_plans,
                };
                self.apply_draft(&loaded);
                let loaded = (self.read_form)();
                match self.tracker.borrow_mut().finish_reload(loaded) {
                    ReloadOutcome::Restore(pending) => self.apply_draft(&pending),
                    ReloadOutcome::Clean | ReloadOutcome::Saved => {}
                }
            }
            Err(_) => self.tracker.borrow_mut().fail_reload(&(self.read_form)()),
        }
        update_save_button(&self.save, &self.tracker.borrow());
        let _ = &self.ctx;
    }

    fn apply_draft(&self, draft: &SettingsDraft) {
        self.reloading.set(true);
        self.country.set_selected(country_index(&draft.country));
        self.country_custom.set_text(
            if country_index(&draft.country) as usize == COUNTRIES.len() {
                &draft.country
            } else {
                ""
            },
        );
        self.hubcap.set_text(&draft.hubcap_key);
        self.ryuu.set_text(&draft.ryuu_key);
        self.source
            .set_selected(source_index(&draft.default_source));
        self.auto_update.set_active(draft.auto_update_plans);
        for (id, row) in self.providers.iter() {
            row.set_active(draft.manifest_providers.iter().any(|entry| entry == id));
        }
        self.custom_chain.set_text(
            &draft
                .manifest_providers
                .iter()
                .filter(|entry| !is_builtin(entry))
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
        self.reloading.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn draft() -> SettingsDraft {
        SettingsDraft {
            country: "us".into(),
            hubcap_key: String::new(),
            ryuu_key: String::new(),
            default_source: "auto".into(),
            manifest_providers: vec!["steamrun".into()],
            auto_update_plans: false,
        }
    }

    #[test]
    fn clean_form_only_becomes_saveable_after_a_real_edit() {
        let baseline = draft();
        let mut tracker = SaveTracker::new(baseline.clone());
        assert_eq!(tracker.state(), SaveState::Clean);
        assert!(!tracker.can_save());

        tracker.sync(&baseline);
        assert_eq!(tracker.state(), SaveState::Clean);
        assert!(!tracker.can_save());

        let mut edited = baseline.clone();
        edited.auto_update_plans = true;
        tracker.sync(&edited);
        assert_eq!(tracker.state(), SaveState::Dirty);
        assert!(tracker.can_save());

        tracker.sync(&baseline);
        assert_eq!(tracker.state(), SaveState::Clean);
        assert!(!tracker.can_save());
    }

    #[test]
    fn every_saved_field_participates_in_dirty_tracking() {
        let baseline = draft();
        let mut edits = Vec::new();

        let mut edited = baseline.clone();
        edited.country = "gb".into();
        edits.push(edited);
        let mut edited = baseline.clone();
        edited.hubcap_key = "test-hubcap".into();
        edits.push(edited);
        let mut edited = baseline.clone();
        edited.ryuu_key = "test-ryuu".into();
        edits.push(edited);
        let mut edited = baseline.clone();
        edited.default_source = "ryuu".into();
        edits.push(edited);
        let mut edited = baseline.clone();
        edited.manifest_providers.push("manifestdex".into());
        edits.push(edited);
        let mut edited = baseline.clone();
        edited.auto_update_plans = true;
        edits.push(edited);

        for edited in edits {
            let mut tracker = SaveTracker::new(baseline.clone());
            tracker.sync(&edited);
            assert_eq!(tracker.state(), SaveState::Dirty);
            assert!(tracker.can_save());
        }
    }

    #[test]
    fn worker_normalized_whitespace_is_not_a_false_dirty_or_error_state() {
        let baseline = draft();
        let mut edited = baseline.clone();
        edited.country = " US ".into();
        edited.hubcap_key = "  ".into();
        edited.ryuu_key = "  ".into();
        edited.default_source = " auto ".into();
        edited.manifest_providers = vec![" steamrun ".into(), "  ".into()];
        let mut tracker = SaveTracker::new(baseline.clone());

        tracker.sync(&edited);
        assert_eq!(tracker.state(), SaveState::Clean);
        assert!(!tracker.can_save());

        let mut actual_edit = edited.clone();
        actual_edit.auto_update_plans = true;
        tracker.sync(&actual_edit);
        assert!(tracker.begin_save(&actual_edit));
        let mut reloaded = baseline.clone();
        reloaded.auto_update_plans = true;
        assert!(matches!(
            tracker.finish_reload(reloaded),
            ReloadOutcome::Saved
        ));
        assert_eq!(tracker.state(), SaveState::Saved);
    }

    #[test]
    fn matching_save_reload_passes_through_saving_to_saved() {
        let baseline = draft();
        let mut edited = baseline.clone();
        edited.auto_update_plans = true;
        let mut tracker = SaveTracker::new(baseline);
        tracker.sync(&edited);

        assert!(tracker.begin_save(&edited));
        assert_eq!(tracker.state(), SaveState::Saving);
        assert!(!tracker.can_save());
        assert!(!tracker.begin_save(&edited));

        assert!(matches!(
            tracker.finish_reload(edited),
            ReloadOutcome::Saved
        ));
        assert_eq!(tracker.state(), SaveState::Saved);
        assert!(!tracker.can_save());
    }

    #[test]
    fn post_submit_edits_survive_matching_reload() {
        let baseline = draft();
        let mut submitted = baseline.clone();
        submitted.default_source = "hubcap".into();
        let mut tracker = SaveTracker::new(baseline);
        tracker.sync(&submitted);
        assert!(tracker.begin_save(&submitted));

        let latest = SettingsDraft {
            country: country_code(COUNTRIES.len() as u32, " NL "),
            hubcap_key: "post-submit-hubcap".into(),
            ryuu_key: "post-submit-ryuu".into(),
            default_source: "custom".into(),
            manifest_providers: vec![
                "manifestdex".into(),
                "{\"name\":\"mirror\",\"url\":\"https://m/{gid}\"}".into(),
            ],
            auto_update_plans: true,
        };
        assert_eq!(latest.country, "nl");
        tracker.sync(&latest);

        let restored = match tracker.finish_reload(submitted) {
            ReloadOutcome::Restore(restored) => restored,
            _ => panic!("post-submit edits must survive a matching reload"),
        };
        assert!(drafts_equal(&restored, &latest));
        assert!(restored.auto_update_plans);
        assert_eq!(tracker.state(), SaveState::Dirty);
        assert!(tracker.can_save());
    }

    #[test]
    fn post_submit_edits_survive_mismatched_reload_as_dirty() {
        let baseline = draft();
        let mut submitted = baseline.clone();
        submitted.auto_update_plans = true;
        let mut tracker = SaveTracker::new(baseline.clone());
        tracker.sync(&submitted);
        assert!(tracker.begin_save(&submitted));

        let mut latest = submitted.clone();
        latest.country = country_code(COUNTRIES.len() as u32, " GB ");
        latest.auto_update_plans = false;
        tracker.sync(&latest);

        let restored = match tracker.finish_reload(baseline) {
            ReloadOutcome::Restore(restored) => restored,
            _ => panic!("post-submit edits must survive a mismatched reload"),
        };
        assert_eq!(restored.country, "gb");
        assert!(!restored.auto_update_plans);
        assert!(drafts_equal(&restored, &latest));
        assert_eq!(tracker.state(), SaveState::Dirty);
        assert!(tracker.can_save());
    }

    #[test]
    fn post_submit_edits_do_not_unlock_single_flight() {
        let baseline = draft();
        let mut submitted = baseline.clone();
        submitted.default_source = "hubcap".into();
        let mut tracker = SaveTracker::new(baseline);
        tracker.sync(&submitted);
        assert!(tracker.begin_save(&submitted));

        let mut latest = submitted.clone();
        latest.default_source = "ryuu".into();
        tracker.sync(&latest);
        assert_eq!(tracker.state(), SaveState::Saving);
        assert!(!tracker.can_save());
        assert!(!tracker.begin_save(&latest));

        let restored = match tracker.finish_reload(submitted) {
            ReloadOutcome::Restore(restored) => restored,
            _ => panic!("the newer draft must be available after the in-flight save"),
        };
        assert!(drafts_equal(&restored, &latest));
        assert_eq!(tracker.state(), SaveState::Dirty);
        assert!(tracker.begin_save(&restored));
        assert!(!tracker.begin_save(&restored));
    }

    #[test]
    fn reload_without_a_pending_save_returns_clean() {
        let loaded = draft();
        let mut tracker = SaveTracker::new(loaded.clone());
        assert!(matches!(
            tracker.finish_reload(loaded),
            ReloadOutcome::Clean
        ));
        assert_eq!(tracker.state(), SaveState::Clean);
        assert!(!tracker.can_save());
    }

    #[test]
    fn mismatched_reload_restores_edits_and_allows_retry() {
        let baseline = draft();
        let mut edited = baseline.clone();
        edited.default_source = "hubcap".into();
        let mut tracker = SaveTracker::new(baseline.clone());
        tracker.sync(&edited);
        assert!(tracker.begin_save(&edited));

        let restored = match tracker.finish_reload(baseline.clone()) {
            ReloadOutcome::Restore(restored) => restored,
            _ => panic!("an unconfirmed save must restore the pending edits"),
        };
        assert!(drafts_equal(&restored, &edited));
        assert_eq!(tracker.state(), SaveState::Error);
        assert!(tracker.can_save());

        tracker.sync(&restored);
        assert_eq!(tracker.state(), SaveState::Error);
        assert!(tracker.begin_save(&restored));
        assert_eq!(tracker.state(), SaveState::Saving);
    }

    #[test]
    fn failed_reload_keeps_edits_saveable_and_reverting_clears_error() {
        let baseline = draft();
        let mut edited = baseline.clone();
        edited.ryuu_key = "test-ryuu".into();
        let mut tracker = SaveTracker::new(baseline.clone());
        tracker.sync(&edited);
        assert!(tracker.begin_save(&edited));

        tracker.fail_reload(&edited);
        assert_eq!(tracker.state(), SaveState::Error);
        assert!(tracker.can_save());

        tracker.sync(&baseline);
        assert_eq!(tracker.state(), SaveState::Clean);
        assert!(!tracker.can_save());
    }

    #[test]
    fn source_labels_fit_normally_and_wrap_instead_of_truncating_when_compact() {
        let layout = SOURCE_LABEL_LAYOUT;
        for label in SOURCE_LABELS {
            assert!(label.chars().count() <= layout.max_width_chars as usize);
        }
        assert!(layout.width_chars >= "Custom plan source".chars().count() as i32);
        assert!(layout.wrap);
        assert!(!layout.truncate);
    }

    #[test]
    fn save_reads_current_form_on_every_click() {
        let form = RefCell::new(SettingsDraft {
            country: "us".into(),
            hubcap_key: String::new(),
            ryuu_key: String::new(),
            default_source: "auto".into(),
            manifest_providers: Vec::new(),
            auto_update_plans: true,
        });
        let sent = RefCell::new(Vec::new());
        let save = save_action(
            || form.borrow().clone(),
            |command| sent.borrow_mut().push(command),
        );
        assert!(sent.borrow().is_empty());

        *form.borrow_mut() = SettingsDraft {
            country: "gb".into(),
            hubcap_key: "test-hubcap".into(),
            ryuu_key: "test-ryuu".into(),
            default_source: "hubcap".into(),
            manifest_providers: Vec::new(),
            auto_update_plans: true,
        };
        save();

        *form.borrow_mut() = SettingsDraft {
            country: "de".into(),
            hubcap_key: String::new(),
            ryuu_key: String::new(),
            default_source: "ryuu".into(),
            manifest_providers: Vec::new(),
            auto_update_plans: true,
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

    #[test]
    fn source_and_country_presets_keep_the_stored_values() {
        assert_eq!(source_name(source_index("auto")), "auto");
        assert_eq!(source_name(source_index("hubcap")), "hubcap");
        assert_eq!(source_name(source_index("ryuu")), "ryuu");
        assert_eq!(source_name(source_index("custom")), "custom");
        assert_eq!(country_code(country_index("ua"), ""), "ua");
        assert_eq!(country_code(country_index("nl"), "NL"), "nl");
    }

    #[test]
    fn custom_chain_filters_builtin_ids() {
        let custom =
            parse_provider_chain("steamrun, {\"name\":\"mirror\",\"url\":\"https://m/{gid}\"}");
        let filtered: Vec<String> = custom
            .into_iter()
            .filter(|entry| !is_builtin(entry))
            .collect();
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].starts_with('{'));
    }
}
