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
#[path = "translation_languages.rs"]
mod languages;
pub use languages::{SourcePolicy, TargetLanguage};
const RETRY: Duration = Duration::from_secs(15);
const KEY_RETRY: Duration = Duration::from_secs(15 * 60);
pub const DEEPL_KEY_LIMIT: usize = 256;

pub fn valid_deepl_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= DEEPL_KEY_LIMIT && key.bytes().all(|b| b.is_ascii_graphic())
}

pub fn source_language(code: &str) -> Option<String> {
    let code = code.split(['-', '_']).next()?.to_ascii_lowercase();
    if !(2..=3).contains(&code.len()) || !code.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let codes = [
        ("eng", "en"),
        ("rus", "ru"),
        ("spa", "es"),
        ("por", "pt"),
        ("ita", "it"),
        ("fra", "fr"),
        ("deu", "de"),
        ("ukr", "uk"),
        ("ara", "ar"),
        ("jpn", "ja"),
        ("cmn", "zh"),
        ("heb", "he"),
        ("pol", "pl"),
        ("kor", "ko"),
        ("nob", "nb"),
        ("dan", "da"),
        ("swe", "sv"),
        ("fin", "fi"),
        ("tur", "tr"),
        ("nld", "nl"),
        ("hun", "hu"),
        ("ces", "cs"),
        ("ell", "el"),
        ("bul", "bg"),
        ("bel", "be"),
        ("ron", "ro"),
        ("slv", "sl"),
        ("hrv", "hr"),
        ("srp", "sr"),
        ("mkd", "mk"),
        ("lit", "lt"),
        ("lav", "lv"),
        ("est", "et"),
        ("vie", "vi"),
        ("hin", "hi"),
        ("ben", "bn"),
        ("kat", "ka"),
        ("yid", "yi"),
        ("amh", "am"),
        ("jav", "jv"),
        ("mar", "mr"),
        ("kan", "kn"),
        ("tam", "ta"),
        ("urd", "ur"),
        ("tha", "th"),
        ("guj", "gu"),
        ("uzb", "uz"),
        ("pan", "pa"),
        ("aze", "az"),
        ("ind", "id"),
        ("tel", "te"),
        ("pes", "fa"),
        ("mal", "ml"),
        ("ori", "or"),
        ("mya", "my"),
        ("nep", "ne"),
        ("sin", "si"),
        ("khm", "km"),
        ("tuk", "tk"),
        ("aka", "ak"),
        ("zul", "zu"),
        ("sna", "sn"),
        ("afr", "af"),
        ("lat", "la"),
        ("slk", "sk"),
        ("cat", "ca"),
        ("tgl", "tl"),
        ("hye", "hy"),
        ("cym", "cy"),
        ("epo", "eo"),
        ("iw", "he"),
    ];
    Some(
        codes
            .iter()
            .find(|(from, _)| *from == code)
            .map_or(code.as_str(), |(_, to)| *to)
            .to_ascii_uppercase(),
    )
}

pub fn detect_source_language(text: &str) -> Option<String> {
    let letters: String = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return None;
    }
    let script = [
        ("JA", '\u{3040}', '\u{30ff}'),
        ("KO", '\u{ac00}', '\u{d7af}'),
        ("ZH", '\u{4e00}', '\u{9fff}'),
        ("AR", '\u{0600}', '\u{06ff}'),
        ("HE", '\u{0590}', '\u{05ff}'),
        ("EL", '\u{0370}', '\u{03ff}'),
        ("HI", '\u{0900}', '\u{097f}'),
        ("TH", '\u{0e00}', '\u{0e7f}'),
    ];
    for (code, from, to) in script {
        if letters.chars().any(|c| (from..=to).contains(&c)) {
            return Some(code.into());
        }
    }
    if letters
        .chars()
        .any(|c| ('\u{0400}'..='\u{052f}').contains(&c))
    {
        return Some(
            if letters.contains(['і', 'ї', 'є', 'ґ']) {
                "UK"
            } else if letters.contains('ў') {
                "BE"
            } else {
                "RU"
            }
            .into(),
        );
    }
    let lower = text.to_lowercase();
    let words: Vec<_> = lower
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    let vocabularies = [
        (
            "EN",
            &[
                "hello", "hi", "hey", "thanks", "thank", "please", "the", "where", "what", "you",
                "your", "with", "have", "this", "there",
            ][..],
        ),
        (
            "ES",
            &[
                "hola", "gracias", "buenos", "buenas", "donde", "dónde", "quiero", "tengo",
                "estoy", "está", "estás", "ustedes", "vosotros",
            ],
        ),
        (
            "FR",
            &[
                "bonjour", "salut", "merci", "avec", "vous", "pour", "suis", "cette", "dans",
                "votre",
            ],
        ),
        (
            "DE",
            &[
                "hallo", "danke", "bitte", "nicht", "ich", "und", "haben", "mein", "deine", "ist",
            ],
        ),
        (
            "PT",
            &[
                "olá", "obrigado", "obrigada", "você", "vocês", "estou", "tenho", "quero",
            ],
        ),
        (
            "IT",
            &[
                "ciao",
                "grazie",
                "buongiorno",
                "sono",
                "dove",
                "questo",
                "voglio",
            ],
        ),
    ];
    let mut best = None;
    let mut count = 0;
    for (code, vocabulary) in vocabularies {
        let score = words
            .iter()
            .filter(|word| vocabulary.contains(word))
            .count();
        if score > count {
            best = Some(code);
            count = score;
        } else if score == count {
            best = None;
        }
    }
    best.map(str::to_owned)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translation {
    pub text: String,
    pub source_language: Option<String>,
}

impl Translation {
    /// True when this is not a real translation: the provider reports the text was already in
    /// `target`, or only case, spacing or punctuation changed. Such results are discarded.
    pub fn is_noop(&self, original: &str, target: TargetLanguage) -> bool {
        let letters = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect()
        };
        self.source_language.as_deref() == Some(target.source_code().as_str())
            || letters(&self.text) == letters(original)
    }
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

    fn request(
        &self,
        text: &str,
        target: TargetLanguage,
        policy: SourcePolicy,
        timeout: Duration,
    ) -> Option<Translation> {
        if text.chars().count() > 500 {
            return None;
        }
        let deadline = Instant::now() + timeout;
        let mut ws =
            WebSocket::connect_with_deadline(&self.url, Some(&self.token), Some(deadline)).ok()?;
        let message = match policy.source {
            Some(source) => serde_json::json!([target.deepl_code(), text, source.source_code()]),
            None => serde_json::json!([target.deepl_code(), text]),
        };
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
                    .map(|text| Translation {
                        text: text.to_owned(),
                        source_language: value[2].as_str().and_then(source_language),
                    });
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

    #[cfg(test)]
    fn translate(&mut self, job: &Job, target: impl Into<TargetLanguage>) -> Option<Translation> {
        self.translate_with_policy(job, target.into(), SourcePolicy::default())
    }
    fn translate_with_policy(
        &mut self,
        job: &Job,
        target: TargetLanguage,
        policy: SourcePolicy,
    ) -> Option<Translation> {
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
            if let Ok(text) = google_request_with_source(
                &self.google,
                url,
                &job.text,
                policy.google_source(),
                target.code(),
                timeout,
            ) {
                return Some(text);
            }
            *retry = Instant::now() + RETRY;
        }
        if Instant::now() < self.deepl_retry {
            return None;
        }
        let timeout = job.timeout(Duration::from_secs(5))?;
        let result = if let Some(key) = &self.deepl_key {
            deepl_request_with_source(
                &self.deepl,
                self.deepl_url,
                key,
                &job.text,
                target,
                policy.source,
                timeout,
            )
        } else {
            self.shared_deepl
                .as_ref()?
                .request(&job.text, target, policy, timeout)
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
    #[cfg(test)]
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
    results: Receiver<(u64, Option<Translation>)>,
    stop: Arc<AtomicBool>,
    target: TargetLanguage,
    source: Option<TargetLanguage>,
}
impl Default for Translator {
    fn default() -> Self {
        Self::new(Language::Russian)
    }
}
impl Translator {
    pub fn new(target: impl Into<TargetLanguage>) -> Self {
        Self::with_deepl(target, None, None)
    }
    pub fn with_deepl(
        target: impl Into<TargetLanguage>,
        key: Option<String>,
        shared: Option<SharedDeepL>,
    ) -> Self {
        Self::with_policy(target.into(), SourcePolicy::default(), key, shared)
    }
    pub fn with_policy(
        target: TargetLanguage,
        policy: SourcePolicy,
        key: Option<String>,
        shared: Option<SharedDeepL>,
    ) -> Self {
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
                        // Skipped text (outside the common languages) returns unchanged without any request.
                        let value = if policy.allows(&job.text) {
                            providers.translate_with_policy(&job, target, policy)?
                        } else {
                            Translation {
                                text: job.text.clone(),
                                source_language: None,
                            }
                        };
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
            source: policy.source,
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
        if text.len() > 4096 || !self.target.should_translate(self.source, text) {
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
        self.poll_translation()
            .map(|(id, value)| (id, value.map(|v| v.text)))
    }
    pub fn poll_translation(&self) -> Option<(u64, Option<Translation>)> {
        self.results.try_recv().ok()
    }
}
impl Drop for Translator {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
/// Any non-command text with a letter is sent; providers detect the source language.
pub fn needs_translation(text: &str) -> bool {
    !text.trim_start().starts_with('/') && text.chars().any(char::is_alphabetic)
}
pub fn needs_english(text: &str) -> bool {
    needs_translation(text)
}
#[cfg(test)]
fn google_request(
    agent: &ureq::Agent,
    endpoint: &str,
    text: &str,
    target: &str,
    timeout: Duration,
) -> Result<Translation, String> {
    google_request_with_source(agent, endpoint, text, "auto", target, timeout)
}
fn google_request_with_source(
    agent: &ureq::Agent,
    endpoint: &str,
    text: &str,
    source: &str,
    target: &str,
    timeout: Duration,
) -> Result<Translation, String> {
    let response = agent
        .get(endpoint)
        .timeout(timeout)
        .query("client", "gtx")
        .query("sl", source)
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
    Ok(Translation {
        text: output,
        source_language: value[2].as_str().and_then(source_language),
    })
}

#[cfg(test)]
fn deepl_request(
    agent: &ureq::Agent,
    endpoint: &str,
    key: &str,
    text: &str,
    target: impl Into<TargetLanguage>,
    timeout: Duration,
) -> Result<Translation, u16> {
    deepl_request_with_source(agent, endpoint, key, text, target.into(), None, timeout)
}
fn deepl_request_with_source(
    agent: &ureq::Agent,
    endpoint: &str,
    key: &str,
    text: &str,
    target: TargetLanguage,
    source: Option<TargetLanguage>,
    timeout: Duration,
) -> Result<Translation, u16> {
    let mut body = serde_json::json!({
        "text": [text],
        "target_lang": target.deepl_code(),
    });
    if let Some(source) = source {
        body["source_lang"] = source.source_code().into();
    }
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
    let value = read_response(response).ok_or(0u16)?;
    value["translations"][0]["text"]
        .as_str()
        .filter(|value| !value.trim().is_empty() && value.len() <= 8192)
        .map(|text| Translation {
            text: text.to_owned(),
            source_language: value["translations"][0]["detected_source_language"]
                .as_str()
                .and_then(source_language),
        })
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
    fn source_language_codes_and_local_estimates_preserve_uncertainty() {
        assert_eq!(source_language("en-US").as_deref(), Some("EN"));
        assert_eq!(source_language("es").as_deref(), Some("ES"));
        assert!(source_language("<script>").is_none());
        assert_eq!(
            detect_source_language("Привет, где портал?").as_deref(),
            Some("RU")
        );
        assert_eq!(
            detect_source_language("hello, where is the base?").as_deref(),
            Some("EN")
        );
        assert_eq!(
            detect_source_language("hola, dónde está la base?").as_deref(),
            Some("ES")
        );
        assert!(detect_source_language("123 :3").is_none());
        assert!(detect_source_language("x").is_none());
    }
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
                .text
                .chars()
                .any(|c| ('\u{0400}'..='\u{052f}').contains(&c))
        );
    }
    const SAMPLES: [&str; 5] = [
        "Hei kaikki",
        "Cześć wszystkim",
        "Hola amigos",
        "こんにちは",
        "Привіт усім",
    ];
    fn serve_once(body: &'static str) -> (String, thread::JoinHandle<(String, String)>) {
        use std::io::Write;
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
            let head = String::from_utf8(input).unwrap();
            let size = head
                .to_ascii_lowercase()
                .lines()
                .find_map(|l| l.strip_prefix("content-length: ")?.parse::<usize>().ok())
                .unwrap_or(0);
            let mut payload = vec![0; size];
            socket.read_exact(&mut payload).unwrap();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            (head, String::from_utf8(payload).unwrap())
        });
        (format!("http://{address}/t"), worker)
    }
    #[test]
    fn any_alphabetic_noncommand_text_is_eligible_for_both_targets() {
        for text in SAMPLES {
            assert!(Language::Russian.needs_translation(text), "{text}");
            assert!(Language::English.needs_translation(text), "{text}");
        }
        assert!(Language::Russian.needs_translation("Привет"));
        assert!(Language::English.needs_translation("Hello"));
        for target in [Language::Russian, Language::English] {
            assert!(!target.needs_translation("/msg Alex hello"));
            assert!(!target.needs_translation("  /msg Alex Привет"));
            assert!(!target.needs_translation("123 :) !!!"));
        }
    }
    #[test]
    fn explicit_input_and_all_common_outputs_reach_both_providers() {
        for target in TargetLanguage::COMMON {
            let (url, worker) = serve_once(r#"[[["ok","x"]]]"#);
            let agent = ureq::AgentBuilder::new().build();
            assert_eq!(
                google_request_with_source(
                    &agent,
                    &url,
                    "hei",
                    "fi",
                    target.code(),
                    Duration::from_secs(2)
                )
                .unwrap()
                .text,
                "ok"
            );
            let (head, _) = worker.join().unwrap();
            assert!(head.lines().next().unwrap().contains("sl=fi&"));
            let (url, worker) = serve_once(r#"{"translations":[{"text":"ok"}]}"#);
            assert_eq!(
                deepl_request_with_source(
                    &agent,
                    &url,
                    "test",
                    "hei",
                    target,
                    Some(TargetLanguage::Finnish),
                    Duration::from_secs(2)
                )
                .unwrap()
                .text,
                "ok"
            );
            let (_, body) = worker.join().unwrap();
            let value: Value = serde_json::from_str(&body).unwrap();
            assert_eq!(value["source_lang"], "FI");
            assert_eq!(value["target_lang"], target.deepl_code());
        }
    }
    #[test]
    fn common_only_skips_unlisted_sources_without_waiting_for_network() {
        let text = "Γεια σας φίλοι, αυτή είναι μια ολόκληρη πρόταση στην ελληνική γλώσσα.";
        let policy = SourcePolicy {
            source: None,
            common_only: true,
        };
        assert!(!policy.allows(text));
        let worker = Translator::with_policy(TargetLanguage::English, policy, None, None);
        assert!(worker.submit(42, text));
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some((id, result)) = worker.poll() {
                assert_eq!(id, 42);
                assert_eq!(result.as_deref(), Some(text));
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Skipped text must return without any provider timeout"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn google_uses_auto_source_for_all_samples_and_targets() {
        for target in ["ru", "en"] {
            for text in SAMPLES {
                let (url, worker) = serve_once(r#"[[["ok","x"]]]"#);
                let agent = ureq::AgentBuilder::new().build();
                assert_eq!(
                    google_request(&agent, &url, text, target, Duration::from_secs(2))
                        .unwrap()
                        .text,
                    "ok"
                );
                let (head, _) = worker.join().unwrap();
                let line = head.lines().next().unwrap();
                assert!(line.contains("sl=auto"), "{line}");
                assert!(line.contains(&format!("tl={target}&")), "{line}");
            }
        }
    }
    #[test]
    fn deepl_omits_source_language_for_all_samples_and_targets() {
        for target in [Language::Russian, Language::English] {
            for text in SAMPLES {
                let (url, worker) = serve_once(r#"{"translations":[{"text":"ok"}]}"#);
                let agent = ureq::AgentBuilder::new().build();
                assert_eq!(
                    deepl_request(&agent, &url, "k", text, target, Duration::from_secs(2))
                        .unwrap()
                        .text,
                    "ok"
                );
                let (_, body) = worker.join().unwrap();
                let value: Value = serde_json::from_str(&body).unwrap();
                assert!(value.get("source_lang").is_none(), "{body}");
                assert_eq!(value["target_lang"], target.deepl_code());
                assert_eq!(value["text"][0], text);
            }
        }
    }
    #[test]
    fn english_translation_covers_mixed_russian_but_never_commands() {
        assert!(needs_english("Привет everyone"));
        assert!(needs_english("Hello"));
        assert!(!needs_english("/msg Alex Привет"));
    }
    #[test]
    fn russian_and_nonlinguistic_text_is_not_sent() {
        assert!(!needs_translation("123 :) !!!"));
        assert!(!needs_english("123 :) !!!"));
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
                .request(
                    "hello",
                    TargetLanguage::Russian,
                    SourcePolicy::default(),
                    Duration::from_secs(2),
                )
                .as_ref()
                .map(|value| value.text.as_str()),
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
            providers
                .translate(&job, Language::English)
                .as_ref()
                .map(|value| value.text.as_str()),
            Some("Google")
        );
        assert_eq!(
            providers
                .translate(&job, Language::English)
                .as_ref()
                .map(|value| value.text.as_str()),
            Some("Mirror")
        );
        assert_eq!(
            providers
                .translate(&job, Language::English)
                .as_ref()
                .map(|value| value.text.as_str()),
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
            let result = google_request(
                &agent,
                &format!("http://{address}/translate"),
                "hello&tl=en",
                target,
                Duration::from_secs(2),
            )
            .unwrap();
            assert_eq!(
                result.source_language.as_deref(),
                Some(if target == "ru" { "EN" } else { "RU" })
            );
            assert_eq!(
                result.text,
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
