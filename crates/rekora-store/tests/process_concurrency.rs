//! R12: independent OS processes contend on a real on-disk SQLite database.

#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rekora_core::{MemoryKind, MemoryStatus, NewMemory, Origin};
use rekora_store::{Store, StoreError};

const WRITERS: usize = 4;
const READERS: usize = 8;
const UPDATES: usize = 25;

// Always reap children, including when an assertion fails or a worker hangs.
struct Workers(Vec<Child>);

impl Drop for Workers {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !ready() {
        assert!(Instant::now() < deadline, "process concurrency timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn r12_process_readers_and_writers_preserve_versions_and_audit() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("rekora.db");
    let (mut store, _) = Store::init(&database).unwrap();
    let memory = NewMemory {
        kind: MemoryKind::Decision,
        content: "Concurrent updates preserve every committed version".into(),
        origin: Origin::Human,
        status: MemoryStatus::Candidate,
        sources: vec![],
        branch: None,
        commit: None,
    };
    let id = store
        .add(memory.validate().unwrap(), "parent")
        .unwrap()
        .memory
        .id;
    drop(store);

    let mut workers = Workers(Vec::new());
    for index in 0..WRITERS + READERS {
        workers.0.push(
            Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "process_worker", "--nocapture"])
                .env("REKORA_PROCESS_TEST_DIR", dir.path())
                .env("REKORA_PROCESS_TEST_ID", &id)
                .env("REKORA_PROCESS_TEST_INDEX", index.to_string())
                .stdin(Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    // A file barrier ensures every process has opened its own connection before
    // contention starts; spawning alone does not guarantee overlapping work.
    wait_until(|| (0..WRITERS + READERS).all(|i| dir.path().join(format!("ready-{i}")).exists()));
    std::fs::write(dir.path().join("start"), []).unwrap();
    wait_until(|| {
        workers.0.iter_mut().all(|child| {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "worker failed: {status}");
                true
            } else {
                false
            }
        })
    });

    let store = Store::open(&database).unwrap();
    let expected = 1 + (WRITERS * UPDATES) as i64;
    assert_eq!(store.get(&id).unwrap().version, expected);
    let history = store.history(&id).unwrap();
    assert_eq!(history.len(), expected as usize);
    let mut sequences: Vec<_> = history.iter().map(|event| event.seq).collect();
    sequences.sort_unstable();
    assert_eq!(sequences, (1..=expected).collect::<Vec<_>>());
    assert!(history.iter().all(|event| event.memory_id == id));
    assert_eq!(
        history
            .iter()
            .filter(|event| event.action == "create")
            .count(),
        1
    );
    assert_eq!(
        history
            .iter()
            .filter(|event| event.action == "status")
            .count(),
        WRITERS * UPDATES
    );
    for index in 0..WRITERS {
        let actor = format!("writer-{index}");
        assert_eq!(
            history.iter().filter(|event| event.actor == actor).count(),
            UPDATES
        );
    }
    assert!(store.check().unwrap().is_empty());
}

#[test]
#[ignore = "subprocess entry point; invoked by the R12 parent test"]
fn process_worker() {
    let directory = std::env::var_os("REKORA_PROCESS_TEST_DIR").unwrap();
    let directory = Path::new(&directory);
    let id = std::env::var("REKORA_PROCESS_TEST_ID").unwrap();
    let index: usize = std::env::var("REKORA_PROCESS_TEST_INDEX")
        .unwrap()
        .parse()
        .unwrap();
    let mut store = Store::open(&directory.join("rekora.db")).unwrap();
    std::fs::write(directory.join(format!("ready-{index}")), []).unwrap();
    wait_until(|| directory.join("start").exists());
    if index < WRITERS {
        let actor = format!("writer-{index}");
        for _ in 0..UPDATES {
            // Only stale-version conflicts are retryable. Lock errors and other
            // failures must surface instead of being hidden by a blanket retry.
            wait_until(|| {
                let version = store.get(&id).unwrap().version;
                match store.set_status(&id, MemoryStatus::Candidate, Some(version), &actor) {
                    Ok(_) => true,
                    Err(StoreError::VersionConflict { .. }) => false,
                    Err(error) => panic!("writer failed: {error}"),
                }
            });
        }
    } else {
        let expected = 1 + (WRITERS * UPDATES) as i64;
        let mut previous = 1;
        wait_until(|| {
            let current = store.get(&id).unwrap();
            assert!(
                current.version >= previous,
                "reader observed version reversal"
            );
            assert_eq!(
                current.content,
                "Concurrent updates preserve every committed version"
            );
            previous = current.version;
            current.version == expected
        });
    }
}
