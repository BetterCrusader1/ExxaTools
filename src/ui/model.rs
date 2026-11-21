//! Pure view models for the dashboard: no GTK here, so every rule is testable.

use serde_json::Value;

use crate::state::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Library,
    Search,
    Discover,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    Ready,
    Planned,
    KeysMissing,
    Disabled,
}

impl GameState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Planned => "planned",
            Self::KeysMissing => "keys_missing",
            Self::Disabled => "disabled",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Planned => "Not set up",
            Self::KeysMissing => "Keys missing",
            Self::Disabled => "Disabled",
        }
    }

    pub fn dot_class(self) -> &'static str {
        match self {
            Self::Ready => "dot-ready",
            Self::Planned => "dot-planned",
            Self::KeysMissing => "dot-warning",
            Self::Disabled => "dot-disabled",
        }
    }

    pub fn pill_class(self) -> &'static str {
        match self {
            Self::Ready => "pill-ready",
            Self::Planned => "pill-planned",
            Self::KeysMissing => "pill-warning",
            Self::Disabled => "pill-disabled",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Depot {
    pub id: u64,
    pub gid: String,
    pub size: Option<u64>,
    pub has_key: bool,
}

impl Depot {
    pub fn manifest(&self) -> &str {
        if self.gid.is_empty() {
            "Provided at launch"
        } else {
            &self.gid
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub appid: u64,
    pub name: String,
    pub branch: String,
    pub depots: Vec<Depot>,
    pub state: GameState,
    pub installed: bool,
    pub updated: Option<u64>,
    pub source: String,
    pub dlc: usize,
    pub origin: Origin,
    pub cover: Option<std::path::PathBuf>,
    pub cover_url: Option<String>,
    pub hero: Option<std::path::PathBuf>,
    pub logo: Option<std::path::PathBuf>,
    pub capsule: Option<std::path::PathBuf>,
}

impl Game {
    pub fn from_record(state: &State, record: &Value) -> Option<Self> {
        let appid = record.get("appid").and_then(Value::as_u64)?;
        let depots = record
            .get("depots")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|depot| {
                        let id = depot.get("depot_id").and_then(Value::as_u64)?;
                        Some(Depot {
                            id,
                            gid: depot
                                .get("gid")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            size: depot.get("size").and_then(Value::as_u64),
                            has_key: state.depot_keys.contains_key(&id.to_string()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let summary = crate::state::app_summary(state, record);
        let status = summary
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("planned");
        let state = match status {
            "ready" => GameState::Ready,
            "keys_missing" => GameState::KeysMissing,
            "disabled" => GameState::Disabled,
            _ => GameState::Planned,
        };
        Some(Self {
            appid,
            name: record
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Unknown app")
                .to_string(),
            branch: record
                .get("branch")
                .and_then(Value::as_str)
                .unwrap_or("public")
                .to_string(),
            depots,
            state,
            installed: summary
                .get("installed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            updated: parse_updated(record.get("updated")),
            source: record
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("local")
                .to_string(),
            dlc: record
                .get("dlc")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0),
            origin: Origin::Library,
            cover: None,
            cover_url: None,
            hero: None,
            logo: None,
            capsule: None,
        })
    }

    pub fn from_search(value: &Value) -> Option<Self> {
        let appid = value.get("appid").and_then(Value::as_u64)?;
        Some(Self {
            appid,
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Unknown app")
                .to_string(),
            branch: "public".into(),
            depots: Vec::new(),
            state: GameState::Planned,
            installed: false,
            updated: None,
            source: "steam".into(),
            dlc: 0,
            origin: Origin::Search,
            cover: None,
            cover_url: value
                .get("image")
                .and_then(Value::as_str)
                .filter(|url| url.starts_with("https://"))
                .map(str::to_string),
            hero: None,
            logo: None,
            capsule: None,
        })
    }

    pub fn from_discover(value: &Value) -> Option<Self> {
        let mut game = Self::from_search(value)?;
        game.origin = Origin::Discover;
        Some(game)
    }

    pub fn subtitle(&self) -> String {
        if self.depots.is_empty() {
            return self.branch.clone();
        }
        format!(
            "{} branch, {} {}",
            self.branch,
            self.depots.len(),
            if self.depots.len() == 1 {
                "depot"
            } else {
                "depots"
            }
        )
    }

    pub fn meta(&self) -> String {
        format!("{} branch, {}", self.branch, self.depot_word())
    }

    pub fn depot_word(&self) -> String {
        format!(
            "{} {}",
            self.depots.len(),
            if self.depots.len() == 1 {
                "depot"
            } else {
                "depots"
            }
        )
    }

    pub fn initial(&self) -> String {
        self.name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoctorItem {
    pub id: String,
    pub label: String,
    pub status: String,
    pub detail: String,
    pub hint: String,
}

impl DoctorItem {
    pub fn ok(&self) -> bool {
        self.status == "ok"
    }

    pub fn bad(&self) -> bool {
        self.status == "bad"
    }

    pub fn group(&self) -> &'static str {
        match self.id.as_str() {
            "engine" | "engine_loaded" | "ld_audit" => "Engine",
            "lifecycle" | "steam" | "debug_port" => "Steam",
            "scripts" | "provider" | "plans" | "keys" => "Plans",
            "disk" | "appmanifest" => "Storage",
            "hubcap" => "Sources",
            _ => "Other",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DoctorReport {
    pub items: Vec<DoctorItem>,
    pub warnings: usize,
    pub ok: bool,
}

impl DoctorReport {
    pub fn from_value(value: &Value) -> Self {
        let items = value
            .get("items")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| DoctorItem {
                        id: item.get("id").and_then(Value::as_str).unwrap_or("").into(),
                        label: item
                            .get("label")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                        status: item
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("warn")
                            .into(),
                        detail: item
                            .get("detail")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                        hint: item
                            .get("hint")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            warnings: value.get("warnings").and_then(Value::as_u64).unwrap_or(0) as usize,
            ok: value.get("ok").and_then(Value::as_bool).unwrap_or(false),
            items,
        }
    }

    pub fn failed(&self) -> usize {
        self.items.iter().filter(|item| item.bad()).count()
    }

    pub fn summary(&self) -> (&'static str, &'static str) {
        if self.failed() > 0 {
            ("dialog-error-symbolic", "Some checks need attention")
        } else if self.warnings > 0 || self.items.iter().any(|item| item.status == "warn") {
            ("dialog-warning-symbolic", "Mostly ready")
        } else {
            ("emblem-ok-symbolic", "Everything is in order")
        }
    }
}

pub fn parse_updated(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::String(text) => text.parse().ok(),
        Value::Number(number) => number.as_u64(),
        _ => None,
    }
}

pub fn relative_time(epoch: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    if epoch == 0 || epoch > now.saturating_add(86_400) {
        return "not yet refreshed".into();
    }
    let seconds = now.saturating_sub(epoch);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", seconds / 60),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        86_400..=2_591_999 => format!("{} days ago", seconds / 86_400),
        _ => format!("{} months ago", seconds / 2_592_000),
    }
}

pub fn human_size(bytes: u64) -> String {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    if bytes as f64 >= GB {
        format!("{:.1} GB", bytes as f64 / GB)
    } else if bytes as f64 >= MB {
        format!("{:.0} MB", bytes as f64 / MB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state_with_key(id: u64) -> State {
        let mut state = State::default();
        state.depot_keys.insert(id.to_string(), "a".repeat(64));
        state
    }

    #[test]
    fn a_record_becomes_a_game_with_manifest_and_key_state() {
        let state = state_with_key(620);
        let record = json!({
            "appid": 620,
            "name": "Portal 2",
            "branch": "public",
            "source": "hubcap",
            "updated": "1700000000",
            "depots": [
                {"depot_id": 620, "gid": "1234567890123456789", "size": 2048},
                {"depot_id": 621, "gid": "", "size": null}
            ]
        });
        let game = Game::from_record(&state, &record).expect("game");
        assert_eq!(game.name, "Portal 2");
        assert_eq!(game.depots.len(), 2);
        assert!(game.depots[0].has_key);
        assert!(!game.depots[1].has_key);
        assert_eq!(game.depots[1].manifest(), "Provided at launch");
        assert_eq!(game.updated, Some(1_700_000_000));
        assert_eq!(game.origin, Origin::Library);
    }

    #[test]
    fn search_rows_are_not_library_rows() {
        let game = Game::from_search(&json!({"appid": 620, "name": "Portal 2"})).expect("game");
        assert_eq!(game.origin, Origin::Search);
        assert!(!game.installed);
        assert!(game.depots.is_empty());
    }

    #[test]
    fn doctor_report_reads_status_and_groups() {
        let report = DoctorReport::from_value(&json!({
            "ok": false,
            "warnings": 1,
            "items": [
                {"id": "engine", "label": "Engine", "status": "ok", "detail": "1.2", "hint": ""},
                {"id": "disk", "label": "Disk", "status": "bad", "detail": "full", "hint": "free space"}
            ]
        }));
        assert_eq!(report.items.len(), 2);
        assert_eq!(report.failed(), 1);
        assert_eq!(report.items[1].group(), "Storage");
        assert_eq!(report.summary().1, "Some checks need attention");
    }

    #[test]
    fn sizes_and_time_read_like_ui_copy() {
        assert_eq!(human_size(2 * 1024 * 1024 * 1024), "2.0 GB");
        assert_eq!(human_size(512 * 1024 * 1024), "512 MB");
        assert_eq!(relative_time(0), "not yet refreshed");
    }
}
