//! Bounded translation workers; callers decide whether to display or send the result.
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
pub struct Translator {
    requests: SyncSender<(u64, String)>,
    results: Receiver<(u64, Option<String>)>,
    stop: Arc<AtomicBool>,
    english: bool,
}
impl Default for Translator {
    fn default() -> Self {
        Self::new(false)
    }
}
impl Translator {
    pub fn english() -> Self {
        Self::new(true)
    }
    fn new(english: bool) -> Self {
        let (tx, rx) = mpsc::sync_channel::<(u64, String)>(32);
        let (done, results) = mpsc::sync_channel(64);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let _ = thread::Builder::new()
            .name("horizon-translate".into())
            .spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout(Duration::from_secs(4))
                    .redirects(0)
                    .build();
                let mut cache = HashMap::new();
                let mut order = VecDeque::new();
                let mut retry = Instant::now();
                while !stopped.load(Ordering::Acquire) {
                    let (id, text) = match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(v) => v,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                    let result = if let Some(cached) = cache.get(&text) {
                        Some(String::clone(cached))
                    } else if Instant::now() < retry {
                        None
                    } else {
                        match request(&agent, ENDPOINT, &text, if english { "en" } else { "ru" }) {
                            Ok(value) => {
                                if cache.len() >= 256
                                    && let Some(old) = order.pop_front()
                                {
                                    cache.remove(&old);
                                }
                                order.push_back(text.clone());
                                cache.insert(text, value.clone());
                                Some(value)
                            }
                            Err(_) => {
                                retry = Instant::now() + Duration::from_secs(15);
                                None
                            }
                        }
                    };
                    let _ = done.try_send((id, result));
                }
            });
        Self {
            requests: tx,
            results,
            stop,
            english,
        }
    }
}
impl Translator {
    pub fn submit(&self, id: u64, text: &str) -> bool {
        if text.len() > 4096
            || !(if self.english {
                needs_english(text)
            } else {
                needs_translation(text)
            })
        {
            return false;
        }
        self.requests.try_send((id, text.into())).is_ok()
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
    text.chars()
        .any(|c| c.is_alphabetic() && !('\u{0400}'..='\u{052f}').contains(&c))
        && !text.chars().any(|c| ('\u{0400}'..='\u{052f}').contains(&c))
}
pub fn needs_english(text: &str) -> bool {
    !text.trim_start().starts_with('/')
        && text.chars().any(|c| ('\u{0400}'..='\u{052f}').contains(&c))
}
fn request(
    agent: &ureq::Agent,
    endpoint: &str,
    text: &str,
    target: &str,
) -> Result<String, String> {
    let response = agent
        .get(endpoint)
        .query("client", "gtx")
        .query("sl", "auto")
        .query("tl", target)
        .query("dt", "t")
        .query("q", text)
        .call()
        .map_err(|_| "translation unavailable")?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "translation response read failed")?;
    if bytes.len() > 65536 {
        return Err("translation response too large".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "invalid translation response")?;
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
#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(needs_translation("こんにちは"));
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
                request(
                    &agent,
                    &format!("http://{address}/translate"),
                    "hello&tl=en",
                    target,
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
