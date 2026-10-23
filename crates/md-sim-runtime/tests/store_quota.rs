use md_sim_runtime::store::{ObjectId, Store, StoreLimits, StoreUsage};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::{Arc, Barrier},
    thread,
};

fn identity(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: hex::encode(Sha256::digest(bytes)),
        size: bytes.len() as u64,
    }
}
fn limits(bytes: u64, objects: u64) -> StoreLimits {
    StoreLimits {
        max_object_bytes: bytes,
        max_total_bytes: bytes,
        max_objects: objects,
    }
}

#[test]
fn append_retry_abort_publish_and_restart_share_one_quota() {
    let root = tempfile::tempdir().unwrap();
    let policy = limits(6, 3);
    let abc = identity(b"abc");
    let def = identity(b"def");
    let xyz = identity(b"xyz");
    let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
    assert_eq!(store.limits(), &policy);
    store.begin(&abc).unwrap();
    store.chunk(&abc, 0, b"abc").unwrap();
    store.finish(&abc).unwrap();
    store.begin(&def).unwrap();
    store.chunk(&def, 0, b"de").unwrap();
    for _ in 0..10 {
        assert_eq!(store.begin(&def).unwrap(), 2);
        assert_eq!(store.chunk(&def, 0, b"de").unwrap(), 2);
    }
    assert_eq!(
        store.usage().unwrap(),
        StoreUsage {
            bytes: 5,
            objects: 2
        }
    );
    store.begin(&xyz).unwrap();
    store.chunk(&xyz, 0, b"x").unwrap();
    assert!(store.chunk(&def, 2, b"f").is_err());
    assert_eq!(
        fs::read(root.path().join("incoming").join(&def.sha256)).unwrap(),
        b"de"
    );
    drop(store);
    let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
    assert_eq!(
        store.usage().unwrap(),
        StoreUsage {
            bytes: 6,
            objects: 3
        }
    );
    assert_eq!(store.begin(&def).unwrap(), 2);
    assert_eq!(store.chunk(&def, 0, b"de").unwrap(), 2);
    assert!(store.chunk(&def, 2, b"f").is_err());
    store.abort(&xyz).unwrap();
    store.abort(&xyz).unwrap();
    store.chunk(&def, 2, b"f").unwrap();
    store.finish(&def).unwrap();
    store.abort(&def).unwrap();
    assert_eq!(
        store.usage().unwrap(),
        StoreUsage {
            bytes: 6,
            objects: 2
        }
    );
    assert_eq!(store.read_chunk(&abc, 0, 3).unwrap(), b"abc");
    assert_eq!(store.read_chunk(&def, 0, 3).unwrap(), b"def");
    let saved_policy = fs::read(root.path().join("limits.json")).unwrap();
    drop(store);
    assert!(Store::open_with_limits(root.path(), limits(7, 3))
        .err()
        .unwrap()
        .to_string()
        .contains("persisted policy"));
    assert_eq!(
        fs::read(root.path().join("limits.json")).unwrap(),
        saved_policy
    );
    let store = Store::open_with_limits(root.path(), policy).unwrap();
    assert_eq!(store.usage().unwrap().bytes, 6);
}

#[test]
fn empty_transfers_consume_slots_across_begin_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let policy = limits(6, 2);
    let first = identity(b"a");
    let second = identity(b"b");
    let third = identity(b"c");
    let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
    store.begin(&first).unwrap();
    store.begin(&second).unwrap();
    assert!(store.begin(&third).is_err());
    drop(store);
    for _ in 0..3 {
        let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
        assert_eq!(store.begin(&first).unwrap(), 0);
        assert!(store.begin(&third).is_err());
        assert_eq!(
            store.usage().unwrap(),
            StoreUsage {
                bytes: 0,
                objects: 2
            }
        );
    }
    assert!(!root.path().join("incoming").join(&third.sha256).exists());
    let store = Store::open_with_limits(root.path(), policy).unwrap();
    store.abort(&second).unwrap();
    store.begin(&third).unwrap();
}

#[test]
fn concurrent_appends_and_store_handles_cannot_bypass_quota() {
    let root = tempfile::tempdir().unwrap();
    let policy = limits(4, 2);
    let store = Arc::new(Store::open_with_limits(root.path(), policy.clone()).unwrap());
    assert!(Store::open_with_limits(root.path(), policy.clone()).is_err());
    let barrier = Arc::new(Barrier::new(3));
    let mut tasks = Vec::new();
    for data in [b"abcd", b"efgh"] {
        let id = identity(data);
        store.begin(&id).unwrap();
        let store = store.clone();
        let barrier = barrier.clone();
        tasks.push(thread::spawn(move || {
            barrier.wait();
            store.chunk(&id, 0, data).is_ok()
        }));
    }
    barrier.wait();
    assert_eq!(
        tasks
            .into_iter()
            .map(|task| task.join().unwrap())
            .filter(|ok| *ok)
            .count(),
        1
    );
    assert_eq!(
        store.usage().unwrap(),
        StoreUsage {
            bytes: 4,
            objects: 2
        }
    );
    drop(store);
    let store = Store::open_with_limits(root.path(), policy).unwrap();
    assert_eq!(store.usage().unwrap().bytes, 4);
}

#[test]
fn interrupted_publication_counts_once_and_never_overwrites_objects() {
    let root = tempfile::tempdir().unwrap();
    let policy = limits(3, 1);
    let id = identity(b"abc");
    let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
    store.begin(&id).unwrap();
    store.chunk(&id, 0, b"abc").unwrap();
    let incoming = root.path().join("incoming").join(&id.sha256);
    let published = root.path().join("objects").join(&id.sha256);
    fs::set_permissions(&incoming, fs::Permissions::from_mode(0o555)).unwrap();
    fs::hard_link(&incoming, &published).unwrap(); // crash point before unlink
    drop(store);
    let store = Store::open_with_limits(root.path(), policy).unwrap();
    assert_eq!(
        store.usage().unwrap(),
        StoreUsage {
            bytes: 3,
            objects: 1
        }
    );
    assert!(store.chunk(&id, 0, b"bad").is_err());
    assert_eq!(store.begin(&id).unwrap(), 3);
    store.finish(&id).unwrap();
    assert!(!incoming.exists());
    assert_eq!(fs::read(&published).unwrap(), b"abc");
    fs::write(&incoming, b"bad").unwrap();
    assert!(store.finish(&id).is_err());
    assert_eq!(fs::read(published).unwrap(), b"abc");
    assert_eq!(fs::read(incoming).unwrap(), b"bad");
}

#[test]
fn unsafe_entries_fail_closed_and_preserve_external_files() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let sentinel = outside.path().join("sentinel");
    fs::write(&sentinel, b"abc").unwrap();
    let policy = limits(6, 2);
    let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
    let id = identity(b"abc");
    let path = root.path().join("incoming").join(&id.sha256);
    std::os::unix::fs::symlink(&sentinel, &path).unwrap();
    assert!(store.begin(&id).is_err());
    assert!(store.abort(&id).is_err());
    fs::remove_file(&path).unwrap();
    fs::hard_link(&sentinel, &path).unwrap();
    assert!(store.usage().is_err());
    assert!(store.abort(&id).is_err());
    drop(store);
    assert!(Store::open_with_limits(root.path(), policy).is_err());
    assert_eq!(fs::read(&sentinel).unwrap(), b"abc");
    assert_eq!(fs::read(&path).unwrap(), b"abc");
}

#[test]
fn incomplete_policy_initialization_recovers_without_replacing_valid_policy() {
    let root = tempfile::tempdir().unwrap();
    let policy = limits(6, 2);
    let pending = root.path().join("limits.pending.json");
    let committed = root.path().join("limits.json");
    fs::write(&pending, b"{\"max_object_bytes\":").unwrap();
    let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
    assert_eq!(store.limits(), &policy);
    assert!(!pending.exists());
    let saved = fs::read(&committed).unwrap();
    drop(store);

    // A crash after atomic publication leaves both names for the same file.
    fs::hard_link(&committed, &pending).unwrap();
    let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
    assert!(!pending.exists());
    assert_eq!(fs::read(&committed).unwrap(), saved);
    drop(store);

    // A complete pending policy also fixes the intended limits before publish.
    fs::rename(&committed, &pending).unwrap();
    assert!(Store::open_with_limits(root.path(), limits(7, 2)).is_err());
    assert_eq!(fs::read(&pending).unwrap(), saved);
    assert!(!committed.exists());
    let store = Store::open_with_limits(root.path(), policy.clone()).unwrap();
    assert_eq!(fs::read(&committed).unwrap(), saved);
    drop(store);

    // A separate pending file is not a recognized publication interruption.
    fs::write(&pending, b"unrelated bytes").unwrap();
    assert!(Store::open_with_limits(root.path(), policy.clone()).is_err());
    assert_eq!(fs::read(&pending).unwrap(), b"unrelated bytes");
    assert_eq!(fs::read(&committed).unwrap(), saved);
    fs::remove_file(&pending).unwrap();

    // Corruption of an already committed policy must remain a visible error.
    fs::write(&committed, b"{").unwrap();
    assert!(Store::open_with_limits(root.path(), policy).is_err());
    assert_eq!(fs::read(&committed).unwrap(), b"{");
}
