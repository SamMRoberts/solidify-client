use super::*;
use std::fs;
static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "solidify-profiles-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> store::Store {
        store::Store::new(Some(self.0.clone()))
    }
    fn file(&self) -> PathBuf {
        self.0.join("profiles.json")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn input(name: &str) -> ProfileInput {
    ProfileInput {
        name: name.into(),
        host: "localhost".into(),
        port: 4000,
        appearance: Appearance::default(),
    }
}
fn ready(store: &mut store::Store) {
    assert!(store.execute(ProfileOperation::Load).unwrap().writable);
}
#[test]
fn round_trip_crud_and_monotonic_identity() {
    let dir = Directory::new();
    let mut store = dir.store();
    ready(&mut store);
    assert!(!dir.file().exists());
    let snapshot = store
        .execute(ProfileOperation::Create(input(" First ")))
        .unwrap();
    assert_eq!(snapshot.selected, Some(1));
    assert_eq!(snapshot.profiles[0].name, "First");
    store
        .execute(ProfileOperation::Update {
            id: 1,
            input: input("Renamed"),
        })
        .unwrap();
    let appearance = Appearance {
        font_size: 24,
        foreground: "#abcdef".into(),
        background: "#010203".into(),
    };
    store
        .execute(ProfileOperation::Appearance {
            id: 1,
            appearance: appearance.clone(),
        })
        .unwrap();
    drop(store);
    let mut store = dir.store();
    ready(&mut store);
    let snapshot = store.execute(ProfileOperation::Load).unwrap();
    assert_eq!(snapshot.profiles[0].appearance, appearance);
    store.execute(ProfileOperation::Select(None)).unwrap();
    store.execute(ProfileOperation::Delete(1)).unwrap();
    assert_eq!(
        store
            .execute(ProfileOperation::Create(input("Next")))
            .unwrap()
            .selected,
        Some(2)
    );
}
#[test]
fn record_validation_and_exact_limits() {
    let dir = Directory::new();
    let mut store = dir.store();
    ready(&mut store);
    store
        .execute(ProfileOperation::Create(input(&"é".repeat(64))))
        .unwrap();
    assert_eq!(
        store
            .execute(ProfileOperation::Create(input(&"é".repeat(65))))
            .unwrap_err(),
        StorageError::InvalidName
    );
    for name in ["", "  ", "bad\nname", "bad\0name"] {
        assert_eq!(
            store
                .execute(ProfileOperation::Create(input(name)))
                .unwrap_err(),
            StorageError::InvalidName
        );
    }
    assert_eq!(
        store
            .execute(ProfileOperation::Create(input(&format!(
                " {} ",
                "é".repeat(64)
            ))))
            .unwrap_err(),
        StorageError::DuplicateName
    );
    for size in [9, 25] {
        let mut p = input("Invalid");
        p.appearance.font_size = size;
        assert_eq!(
            store.execute(ProfileOperation::Create(p)).unwrap_err(),
            StorageError::InvalidAppearance
        );
    }
    for color in ["red", "#fff", "#00000z", "#00000000"] {
        let mut p = input("Invalid");
        p.appearance.background = color.into();
        assert_eq!(
            store.execute(ProfileOperation::Create(p)).unwrap_err(),
            StorageError::InvalidAppearance
        );
    }
    for host in ["https://localhost", "bad host", "é.com"] {
        let mut p = input("Invalid");
        p.host = host.into();
        assert_eq!(
            store.execute(ProfileOperation::Create(p)).unwrap_err(),
            StorageError::InvalidEndpoint
        );
    }
    for port in [0, 65536] {
        let mut p = input("Invalid");
        p.port = port;
        assert_eq!(
            store.execute(ProfileOperation::Create(p)).unwrap_err(),
            StorageError::InvalidEndpoint
        );
    }
    for index in 1..MAX_PROFILES {
        store
            .execute(ProfileOperation::Create(input(&format!("Profile {index}"))))
            .unwrap();
    }
    assert_eq!(
        store
            .execute(ProfileOperation::Create(input("Overflow")))
            .unwrap_err(),
        StorageError::Limit
    );
}
#[test]
fn corrupt_files_are_untouched_and_retry_recovers() {
    let dir = Directory::new();
    let mut store = dir.store();
    for text in [
        "broken",
        r#"{"version":2,"nextId":1,"selected":null,"profiles":[]}"#,
        r#"{"version":1,"nextId":1,"selected":null,"profiles":[],"extra":true}"#,
        r#"{"version":1,"nextId":1,"selected":3,"profiles":[]}"#,
    ] {
        fs::write(dir.file(), text).unwrap();
        let snapshot = store.execute(ProfileOperation::Load).unwrap();
        assert!(snapshot.warning.is_some());
        assert!(!snapshot.writable);
        assert_eq!(
            store
                .execute(ProfileOperation::Create(input("No")))
                .unwrap_err(),
            StorageError::ReadOnly
        );
        assert_eq!(fs::read_to_string(dir.file()).unwrap(), text);
    }
    fs::write(
        dir.file(),
        serde_json::to_vec(&model::Document::default()).unwrap(),
    )
    .unwrap();
    ready(&mut store);
}
#[test]
fn file_size_exact_limit_and_one_byte_overflow() {
    let dir = Directory::new();
    let mut bytes = serde_json::to_vec(&model::Document::default()).unwrap();
    bytes.resize(MAX_FILE_BYTES, b' ');
    fs::write(dir.file(), &bytes).unwrap();
    let mut store = dir.store();
    ready(&mut store);
    bytes.push(b' ');
    fs::write(dir.file(), &bytes).unwrap();
    assert_eq!(
        store.execute(ProfileOperation::Load).unwrap().warning,
        Some(StorageError::Limit)
    );
    assert_eq!(fs::read(dir.file()).unwrap(), bytes);
}
#[test]
fn invalid_records_ids_references_and_unknown_fields_are_rejected() {
    let dir = Directory::new();
    let mut store = dir.store();
    ready(&mut store);
    store
        .execute(ProfileOperation::Create(input("One")))
        .unwrap();
    let base: serde_json::Value = serde_json::from_slice(&fs::read(dir.file()).unwrap()).unwrap();
    for (pointer, value) in [
        ("/profiles/0/id", serde_json::json!(0)),
        ("/nextId", serde_json::json!(1)),
        ("/selected", serde_json::json!(999)),
        ("/profiles/0/appearance/fontSize", serde_json::json!(25)),
        ("/profiles/0/appearance/extra", serde_json::json!(true)),
        ("/profiles/0/extra", serde_json::json!(true)),
    ] {
        let mut doc = base.clone();
        if pointer.ends_with("/extra") {
            let parent = pointer.strip_suffix("/extra").unwrap();
            doc.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("extra".into(), value);
        } else {
            *doc.pointer_mut(pointer).unwrap() = value;
        }
        let bytes = serde_json::to_vec(&doc).unwrap();
        fs::write(dir.file(), &bytes).unwrap();
        assert!(
            store
                .execute(ProfileOperation::Load)
                .unwrap()
                .warning
                .is_some()
        );
        assert_eq!(fs::read(dir.file()).unwrap(), bytes);
    }
}
#[test]
fn lock_contention_is_read_only_until_explicit_retry() {
    let dir = Directory::new();
    let mut first = dir.store();
    ready(&mut first);
    first
        .execute(ProfileOperation::Create(input("One")))
        .unwrap();
    let mut second = dir.store();
    let snapshot = second.execute(ProfileOperation::Load).unwrap();
    assert_eq!(snapshot.warning, Some(StorageError::ReadOnly));
    assert_eq!(snapshot.profiles.len(), 1);
    drop(first);
    assert_eq!(
        second.execute(ProfileOperation::Select(None)).unwrap_err(),
        StorageError::ReadOnly
    );
    ready(&mut second);
    second.execute(ProfileOperation::Select(None)).unwrap();
}
#[test]
fn failed_commit_preserves_file_and_published_state_and_ignores_orphan_temp() {
    let dir = Directory::new();
    let mut store = dir.store();
    ready(&mut store);
    store
        .execute(ProfileOperation::Create(input("One")))
        .unwrap();
    let before = fs::read(dir.file()).unwrap();
    fs::write(dir.0.join(".profiles-interrupted.tmp"), "partial").unwrap();
    store.fail_at = Some(store::WriteStage::Replace);
    assert_eq!(
        store
            .execute(ProfileOperation::Create(input("Two")))
            .unwrap_err(),
        StorageError::Unavailable
    );
    assert_eq!(fs::read(dir.file()).unwrap(), before);
    let snapshot = store.execute(ProfileOperation::Load).unwrap();
    assert_eq!(snapshot.profiles.len(), 1);
    assert_eq!(snapshot.selected, Some(1));
    store.fail_at = None;
    assert_eq!(
        store
            .execute(ProfileOperation::Create(input("Two")))
            .unwrap()
            .selected,
        Some(2)
    );
}
#[test]
fn unavailable_directory_and_unreadable_destination_stay_safe() {
    let dir = Directory::new();
    fs::create_dir(dir.file()).unwrap();
    let mut store = dir.store();
    assert!(
        store
            .execute(ProfileOperation::Load)
            .unwrap()
            .warning
            .is_some()
    );
    assert!(dir.file().is_dir());
    let mut unavailable = store::Store::new(None);
    assert_eq!(
        unavailable.execute(ProfileOperation::Load).unwrap().warning,
        Some(StorageError::Unavailable)
    );
}
#[tokio::test]
async fn bounded_admission_and_shutdown_wait_for_accepted_write() {
    let dir = Directory::new();
    let service = Arc::new(ProfileService::new(Some(dir.0.clone())));
    service.execute(ProfileOperation::Load).await.unwrap();
    // Holding the store mutex makes the accepted operation wait inside spawn_blocking.
    let (announce_ready, wait_ready) = tokio::sync::oneshot::channel();
    let (release, wait_release) = std::sync::mpsc::channel();
    let held_store = service.store.clone();
    let holder = tokio::task::spawn_blocking(move || {
        let _guard = held_store.lock().unwrap();
        announce_ready.send(()).unwrap();
        wait_release
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    });
    wait_ready.await.unwrap();
    let writer = service.clone();
    let task = tokio::spawn(async move {
        writer
            .execute(ProfileOperation::Create(input("Accepted")))
            .await
    });
    tokio::task::yield_now().await;
    assert_eq!(
        service.execute(ProfileOperation::Load).await.unwrap_err(),
        StorageError::Busy
    );
    let shutdown = service.clone();
    let closing = tokio::spawn(async move { shutdown.shutdown().await });
    tokio::task::yield_now().await;
    assert!(!closing.is_finished());
    release.send(()).unwrap();
    holder.await.unwrap();
    task.await.unwrap().unwrap();
    closing.await.unwrap();
    assert_eq!(
        service.execute(ProfileOperation::Load).await.unwrap_err(),
        StorageError::ShuttingDown
    );
    let mut store = dir.store();
    ready(&mut store);
    assert_eq!(
        store.execute(ProfileOperation::Load).unwrap().profiles[0].name,
        "Accepted"
    );
}

#[test]
fn every_precommit_io_failure_preserves_the_snapshot() {
    use store::WriteStage::*;
    let dir = Directory::new();
    let mut store = dir.store();
    ready(&mut store);
    store
        .execute(ProfileOperation::Create(input("Original")))
        .unwrap();
    let before = fs::read(dir.file()).unwrap();
    for stage in [Create, Write, Flush, Sync, Replace] {
        store.fail_at = Some(stage);
        assert_eq!(
            store
                .execute(ProfileOperation::Create(input("Rejected")))
                .unwrap_err(),
            StorageError::Unavailable
        );
        assert_eq!(fs::read(dir.file()).unwrap(), before);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
    }
    store.fail_at = None;
    assert_eq!(
        store
            .execute(ProfileOperation::Create(input("Accepted")))
            .unwrap()
            .selected,
        Some(2)
    );
}
#[test]
fn invalid_id_references_duplicate_json_fields_and_counter_exhaustion() {
    let dir = Directory::new();
    let mut store = dir.store();
    ready(&mut store);
    assert_eq!(
        store
            .execute(ProfileOperation::Select(Some(9)))
            .unwrap_err(),
        StorageError::MissingProfile
    );
    assert_eq!(
        store.execute(ProfileOperation::Delete(9)).unwrap_err(),
        StorageError::MissingProfile
    );
    fs::write(
        dir.file(),
        r#"{"version":1,"version":1,"nextId":1,"selected":null,"profiles":[]}"#,
    )
    .unwrap();
    assert_eq!(
        store.execute(ProfileOperation::Load).unwrap().warning,
        Some(StorageError::InvalidFile)
    );
    let document = model::Document {
        next_id: u64::MAX,
        ..model::Document::default()
    };
    fs::write(dir.file(), serde_json::to_vec(&document).unwrap()).unwrap();
    ready(&mut store);
    assert_eq!(
        store
            .execute(ProfileOperation::Create(input("No IDs left")))
            .unwrap_err(),
        StorageError::Limit
    );
}
