mod support;

#[test]
fn refresh_falls_back_to_the_saved_catalog() {
    let tree = support::Tree::new("refresh");
    tree.apply();
    std::env::set_var("EXXATOOLS_OFFLINE", "1");

    let mut state = exxa::state::State::default();
    state.apps.insert(
        "620".to_string(),
        serde_json::json!({
            "appid": 620,
            "name": "Portal 2",
            "branch": "public",
            "source": "auto",
            "updated": "1794480536",
            "depots": [
                {"depot_id": 621, "gid": "111", "size": 100, "enabled": true},
                {"depot_id": 622, "gid": "222", "size": 50, "enabled": false}
            ],
            "dlc": [],
            "catalog": {
                "name": "Portal 2",
                "depots": [
                    {"depot_id": 621, "manifests": {"public": {"gid": "999", "size": 700}}},
                    {"depot_id": 622, "manifests": {"public": {"gid": "888", "size": 70}}}
                ]
            }
        }),
    );
    exxa::state::save(&state).expect("state save");
    std::fs::write(
        exxa::paths::vf_scripts().join("exxatools-620.lua"),
        "-- stale plan\naddappid(620)\n",
    )
    .expect("stale script");

    let report = exxa::plans::refresh_manifests().expect("refresh");
    assert_eq!(report["failures"].as_array().map(Vec::len), Some(0), "refresh failed: {report}");
    assert_eq!(report["snapshots"], 1, "snapshot not used: {report}");
    assert_eq!(report["scripts"], 1, "script not rewritten: {report}");

    let saved = exxa::state::load().expect("state");
    let depots = saved.apps["620"]["depots"].as_array().expect("depots");
    assert_eq!(depots[0]["gid"], "999", "depot not refreshed from snapshot");
    assert_eq!(depots[1]["enabled"], false, "disabled flag lost");
    assert_eq!(depots[1]["gid"], "888");

    let script = std::fs::read_to_string(exxa::paths::vf_scripts().join("exxatools-620.lua")).expect("script");
    assert!(script.contains("setmanifestid(621, \"999\""), "script not rewritten: {script}");
    assert!(!script.contains("setmanifestid(622"), "disabled depot leaked into the script");
}
