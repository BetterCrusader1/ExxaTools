//! Plan sources: `.lua` files that carry depot keys and manifest pins.
//!
//! Every configured source is fetched. Steam's current catalog is the freshness
//! authority; if it is offline, the largest gid per depot is the fallback. Keys
//! merge across sources, while enabled flags and fallback sizes stay local.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};

use crate::state::{self, Settings};

pub struct PlanSource {
    pub name: String,
    pub kind: Kind,
}

pub enum Kind {
    Hubcap(String),
    Ryuu(String),
    Catalog,
    Manilua,
    Steamtools,
    Custom {
        url: String,
        header: Option<(String, String)>,
    },
}

/// Built-in sources in the order they are tried. A source without a key is
/// skipped instead of failing the whole request.
pub fn sources(settings: &Settings) -> Vec<PlanSource> {
    let mut out = Vec::new();
    // Steam catalog is always available as a keyless fallback (no depot keys).
    out.push(PlanSource {
        name: "catalog".into(),
        kind: Kind::Catalog,
    });
    if !settings.manilua_disabled {
        out.push(PlanSource {
            name: "manilua".into(),
            kind: Kind::Manilua,
        });
    }
    if !settings.steamtools_disabled {
        out.push(PlanSource {
            name: "steamtools".into(),
            kind: Kind::Steamtools,
        });
    }
    if !settings.hubcap_key.trim().is_empty() {
        out.push(PlanSource {
            name: "hubcap".into(),
            kind: Kind::Hubcap(settings.hubcap_key.clone()),
        });
    }
    if !settings.ryuu_key.trim().is_empty() {
        out.push(PlanSource {
            name: "ryuu".into(),
            kind: Kind::Ryuu(settings.ryuu_key.clone()),
        });
    }
    for (name, spec) in &settings.plan_sources {
        let Some(object) = spec.as_object() else {
            continue;
        };
        let Some(url) = object
            .get("url")
            .and_then(Value::as_str)
            .filter(|value| value.contains("{appid}"))
        else {
            continue;
        };
        let header = object
            .get("header")
            .and_then(Value::as_str)
            .and_then(|pair| pair.split_once(':'))
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()));
        out.push(PlanSource {
            name: name.clone(),
            kind: Kind::Custom {
                url: url.to_string(),
                header,
            },
        });
    }
    out
}

impl PlanSource {
    fn fetch(&self, appid: u64) -> Result<String, String> {
        match &self.kind {
            Kind::Catalog => {
                let catalog = crate::steam::catalog(appid).map_err(|e| e.to_string())?;
                let branch = "public";
                let depots = catalog_depots(Some(&catalog), branch);
                if depots.is_empty() {
                    return Err(format!("no downloadable depots found for {appid}"));
                }
                let depots: Vec<Value> = depots
                    .into_iter()
                    .map(|(depot_id, (gid, size))| {
                        json!({ "depot_id": depot_id, "gid": gid, "size": size, "enabled": true })
                    })
                    .collect();
                let name = catalog
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let dlc: Vec<u64> = catalog
                    .get("dlc")
                    .and_then(Value::as_array)
                    .map(|list| list.iter().filter_map(Value::as_u64).collect())
                    .unwrap_or_default();
                let text = crate::lua::render_plan(
                    appid,
                    &name,
                    branch,
                    &depots,
                    &BTreeMap::new(),
                    &dlc,
                    &crate::vfsys::stamp(),
                    "catalog",
                );
                Ok(text)
            }
            Kind::Hubcap(key) => crate::hubcap::fetch(key, appid)
                .map(|fetched| fetched.lua)
                .map_err(|error| error.to_string()),
            Kind::Ryuu(key) => {
                let url = format!("https://generator.ryuu.lol/api/download/{appid}?file_type=lua");
                let body = crate::http::get_with_headers(&url, &[("X-Auth-Key", key.as_str())])
                    .map_err(|error| error.to_string())?;
                let text = String::from_utf8_lossy(&body).into_owned();
                if text.trim_start().starts_with('<') {
                    return Err("ryuu answered with an HTML page (login or challenge)".into());
                }
                if text.trim().is_empty() {
                    return Err("ryuu returned an empty file".into());
                }
                Ok(text)
            }
            Kind::Manilua => keyless_lua(
                &format!("https://maniluahub.com/api/files/{appid}/lua"),
                "manilua",
            ),
            Kind::Steamtools => keyless_lua(
                &format!("https://steamtools.games/api/files/{appid}/lua"),
                "steamtools",
            ),
            Kind::Custom { url, header } => {
                let target = url.replace("{appid}", &appid.to_string());
                let headers: Vec<(&str, &str)> = header
                    .as_ref()
                    .map(|(key, value)| vec![(key.as_str(), value.as_str())])
                    .unwrap_or_default();
                let body = crate::http::get_with_headers(&target, &headers)
                    .map_err(|error| error.to_string())?;
                Ok(String::from_utf8_lossy(&body).into_owned())
            }
        }
    }
}

struct SourcePlan {
    name: String,
    parsed: crate::hubcap::Parsed,
}

/// A keyless registry answers with a plan lua. Require at least one real depot
/// key: registries inflate placeholders (an all-zero key for an unknown app),
/// and passing one on would turn into a keyless plan instead of letting the
/// next source answer.
fn keyless_lua(url: &str, label: &str) -> Result<String, String> {
    let body = crate::http::get_with(url, None, None).map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&body).into_owned();
    if text.trim_start().starts_with('<') {
        return Err(format!("{label} answered with an HTML page"));
    }
    let parsed = crate::hubcap::parse_lua(&text, None);
    let has_real_key = parsed.depots.iter().any(|depot| {
        depot
            .get("key")
            .and_then(Value::as_str)
            .is_some_and(|key| {
                key.len() == 64
                    && key.chars().all(|ch| ch.is_ascii_hexdigit())
                    && key.chars().any(|ch| ch != '0')
            })
    });
    if !has_real_key {
        return Err(format!("{label} has no depot keys for this app"));
    }
    Ok(text)
}

fn fetch_all_sources(appid: u64, settings: &Settings) -> (Vec<SourcePlan>, Vec<Value>) {
    fetch_sources_filtered(appid, settings, false)
}

/// Quota-carrying sources (Hubcap daily limits, Ryuu free keys) burn their
/// allowance on every round otherwise. When each planned depot already has a
/// key they add nothing, so only the free keyless registries are asked.
fn fetch_sources_filtered(
    appid: u64,
    settings: &Settings,
    skip_quota: bool,
) -> (Vec<SourcePlan>, Vec<Value>) {
    let mut plans = Vec::new();
    let mut failures = Vec::new();
    for source in sources(settings) {
        if skip_quota && matches!(source.kind, Kind::Hubcap(_) | Kind::Ryuu(_)) {
            continue;
        }
        match source.fetch(appid) {
            Ok(lua) => {
                let parsed = crate::hubcap::parse_lua(&lua, Some(appid));
                if parsed.appid.is_none() {
                    failures.push(
                        json!({ "provider": source.name, "error": "the file has no app id" }),
                    );
                } else {
                    plans.push(SourcePlan {
                        name: source.name,
                        parsed,
                    });
                }
            }
            Err(error) => failures.push(json!({ "provider": source.name, "error": error })),
        }
    }
    (plans, failures)
}

fn import_keys(st: &mut state::State, plans: &[SourcePlan]) -> usize {
    let mut imported = 0;
    for plan in plans {
        for depot in &plan.parsed.depots {
            let Some(depot_id) = depot.get("depot_id").and_then(Value::as_u64) else {
                continue;
            };
            let key = depot.get("key").and_then(Value::as_str).unwrap_or("");
            if key.len() == 64 && st.depot_keys.get(&depot_id.to_string()) != Some(&key.to_string())
            {
                st.depot_keys.insert(depot_id.to_string(), key.to_string());
                imported += 1;
            }
        }
    }
    imported
}

fn catalog_depots(catalog: Option<&Value>, branch: &str) -> BTreeMap<u64, (String, Option<u64>)> {
    let mut out = BTreeMap::new();
    let Some(list) = catalog
        .and_then(|value| value.get("depots"))
        .and_then(Value::as_array)
    else {
        return out;
    };
    for depot in list {
        let Some(depot_id) = depot.get("depot_id").and_then(Value::as_u64) else {
            continue;
        };
        let Some(manifest) = crate::steam::pick_manifest(depot, branch) else {
            continue;
        };
        let gid = manifest.get("gid").and_then(Value::as_str).unwrap_or("");
        if gid.is_empty() {
            continue;
        }
        out.insert(
            depot_id,
            (
                gid.to_string(),
                manifest.get("size").and_then(Value::as_u64),
            ),
        );
    }
    out
}

/// `oslist` of every depot the catalog knows, keyed by depot id. An empty list
/// means the depot is platform-neutral. Depots missing from the catalog are not
/// in the map at all: they are shared redistributables or source-only entries.
fn catalog_oslists(catalog: Option<&Value>) -> BTreeMap<u64, Vec<String>> {
    let mut out = BTreeMap::new();
    let Some(list) = catalog
        .and_then(|value| value.get("depots"))
        .and_then(Value::as_array)
    else {
        return out;
    };
    for depot in list {
        let Some(depot_id) = depot.get("depot_id").and_then(Value::as_u64) else {
            continue;
        };
        let oslist: Vec<String> = depot
            .get("oslist")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        out.insert(depot_id, oslist);
    }
    out
}

fn os_allows(oslist: &[String], os: &str) -> bool {
    oslist.is_empty() || oslist.iter().any(|value| value == os)
}

/// The platform this app is actually going to run on. A game with native depots
/// for the preferred OS uses them; a game without them runs through Proton, so
/// its Windows depots are the right ones and must stay.
fn effective_os(catalog: Option<&Value>, preferred: &str) -> String {
    let oslists = catalog_oslists(catalog);
    let has = |os: &str| {
        oslists
            .values()
            .any(|list| list.iter().any(|value| value == os))
    };
    if has(preferred) || !has("windows") {
        preferred.to_string()
    } else {
        "windows".to_string()
    }
}

fn fresh_depots(plans: &[SourcePlan], catalog: Option<&Value>, branch: &str, os: &str) -> Vec<Value> {
    // A depot built for another OS must never enter the plan: platform depots
    // share file paths with the native ones (a macOS `launcher/dowser` next to
    // the Linux one), so mounting them overwrites the native files during the
    // download and the game cannot start. Depots the catalog does not list are
    // shared redistributables and are always kept.
    let oslists = catalog_oslists(catalog);
    let platform_ok = |depot_id: u64| match oslists.get(&depot_id) {
        Some(oslist) => os_allows(oslist, os),
        None => true,
    };
    let mut candidates: BTreeMap<u64, Vec<(String, Option<u64>)>> = BTreeMap::new();
    // Add catalog depots as candidates (same weight as any source)
    if let Some(cat) = catalog {
        let cat_depots = catalog_depots(Some(cat), branch);
        for (depot_id, (gid, size)) in cat_depots {
            if !platform_ok(depot_id) {
                continue;
            }
            candidates.entry(depot_id).or_default().push((gid, size));
        }
    }
    // Add plan source depots as candidates
    for plan in plans {
        for depot in &plan.parsed.depots {
            let Some(depot_id) = depot.get("depot_id").and_then(Value::as_u64) else {
                continue;
            };
            if !platform_ok(depot_id) {
                continue;
            }
            let gid = depot.get("gid").and_then(Value::as_str).unwrap_or("");
            if gid.is_empty() || !gid.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            candidates
                .entry(depot_id)
                .or_default()
                .push((gid.to_string(), depot.get("size").and_then(Value::as_u64)));
        }
    }
    let mut out = Vec::new();
    for (depot_id, list) in candidates {
        let mut best = &list[0];
        for candidate in &list[1..] {
            let candidate_gid = candidate.0.parse::<u64>().unwrap_or(0);
            let best_gid = best.0.parse::<u64>().unwrap_or(0);
            if candidate_gid > best_gid {
                best = candidate;
            }
        }
        out.push(json!({ "depot_id": depot_id, "gid": best.0, "size": best.1 }));
    }
    out
}

fn preserve_previous(previous: &[Value], fresh: Vec<Value>) -> Vec<Value> {
    fresh
        .into_iter()
        .map(|mut entry| {
            let depot_id = entry.get("depot_id").and_then(Value::as_u64);
            if let Some(old) = previous
                .iter()
                .find(|old| old.get("depot_id").and_then(Value::as_u64) == depot_id)
            {
                if let Some(enabled) = old.get("enabled") {
                    entry["enabled"] = enabled.clone();
                }
                if entry["size"].is_null() {
                    if let Some(size) = old.get("size") {
                        entry["size"] = size.clone();
                    }
                }
            }
            entry
        })
        .collect()
}

/// A depot with no key can never be mounted: Steam would retry an encrypted
/// mount and cancel the update with "content still encrypted". Keep the record
/// (a later key restores it) but switch it off so the renderer, the summary
/// and the doctor all agree on which depots are actually installable.
///
/// Depots the catalog does not list for this app are shared redistributables
/// (228981/228988 and friends): they are not encrypted, so they mount without
/// a key and must stay on.
fn sync_enabled(
    mut depots: Vec<Value>,
    keys: &BTreeMap<String, String>,
    own: Option<&BTreeSet<u64>>,
) -> Vec<Value> {
    for depot in depots.iter_mut() {
        let Some(id) = depot.get("depot_id").and_then(Value::as_u64) else {
            continue;
        };
        let keyed = keys
            .get(&id.to_string())
            .and_then(|key| state::valid_key(key))
            .is_some();
        // Without a catalog every keyless depot is treated as encrypted, so a
        // failed catalog fetch can never mount something undecryptable.
        let shared = !keyed && own.is_some_and(|own| !own.contains(&id));
        if let Some(object) = depot.as_object_mut() {
            object.insert("enabled".into(), Value::Bool(keyed || shared));
            if shared {
                object.insert("shared".into(), Value::Bool(true));
            } else {
                object.remove("shared");
            }
        }
    }
    depots
}

fn catalog_depot_ids(catalog: Option<&Value>) -> Option<BTreeSet<u64>> {
    catalog
        .and_then(|value| value.get("depots"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|depot| depot.get("depot_id").and_then(Value::as_u64))
                .collect::<BTreeSet<u64>>()
        })
        .filter(|set| !set.is_empty())
}

fn dlc_union(plans: &[SourcePlan]) -> Vec<u64> {
    plans
        .iter()
        .flat_map(|plan| plan.parsed.dlc.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Depots straight from the public Steam catalog, used when no plan source is configured.
fn catalog_only_depots(catalog: &Value, branch: &str) -> Vec<Value> {
    catalog_depots(Some(catalog), branch)
        .into_iter()
        .map(|(depot_id, (gid, size))| json!({ "depot_id": depot_id, "gid": gid, "size": size }))
        .collect()
}

/// Everything a plan record needs, gathered before the state transaction.
struct PlanDraft {
    appid: u64,
    branch: String,
    depots: Vec<Value>,
    dlc: Vec<u64>,
    providers: Vec<String>,
    resolved: Option<String>,
    source_depots: usize,
    /// Depot ids the app's own catalog lists; anything else in the plan is a
    /// shared redistributable depot. None when the catalog is unavailable.
    own_depots: Option<BTreeSet<u64>>,
}

/// Persists a plan record from prepared depots and returns the summary payload.
fn save_plan(draft: PlanDraft, plans: &[SourcePlan]) -> Result<Value, String> {
    let PlanDraft {
        appid,
        branch,
        depots,
        dlc,
        providers,
        resolved,
        source_depots,
        own_depots,
    } = draft;
    state::update(|fresh| {
        let keys = import_keys(fresh, plans);
        let created = !fresh.apps.contains_key(&appid.to_string());
        let previous = fresh
            .apps
            .get(&appid.to_string())
            .and_then(|record| record.get("depots"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let depots = preserve_previous(&previous, depots);
        let depots = sync_enabled(depots, &fresh.depot_keys, own_depots.as_ref());
        let missing: Vec<u64> = depots
            .iter()
            .filter(|depot| !depot.get("shared").and_then(Value::as_bool).unwrap_or(false))
            .filter_map(|depot| depot.get("depot_id").and_then(Value::as_u64))
            .filter(|id| !fresh.depot_keys.contains_key(&id.to_string()))
            .collect();
        let source = if providers.len() == 1 {
            providers[0].clone()
        } else if providers.is_empty() {
            "catalog".to_string()
        } else {
            "multi".to_string()
        };
        if created {
            fresh.apps.insert(appid.to_string(), json!({
                "appid": appid,
                "name": resolved.unwrap_or_else(|| appid.to_string()),
                "branch": branch,
                "depots": depots,
                "dlc": dlc.clone(),
                "source": source,
                "sources": &providers,
                "fixes": { "ticket": "forge", "steam_id": "", "avatar": 0, "purchase_time": 0, "online_fix": false },
                "updated": crate::vfsys::stamp(),
            }));
        } else if let Some(record) = fresh.apps.get_mut(&appid.to_string()) {
            record["depots"] = Value::Array(depots);
            record["sources"] = json!(&providers);
            record["source"] = json!(source);
            record["updated"] = json!(crate::vfsys::stamp());
        }
        Ok(json!({
            "appid": appid,
            "keys": keys,
            "depots": source_depots,
            "created_plan": created,
            "missing_keys": missing,
            "dlc": dlc,
        }))
    })
}

#[allow(dead_code)]
/// Builds and installs a plan from the public Steam catalog (no depot keys).
fn catalog_plan(
    appid: u64,
    branch: &str,
    catalog: &Value,
    resolved: Option<String>,
    failures: Vec<Value>,
) -> Result<Value, String> {
    let depots = catalog_only_depots(catalog, branch);
    if depots.is_empty() {
        return Err(format!("no downloadable depots found for {appid}"));
    }
    let dlc: Vec<u64> = catalog
        .get("dlc")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let count = depots.len();
    let own_depots = catalog_depot_ids(Some(catalog));
    let summary = save_plan(
        PlanDraft {
            appid,
            branch: branch.to_string(),
            depots,
            dlc,
            providers: Vec::new(),
            resolved,
            source_depots: count,
            own_depots,
        },
        &[],
    )?;
    let installed = write_script(appid).unwrap_or(false);
    Ok(json!({
        "provider": "catalog",
        "providers": [],
        "summary": summary,
        "installed": installed,
        "failures": failures,
    }))
}

pub fn fetch_plan(appid: u64, name_hint: Option<&str>) -> Result<Value, String> {
    let settings = state::load()?;
    let branch = settings
        .apps
        .get(&appid.to_string())
        .and_then(|record| record.get("branch"))
        .and_then(Value::as_str)
        .unwrap_or("public")
        .to_string();
    let catalog = crate::steam::catalog(appid).ok();
    let resolved = name_hint.map(str::to_string).or_else(|| {
        catalog.as_ref().and_then(|catalog| {
            catalog
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    });
    let label = resolved.clone().unwrap_or_else(|| format!("app {appid}"));
    if sources(&settings.settings).is_empty() {
        // Without depot keys Steam downloads nothing usable, so a keyless plan
        // is never written: the add fails and says what is missing.
        return Err(format!(
            "Not added: no key source is set up, so there are no depot keys for {label}. Add a Hubcap or Ryuu key in Settings."
        ));
    }
    let (plans, failures) = fetch_all_sources(appid, &settings.settings);
    if plans.is_empty() {
        let reasons = failures
            .iter()
            .filter_map(|entry| {
                let provider = entry.get("provider").and_then(Value::as_str).unwrap_or("source");
                let error = entry.get("error").and_then(Value::as_str)?;
                Some(if error.to_ascii_lowercase().contains(&provider.to_ascii_lowercase()) {
                    error.to_string()
                } else {
                    format!("{provider}: {error}")
                })
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "Not added: no depot keys for {label} ({reasons}). Nothing was written; try again once the source answers."
        ));
    }
    let os = effective_os(catalog.as_ref(), &settings.settings.default_os);
    let depots = fresh_depots(&plans, catalog.as_ref(), &branch, &os);
    if depots.is_empty() {
        return Err(format!("no downloadable depots found for {appid}"));
    }
    let providers: Vec<String> = plans.iter().map(|plan| plan.name.clone()).collect();
    let dlc = dlc_union(&plans);
    let count = plans
        .iter()
        .map(|plan| plan.parsed.depots.len())
        .sum::<usize>();
    let own_depots = catalog_depot_ids(catalog.as_ref());
    let summary = save_plan(
        PlanDraft {
            appid,
            branch: branch.clone(),
            depots,
            dlc,
            providers: providers.clone(),
            resolved,
            source_depots: count,
            own_depots,
        },
        &plans,
    )?;
    let installed = write_script(appid).unwrap_or(false);
    Ok(json!({
        "provider": providers.join("+"),
        "providers": providers,
        "summary": summary,
        "installed": installed,
        "failures": failures,
    }))
}

/// Rebuilds every saved plan from all sources and the current Steam catalog.
pub fn refresh_manifests() -> Result<Value, String> {
    refresh_filtered(None)
}

/// Rebuilds one plan, for the per-row action in the Steam panel.
pub fn refresh_plan(appid: u64) -> Result<Value, String> {
    refresh_filtered(Some(appid))
}

struct RefreshInput {
    appid: u64,
    catalog: Result<Value, String>,
    sources: Vec<SourcePlan>,
    failures: Vec<Value>,
}

static REFRESH_ACTIVE: AtomicBool = AtomicBool::new(false);
static AUTO_STARTED: AtomicBool = AtomicBool::new(false);

struct RefreshGuard;

impl RefreshGuard {
    fn new() -> Result<Self, String> {
        if REFRESH_ACTIVE.swap(true, Ordering::SeqCst) {
            Err("a plan update is already running".into())
        } else {
            Ok(Self)
        }
    }
}

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        REFRESH_ACTIVE.store(false, Ordering::SeqCst);
    }
}

fn refresh_filtered(only: Option<u64>) -> Result<Value, String> {
    let _guard = RefreshGuard::new()?;
    let st = state::load()?;
    let appids: Vec<u64> = st
        .apps
        .values()
        .filter_map(|record| record.get("appid").and_then(Value::as_u64))
        .filter(|appid| only.map(|value| value == *appid).unwrap_or(true))
        .collect();
    if appids.is_empty() {
        return Ok(json!({ "updated": 0, "scripts": 0, "snapshots": 0, "failures": [] }));
    }
    let settings = st.settings.clone();
    let coverage: std::collections::BTreeMap<u64, bool> = appids
        .iter()
        .map(|appid| {
            let covered = st
                .apps
                .get(&appid.to_string())
                .and_then(|record| record.get("depots"))
                .and_then(Value::as_array)
                .is_some_and(|depots| {
                    !depots.is_empty()
                        && depots
                            .iter()
                            .all(|depot| depot.get("enabled").and_then(Value::as_bool).unwrap_or(false))
                });
            (*appid, covered)
        })
        .collect();
    drop(st);
    // Network and source I/O happen outside the writer transaction. The merge
    // later reads the newest state, so edits made meanwhile are not overwritten.
    let inputs: Vec<RefreshInput> = appids
        .into_iter()
        .map(|appid| {
            let covered = coverage.get(&appid).copied().unwrap_or(false);
            let catalog = crate::steam::catalog(appid);
            let (sources, failures) = fetch_sources_filtered(appid, &settings, covered);
            RefreshInput {
                appid,
                catalog,
                sources,
                failures,
            }
        })
        .collect();
    let (refreshed, snapshots, failures) =
        state::update(|fresh| Ok(merge_refresh_inputs(fresh, inputs)))?;
    let mut rewritten = 0;
    for &appid in &refreshed {
        if crate::vfsys::script_path(appid).exists() && write_script(appid).unwrap_or(false) {
            rewritten += 1;
        }
    }
    Ok(
        json!({ "updated": refreshed.len(), "scripts": rewritten, "snapshots": snapshots, "failures": failures }),
    )
}

fn merge_refresh_inputs(
    st: &mut state::State,
    inputs: Vec<RefreshInput>,
) -> (Vec<u64>, usize, Vec<Value>) {
    let mut snapshots = 0;
    let mut failures: Vec<Value> = Vec::new();
    let mut refreshed: Vec<u64> = Vec::new();
    for input in inputs {
        let RefreshInput {
            appid,
            catalog,
            sources,
            failures: source_failures,
        } = input;
        failures.extend(source_failures);
        let keys_changed = import_keys(st, &sources);
        let keyed_now = st.depot_keys.clone();
        let provider_names: Vec<String> = sources.iter().map(|plan| plan.name.clone()).collect();
        let Some(record) = st.apps.get_mut(&appid.to_string()) else {
            continue;
        };
        let branch = record
            .get("branch")
            .and_then(Value::as_str)
            .unwrap_or("public")
            .to_string();
        let catalog = match catalog {
            Ok(catalog) => Some(catalog),
            Err(error) => match record.get("catalog").filter(|value| value.is_object()) {
                Some(snapshot) => {
                    snapshots += 1;
                    Some(snapshot.clone())
                }
                None if sources.is_empty() => {
                    failures.push(json!({ "appid": appid, "error": error }));
                    continue;
                }
                None => None,
            },
        };
        let previous = record
            .get("depots")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let os = effective_os(catalog.as_ref(), &st.settings.default_os);
        let latest = fresh_depots(&sources, catalog.as_ref(), &branch, &os);
        let own = catalog_depot_ids(catalog.as_ref());
        let scoped = if previous.is_empty() {
            latest
        } else {
            latest
                .into_iter()
                .filter(|entry| {
                    let depot_id = entry.get("depot_id").and_then(Value::as_u64);
                    // Depots outside the app's own catalog are shared
                    // redistributables the plan needs even on first refresh.
                    let shared = own
                        .as_ref()
                        .is_some_and(|own| depot_id.is_some_and(|id| !own.contains(&id)));
                    shared
                        || previous
                            .iter()
                            .any(|old| old.get("depot_id").and_then(Value::as_u64) == depot_id)
                })
                .collect()
        };
        let planned = preserve_previous(&previous, scoped);
        let planned = sync_enabled(planned, &keyed_now, own.as_ref());
        if planned.is_empty() {
            failures.push(json!({ "appid": appid, "error": "no downloadable depots in the catalog or sources" }));
            continue;
        }
        if planned != previous {
            record["depots"] = Value::Array(planned);
            record["updated"] = json!(crate::vfsys::stamp());
            refreshed.push(appid);
        } else if keys_changed > 0 {
            // A new key still needs a script rewrite even when no gid moved.
            refreshed.push(appid);
        }
        record["sources"] = json!(provider_names);
    }
    (refreshed, snapshots, failures)
}

/// Compatibility wrapper for the pure merge tests and offline callers.
#[cfg(test)]
fn merge_refresh(
    st: &mut state::State,
    catalogs: Vec<(u64, Result<Value, String>)>,
) -> (Vec<u64>, usize, Vec<Value>) {
    let inputs = catalogs
        .into_iter()
        .map(|(appid, catalog)| RefreshInput {
            appid,
            catalog,
            sources: Vec::new(),
            failures: Vec::new(),
        })
        .collect();
    merge_refresh_inputs(st, inputs)
}

/// One refresh on server start and then every thirty minutes. Steam starting the
/// companion server therefore also starts a freshness pass.
/// Remembers why a game has no keys (shown in System) or clears it.
fn record_key_error(appid: u64, reason: Option<&str>) {
    let _ = state::update(|state| {
        if let Some(record) = state.apps.get_mut(&appid.to_string()).and_then(Value::as_object_mut) {
            match reason {
                Some(reason) => {
                    record.insert("key_error".into(), Value::String(reason.to_string()));
                }
                None => {
                    record.remove("key_error");
                }
            }
        }
        Ok(())
    });
}

/// Games added while every key source failed (a Hubcap daily limit, say)
/// hold a catalog-only plan without keys. Each auto-update round asks the
/// sources again, so they start downloading once a source has keys.
fn retry_keyless() {
    let Ok(current) = state::load() else { return };
    let keyless: Vec<u64> = current
        .apps
        .values()
        .filter(|record| record.get("source").and_then(Value::as_str) == Some("catalog"))
        .filter_map(|record| record.get("appid").and_then(Value::as_u64))
        .collect();
    if keyless.is_empty() || sources(&current.settings).is_empty() {
        return;
    }
    for appid in keyless {
        match fetch_plan(appid, None) {
            Ok(plan) if plan.get("provider").and_then(Value::as_str) != Some("catalog") => {
                record_key_error(appid, None);
                eprintln!("plans: keys found for {appid}");
            }
            Ok(_) => eprintln!("plans: still no keys for {appid}"),
            Err(error) => eprintln!("plans: retry for {appid} failed: {error}"),
        }
    }
}

pub fn start_auto_update() {
    if AUTO_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        loop {
            let enabled = state::load()
                .map(|state| state.settings.auto_update_plans)
                .unwrap_or(false);
            if enabled {
                match refresh_manifests() {
                    Ok(report) => eprintln!(
                        "plans: auto update: {} changed, {} script(s), {} failure(s)",
                        report.get("updated").and_then(Value::as_u64).unwrap_or(0),
                        report.get("scripts").and_then(Value::as_u64).unwrap_or(0),
                        report
                            .get("failures")
                            .and_then(Value::as_array)
                            .map(Vec::len)
                            .unwrap_or(0)
                    ),
                    Err(error) => eprintln!("plans: auto update failed: {error}"),
                }
                retry_keyless();
            }
            std::thread::sleep(std::time::Duration::from_secs(30 * 60));
        }
    });
}

/// Rewrites the installed script from the saved plan, so keys that arrived after
/// the plan was created are picked up too.
pub fn write_script(appid: u64) -> Result<bool, String> {
    let st = state::load()?;
    let Some(record) = st.apps.get(&appid.to_string()) else {
        return Ok(false);
    };
    let text = crate::lua::render_record(record, &st.depot_keys);
    crate::vfsys::write_script(appid, &text, true).map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn settings_with(hubcap: &str, ryuu: &str) -> Settings {
        Settings {
            hubcap_key: hubcap.into(),
            ryuu_key: ryuu.into(),
            ..Default::default()
        }
    }

    fn source(name: &str, appid: u64, depot_id: u64, gid: &str, key: &str) -> SourcePlan {
        SourcePlan {
            name: name.into(),
            parsed: crate::hubcap::Parsed {
                appid: Some(appid),
                dlc: Vec::new(),
                depots: vec![json!({ "depot_id": depot_id, "gid": gid, "size": 42, "key": key })],
            },
        }
    }

    #[test]
    fn catalog_only_depots_carry_manifest_and_size() {
        let catalog = json!({
            "depots": [
                { "depot_id": 620, "manifests": { "public": { "gid": "123456789012345678", "size": 987 } } },
                { "depot_id": 621, "manifests": { "public": { "gid": "223456789012345678", "size": 5 } } },
            ]
        });
        let depots = catalog_only_depots(&catalog, "public");
        assert_eq!(depots.len(), 2);
        assert_eq!(depots[0]["depot_id"], 620);
        assert_eq!(depots[0]["gid"], "123456789012345678");
        assert_eq!(depots[0]["size"], 987);
        assert_eq!(depots[1]["depot_id"], 621);
    }

    #[test]
    fn catalog_wins_over_every_source() {
        let plans = vec![
            source("old", 620, 621, "100", &"a".repeat(64)),
            source("new", 620, 621, "200", &"b".repeat(64)),
        ];
        let catalog = json!({"depots": [{"depot_id": 621, "manifests": {"public": {"gid": "200", "size": 99}}}]});
        let depots = fresh_depots(&plans, Some(&catalog), "public", "linux");
        assert_eq!(depots[0]["gid"], "200");
        assert_eq!(depots[0]["size"], 99);
        let mut st = state::State::default();
        assert_eq!(
            import_keys(&mut st, &plans),
            2,
            "both sources contribute keys"
        );
    }

    #[test]
    fn offline_fallback_takes_the_largest_gid_per_depot() {
        let plans = vec![
            source("old", 620, 621, "100", ""),
            source("new", 620, 621, "200", ""),
        ];
        let depots = fresh_depots(&plans, None, "public", "linux");
        assert_eq!(depots[0]["gid"], "200");
        assert_eq!(depots[0]["size"], 42);
    }

    fn multi_source(appid: u64, depots: &[(u64, &str)]) -> SourcePlan {
        SourcePlan {
            name: "multi".into(),
            parsed: crate::hubcap::Parsed {
                appid: Some(appid),
                dlc: Vec::new(),
                depots: depots
                    .iter()
                    .map(|(depot_id, gid)| json!({ "depot_id": depot_id, "gid": gid, "size": 42 }))
                    .collect(),
            },
        }
    }

    #[test]
    fn wrong_platform_depots_never_enter_the_plan() {
        // Mirrors the CK3 layout: platform depots share file paths, so mounting
        // the Windows or macOS ones overwrites the Linux files on disk.
        let catalog = json!({"depots": [
            { "depot_id": 100, "oslist": [], "manifests": { "public": { "gid": "10", "size": 1 } } },
            { "depot_id": 112, "oslist": ["windows"], "manifests": { "public": { "gid": "12", "size": 1 } } },
            { "depot_id": 113, "oslist": ["macos"], "manifests": { "public": { "gid": "13", "size": 1 } } },
            { "depot_id": 114, "oslist": ["linux"], "manifests": { "public": { "gid": "14", "size": 1 } } },
        ]});
        let plans = vec![multi_source(620, &[(100, "10"), (112, "12"), (113, "13"), (114, "14"), (228981, "15")])];
        let os = effective_os(Some(&catalog), "linux");
        assert_eq!(os, "linux");
        let depots = fresh_depots(&plans, Some(&catalog), "public", &os);
        let ids: Vec<u64> = depots
            .iter()
            .filter_map(|depot| depot.get("depot_id").and_then(Value::as_u64))
            .collect();
        assert!(ids.contains(&100), "platform-neutral depot dropped: {ids:?}");
        assert!(ids.contains(&114), "linux depot dropped: {ids:?}");
        assert!(ids.contains(&228981), "shared redistributable dropped: {ids:?}");
        assert!(!ids.contains(&112), "windows depot survived: {ids:?}");
        assert!(!ids.contains(&113), "macos depot survived: {ids:?}");
    }

    #[test]
    fn windows_only_game_keeps_windows_depots_for_proton() {
        let catalog = json!({"depots": [
            { "depot_id": 112, "oslist": ["windows"], "manifests": { "public": { "gid": "12", "size": 1 } } },
        ]});
        let os = effective_os(Some(&catalog), "linux");
        assert_eq!(os, "windows", "a game without linux depots runs through proton");
        let plans = vec![multi_source(620, &[(112, "12")])];
        let depots = fresh_depots(&plans, Some(&catalog), "public", &os);
        let ids: Vec<u64> = depots
            .iter()
            .filter_map(|depot| depot.get("depot_id").and_then(Value::as_u64))
            .collect();
        assert_eq!(ids, vec![112]);
    }

    #[test]
    fn offline_without_catalog_keeps_every_source_depot() {
        // No catalog means no oslist knowledge: nothing may be filtered away,
        // otherwise a Steam outage would silently gut every plan.
        let plans = vec![multi_source(620, &[(112, "12"), (114, "14")])];
        let depots = fresh_depots(&plans, None, "public", "linux");
        assert_eq!(depots.len(), 2);
    }

    #[test]
    fn refresh_merges_latest_record_and_skips_deleted_apps() {
        let mut fresh = state::State::default();
        fresh.apps.insert(
            "620".into(),
            json!({
                "appid": 620, "name": "old name", "branch": "public",
                "depots": [{"depot_id": 621, "gid": "1", "size": 10, "enabled": true}],
                "dlc": [], "fixes": {"ticket": "forge"},
            }),
        );
        fresh
            .apps
            .insert("650".into(), json!({"appid": 650, "depots": []}));
        let stale = fresh.clone();
        let catalog = json!({"depots": [{"depot_id": 621, "manifests": {
            "public": {"gid": "100", "size": 1000},
            "beta": {"gid": "200"}
        }}]});
        let fetched: Vec<_> = stale
            .apps
            .values()
            .map(|record| (record["appid"].as_u64().unwrap(), Ok(catalog.clone())))
            .collect();
        let record = fresh.apps.get_mut("620").unwrap();
        record["name"] = json!("renamed while fetching");
        record["branch"] = json!("beta");
        record["depots"][0]["enabled"] = json!(false);
        record["depots"][0]["size"] = json!(777);
        record["dlc"] = json!([99]);
        record["fixes"]["ticket"] = json!("manual");
        fresh.apps.remove("650");
        fresh
            .apps
            .insert("700".into(), json!({"appid": 700, "name": "new plan"}));
        fresh.codes.insert("later".into(), 7);
        fresh.depot_keys.insert("621".into(), "a".repeat(64));
        fresh.settings.theme = "light".into();
        fresh.plugins.insert("example".into(), false);
        fresh
            .plugin_config
            .insert("example".into(), Default::default());
        state::record_gid(&mut fresh, 620, 621, "50", Some(77), "beta");
        let before = serde_json::to_value(&fresh).unwrap();

        let (refreshed, snapshots, failures) = merge_refresh(&mut fresh, fetched);
        assert_eq!(refreshed, vec![620]);
        assert_eq!(snapshots, 0);
        assert!(failures.is_empty());
        let depots = &fresh.apps["620"]["depots"];
        assert_eq!(depots[0]["gid"], "200");
        // A key landed during the fetch, so the depot is mountable again.
        assert_eq!(depots[0]["enabled"], true);
        assert_eq!(depots[0]["size"], 777);
        let mut after = serde_json::to_value(&fresh).unwrap();
        after["apps"]["620"]["depots"] = before["apps"]["620"]["depots"].clone();
        after["apps"]["620"]
            .as_object_mut()
            .unwrap()
            .remove("updated");
        after["apps"]["620"]
            .as_object_mut()
            .unwrap()
            .remove("sources");
        assert_eq!(after, before);
    }

    #[test]
    fn refresh_uses_latest_offline_catalog_and_leaves_failures_untouched() {
        let mut fresh = state::State::default();
        fresh.apps.insert(
            "620".into(),
            json!({
                "appid": 620, "branch": "public", "updated": "old",
                "depots": [{"depot_id": 621, "gid": "old", "size": 5}],
                "catalog": {"depots": [{"depot_id": 621,
                    "manifests": {"public": {"gid": "new", "size": 42}}
                }]}
            }),
        );
        let untouched = json!({"appid": 650, "depots": [], "updated": "old"});
        fresh.apps.insert("650".into(), untouched.clone());
        let (refreshed, snapshots, failures) = merge_refresh(
            &mut fresh,
            vec![(620, Err("offline".into())), (650, Err("offline".into()))],
        );
        assert_eq!(refreshed, vec![620]);
        assert_eq!(snapshots, 1);
        assert_eq!(failures, vec![json!({"appid": 650, "error": "offline"})]);
        assert_eq!(fresh.apps["620"]["depots"][0]["gid"], "new");
        assert_eq!(fresh.apps["650"], untouched);
    }

    #[test]
    fn refresh_keeps_the_existing_depot_membership() {
        let mut fresh = state::State::default();
        fresh.apps.insert(
            "108600".into(),
            json!({
                "appid": 108600, "branch": "public", "updated": "old",
                "depots": [{"depot_id": 108603, "gid": "old", "size": 10}]
            }),
        );
        let catalog = json!({"depots": [
            {"depot_id": 108602, "manifests": {"public": {"gid": "200", "size": 20}}},
            {"depot_id": 108603, "manifests": {"public": {"gid": "300", "size": 30}}},
            {"depot_id": 108604, "manifests": {"public": {"gid": "400", "size": 40}}}
        ]});
        let (refreshed, _, _) = merge_refresh(&mut fresh, vec![(108600, Ok(catalog))]);
        assert_eq!(refreshed, vec![108600]);
        let depots = fresh.apps["108600"]["depots"].as_array().unwrap();
        assert_eq!(
            depots.len(),
            1,
            "background refresh must not mount every OS depot"
        );
        assert_eq!(depots[0]["depot_id"], 108603);
        assert_eq!(depots[0]["gid"], "300");
    }

    #[test]
    fn pin_only_depots_enter_the_plan() {
        let lua = "addappid(1158310)\nsetManifestid(228981,\"7613356809904826842\")\nsetmanifestid(1158311,\"7584914003502786044\",20381203637)\n";
        let parsed = crate::hubcap::parse_lua(lua, Some(1158310));
        let ids: Vec<u64> = parsed
            .depots
            .iter()
            .filter_map(|d| d.get("depot_id").and_then(Value::as_u64))
            .collect();
        assert!(ids.contains(&228981), "pin-only depot missing: {ids:?}");
        let plan = SourcePlan {
            name: "manilua".into(),
            parsed,
        };
        let fresh = fresh_depots(&[plan], None, "public", "linux");
        let fresh_ids: Vec<u64> = fresh
            .iter()
            .filter_map(|d| d.get("depot_id").and_then(Value::as_u64))
            .collect();
        assert!(fresh_ids.contains(&228981), "fresh depots dropped it: {fresh_ids:?}");
    }

    #[test]
    fn shared_depots_stay_on_without_keys() {
        let keys = BTreeMap::new();
        let own: BTreeSet<u64> = [621u64].into_iter().collect();
        let depots = vec![
            json!({ "depot_id": 621, "gid": "1" }),
            json!({ "depot_id": 228981, "gid": "2" }),
        ];
        let out = sync_enabled(depots, &keys, Some(&own));
        assert_eq!(out[0]["enabled"], false, "own keyless depot must switch off");
        assert_eq!(out[1]["enabled"], true, "shared depot mounts without a key");
        assert_eq!(out[1]["shared"], true);
    }

    #[test]
    fn unchanged_plan_is_not_rewritten() {
        let mut fresh = state::State::default();
        fresh.depot_keys.insert("621".into(), "a".repeat(64));
        fresh.apps.insert(
            "620".into(),
            json!({
                "appid": 620, "branch": "public", "updated": "old",
                "depots": [{"depot_id": 621, "gid": "100", "size": 1000, "enabled": true}]
            }),
        );
        let catalog = json!({"depots": [{"depot_id": 621, "manifests": {"public": {"gid": "100", "size": 1000}}}]});
        let (refreshed, _, _) = merge_refresh(&mut fresh, vec![(620, Ok(catalog))]);
        assert!(refreshed.is_empty());
        assert_eq!(fresh.apps["620"]["updated"], "old");
    }

    #[test]
    fn skips_sources_without_keys() {
        let list = sources(&settings_with("", ""));
        let names: Vec<&str> = list.iter().map(|source| source.name.as_str()).collect();
        assert_eq!(names, vec!["catalog", "manilua", "steamtools"]);
        let list = sources(&settings_with("", "abc"));
        let names: Vec<&str> = list.iter().map(|source| source.name.as_str()).collect();
        assert_eq!(names, vec!["catalog", "manilua", "steamtools", "ryuu"]);
    }

    #[test]
    fn keeps_the_configured_order() {
        let list = sources(&settings_with("hub", "ryuu"));
        let names: Vec<&str> = list.iter().map(|source| source.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["catalog", "manilua", "steamtools", "hubcap", "ryuu"]
        );
    }

    #[test]
    fn custom_source_needs_appid_placeholder() {
        let mut settings = Settings::default();
        settings.plan_sources.insert(
            "bad".into(),
            json!({ "url": "http://example.com/fixed.lua" }),
        );
        settings.plan_sources.insert(
            "good".into(),
            json!({ "url": "http://127.0.0.1:1/lua/{appid}.lua" }),
        );
        let list = sources(&settings);
        let names: Vec<&str> = list.iter().map(|source| source.name.as_str()).collect();
        assert_eq!(names, vec!["catalog", "manilua", "steamtools", "good"]);
    }

    #[test]
    fn falls_back_to_the_next_source() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let body = "addappid(620)\naddappid(621, 1, \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\")\n";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        let mut settings = Settings::default();
        settings.plan_sources.insert(
            "dead".into(),
            json!({ "url": "http://127.0.0.1:1/lua/{appid}.lua" }),
        );
        settings.plan_sources.insert(
            "live".into(),
            json!({ "url": format!("http://127.0.0.1:{port}/lua/{{appid}}.lua") }),
        );
        // Use a non-existent appid so catalog source fails, then dead fails, then live succeeds
        let list = sources(&settings);
        assert_eq!(
            list.len(),
            5,
            "catalog, manilua, steamtools, plus both custom sources should register"
        );
        assert_eq!(list[0].name, "catalog");
        let mut tried = Vec::new();
        let mut lua = None;
        for source in list {
            tried.push(source.name.clone());
            if let Ok(text) = source.fetch(99999999) {
                lua = Some(text);
                break;
            }
        }
        // Every real registry answers a placeholder for an unknown app; the
        // all-zero key is rejected, so the chain reaches the live custom source.
        assert_eq!(
            tried,
            vec!["catalog", "manilua", "steamtools", "dead", "live"]
        );
        assert!(lua.unwrap().contains("addappid(621"));
    }
}
