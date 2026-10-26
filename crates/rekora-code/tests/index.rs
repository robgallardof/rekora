//! Integration: real fixtures copied into real Git repositories, indexed into a
//! real SQLite store (S05, S16, R05, R16, R25, R36).

#![allow(clippy::unwrap_used)] // test helpers fail loudly by design

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rekora_code::index;
use rekora_core::{MemoryKind, MemoryStatus, NewMemory, Origin, SourcePolicy};
use rekora_store::{SearchOptions, Store};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(["-c", "core.autocrlf=false"])
        .args(args)
        .stderr(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// All four language fixtures in one committed repository.
fn polyglot() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "csharp-service",
        "typescript-web",
        "rust-lib",
        "python-package",
    ] {
        copy_dir(&fixtures().join(name), &dir.path().join(name));
    }
    std::fs::write(dir.path().join(".gitignore"), "generated/\n").unwrap();
    std::fs::create_dir_all(dir.path().join("generated")).unwrap();
    std::fs::write(
        dir.path().join("generated/out.ts"),
        "export function shouldNotBeIndexed() {}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join(".env"), "TOKEN=x\n").unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "fixtures"]);
    let (store, _) = Store::init(&dir.path().join(".rekora/rekora.db")).unwrap();
    (dir, store)
}

fn branch_of(dir: &Path) -> String {
    git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
}

#[test]
fn s16_r16_symbols_across_languages_with_exact_locations() {
    let (dir, mut store) = polyglot();
    let root = dir.path().to_owned();
    let report = index(&mut store, &root, &|| branch_of(&root)).unwrap();
    assert_eq!(report.snapshot, "main");
    assert!(report.parsed >= 9, "{report:?}");
    assert!(report.limits.contains("no references"));

    let expect = [
        (
            "OrderService",
            "csharp-service/src/Orders/OrderService.cs",
            4,
            "class",
        ),
        (
            "IOrderRepository",
            "csharp-service/src/Orders/Model.cs",
            7,
            "interface",
        ),
        (
            "UpdateLineAsync",
            "csharp-service/src/Orders/OrderService.cs",
            15,
            "method",
        ),
        (
            "OrderDto",
            "typescript-web/src/api/contract.ts",
            1,
            "interface",
        ),
        (
            "saveOrder",
            "typescript-web/src/api/contract.ts",
            19,
            "function",
        ),
        (
            "OrderEditor",
            "typescript-web/src/components/OrderEditor.tsx",
            3,
            "function",
        ),
        ("Storage", "rust-lib/src/storage.rs", 4, "trait"),
        ("Memory", "rust-lib/src/memory.rs", 3, "struct"),
        (
            "OrderRepository",
            "python-package/orders/repository.py",
            1,
            "class",
        ),
        (
            "load_orders",
            "python-package/orders/repository.py",
            11,
            "function",
        ),
    ];
    for (name, path, line, kind) in expect {
        let hits = store.find_symbols(name, Some("main"), 20).unwrap();
        assert!(
            hits.iter()
                .any(|h| h.name == name && h.path == path && h.line == line && h.kind == kind),
            "{name} not at {path}:{line} as {kind}: {hits:?}"
        );
    }
    let hashes = store.indexed_hashes("main").unwrap();
    // Vue is file-level only, honestly: the file is known but has no symbols.
    let vue = "typescript-web/src/components/OrderList.vue";
    assert!(hashes.contains_key(vue));
    assert!(store.file_symbols("main", vue).unwrap().is_empty());
    // Dynamic Python attribute is not claimed; ignored and secret files not indexed.
    assert!(
        store
            .find_symbols("export_all", None, 10)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .find_symbols("shouldNotBeIndexed", None, 10)
            .unwrap()
            .is_empty()
    );
    assert!(!hashes.contains_key(".env"));
}

#[test]
fn s05_r05_r36_incremental_reindex_and_stale_memory() {
    let (dir, mut store) = polyglot();
    let root = dir.path().to_owned();
    let first = index(&mut store, &root, &|| branch_of(&root)).unwrap();

    let again = index(&mut store, &root, &|| branch_of(&root)).unwrap();
    assert_eq!(again.parsed, 0, "unchanged files reparsed: {again:?}");
    assert_eq!(again.unchanged, first.parsed + first.file_only);

    // A verified decision citing the storage trait.
    let policy = SourcePolicy::new(&root).unwrap();
    let memory = NewMemory {
        kind: MemoryKind::Decision,
        content: "Storage trait stays object-safe; no generic methods".into(),
        origin: Origin::Human,
        status: MemoryStatus::Verified,
        sources: vec![policy.resolve("rust-lib/src/storage.rs:4", &root).unwrap()],
        branch: Some("main".into()),
        commit: None,
    };
    let id = store
        .add(memory.validate().unwrap(), "t")
        .unwrap()
        .memory
        .id;

    // Rename the trait and delete one Python file; nothing else changes.
    let storage = root.join("rust-lib/src/storage.rs");
    let text = std::fs::read_to_string(&storage)
        .unwrap()
        .replace("trait Storage", "trait MemoryStore");
    std::fs::write(&storage, text).unwrap();
    std::fs::remove_file(root.join("python-package/orders/__init__.py")).unwrap();

    let third = index(&mut store, &root, &|| branch_of(&root)).unwrap();
    assert_eq!(
        third.parsed, 1,
        "only the edited file should be parsed: {third:?}"
    );
    assert_eq!(third.removed, 1);
    let renamed = store.find_symbols("MemoryStore", Some("main"), 5).unwrap();
    assert!(renamed.iter().any(|h| h.path == "rust-lib/src/storage.rs"));
    let old = store.find_symbols("Storage", Some("main"), 50).unwrap();
    assert!(!old.iter().any(|h| h.name == "Storage" && h.kind == "trait"));

    // The decision is flagged, not deleted; its history stays.
    let hit = store
        .search(
            "storage trait object safe",
            &SearchOptions::default(),
            Some(&policy),
        )
        .unwrap()
        .hits
        .into_iter()
        .find(|h| h.memory.id == id)
        .unwrap();
    assert!(hit.possibly_stale);
    assert_eq!(store.history(&id).unwrap().len(), 1);
}

#[test]
fn branches_have_separate_snapshots() {
    let (dir, mut store) = polyglot();
    let root = dir.path().to_owned();
    index(&mut store, &root, &|| branch_of(&root)).unwrap();
    git(&root, &["checkout", "-q", "-b", "feature/rename"]);
    let model = root.join("csharp-service/src/Orders/Model.cs");
    let text = std::fs::read_to_string(&model)
        .unwrap()
        .replace("IOrderRepository", "IOrderStore");
    std::fs::write(&model, text).unwrap();
    git(&root, &["commit", "-q", "-am", "rename"]);
    let report = index(&mut store, &root, &|| branch_of(&root)).unwrap();
    assert_eq!(report.snapshot, "feature/rename");

    let found = |name: &str, snap: &str| {
        store
            .find_symbols(name, Some(snap), 5)
            .unwrap()
            .iter()
            .any(|h| h.name == name)
    };
    assert!(!found("IOrderStore", "main"));
    assert!(found("IOrderStore", "feature/rename"));
    assert!(found("IOrderRepository", "main"));
}

#[test]
fn r25_snapshot_change_during_scan_is_refused() {
    let (dir, mut store) = polyglot();
    let root = dir.path().to_owned();
    // A branch switch landing between the pre- and post-scan checks on every
    // attempt: the index must refuse rather than mix snapshots.
    let calls = Cell::new(0);
    let flaky = || {
        calls.set(calls.get() + 1);
        format!("branch-{}", calls.get())
    };
    let err = index(&mut store, &root, &flaky).unwrap_err();
    assert_eq!(err.code(), "snapshot_changed");
    assert_eq!(calls.get(), 4, "expected exactly one retry");
    assert!(store.indexed_hashes("branch-1").unwrap().is_empty());
    assert!(store.indexed_hashes("branch-3").unwrap().is_empty());

    // A switch during the first attempt only is absorbed by the retry.
    let calls = Cell::new(0);
    let once = || {
        calls.set(calls.get() + 1);
        if calls.get() == 1 { "old" } else { "new" }.to_owned()
    };
    let report = index(&mut store, &root, &once).unwrap();
    assert_eq!(report.snapshot, "new");
}
