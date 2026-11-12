mod support;

const BROKEN: &str = "\"AppState\"\n{\n\t\"appid\"\t\t\"620\"\n\t\"name\"\t\t\"Portal 2\"\n\t\"StateFlags\"\t\t\"1026\"\n\t\"BytesToDownload\"\t\t\"1000\"\n\t\"BytesDownloaded\"\t\t\"400\"\n\t\"AutoUpdateBehavior\"\t\t\"0\"\n\t\"ScheduledAutoUpdate\"\t\t\"1700000000\"\n\t\"InstalledDepots\"\n\t{\n\t\t\"621\"\n\t\t{\n\t\t\t\"manifest\"\t\t\"111\"\n\t\t}\n\t}\n}\n";

#[test]
fn repair_leaves_a_manifest_steam_accepts() {
    let tree = support::Tree::new("manifest");
    tree.apply();
    let path = tree.steam().join("steamapps/appmanifest_620.acf");
    std::fs::write(&path, BROKEN).expect("write manifest");

    let result = exxa::appmanifest::patch(620, &exxa::appmanifest::Mode::Fix, false).expect("patch");
    assert!(result["changes"].as_array().map(|c| !c.is_empty()).unwrap_or(false), "no changes: {result}");

    let after = exxa::appmanifest::read(620).expect("read").expect("manifest");
    assert_eq!(after.state_flags, 4, "StateFlags not repaired: {after:?}");
    assert_eq!(after.bytes_downloaded, after.bytes_to_download, "byte counters disagree");
    assert_eq!(after.bytes_to_download, Some(1000));

    let backup = path.with_extension("acf.exxatools.bak");
    assert_eq!(std::fs::read_to_string(&backup).expect("backup"), BROKEN, "backup is not the original");

    let once = std::fs::read_to_string(&path).expect("read once");
    exxa::appmanifest::patch(620, &exxa::appmanifest::Mode::Fix, false).expect("second patch");
    let twice = std::fs::read_to_string(&path).expect("read twice");
    assert_eq!(once, twice, "a second repair changed the file again");
    assert_eq!(std::fs::read_to_string(&backup).expect("backup kept"), BROKEN);
}
