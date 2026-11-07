//! Bounded translation workers; callers decide whether to display or send the result.
use crate::websocket::{Url, WebSocket};
use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
const ENDPOINT: &str = "https://translate.googleapis.com/translate_a/single";
const GOOGLE_MIRROR: &str = "https://translate.google.com/translate_a/single";
const DEEPL_FREE: &str = "https://api-free.deepl.com/v2/translate";
const RETRY: Duration = Duration::from_secs(15);
const KEY_RETRY: Duration = Duration::from_secs(15 * 60);
pub const DEEPL_KEY_LIMIT: usize = 256;

pub fn valid_deepl_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= DEEPL_KEY_LIMIT && key.bytes().all(|b| b.is_ascii_graphic())
}

pub struct SharedDeepL {
    url: Url,
    token: String,
}
impl SharedDeepL {
    pub fn new(url: &str, token: String) -> Option<Self> {
        if token.is_empty() || token.len() > 512 || !token.bytes().all(|b| b.is_ascii_graphic()) {
            return None;
        }
        let mut url = Url::parse(url).ok()?;
        url.path = "/v1/translate".into();
        Some(Self { url, token })
    }

    fn request(&self, text: &str, target: Language, timeout: Duration) -> Option<String> {
        if text.chars().count() > 500 {
            return None;
        }
        let deadline = Instant::now() + timeout;
        let mut ws =
            WebSocket::connect_with_deadline(&self.url, Some(&self.token), Some(deadline)).ok()?;
        let message = serde_json::json!([target.deepl_code(), text]);
        if Instant::now() >= deadline {
            return None;
        }
        ws.send(&message.to_string()).ok()?;
        while Instant::now() < deadline {
            if let Some(reply) = ws.poll().ok()? {
                let value: Value = serde_json::from_str(&reply).ok()?;
                return (value[0] == "ok")
                    .then(|| value[1].as_str())
                    .flatten()
                    .filter(|s| !s.trim().is_empty() && s.len() <= 8192)
                    .map(str::to_owned);
            }
        }
        None
    }
}

struct Job {
    id: u64,
    text: String,
    deadline: Option<Instant>,
}
impl Job {
    fn timeout(&self, limit: Duration) -> Option<Duration> {
        match self.deadline {
            Some(end) => end
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .map(|remaining| remaining.min(limit)),
            None => Some(limit),
        }
    }
}

struct Providers<'a> {
    google: ureq::Agent,
    deepl: ureq::Agent,
    google_urls: [&'a str; 2],
    deepl_url: &'a str,
    deepl_key: Option<String>,
    shared_deepl: Option<SharedDeepL>,
    google_retry: [Instant; 2],
    deepl_retry: Instant,
}
impl Providers<'_> {
    fn new(key: Option<String>, shared: Option<SharedDeepL>) -> Self {
        let agent = |redirects| {
            ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(6))
                .user_agent("DraimHorizons translation")
                .redirects(redirects)
                .build()
        };
        Self {
            google: agent(3),
            deepl: agent(0),
            google_urls: [ENDPOINT, GOOGLE_MIRROR],
            deepl_url: DEEPL_FREE,
            deepl_key: key.filter(|key| valid_deepl_key(key)),
            shared_deepl: shared,
            google_retry: [Instant::now(); 2],
            deepl_retry: Instant::now(),
        }
    }

    fn translate(&mut self, job: &Job, target: Language) -> Option<String> {
        let limit =
            Duration::from_secs(if self.deepl_key.is_some() || self.shared_deepl.is_some() {
                4
            } else {
                6
            });
        for (url, retry) in self.google_urls.into_iter().zip(&mut self.google_retry) {
            if Instant::now() < *retry {
                continue;
            }
            let timeout = job.timeout(limit)?;
            if let Ok(text) = google_request(&self.google, url, &job.text, target.code(), timeout) {
                return Some(text);
            }
            *retry = Instant::now() + RETRY;
        }
        if Instant::now() < self.deepl_retry {
            return None;
        }
        let timeout = job.timeout(Duration::from_secs(5))?;
        let result = if let Some(key) = &self.deepl_key {
            deepl_request(&self.deepl, self.deepl_url, key, &job.text, target, timeout)
        } else {
            self.shared_deepl
                .as_ref()?
                .request(&job.text, target, timeout)
                .ok_or(0)
        };
        match result {
            Ok(text) => Some(text),
            Err(status) => {
                let delay = if matches!(status, 401 | 403 | 456) {
                    KEY_RETRY
                } else {
                    RETRY
                };
                self.deepl_retry = Instant::now() + delay;
                None
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    Russian,
    English,
}
impl Language {
    fn deepl_code(self) -> &'static str {
        self.text("RU", "EN-US")
    }
    pub fn from_code(code: &str) -> Self {
        if code == "en" {
            Self::English
        } else {
            Self::Russian
        }
    }
    pub fn code(self) -> &'static str {
        self.text("ru", "en")
    }
    pub fn other(self) -> Self {
        match self {
            Self::Russian => Self::English,
            Self::English => Self::Russian,
        }
    }
    pub fn text<'a>(self, russian: &'a str, english: &'a str) -> &'a str {
        match self {
            Self::Russian => russian,
            Self::English => english,
        }
    }
    pub fn needs_translation(self, text: &str) -> bool {
        match self {
            Self::Russian => needs_translation(text),
            Self::English => needs_english(text),
        }
    }
}
pub struct Translator {
    requests: SyncSender<Job>,
    results: Receiver<(u64, Option<String>)>,
    stop: Arc<AtomicBool>,
    target: Language,
}
impl Default for Translator {
    fn default() -> Self {
        Self::new(Language::Russian)
    }
}
impl Translator {
    pub fn new(target: Language) -> Self {
        Self::with_deepl(target, None, None)
    }
    pub fn with_deepl(target: Language, key: Option<String>, shared: Option<SharedDeepL>) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Job>(32);
        let (done, results) = mpsc::sync_channel(64);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let _ = thread::Builder::new()
            .name("horizon-translate".into())
            .spawn(move || {
                let mut providers = Providers::new(key, shared);
                let mut cache = HashMap::new();
                let mut order = VecDeque::new();
                while !stopped.load(Ordering::Acquire) {
                    let job = match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(v) => v,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                    let id = job.id;
                    let result = cache.get(&job.text).cloned().or_else(|| {
                        let value = providers.translate(&job, target)?;
                        if cache.len() >= 256
                            && let Some(old) = order.pop_front()
                        {
                            cache.remove(&old);
                        }
                        order.push_back(job.text.clone());
                        cache.insert(job.text, value.clone());
                        Some(value)
                    });
                    let _ = done.try_send((id, result));
                }
            });
        Self {
            requests: tx,
            results,
            stop,
            target,
        }
    }
}
impl Translator {
    pub fn submit(&self, id: u64, text: &str) -> bool {
        self.enqueue(id, text, None)
    }
    pub fn submit_until(&self, id: u64, text: &str, deadline: Instant) -> bool {
        self.enqueue(id, text, Some(deadline))
    }
    fn enqueue(&self, id: u64, text: &str, deadline: Option<Instant>) -> bool {
        if text.len() > 4096 || !self.target.needs_translation(text) {
            return false;
        }
        self.requests
            .try_send(Job {
                id,
                text: text.into(),
                deadline,
            })
            .is_ok()
    }
    pub fn poll(&self) -> Option<(u64, Option<String>)> {
        self.results.try_recv().ok()
    }
}
impl Drop for Translator {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
pub fn needs_translation(text: &str) -> bool {
    if text.trim_start().starts_with('/') {
        return false;
    }
    let foreign = |c: char| c.is_alphabetic() && !('\u{0400}'..='\u{052f}').contains(&c);
    if !text.chars().any(|c| ('\u{0400}'..='\u{052f}').contains(&c)) {
        return text.chars().any(foreign);
    }
    text.split_whitespace()
        .any(|word| word.chars().filter(|c| foreign(*c)).count() >= 2)
}
pub fn needs_english(text: &str) -> bool {
    !text.trim_start().starts_with('/')
        && text.chars().any(|c| ('\u{0400}'..='\u{052f}').contains(&c))
}
fn google_request(
    agent: &ureq::Agent,
    endpoint: &str,
    text: &str,
    target: &str,
    timeout: Duration,
) -> Result<String, String> {
    let response = agent
        .get(endpoint)
        .timeout(timeout)
        .query("client", "gtx")
        .query("sl", "auto")
        .query("tl", target)
        .query("dt", "t")
        .query("q", text)
        .call()
        .map_err(|_| "translation unavailable")?;
    let value = read_response(response).ok_or("invalid translation response")?;
    let segments = value[0].as_array().ok_or("missing translation")?;
    let mut output = String::new();
    for segment in segments {
        output.push_str(segment[0].as_str().ok_or("invalid translation segment")?);
        if output.len() > 8192 {
            return Err("translation too large".into());
        }
    }
    if output.is_empty() {
        return Err("empty translation".into());
    }
    Ok(output)
}

fn deepl_request(
    agent: &ureq::Agent,
    endpoint: &str,
    key: &str,
    text: &str,
    target: Language,
    timeout: Duration,
) -> Result<String, u16> {
    let body = serde_json::json!({
        "text": [text],
        "target_lang": target.deepl_code(),
    });
    let response = agent
        .post(endpoint)
        .timeout(timeout)
        .set("Authorization", &format!("DeepL-Auth-Key {key}"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|error| match error {
            ureq::Error::Status(status, _) => status,
            ureq::Error::Transport(_) => 0,
        })?;
    read_response(response)
        .as_ref()
        .and_then(|value| value["translations"][0]["text"].as_str())
        .filter(|value| !value.trim().is_empty() && value.len() <= 8192)
        .map(str::to_owned)
        .ok_or(0)
}

fn read_response(response: ureq::Response) -> Option<Value> {
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(65537)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 65536 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Public provider smoke test; sends only a fixed greeting"]
    fn public_provider_translates_fixed_greeting() {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(8))
            .redirects(3)
            .build();
        let value = google_request(
            &agent,
            ENDPOINT,
            "Hello world",
            "ru",
            Duration::from_secs(8),
        )
        .unwrap();
        assert!(
            value
                .chars()
                .any(|c| ('\u{0400}'..='\u{052f}').contains(&c))
        );
    }
    #[test]
    fn english_translation_covers_mixed_russian_but_never_commands() {
        assert!(needs_english("Привет everyone"));
        assert!(!needs_english("Hello"));
        assert!(!needs_english("/msg Alex Привет"));
    }
    #[test]
    fn russian_and_nonlinguistic_text_is_not_sent() {
        assert!(!needs_translation("Привет"));
        assert!(!needs_translation("123 :D Привет"));
        assert!(needs_translation("hello"));
        assert!(needs_translation("Привет, how are you?"));
        assert!(!needs_translation("/msg Alex hello"));
        assert!(needs_translation("こんにちは"));
    }
    #[test]
    fn shared_deepl_sends_only_the_relay_token_and_translation_request() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use sha1::{Digest, Sha1};
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(socket);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, "GET /v1/translate HTTP/1.1\r\n");
            let mut nonce = None;
            let mut authorized = false;
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(key) = line.strip_prefix("Sec-WebSocket-Key: ") {
                    nonce = Some(key.trim().to_owned());
                }
                authorized |= line == "Authorization: Bearer private-relay-token\r\n";
            }
            assert!(authorized);
            let accept = STANDARD.encode(Sha1::digest(format!(
                "{}258EAFA5-E914-47DA-95CA-C5AB0DC85B11",
                nonce.unwrap()
            )));
            write!(reader.get_mut(),
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").unwrap();
            let mut header = [0; 2];
            reader.read_exact(&mut header).unwrap();
            assert_eq!(header[0], 0x81);
            assert_ne!(header[1] & 0x80, 0);
            let mut mask = [0; 4];
            reader.read_exact(&mut mask).unwrap();
            let mut payload = vec![0; usize::from(header[1] & 0x7f)];
            reader.read_exact(&mut payload).unwrap();
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[i % 4];
            }
            assert_eq!(
                serde_json::from_slice::<Value>(&payload).unwrap(),
                serde_json::json!(["RU", "hello"])
            );
            let response = r#"["ok","Привет"]"#.as_bytes();
            reader
                .get_mut()
                .write_all(&[0x81, response.len() as u8])
                .unwrap();
            reader.get_mut().write_all(response).unwrap();
        });
        let shared = SharedDeepL::new(
            &format!("ws://{address}/v1/relay"),
            "private-relay-token".into(),
        )
        .unwrap();
        assert_eq!(
            shared
                .request("hello", Language::Russian, Duration::from_secs(2))
                .as_deref(),
            Some("Привет")
        );
        server.join().unwrap();
    }
    #[test]
    fn google_precedes_deepl_and_deepl_quota_errors_pause_requests() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for (path, status, body) in [
                ("/google", 200, r#"[[["Google"]]]"#),
                ("/google", 503, "{}"),
                ("/mirror", 200, r#"[[["Mirror"]]]"#),
                ("/mirror", 429, "{}"),
                ("/deepl", 200, r#"{"translations":[{"text":"DeepL"}]}"#),
                ("/deepl", 456, "{}"),
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut reader = BufReader::new(&mut socket);
                let mut start = String::new();
                reader.read_line(&mut start).unwrap();
                let mut headers = String::new();
                loop {
                    let mut line = String::new();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                    headers.push_str(&line.to_ascii_lowercase());
                }
                if path == "/deepl" {
                    assert!(start.starts_with("POST /deepl "));
                    assert!(headers.contains("authorization: deepl-auth-key dummy:fx"));
                    let size = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")?.parse::<usize>().ok()
                        })
                        .unwrap();
                    let mut request = vec![0; size];
                    reader.read_exact(&mut request).unwrap();
                    let value: Value = serde_json::from_slice(&request).unwrap();
                    assert_eq!(value["target_lang"], "EN-US");
                    assert_eq!(value["text"][0], "Привет");
                } else {
                    assert!(start.starts_with(&format!("GET {path}?")));
                    assert!(!headers.contains("authorization:"));
                }
                write!(socket, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let google = format!("http://{address}/google");
        let mirror = format!("http://{address}/mirror");
        let deepl = format!("http://{address}/deepl");
        let mut providers = Providers::new(Some("dummy:fx".into()), None);
        providers.google_urls = [&google, &mirror];
        providers.deepl_url = &deepl;
        let job = Job {
            id: 1,
            text: "Привет".into(),
            deadline: None,
        };
        assert_eq!(
            providers.translate(&job, Language::English).as_deref(),
            Some("Google")
        );
        assert_eq!(
            providers.translate(&job, Language::English).as_deref(),
            Some("Mirror")
        );
        assert_eq!(
            providers.translate(&job, Language::English).as_deref(),
            Some("DeepL")
        );
        assert_eq!(providers.translate(&job, Language::English), None);
        assert!(providers.deepl_retry >= Instant::now() + RETRY);
        assert_eq!(providers.translate(&job, Language::English), None);
        server.join().unwrap();
    }

    #[test]
    fn provider_response_and_query_are_handled_on_loopback() {
        use std::io::Write;
        for target in ["ru", "en"] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let worker = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut input = Vec::new();
                let mut byte = [0];
                while !input.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    input.push(byte[0]);
                }
                let request = String::from_utf8(input).unwrap();
                assert!(request.contains(&format!("tl={target}")));
                assert!(!request.contains("q=hello&tl=en"));
                let body = if target == "ru" {
                    r#"[[["Привет","hello",null,null]],null,"en"]"#
                } else {
                    r#"[[["Hello","Привет",null,null]],null,"ru"]"#
                };
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            });
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(2))
                .build();
            assert_eq!(
                google_request(
                    &agent,
                    &format!("http://{address}/translate"),
                    "hello&tl=en",
                    target,
                    Duration::from_secs(2),
                )
                .unwrap(),
                if target == "ru" {
                    "Привет"
                } else {
                    "Hello"
                }
            );
            worker.join().unwrap();
        }
    }
}
