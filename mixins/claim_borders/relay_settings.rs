// Explicit relay configuration and selected contact identity pins.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::IpAddr,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

/// UUID and nickname are only metadata. Consent is bound to the selected peer key, not its displayed name.
pub fn permits_frame(
    pinned: Option<&str>,
    pending: Option<&str>,
    awaiting: bool,
    kind: &str,
    code: &str,
) -> bool {
    if let Some(expected) = pinned {
        return expected == code;
    }
    match kind {
        "i" => !awaiting || pending == Some(code),
        "a" | "n" => awaiting && pending == Some(code),
        _ => false,
    }
}

const SETTINGS_LIMIT: u64 = 16 * 1024;
const PINS_LIMIT: u64 = 256 * 1024;
const MAX_SCOPES: usize = 128;
const MAX_PINS: usize = 64;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Settings {
    pub url: String,
    pub access_token: Option<String>,
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.url.is_empty() {
            validate_url(&self.url)?;
        }
        if self
            .access_token
            .as_ref()
            .is_some_and(|token| token.len() > 512 || token.chars().any(char::is_control))
        {
            return Err("Invalid relay access token.".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        read(path, SETTINGS_LIMIT)?
            .map_or_else(|| Ok(Self::default()), |bytes| Self::decode(&bytes))
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let value: Value =
            serde_json::from_slice(bytes).map_err(|_| "Invalid relay settings JSON.")?;
        fields(&value, &["schema", "url", "access_token"])?;
        if value["schema"].as_u64() != Some(1) {
            return Err("Unknown relay settings schema.".into());
        }
        let settings = Self {
            url: value["url"]
                .as_str()
                .ok_or("Invalid relay URL.")?
                .to_owned(),
            access_token: match &value["access_token"] {
                Value::Null => None,
                Value::String(token) => Some(token.clone()),
                _ => return Err("Invalid relay access token.".into()),
            },
        };
        settings.validate()?;
        Ok(settings)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        Self::load(path)?;
        let bytes = serde_json::to_vec_pretty(&json!({
            "schema": 1, "url": self.url, "access_token": self.access_token,
        }))
        .map_err(|_| "Cannot encode relay settings.")?;
        write_atomic(path, &bytes, SETTINGS_LIMIT)
    }
}

/// Empty URL disconnects. Plaintext is allowed only for local development.
pub fn validate_url(url: &str) -> Result<(), String> {
    let invalid = || "Invalid relay URL; use wss:// or ws:// on loopback.".to_owned();
    if url.len() > 2048
        || !url.is_ascii()
        || url.bytes().any(|b| b <= 32 || b == 127)
        || url.contains(['@', '?', '#', '\\'])
    {
        return Err(invalid());
    }
    let (tls, rest) = if let Some(rest) = url.strip_prefix("wss://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("ws://") {
        (false, rest)
    } else {
        return Err(invalid());
    };
    let authority = rest.split('/').next().ok_or_else(invalid)?;
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (host, suffix) = rest.split_once(']').ok_or_else(invalid)?;
        if !host.parse::<std::net::Ipv6Addr>().is_ok() {
            return Err(invalid());
        }
        (
            host,
            if suffix.is_empty() {
                None
            } else {
                Some(suffix.strip_prefix(':').ok_or_else(invalid)?)
            },
        )
    } else {
        let (host, port) = authority
            .split_once(':')
            .map_or((authority, None), |(h, p)| (h, Some(p)));
        if host.is_empty()
            || host.len() > 253
            || !host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
        {
            return Err(invalid());
        }
        (host, port)
    };
    if port.is_some_and(|p| {
        p.is_empty()
            || !p.bytes().all(|b| b.is_ascii_digit())
            || p.parse::<u16>().map_or(true, |p| p == 0)
    }) || (!tls
        && !host.eq_ignore_ascii_case("localhost")
        && !host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()))
    {
        return Err(invalid());
    }
    Ok(())
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct PinStore {
    scopes: BTreeMap<String, BTreeMap<u128, String>>,
}
impl PinStore {
    pub fn get(&self, scope: &str, uuid: u128) -> Option<&str> {
        self.scopes.get(scope)?.get(&uuid).map(String::as_str)
    }
    pub fn pin(&mut self, scope: &str, uuid: u128, code: &str) -> Result<(), String> {
        valid_scope(scope)?;
        if code.len() != 24 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Identity code must contain 24 hex digits.".into());
        }
        if !self.scopes.contains_key(scope) && self.scopes.len() >= MAX_SCOPES {
            return Err("Too many relay contact scopes.".into());
        }
        if self
            .scopes
            .get(scope)
            .is_some_and(|pins| !pins.contains_key(&uuid) && pins.len() >= MAX_PINS)
        {
            return Err("Too many relay contacts in this scope.".into());
        }
        self.scopes
            .entry(scope.to_owned())
            .or_default()
            .insert(uuid, code.to_ascii_lowercase());
        Ok(())
    }
    pub fn remove(&mut self, scope: &str, uuid: u128) -> bool {
        let removed = self
            .scopes
            .get_mut(scope)
            .is_some_and(|pins| pins.remove(&uuid).is_some());
        if self.scopes.get(scope).is_some_and(BTreeMap::is_empty) {
            self.scopes.remove(scope);
        }
        removed
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        read(path, PINS_LIMIT)?.map_or_else(|| Ok(Self::default()), |bytes| Self::decode(&bytes))
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let value: Value =
            serde_json::from_slice(bytes).map_err(|_| "Invalid relay contacts JSON.")?;
        fields(&value, &["schema", "scopes"])?;
        if value["schema"].as_u64() != Some(1) {
            return Err("Unknown relay contacts schema.".into());
        }
        let scopes = value["scopes"].as_array().ok_or("Invalid relay scopes.")?;
        if scopes.len() > MAX_SCOPES {
            return Err("Too many relay scopes.".into());
        }
        let mut store = Self::default();
        for row in scopes {
            let row = row
                .as_array()
                .filter(|r| r.len() == 2)
                .ok_or("Invalid relay scope row.")?;
            let scope = row[0].as_str().ok_or("Invalid relay scope.")?;
            valid_scope(scope)?;
            if store.scopes.contains_key(scope) {
                return Err("Duplicate relay scope.".into());
            }
            let pins = row[1].as_array().ok_or("Invalid relay contacts.")?;
            if pins.len() > MAX_PINS {
                return Err("Too many relay contacts.".into());
            }
            store.scopes.insert(scope.to_owned(), BTreeMap::new());
            for pin in pins {
                let pin = pin
                    .as_array()
                    .filter(|r| r.len() == 2)
                    .ok_or("Invalid relay contact row.")?;
                let uuid = pin[0]
                    .as_str()
                    .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                    .ok_or("Invalid contact UUID.")?;
                let uuid = u128::from_str_radix(uuid, 16).map_err(|_| "Invalid contact UUID.")?;
                if store.get(scope, uuid).is_some() {
                    return Err("Duplicate contact UUID.".into());
                }
                store.pin(scope, uuid, pin[1].as_str().ok_or("Invalid contact code.")?)?;
            }
        }
        Ok(store)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        Self::load(path)?;
        let scopes: Vec<Value> = self
            .scopes
            .iter()
            .map(|(scope, pins)| {
                let pins: Vec<Value> = pins
                    .iter()
                    .map(|(uuid, code)| json!([format!("{uuid:032x}"), code]))
                    .collect();
                json!([scope, pins])
            })
            .collect();
        let bytes = serde_json::to_vec_pretty(&json!({"schema":1,"scopes":scopes}))
            .map_err(|_| "Cannot encode relay contacts.")?;
        write_atomic(path, &bytes, PINS_LIMIT)
    }
}

fn valid_scope(scope: &str) -> Result<(), String> {
    if scope.is_empty() || scope.len() > 384 || scope.chars().any(char::is_control) {
        return Err("Invalid relay server/account scope.".into());
    }
    Ok(())
}
fn fields(value: &Value, names: &[&str]) -> Result<(), String> {
    if !value.as_object().is_some_and(|object| {
        object.len() == names.len() && names.iter().all(|name| object.contains_key(*name))
    }) {
        return Err("Unknown relay storage fields.".into());
    }
    Ok(())
}
fn read(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() || metadata.len() > limit {
        return Err("Relay storage must be a bounded regular file.".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Relay storage file is too large.".into());
    }
    Ok(Some(bytes))
}
fn write_atomic(path: &Path, bytes: &[u8], limit: u64) -> Result<(), String> {
    if bytes.len() as u64 > limit {
        return Err("Relay storage file is too large.".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .ok_or("Relay path needs a filename.")?
            .to_string_lossy(),
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "claim-relay-storage-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> std::path::PathBuf {
            self.0.join("relay.json")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    const CODE: &str = "0123456789abcdef01234567";

    #[test]
    fn disconnected_default_and_token_round_trip() {
        let temp = Temp::new();
        let path = temp.path();
        assert!(Settings::load(&path).unwrap().url.is_empty());
        let settings = Settings {
            url: "wss://relay.example.test:443/channel".into(),
            access_token: Some("secret-local-test-token".into()),
        };
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert!(loaded == settings);
        Settings::default().save(&path).unwrap();
        assert!(Settings::load(&path).unwrap() == Settings::default());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn strict_websocket_urls_and_tokens() {
        for url in [
            "wss://relay.example",
            "wss://relay.example:8443/path",
            "wss://[2001:db8::1]/ws",
            "ws://127.0.0.1:8080/ws",
            "ws://[::1]/",
            "ws://localhost/ws",
        ] {
            assert!(validate_url(url).is_ok(), "{url}");
        }
        for url in [
            "https://relay.example",
            "ws://relay.example",
            "wss://user@host/",
            "wss://host/?token=secret",
            "wss://host/#frag",
            "wss://host/\r\nInjected:yes",
            "wss://host:0",
            "wss://host:65536",
            "wss://host:abc/",
            "wss://host:/",
            "wss://[bad]/",
            "wss://a:b:c/",
            "wss://-bad.example/",
            "wss:///path",
            "wss://host\\evil/",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
        assert!(
            Settings {
                access_token: Some("token\r\nHeader: value".into()),
                ..Settings::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Settings {
                access_token: Some("x".repeat(513)),
                ..Settings::default()
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn contacts_are_separate_per_server_and_account() {
        let temp = Temp::new();
        let mut pins = PinStore::default();
        pins.pin("server-a#account-a", 7, CODE).unwrap();
        assert_eq!(pins.get("server-a#account-a", 7), Some(CODE));
        assert_eq!(pins.get("server-b#account-a", 7), None);
        assert_eq!(pins.get("server-a#account-b", 7), None);
        pins.pin("server-b#account-a", 7, "abcdef0123456789abcdef01")
            .unwrap();
        pins.save(&temp.path()).unwrap();
        assert_eq!(PinStore::load(&temp.path()).unwrap(), pins);
        assert!(pins.remove("server-a#account-a", 7));
        assert_eq!(
            pins.get("server-b#account-a", 7),
            Some("abcdef0123456789abcdef01")
        );
    }
    #[test]
    fn invalid_or_oversized_storage_is_never_replaced() {
        let temp = Temp::new();
        let path = temp.path();
        for bytes in [
            b"not json".to_vec(),
            br#"{"schema":1,"url":"","access_token":null,"unknown":true}"#.to_vec(),
            vec![b' '; SETTINGS_LIMIT as usize + 1],
        ] {
            fs::write(&path, &bytes).unwrap();
            assert!(Settings::load(&path).is_err());
            assert!(Settings::default().save(&path).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        let duplicate = json!({"schema":1,"scopes":[["server#account",[[format!("{:032x}",7), CODE],[format!("{:032x}",7), CODE]]]]});
        let malformed =
            json!({"schema":1,"scopes":[["server#account",[[format!("{:032x}",7), "bad-code"]]]]});
        for value in [
            duplicate,
            malformed,
            json!({"schema":1,"scopes":[],"unknown":true}),
        ] {
            let bytes = serde_json::to_vec(&value).unwrap();
            fs::write(&path, &bytes).unwrap();
            assert!(PinStore::load(&path).is_err());
            assert!(PinStore::default().save(&path).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
    #[test]
    fn bounded_scopes_and_contacts_reject_without_mutation() {
        let mut pins = PinStore::default();
        for uuid in 0..MAX_PINS as u128 {
            pins.pin("server#account", uuid, CODE).unwrap();
        }
        let before = pins.clone();
        assert!(
            pins.pin("server#account", MAX_PINS as u128 + 1, CODE)
                .is_err()
        );
        assert_eq!(pins, before);
        assert!(pins.pin("server#account", 0, "not-hex").is_err());
        assert_eq!(pins, before);
        for i in 1..MAX_SCOPES {
            pins.pin(&format!("server-{i}#account"), 1, CODE).unwrap();
        }
        let before = pins.clone();
        assert!(pins.pin("another#account", 1, CODE).is_err());
        assert_eq!(pins, before);
    }
    #[test]
    #[cfg(unix)]
    fn symlinks_are_rejected_without_touching_target() {
        let temp = Temp::new();
        let path = temp.path();
        let target = temp.0.join("target.json");
        Settings::default().save(&target).unwrap();
        let before = fs::read(&target).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(Settings::load(&path).is_err());
        assert!(Settings::default().save(&path).is_err());
        assert!(PinStore::load(&path).is_err());
        assert!(PinStore::default().save(&path).is_err());
        assert_eq!(fs::read(&target).unwrap(), before);
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    }
}

#[cfg(test)]
mod consent_tests {
    use super::permits_frame;
    #[test]
    fn account_metadata_cannot_replace_manual_key_verification() {
        assert!(!permits_frame(
            Some("alice"),
            Some("mallory"),
            true,
            "s",
            "mallory"
        ));
        assert!(permits_frame(Some("alice"), None, false, "s", "alice"));
        assert!(!permits_frame(None, None, false, "s", "alice"));
        assert!(permits_frame(None, None, false, "i", "alice"));
        assert!(!permits_frame(None, Some("alice"), true, "i", "mallory"));
        assert!(!permits_frame(None, Some("alice"), true, "a", "mallory"));
        assert!(permits_frame(None, Some("alice"), true, "a", "alice"));
        assert!(!permits_frame(None, Some("alice"), false, "a", "alice"));
    }
}
