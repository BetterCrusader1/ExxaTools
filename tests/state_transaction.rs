mod support;

// One test, not two: every test in a binary shares one process environment, and a
// second fixture would repoint EXXATOOLS_HOME while the first one is still writing.
#[test]
fn updates_are_atomic_and_roll_back_on_failure() {
    let tree = support::Tree::new("state-tx");
    tree.apply();
    exxa::state::save(&exxa::state::State::default()).expect("initial state");

    let mut handles = Vec::new();
    for index in 0..8u64 {
        handles.push(std::thread::spawn(move || {
            exxa::state::update(|state| {
                state.codes.insert(format!("gid-{index}"), index);
                Ok(())
            })
            .unwrap_or_else(|error| panic!("update {index} failed: {error}"));
        }));
    }
    for handle in handles {
        handle.join().expect("thread");
    }
    let state = exxa::state::load().expect("state");
    assert_eq!(state.codes.len(), 8, "a concurrent update was lost: {:?}", state.codes);

    let result: Result<(), String> = exxa::state::update(|state| {
        state.codes.insert("gid-rollback".into(), 99);
        Err("nope".into())
    });
    assert!(result.is_err());
    let after = exxa::state::load().expect("state");
    assert!(!after.codes.contains_key("gid-rollback"), "a failed update was persisted");
    assert_eq!(after.codes.len(), 8);
}
