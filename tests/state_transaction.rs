mod support;

// One fixture-owning test: tests in a binary share one process environment.
// The ignored subprocess helper never changes that environment or creates a Tree.
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

    // A snapshot may be stale by the time expensive work finishes. Commit only
    // that work's fields, not the old snapshot's unrelated settings or codes.
    let stale = after;
    exxa::state::update(|state| {
        state.settings.theme = "light".into();
        state.codes.insert("arrived-later".into(), 100);
        Ok(())
    }).expect("intervening update");
    exxa::state::update(|state| {
        state.apps.insert("620".into(), serde_json::json!({
            "appid": 620,
            "depots": [],
            "name": format!("plan from {} cached codes", stale.codes.len()),
        }));
        Ok(())
    }).expect("merge result");
    let merged = exxa::state::load().expect("merged state");
    assert_eq!(merged.settings.theme, "light");
    assert_eq!(merged.codes["arrived-later"], 100);
    assert!(merged.apps.contains_key("620"));

    // Race first-run token creation against both other initializers and ordinary
    // writes. Repeat with an empty token so a cached-token fast path cannot hide
    // the load/mutate/save bug. Every returned token must be the persisted one.
    for round in 0..8 {
        exxa::state::update(|state| {
            state.token.clear();
            Ok(())
        }).expect("clear fixture token");
        let start = std::sync::Arc::new(std::sync::Barrier::new(17));
        let mut workers = Vec::new();
        for index in 0..16 {
            let start = start.clone();
            workers.push(std::thread::spawn(move || {
                start.wait();
                if index % 2 == 0 {
                    Some(exxa::state::ensure_token().expect("initialize token"))
                } else {
                    exxa::state::update(|state| {
                        state.codes.insert(format!("race-{round}-{index}"), 1);
                        state.plugins.insert(format!("plugin-{round}-{index}"), false);
                        Ok(())
                    }).expect("racing update");
                    None
                }
            }));
        }
        start.wait();
        let tokens: Vec<_> = workers.into_iter()
            .filter_map(|worker| worker.join().expect("racing worker"))
            .collect();
        let saved = exxa::state::load().expect("state after token race");
        assert_eq!(saved.token.len(), 64);
        assert!(tokens.iter().all(|token| token == &saved.token), "initializers disagreed");
        assert_eq!(saved.codes.len(), 9 + (round + 1) * 8);
        assert_eq!(saved.plugins.len(), (round + 1) * 8);
        assert_eq!(saved.settings.theme, "light");
        assert!(saved.apps.contains_key("620"));
        let bytes = std::fs::read(exxa::state::state_path()).expect("saved bytes");
        assert!(exxa::state::ensure_token().expect("reuse token") == saved.token);
        assert_eq!(std::fs::read(exxa::state::state_path()).expect("unchanged bytes"), bytes);
    }

    subprocess_updates_preserve_migration_and_atomic_reads(&tree);
}

const PROCESS_WORKERS: usize = 6;
const PROCESS_UPDATES: usize = 12;

// Reap children even if an assertion or spawn fails, before Tree removes their
// fixture. A deadlocked implementation fails by timeout instead of hanging CI.
struct Workers(Vec<std::process::Child>);

impl Drop for Workers {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn subprocess_updates_preserve_migration_and_atomic_reads(tree: &support::Tree) {
    use std::time::{Duration, Instant};

    let home = tree.root.join("process-state");
    let path = home.join("state.json");
    let legacy = tree.root.join("vapordepot");
    std::fs::create_dir_all(&legacy).expect("legacy fixture directory");
    let mut initial = exxa::state::State::default();
    initial.codes.insert("legacy-sentinel".into(), 42);
    // Large enough that an in-place migration copy can expose partial JSON.
    initial.apps.insert("migration-payload".into(), serde_json::json!("x".repeat(256 * 1024)));
    std::fs::write(legacy.join("state.json"), serde_json::to_vec(&initial).unwrap())
        .expect("legacy fixture state");
    std::fs::write(tree.root.join("state-tx-fixture"), "state transaction fixture")
        .expect("fixture marker");

    let exe = std::env::current_exe().expect("test executable");
    let mut workers = Workers(Vec::new());
    for index in 0..PROCESS_WORKERS {
        workers.0.push(std::process::Command::new(&exe)
            .args(["--exact", "state_transaction_child", "--ignored", "--nocapture"])
            .env("EXXA_STATE_TX_ROOT", &tree.root)
            .env("EXXA_STATE_TX_WORKER", index.to_string())
            .env("EXXATOOLS_HOME", &home)
            .spawn().expect("spawn test child"));
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    while !(0..PROCESS_WORKERS).all(|index| tree.root.join(format!("ready-{index}")).is_file()) {
        assert!(Instant::now() < deadline, "children did not reach start barrier");
        for child in &mut workers.0 {
            assert!(child.try_wait().expect("poll child").is_none(), "child exited before start");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    std::fs::write(tree.root.join("start"), b"go").expect("release children");
    let mut finished = vec![false; PROCESS_WORKERS];
    while finished.iter().any(|done| !done) {
        assert!(Instant::now() < deadline, "cross-process transactions timed out");
        // Read without any lock while other processes migrate and replace state.
        match std::fs::read(&path) {
            Ok(bytes) => {
                let snapshot: exxa::state::State = serde_json::from_slice(&bytes)
                    .expect("readers must only see complete JSON");
                assert_eq!(snapshot.codes["legacy-sentinel"], 42);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("read concurrent state: {error}"),
        }
        for (index, child) in workers.0.iter_mut().enumerate() {
            if !finished[index] {
                if let Some(status) = child.try_wait().expect("poll child") {
                    assert!(status.success(), "child {index} failed: {status}");
                    finished[index] = true;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let saved: exxa::state::State = serde_json::from_slice(&std::fs::read(&path).unwrap())
        .expect("final subprocess state");
    assert_eq!(saved.codes["legacy-sentinel"], 42);
    assert_eq!(saved.codes["process-counter"], (PROCESS_WORKERS * PROCESS_UPDATES) as u64);
    for worker in 0..PROCESS_WORKERS {
        for index in 0..PROCESS_UPDATES {
            assert_eq!(saved.codes[&format!("process-{worker}-{index}")], index as u64);
        }
    }
    assert_eq!(saved.codes.len(), 2 + PROCESS_WORKERS * PROCESS_UPDATES);
    assert_eq!(saved.apps["migration-payload"], initial.apps["migration-payload"]);
    assert!(path.with_extension("lock").is_file(), "the lock sidecar must persist");
}

#[test]
#[ignore = "subprocess helper; launched only by the fixture-owning test"]
fn state_transaction_child() {
    use std::time::{Duration, Instant};

    // Running all ignored tests manually must never touch personal state.
    let Some(root) = std::env::var_os("EXXA_STATE_TX_ROOT") else { return };
    let root = std::path::PathBuf::from(root);
    assert!(root.is_absolute());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(root.file_name().unwrap().to_string_lossy().starts_with("exxa-state-tx-"));
    assert_eq!(std::fs::read_to_string(root.join("state-tx-fixture")).unwrap(), "state transaction fixture");
    assert_eq!(exxa::state::state_path(), root.join("process-state/state.json"));
    let worker: usize = std::env::var("EXXA_STATE_TX_WORKER").unwrap().parse().unwrap();
    assert!(worker < PROCESS_WORKERS);
    std::fs::write(root.join(format!("ready-{worker}")), b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !root.join("start").is_file() {
        assert!(Instant::now() < deadline, "parent did not release barrier");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(exxa::state::load().expect("concurrent migration").codes["legacy-sentinel"], 42);
    for index in 0..PROCESS_UPDATES {
        exxa::state::update(|state| {
            let count = state.codes.get("process-counter").copied().unwrap_or(0);
            // Deliberately widen the lost-update window when the OS lock is absent.
            std::thread::sleep(Duration::from_millis(5));
            state.codes.insert("process-counter".into(), count + 1);
            state.codes.insert(format!("process-{worker}-{index}"), index as u64);
            Ok(())
        }).expect("cross-process update");
    }
}
