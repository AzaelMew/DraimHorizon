// Bounded local chat archives. All filesystem work runs on one background thread.
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};

pub const MAX_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SCOPES: usize = 16;

fn path(root: &Path, scope: &str) -> PathBuf {
    let hash = scope.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    root.join(format!("chat-{hash:016x}.json"))
}

fn check_directories(root: &Path) -> Result<(), String> {
    for directory in root.ancestors() {
        if directory.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(directory) {
            Ok(meta) if !meta.file_type().is_symlink() && meta.is_dir() => {}
            Ok(_) => return Err("Unsafe chat archive directory".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Cannot inspect chat archive directory".into()),
        }
    }
    Ok(())
}
fn check_file(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => Ok(()),
        Ok(_) => Err("Unsafe chat archive file".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Cannot inspect chat archive file".into()),
    }
}

fn load(root: &Path, scope: &str) -> Result<Value, String> {
    check_directories(root)?;
    let source = path(root, scope);
    check_file(&source)?;
    let file = match File::open(&source) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!([])),
        Err(_) => return Err("Cannot open chat archive".into()),
    };
    check_file(&source)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file
            .metadata()
            .map_err(|_| "Cannot inspect open chat archive")?;
        let current = fs::symlink_metadata(&source).map_err(|_| "Cannot inspect chat archive")?;
        if opened.dev() != current.dev() || opened.ino() != current.ino() {
            return Err("Chat archive changed while opening".into());
        }
    }
    if file
        .metadata()
        .map_err(|_| "Cannot inspect chat archive")?
        .len()
        > MAX_BYTES
    {
        return Err("Chat archive exceeds size limit".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read chat archive")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Chat archive exceeds size limit".into());
    }
    let doc: Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid chat archive JSON")?;
    if doc["schema"].as_u64() != Some(1) || doc["scope"].as_str() != Some(scope) {
        return Err("Chat archive schema or scope mismatch".into());
    }
    if !doc["entries"]
        .as_array()
        .is_some_and(|v| v.len() <= super::claim_borders_chat_feed::MAX_ENTRIES)
    {
        return Err("Invalid chat archive entries".into());
    }
    Ok(doc["entries"].clone())
}

fn save(root: &Path, scope: &str, entries: Value) -> Result<(), String> {
    if !entries
        .as_array()
        .is_some_and(|v| v.len() <= super::claim_borders_chat_feed::MAX_ENTRIES)
    {
        return Err("Invalid chat archive entries".into());
    }
    let bytes = serde_json::to_vec(&json!({"schema": 1, "scope": scope, "entries": entries}))
        .map_err(|_| "Cannot encode chat archive")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Chat archive exceeds size limit".into());
    }
    check_directories(root)?;
    fs::create_dir_all(root).map_err(|_| "Cannot create chat archive directory")?;
    check_directories(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect chat archive directory")?;
    }
    let destination = path(root, scope);
    let temporary = destination.with_extension("tmp");
    check_file(&destination)?;
    check_file(&temporary)?;
    match fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Cannot remove stale chat archive temporary file".into()),
    }
    let mut options = OpenOptions::new();
    // Exclusive creation never follows a pre-existing temporary symlink.
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "Cannot create temporary chat archive")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot write chat archive")?;
    drop(file);
    check_file(&destination)?;
    fs::rename(&temporary, &destination).map_err(|_| "Cannot replace chat archive")?;
    // Keep a bounded number of scopes, evicting the oldest saved archive first.
    let mut archives: Vec<_> = fs::read_dir(root)
        .map_err(|_| "Cannot list chat archives")?
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|s| s.starts_with("chat-") && s.ends_with(".json"))
        })
        .filter_map(|e| {
            fs::symlink_metadata(e.path())
                .ok()
                .filter(|m| m.is_file())
                .map(|m| (m.modified().ok(), e.path()))
        })
        .collect();
    archives.sort_unstable();
    let excess = archives.len().saturating_sub(MAX_SCOPES);
    for (_, old) in archives
        .into_iter()
        .filter(|(_, p)| p != &destination)
        .take(excess)
    {
        fs::remove_file(old).map_err(|_| "Cannot prune old chat archives")?;
    }
    Ok(())
}

fn merge(root: &Path, scope: &str, live: Value) -> Result<(), String> {
    let mut feed = super::claim_borders_chat_feed::Feed::default();
    let archived = load(root, scope).and_then(|archived| {
        feed.restore(
            &archived,
            super::claim_borders_chat_preferences::Preferences::default(),
        )
    });
    if let Err(error) = archived {
        // Corrupt history must not discard new live messages on a quick disconnect.
        // save() still rejects unsafe archive/directory paths before writing.
        eprintln!("[Horizon] {error}");
    }
    let mut rows = feed
        .archive()
        .as_array()
        .cloned()
        .ok_or("Invalid chat archive entries")?;
    rows.extend(
        live.as_array()
            .ok_or("Invalid live chat entries")?
            .iter()
            .cloned(),
    );
    let excess = rows
        .len()
        .saturating_sub(super::claim_borders_chat_feed::MAX_ENTRIES);
    rows.drain(..excess);
    save(root, scope, Value::Array(rows))
}

enum Command {
    Load(String, u64),
    Save(String, Value),
    Merge(String, Value),
    Flush(mpsc::Sender<bool>),
}
pub struct Loaded {
    pub scope: String,
    pub generation: u64,
    pub entries: Result<Value, String>,
}
pub struct Worker {
    tx: SyncSender<Command>,
    rx: Receiver<Loaded>,
    failed: Arc<AtomicBool>,
}
impl Worker {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let (tx, commands) = mpsc::sync_channel(8);
        let (results, rx) = mpsc::channel();
        let failed = Arc::new(AtomicBool::new(false));
        let worker_failed = Arc::clone(&failed);
        std::thread::Builder::new()
            .name("chat-archive".into())
            .spawn(move || {
                let mut healthy = true;
                while let Ok(command) = commands.recv() {
                    match command {
                        Command::Load(scope, generation) => {
                            let entries = load(&root, &scope);
                            let _ = results.send(Loaded {
                                scope,
                                generation,
                                entries,
                            });
                        }
                        Command::Save(scope, entries) => {
                            if let Err(error) = save(&root, &scope, entries) {
                                eprintln!("[Horizon] {error}");
                                healthy = false;
                                worker_failed.store(true, Ordering::Relaxed);
                            }
                        }
                        Command::Merge(scope, live) => {
                            if let Err(error) = merge(&root, &scope, live) {
                                eprintln!("[Horizon] {error}");
                                healthy = false;
                                worker_failed.store(true, Ordering::Relaxed);
                            }
                        }
                        Command::Flush(done) => {
                            let _ = done.send(healthy);
                        }
                    }
                }
            })
            .map_err(|_| "Cannot start chat archive worker")?;
        Ok(Self { tx, rx, failed })
    }
    pub fn load(&self, scope: String, generation: u64) -> bool {
        self.tx.send(Command::Load(scope, generation)).is_ok()
    }
    pub fn save(&self, scope: String, entries: Value, wait: bool) -> bool {
        let command = Command::Save(scope, entries);
        if wait {
            self.tx.send(command).is_ok()
        } else {
            self.tx.try_send(command).is_ok()
        }
    }
    /// Disconnect before the asynchronous load returned: merge on the worker instead.
    pub fn merge(&self, scope: String, live: Value) -> bool {
        self.tx.send(Command::Merge(scope, live)).is_ok()
    }
    pub fn poll(&self) -> Option<Loaded> {
        self.rx.try_recv().ok()
    }
    pub fn failed(&self) -> bool {
        self.failed.swap(false, Ordering::Relaxed)
    }
    pub fn flush(&self) -> bool {
        let (tx, rx) = mpsc::channel();
        self.tx.send(Command::Flush(tx)).is_ok()
            && rx.recv_timeout(Duration::from_secs(3)).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_feed::Feed;
    use crate::chat_preferences::Preferences;
    fn root() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "horizon-chat-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn disconnect_before_load_completion_merges_live_messages_on_worker() {
        let root = root();
        let p = Preferences::default();
        let mut old = Feed::default();
        old.push_with("<Alex> archived", 1, p);
        save(&root, "scope", old.archive()).unwrap();
        let worker = Worker::new(root.clone()).unwrap();
        assert!(worker.load("scope".into(), 1));
        let mut live = Feed::default();
        live.push_with("<Bob> live", 2, p);
        // Do not consume the pending load, as when disconnect/reset runs first.
        assert!(worker.merge("scope".into(), live.archive()));
        assert!(worker.flush());
        let mut restored = Feed::default();
        restored.restore(&load(&root, "scope").unwrap(), p).unwrap();
        assert_eq!(restored.entries.len(), 2);
        assert_eq!(restored.entries[0].original, "archived");
        assert_eq!(restored.entries[1].original, "live");
        assert_eq!(worker.poll().unwrap().generation, 1);
        fs::write(path(&root, "scope"), "{broken").unwrap();
        assert!(worker.merge("scope".into(), live.archive()));
        assert!(worker.flush());
        let mut recovered = Feed::default();
        recovered
            .restore(&load(&root, "scope").unwrap(), p)
            .unwrap();
        assert_eq!(recovered.entries.len(), 1);
        assert_eq!(recovered.entries[0].original, "live");
        drop(worker);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_archive_temp_and_directory_never_read_or_overwrite_targets() {
        use std::os::unix::fs::symlink;
        let root = root();
        let outside = root.join("outside");
        fs::write(&outside, b"private sentinel").unwrap();
        let archive = path(&root, "scope");
        symlink(&outside, &archive).unwrap();
        assert!(load(&root, "scope").is_err());
        assert!(save(&root, "scope", json!([])).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"private sentinel");
        fs::remove_file(&archive).unwrap();
        let temporary = archive.with_extension("tmp");
        symlink(&outside, &temporary).unwrap();
        assert!(save(&root, "scope", json!([])).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"private sentinel");
        let actual = root.join("actual");
        fs::create_dir(&actual).unwrap();
        let link = root.join("linked");
        symlink(&actual, &link).unwrap();
        assert!(load(&link, "scope").is_err());
        assert!(save(&link, "scope", json!([])).is_err());
        assert!(save(&link.join("child"), "scope", json!([])).is_err());
        assert_eq!(fs::read_dir(&actual).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn disk_roundtrip_preserves_display_data_without_live_state() {
        let root = root();
        let mut feed = Feed::default();
        let p = Preferences::default();
        let id = feed.push_local("[Party] hello", 5, p).unwrap();
        feed.translate(
            id,
            "привет".into(),
            claim_borders_relay_client::translation::Language::Russian,
        );
        feed.toggle_original(id);
        let worker = Worker::new(root.clone()).unwrap();
        assert!(worker.save("server#account".into(), feed.archive(), true));
        assert!(worker.flush());
        let mut restored = Feed::default();
        restored
            .restore(&load(&root, "server#account").unwrap(), p)
            .unwrap();
        let e = &restored.entries[0];
        assert_eq!(e.original, "[Party] hello");
        assert_eq!(e.translated.as_deref(), Some("привет"));
        assert!(e.show_original && e.archived);
        assert_eq!(e.channel, crate::chat_feed::Channel::Party);
        assert!(restored.last_whisper_sender.is_none());
        drop(worker);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupt_oversized_and_wrong_scope_are_rejected() {
        let root = root();
        fs::write(path(&root, "a"), "{broken").unwrap();
        assert!(load(&root, "a").is_err());
        let file = File::create(path(&root, "a")).unwrap();
        file.set_len(MAX_BYTES + 1).unwrap();
        assert!(load(&root, "a").is_err());
        save(&root, "a", json!([])).unwrap();
        fs::copy(path(&root, "a"), path(&root, "b")).unwrap();
        assert!(load(&root, "b").is_err());
        assert_eq!(load(&root, "missing").unwrap(), json!([]));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scopes_are_independent_and_retention_is_bounded() {
        let root = root();
        for i in 0..20 {
            save(&root, &format!("server#{i}"), json!([])).unwrap();
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), MAX_SCOPES);
        save(&root, "server#alice", json!([{"test": "alice"}])).unwrap();
        save(&root, "server#bob", json!([{"test": "bob"}])).unwrap();
        assert_eq!(load(&root, "server#alice").unwrap()[0]["test"], "alice");
        assert_eq!(load(&root, "server#bob").unwrap()[0]["test"], "bob");
        fs::remove_dir_all(root).unwrap();
    }
}
