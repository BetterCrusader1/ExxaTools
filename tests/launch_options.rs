mod support;

#[test]
fn the_onlinefix_flag_round_trips_through_localconfig() {
    let tree = support::Tree::new("launch");
    tree.apply();

    assert_eq!(exxa::steamctl::read_launch_options(620).as_deref(), Some(""), "fixture starts empty");
    exxa::steamctl::set_launch_options(620, "-onlinefix").expect("set flag");
    assert_eq!(exxa::steamctl::read_launch_options(620).as_deref(), Some("-onlinefix"));

    exxa::steamctl::set_launch_options(620, "").expect("clear flag");
    assert_eq!(exxa::steamctl::read_launch_options(620).as_deref(), Some(""));

    assert_eq!(exxa::verify::merge_launch_option(Some("-onlinefix"), "-onlinefix"), "-onlinefix");
    assert_eq!(exxa::verify::merge_launch_option(Some(""), "-onlinefix"), "-onlinefix");
    assert_eq!(
        exxa::verify::merge_launch_option(Some("-novid"), "-onlinefix"),
        "-novid -onlinefix"
    );

    let config = std::fs::read_to_string(tree.steam().join("userdata/12345/config/localconfig.vdf")).expect("config");
    assert!(config.contains("\"LaunchOptions\""));
}
