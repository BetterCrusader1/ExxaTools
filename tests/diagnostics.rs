mod support;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn the_bundle_names_plans_and_never_carries_a_key() {
    let tree = support::Tree::new("diagnostics");
    tree.apply();
    let mut state = exxa::state::State::default();
    state.depot_keys.insert("621".to_string(), KEY.to_string());
    state.apps.insert(
        "620".to_string(),
        serde_json::json!({
            "appid": 620,
            "name": "Portal 2",
            "branch": "public",
            "updated": "1794480536",
            "depots": [{"depot_id": 621, "gid": "9044256803759256265", "enabled": true}],
            "dlc": []
        }),
    );
    exxa::state::save(&state).expect("state");
    std::fs::write(
        tree.data().join("Steam/logs/console-linux.txt"),
        format!("Loading depot 621 with key {KEY}\nSome other line\n"),
    )
    .expect("log");

    let bundle = exxa::diagnostics::build();
    let text = serde_json::to_string(&bundle).expect("json");
    assert!(!text.contains(KEY), "the bundle leaked a depot key");
    assert!(text.contains("012345…cdef"), "the masked key is missing");
    assert_eq!(bundle["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(bundle["keys"], 1);
    assert_eq!(bundle["plans"][0]["appid"], 620);
    assert!(bundle["doctor"].is_object(), "doctor section missing");
    assert!(bundle["log"].is_string(), "log section missing");
    assert!(bundle["generated"].is_string());
}
