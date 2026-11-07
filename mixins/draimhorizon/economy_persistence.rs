// Small bounded background lane for observed prices; it never overwrites an unknown schema.
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    time::Duration,
};
const LIMIT: u64 = 4 * 1024 * 1024;
pub fn load(path: &Path, scope: &str) -> Result<Option<String>, String> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut raw = String::new();
    file.take(LIMIT + 1)
        .read_to_string(&mut raw)
        .map_err(|e| e.to_string())?;
    if raw.len() as u64 > LIMIT {
        return Err("economy cache too large".into());
    }
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|_| "invalid economy JSON")?;
    if v["schema"] != 1 || v["scope"].as_str() != Some(scope) {
        return Err("economy cache scope/schema mismatch".into());
    }
    Ok(Some(raw))
}
fn save(path: &Path, scope: &str, raw: &str) -> Result<(), String> {
    load(path, scope)?;
    if raw.len() as u64 > LIMIT {
        return Err("economy cache too large".into());
    }
    fs::create_dir_all(path.parent().ok_or("invalid economy path")?).map_err(|e| e.to_string())?;
    let temp = path.with_extension("pending");
    let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(raw.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}
enum Job {
    Save(PathBuf, String, String),
    Finish(SyncSender<()>),
}
pub struct Worker {
    tx: SyncSender<Job>,
    pub errors: Receiver<String>,
}
impl Worker {
    pub fn spawn() -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel(4);
        let (errors, results) = mpsc::sync_channel(4);
        std::thread::Builder::new()
            .name("draimhorizons-economy-io".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    match job {
                        Job::Save(path, scope, raw) => {
                            if let Err(e) = save(&path, &scope, &raw) {
                                let _ = errors.try_send(e);
                            }
                        }
                        Job::Finish(done) => {
                            let _ = done.try_send(());
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            tx,
            errors: results,
        })
    }
    pub fn save(&self, path: PathBuf, scope: String, raw: String) -> bool {
        self.tx.try_send(Job::Save(path, scope, raw)).is_ok()
    }
    pub fn finish(&self) {
        let (tx, rx) = mpsc::sync_channel(1);
        if self.tx.try_send(Job::Finish(tx)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_save_preserves_unknown_schema_and_scope() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "draimhorizons-economy-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("prices.json");
        let valid = r#"{"schema":1,"scope":"s","quotes":[]}"#;
        let worker = Worker::spawn().unwrap();
        assert!(worker.save(path.clone(), "s".into(), valid.into()));
        worker.finish();
        assert_eq!(load(&path, "s").unwrap().as_deref(), Some(valid));
        assert!(load(&path, "other").is_err());
        let future = r#"{"schema":2,"scope":"s"}"#;
        fs::write(&path, future).unwrap();
        assert!(worker.save(path.clone(), "s".into(), valid.into()));
        worker.finish();
        assert!(worker.errors.try_recv().is_ok());
        assert_eq!(fs::read_to_string(&path).unwrap(), future);
        fs::remove_file(&path).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
