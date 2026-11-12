mod support;

#[test]
fn the_fixture_owns_every_path_the_product_reads() {
    let tree = support::Tree::new("fixture");
    tree.apply();
    let root = exxa::steamctl::root().expect("steam root");
    assert_eq!(root, tree.steam(), "steam root must resolve inside the fixture");
    assert!(exxa::paths::vf_scripts().starts_with(tree.vf()), "scripts dir escaped: {:?}", exxa::paths::vf_scripts());
    assert!(exxa::paths::state_path().starts_with(tree.home()), "state escaped: {:?}", exxa::paths::state_path());
    assert!(tree.steam().join("steamapps/appmanifest_620.acf").is_file());
    assert!(exxa::paths::state_dir().starts_with(tree.home()));
}
