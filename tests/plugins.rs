mod support;

use std::path::PathBuf;

fn write_plugin(root: &PathBuf, folder: &str, manifest: &serde_json::Value, entry: Option<&str>) {
    let dir = root.join(folder);
    std::fs::create_dir_all(&dir).expect("plugin dir");
    std::fs::write(dir.join("exxa-plugin.json"), manifest.to_string()).expect("manifest");
    if let Some(entry) = entry {
        std::fs::write(dir.join(entry), "window.__exxaProbe = 1;").expect("entry");
    }
}

// One test: the fixture repoints process-wide paths, so a second test in this
// binary would fight over the same environment.
#[test]
fn registry_validates_toggles_and_removes() {
    let tree = support::Tree::new("plugins");
    tree.apply();
    let root = exxa::plugins::plugins_dir();

    write_plugin(
        &root,
        "hello",
        &serde_json::json!({
            "schema": 1, "id": "hello", "name": "Hello", "version": "1.0.0",
            "author": "test", "description": "sample", "entry": "frontend.js",
            "permissions": ["ui"]
        }),
        Some("frontend.js"),
    );
    write_plugin(
        &root,
        "no-entry",
        &serde_json::json!({
            "schema": 1, "id": "no-entry", "name": "No entry", "entry": "missing.js"
        }),
        None,
    );
    write_plugin(
        &root,
        "escape",
        &serde_json::json!({
            "schema": 1, "id": "../escape", "name": "Escape", "entry": "frontend.js"
        }),
        Some("frontend.js"),
    );
    write_plugin(
        &root,
        "big",
        &serde_json::json!({
            "schema": 1, "id": "big", "name": "Big", "entry": "frontend.js"
        }),
        None,
    );
    std::fs::write(root.join("big/frontend.js"), vec![b'x'; 600 * 1024]).expect("big entry");

    let list = exxa::plugins::list_json();
    let plugins = list["plugins"].as_array().expect("plugins");
    let by_id = |id: &str| plugins.iter().find(|plugin| plugin["id"] == id).cloned();

    let hello = by_id("hello").expect("hello listed");
    assert_eq!(hello["valid"], true);
    assert_eq!(hello["enabled"], true);

    let no_entry = by_id("no-entry").expect("no-entry listed");
    assert_eq!(no_entry["valid"], false);
    assert!(no_entry["error"].as_str().unwrap_or("").contains("entry not found"));

    let escape = plugins
        .iter()
        .find(|plugin| plugin["dir"].as_str().unwrap_or("").ends_with("/escape"))
        .cloned()
        .expect("escape listed");
    assert_eq!(escape["valid"], false);
    assert!(escape["error"].as_str().unwrap_or("").contains("id"));

    let big = by_id("big").expect("big listed");
    assert_eq!(big["valid"], false);
    assert!(big["error"].as_str().unwrap_or("").contains("larger"));

    // Only the valid plugin is offered to the host.
    let frontends = exxa::plugins::frontends();
    assert_eq!(frontends.len(), 1);
    assert_eq!(frontends[0].0.id, "hello");

    // Disable, then confirm both the state and the injection list follow.
    exxa::plugins::set_enabled("hello", false).expect("disable");
    assert!(exxa::plugins::frontends().is_empty());
    let listed = exxa::plugins::list_json();
    let hello = listed["plugins"]
        .as_array()
        .expect("plugins")
        .iter()
        .find(|plugin| plugin["id"] == "hello")
        .cloned()
        .expect("hello");
    assert_eq!(hello["enabled"], false);

    exxa::plugins::set_enabled("hello", true).expect("enable");
    assert_eq!(exxa::plugins::frontends().len(), 1);

    // Removal deletes the folder and forgets the toggle.
    exxa::plugins::remove("hello").expect("remove");
    assert!(!root.join("hello").exists());
    assert!(exxa::plugins::list_json()["plugins"]
        .as_array()
        .expect("plugins")
        .iter()
        .all(|plugin| plugin["id"] != "hello"));
}