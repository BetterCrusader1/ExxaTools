mod support;

fn web_root(tag: &str) -> support::Tree {
    let tree = support::Tree::new(tag);
    tree.apply();
    let web = tree.root.join("project/web");
    std::fs::create_dir_all(&web).expect("web dir");
    std::fs::write(web.join("index.html"), "<html>ok</html>").expect("index");
    std::fs::write(tree.root.join("secret.txt"), "do not serve").expect("secret");
    std::os::unix::fs::symlink(tree.root.join("secret.txt"), web.join("leak.txt")).expect("symlink");
    std::env::set_var("EXXATOOLS_PROJECT", tree.root.join("project"));
    tree
}

#[test]
fn static_serving_cannot_leave_the_web_root() {
    let tree = web_root("static");
    assert!(exxa::server::static_file("/").is_some(), "index not served");
    assert!(exxa::server::static_file("index.html").is_some());

    for attempt in ["../secret.txt", "../../etc/passwd", "/etc/passwd", "leak.txt"] {
        assert!(
            exxa::server::static_file(attempt).is_none(),
            "{attempt} escaped the web root"
        );
    }
    drop(tree);
}
