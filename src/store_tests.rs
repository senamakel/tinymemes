use super::*;

fn tmp() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tinymemes-store-{}-{}",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn atomic_writes_replace_the_file_and_leave_no_temp_files() {
    let dir = tmp();
    let path = dir.join(INDEX_FILE);
    write_atomic(&path, "{\"a\":1}").unwrap();
    write_atomic(&path, "{\"a\":2}").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn a_failed_write_leaves_no_temp_file() {
    let dir = tmp();
    let target = dir.join("taken");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("child"), "x").unwrap();
    assert!(write_atomic(&target, "{}").is_err());
    let leftovers = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn an_oversized_remixed_file_is_capped_on_load() {
    let dir = tmp();
    let ids: Vec<String> = (0..REMIXED_CAP + 50).map(|i| format!("id-{i}")).collect();
    std::fs::write(dir.join(REMIXED_FILE), serde_json::to_string(&ids).unwrap()).unwrap();
    let loaded = FileIndexStore::new(&dir).load_remixed();
    assert_eq!(loaded.len(), REMIXED_CAP);
    assert_eq!(loaded.front().map(String::as_str), Some("id-50"));
    assert_eq!(
        loaded.back().map(String::as_str),
        Some(format!("id-{}", REMIXED_CAP + 49)).as_deref()
    );
}

#[test]
fn unreadable_or_missing_state_starts_fresh() {
    let dir = tmp();
    for file in [INDEX_FILE, MEME_INDEX_FILE, REMIXED_FILE] {
        std::fs::write(dir.join(file), "not json").unwrap();
    }
    let store = FileIndexStore::new(&dir);
    assert_eq!(store.load_slang().len("IN"), 0);
    assert_eq!(store.load_memes().len("IN"), 0);
    assert!(store.load_remixed().is_empty());
    let empty = FileIndexStore::new(tmp());
    assert_eq!(empty.load_slang().len("IN"), 0);
    assert!(empty.load_remixed().is_empty());
}

#[test]
fn state_round_trips_and_creates_the_directory() {
    let dir = tmp().join("nested");
    let store = FileIndexStore::new(&dir);
    let slang = store.load_slang();
    let memes = store.load_memes();
    store.save_slang(&slang).unwrap();
    store.save_memes(&memes).unwrap();
    let ids: VecDeque<String> = ["m1", "m2"].map(String::from).into();
    store.save_remixed(&ids).unwrap();
    assert_eq!(store.load_slang().to_json(), slang.to_json());
    assert_eq!(store.load_memes().to_json(), memes.to_json());
    assert_eq!(store.load_remixed(), ids);
}

#[test]
fn saving_remixed_caps_the_ring_keeping_the_newest() {
    let store = FileIndexStore::new(tmp());
    let ids: VecDeque<String> = (0..REMIXED_CAP + 3).map(|i| i.to_string()).collect();
    store.save_remixed(&ids).unwrap();
    let loaded = store.load_remixed();
    assert_eq!(loaded.len(), REMIXED_CAP);
    assert_eq!(loaded.front().map(String::as_str), Some("3"));
}

#[test]
fn legacy_index_fixture_loads_unchanged() {
    let dir = tmp();
    std::fs::write(dir.join(REMIXED_FILE), r#"["a","b","c"]"#).unwrap();
    // Re-save through the port and confirm the legacy ids survive untouched.
    let store = FileIndexStore::new(&dir);
    let ids = store.load_remixed();
    assert_eq!(ids, ["a", "b", "c"]);
    store.save_remixed(&ids).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join(REMIXED_FILE)).unwrap(),
        r#"["a","b","c"]"#
    );
    // Index files written by the engine reload to the same JSON.
    let slang = store.load_slang();
    std::fs::write(dir.join(INDEX_FILE), slang.to_json()).unwrap();
    assert_eq!(store.load_slang().to_json(), slang.to_json());
}
