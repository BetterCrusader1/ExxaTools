#[test]
fn the_library_exports_the_plan_renderer() {
    let script = exxa::lua::render_plan(
        620,
        "Portal 2",
        "public",
        &[],
        &std::collections::BTreeMap::new(),
        &[],
        "test",
        "auto",
    );
    assert!(script.starts_with("-- ExxaTools plan"));
}
