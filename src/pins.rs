//! Pinned manifests: a plan can hold an older gid than the branch serves.

use serde_json::{json, Value};

use crate::state::State;
use crate::steam;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinState {
    Branch,
    Pinned,
    Stale,
}

impl PinState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Branch => "branch",
            Self::Pinned => "pinned",
            Self::Stale => "stale",
        }
    }
}

/// A preset gid equal to the branch is not a pin; anything else is.
/// "stale" means the branch moved forward while the plan kept the old build,
/// which is the normal state of a deliberate pin.
pub fn classify(planned: Option<&str>, branch: Option<&str>, branch_build: Option<&str>, plan_build: Option<&str>) -> PinState {
    match (planned, branch) {
        (None, _) => PinState::Branch,
        (Some(""), _) => PinState::Branch,
        (Some(planned), Some(branch)) if planned == branch => PinState::Branch,
        (Some(_), _) => {
            if let (Some(branch_build), Some(plan_build)) = (branch_build, plan_build) {
                if !branch_build.is_empty() && !plan_build.is_empty() && branch_build != plan_build {
                    return PinState::Stale;
                }
            }
            PinState::Pinned
        }
    }
}

pub fn compare(st: &State, appid: u64, branch: &str) -> Result<Value, String> {
    let catalog = steam::catalog(appid)?;
    let record = st.apps.get(&appid.to_string());
    let planned: Vec<Value> = record
        .and_then(|value| value.get("depots"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    let mut pinned = 0;
    let mut stale = 0;
    for depot in catalog.get("depots").and_then(Value::as_array).cloned().unwrap_or_default() {
        let depot_id = depot.get("depot_id").and_then(Value::as_u64).unwrap_or(0);
        let entry = planned
            .iter()
            .find(|item| item.get("depot_id").and_then(Value::as_u64) == Some(depot_id));
        let manifest = steam::pick_manifest(&depot, branch);
        let branch_gid = manifest.as_ref().and_then(|value| value.get("gid")).and_then(Value::as_str);
        let branch_size = manifest.as_ref().and_then(|value| value.get("size")).and_then(Value::as_u64);
        let planned_gid = entry.and_then(|value| value.get("gid")).and_then(Value::as_str).filter(|text| !text.is_empty());
        let planned_size = entry.and_then(|value| value.get("size")).and_then(Value::as_u64);
        let state = classify(planned_gid, branch_gid, None, None);
        match state {
            PinState::Pinned => pinned += 1,
            PinState::Stale => stale += 1,
            PinState::Branch => {}
        }
        if entry.is_some() || planned_gid.is_some() {
            items.push(json!({
                "depot_id": depot_id,
                "branch_gid": branch_gid,
                "planned_gid": planned_gid,
                "branch_size": branch_size,
                "planned_size": planned_size,
                "state": state.as_str(),
                "in_plan": entry.is_some(),
            }));
        }
    }
    Ok(json!({
        "appid": appid,
        "branch": branch,
        "items": items,
        "pinned": pinned,
        "stale": stale,
    }))
}

/// Set or clear one depot's gid inside the saved plan.
pub fn set_pin(st: &mut State, appid: u64, depot_id: u64, gid: Option<&str>) -> Result<Value, String> {
    if let Some(value) = gid {
        if !value.chars().all(|ch| ch.is_ascii_digit()) {
            return Err("a manifest id is decimal digits".into());
        }
    }
    let record = st.apps.get_mut(&appid.to_string()).ok_or("save a plan first")?;
    let depots = record
        .get_mut("depots")
        .and_then(Value::as_array_mut)
        .ok_or("this plan has no depots")?;
    let entry = depots
        .iter_mut()
        .find(|item| item.get("depot_id").and_then(Value::as_u64) == Some(depot_id))
        .ok_or("that depot is not in the plan")?;
    entry["gid"] = match gid {
        Some(value) if !value.is_empty() => json!(value),
        _ => json!(""),
    };
    let stored = entry.get("gid").cloned().unwrap_or(Value::Null);
    record["updated"] = json!(crate::vfsys::stamp());
    Ok(json!({ "appid": appid, "depot_id": depot_id, "gid": stored }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_gid_is_the_branch() {
        assert_eq!(classify(Some("111"), Some("111"), None, None), PinState::Branch);
        assert_eq!(classify(None, Some("111"), None, None), PinState::Branch);
        assert_eq!(classify(Some(""), Some("111"), None, None), PinState::Branch);
    }

    #[test]
    fn different_gid_is_a_pin() {
        assert_eq!(classify(Some("222"), Some("111"), None, None), PinState::Pinned);
        assert_eq!(classify(Some("222"), None, None, None), PinState::Pinned);
    }

    #[test]
    fn build_ids_mark_a_pin_stale() {
        assert_eq!(classify(Some("222"), Some("111"), Some("900"), Some("800")), PinState::Stale);
        assert_eq!(classify(Some("222"), Some("111"), Some("800"), Some("800")), PinState::Pinned);
    }
}
