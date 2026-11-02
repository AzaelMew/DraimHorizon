//! Small bounded RFC 6455 client. TLS is validated against Mozilla roots.
use base64::{Engine, engine::general_purpose::STANDARD};
use ring::rand::{SecureRandom, SystemRandom};
use sha1::{Digest, Sha1};
use std::{
    io::{Read, Write},
    net::{IpAddr, TcpStream, ToSocketAddrs},
    sync::Arc,
    time::{Duration, Instant},
};

pub(crate) const MAX_FRAME: usize = 16 * 1024;
type Result<T> = std::result::Result<T, String>;

pub(crate) struct Url {
    pub host: String,
    pub port: u16,
    pub path: String,
    pub tls: bool,
}
impl Url {
    pub fn parse(url: &str) -> Result<Self> {
        if url.bytes().any(|b| b <= 32 || b == 127) {
            return Err("URL contains whitespace/control characters".into());
        }
        let (tls, rest) = if let Some(s) = url.strip_prefix("wss://") {
            (true, s)
        } else if let Some(s) = url.strip_prefix("ws://") {
            (false, s)
        } else {
            return Err("expected ws:// or wss://".into());
        };
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, "/".to_owned()), |(a, p)| (a, format!("/{p}")));
        if authority.is_empty() || authority.contains(['@', '#', '?']) || path.contains('#') {
            return Err("invalid WebSocket URL".into());
        }
        let (host, port) = if authority.starts_with('[') {
            let end = authority.find(']').ok_or("invalid IPv6 URL")?;
            let host = &authority[1..end];
            let suffix = &authority[end + 1..];
            let port = if suffix.is_empty() {
                if tls { 443 } else { 80 }
            } else {
                suffix
                    .strip_prefix(':')
                    .ok_or("invalid authority")?
                    .parse()
                    .map_err(|_| "invalid port")?
            };
            (host.to_owned(), port)
        } else if let Some((host, port)) = authority.rsplit_once(':') {
            (host.to_owned(), port.parse().map_err(|_| "invalid port")?)
        } else {
            (authority.to_owned(), if tls { 443 } else { 80 })
        };
        if host.is_empty()
            || port == 0
            || (!tls
                && host != "localhost"
                && !host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()))
        {
            return Err("plaintext ws is restricted to loopback".into());
        }
        Ok(Self {
            host,
            port,
            path,
            tls,
        })
    }
}
trait Io: Read + Write + Send {}
impl<T: Read + Write + Send> Io for T {}
pub(crate) struct WebSocket {
    io: Box<dyn Io>,
    input: Vec<u8>,
    fragment: Vec<u8>,
    fragmented: bool,
}
impl WebSocket {
    pub fn connect(url: &Url, token: Option<&str>) -> Result<Self> {
        let addresses: Vec<_> = (url.host.as_str(), url.port)
            .to_socket_addrs()
            .map_err(|e| e.to_string())?
            .collect();
        if !url.tls && addresses.iter().any(|a| !a.ip().is_loopback()) {
            return Err("plaintext address is not loopback".into());
        }
        let mut tcp = None;
        for address in addresses.into_iter().take(8) {
            if let Ok(s) = TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
                tcp = Some(s);
                break;
            }
        }
        let tcp = tcp.ok_or("connection failed")?;
        tcp.set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        tcp.set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        let timeout_handle = tcp.try_clone().map_err(|e| e.to_string())?;
        let mut io: Box<dyn Io> = if url.tls {
            let roots =
                rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .with_root_certificates(roots)
                .with_no_client_auth();
            let name = rustls::pki_types::ServerName::try_from(url.host.clone())
                .map_err(|_| "invalid TLS hostname")?;
            let conn =
                rustls::ClientConnection::new(Arc::new(config), name).map_err(|e| e.to_string())?;
            Box::new(rustls::StreamOwned::new(conn, tcp))
        } else {
            Box::new(tcp)
        };
        let mut random = [0; 16];
        SystemRandom::new()
            .fill(&mut random)
            .map_err(|_| "random failed")?;
        let key = STANDARD.encode(random);
        let host = if url.host.contains(':') {
            format!("[{}]:{}", url.host, url.port)
        } else {
            format!("{}:{}", url.host, url.port)
        };
        let authorization = token
            .map(|t| format!("Authorization: Bearer {t}\r\n"))
            .unwrap_or_default();
        write!(io,"GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n{}\r\n",url.path,host,key,authorization).map_err(|e|e.to_string())?;
        io.flush().map_err(|e| e.to_string())?;
        let mut header = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !header.ends_with(b"\r\n\r\n") {
            if Instant::now() >= deadline {
                return Err("WebSocket upgrade timeout".into());
            }
            timeout_handle
                .set_read_timeout(Some(
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_secs(1))
                        .max(Duration::from_millis(1)),
                ))
                .map_err(|e| e.to_string())?;
            let mut b = [0];
            io.read_exact(&mut b).map_err(|e| e.to_string())?;
            header.push(b[0]);
            if header.len() > 8192 {
                return Err("upgrade headers too large".into());
            }
        }
        let header = std::str::from_utf8(&header).map_err(|_| "invalid HTTP header")?;
        let mut lines = header.split("\r\n");
        if !lines.next().is_some_and(|s| s.starts_with("HTTP/1.1 101 ")) {
            return Err("WebSocket upgrade rejected".into());
        }
        let fields: Vec<_> = lines
            .filter_map(|s| s.split_once(':'))
            .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
            .collect();
        let field = |key: &str| {
            fields
                .iter()
                .filter(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
        };
        let expected = STANDARD.encode(Sha1::digest(format!(
            "{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
        )));
        if field("sec-websocket-accept") != vec![expected.as_str()]
            || !field("upgrade")
                .iter()
                .any(|s| s.eq_ignore_ascii_case("websocket"))
            || !field("connection").iter().any(|s| {
                s.split(',')
                    .any(|v| v.trim().eq_ignore_ascii_case("upgrade"))
            })
        {
            return Err("invalid WebSocket upgrade".into());
        }
        timeout_handle
            .set_read_timeout(Some(Duration::from_millis(20)))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            io,
            input: Vec::new(),
            fragment: Vec::new(),
            fragmented: false,
        })
    }
    pub fn send(&mut self, text: &str) -> Result<()> {
        self.write_frame(1, text.as_bytes())
    }
    fn write_frame(&mut self, opcode: u8, data: &[u8]) -> Result<()> {
        if data.len() > MAX_FRAME {
            return Err("frame too large".into());
        }
        let mut mask = [0; 4];
        SystemRandom::new()
            .fill(&mut mask)
            .map_err(|_| "random failed")?;
        let mut frame = vec![0x80 | opcode];
        if data.len() < 126 {
            frame.push(0x80 | data.len() as u8)
        } else {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(data.len() as u16).to_be_bytes())
        }
        frame.extend_from_slice(&mask);
        frame.extend(data.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.io
            .write_all(&frame)
            .and_then(|()| self.io.flush())
            .map_err(|e| e.to_string())
    }
    pub fn poll(&mut self) -> Result<Option<String>> {
        let deadline = Instant::now() + Duration::from_millis(25);
        for _ in 0..32 {
            if Instant::now() >= deadline {
                return Ok(None);
            }
            if let Some((used, fin, opcode, data)) = parse_frame(&self.input)? {
                self.input.drain(..used);
                match opcode {
                    8 => return Err("relay closed connection".into()),
                    9 => {
                        self.write_frame(10, &data)?;
                        continue;
                    }
                    10 => continue,
                    1 => {
                        if self.fragmented {
                            return Err("nested fragmented message".into());
                        }
                        if fin {
                            return String::from_utf8(data)
                                .map(Some)
                                .map_err(|_| "invalid UTF-8".into());
                        }
                        self.fragmented = true;
                        self.fragment = data;
                    }
                    0 => {
                        if !self.fragmented {
                            return Err("unexpected continuation".into());
                        }
                        if self.fragment.len() + data.len() > MAX_FRAME {
                            return Err("message too large".into());
                        }
                        self.fragment.extend(data);
                        if fin {
                            self.fragmented = false;
                            return String::from_utf8(std::mem::take(&mut self.fragment))
                                .map(Some)
                                .map_err(|_| "invalid UTF-8".into());
                        }
                    }
                    _ => return Err("unsupported WebSocket opcode".into()),
                }
                continue;
            }
            let mut buf = [0; 4096];
            match self.io.read(&mut buf) {
                Ok(0) => return Err("relay disconnected".into()),
                Ok(n) => {
                    self.input.extend_from_slice(&buf[..n]);
                    if self.input.len() > MAX_FRAME + 4096 + 14 {
                        return Err("input exceeds frame limit".into());
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(None)
    }
}

type Frame = (usize, bool, u8, Vec<u8>);
fn parse_frame(input: &[u8]) -> Result<Option<Frame>> {
    if input.len() < 2 {
        return Ok(None);
    }
    let fin = input[0] & 0x80 != 0;
    let opcode = input[0] & 15;
    if input[0] & 0x70 != 0 || input[1] & 0x80 != 0 {
        return Err("invalid RSV or masked server frame".into());
    }
    if !matches!(opcode, 0 | 1 | 8 | 9 | 10) {
        return Err("unsupported opcode".into());
    }
    let short = (input[1] & 127) as usize;
    let (len, offset) = match short {
        126 => {
            if input.len() < 4 {
                return Ok(None);
            }
            let n = u16::from_be_bytes([input[2], input[3]]) as usize;
            if n < 126 {
                return Err("nonminimal length".into());
            }
            (n, 4)
        }
        127 => return Err("frame exceeds limit".into()),
        n => (n, 2),
    };
    if len > MAX_FRAME || (opcode >= 8 && (!fin || len > 125)) || (opcode == 8 && len == 1) {
        return Err("invalid frame size/control fragmentation".into());
    }
    if input.len() < offset + len {
        return Ok(None);
    }
    Ok(Some((
        offset + len,
        fin,
        opcode,
        input[offset..offset + len].to_vec(),
    )))
}
#[cfg(test)]
mod tests {
    use super::*;
    struct TestIo {
        input: std::io::Cursor<Vec<u8>>,
        output: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    }
    impl Read for TestIo {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(out)
        }
    }
    impl Write for TestIo {
        fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
            self.output.lock().unwrap().extend_from_slice(input);
            Ok(input.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    fn socket(input: Vec<u8>) -> (WebSocket, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
        let output = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        (
            WebSocket {
                io: Box::new(TestIo {
                    input: std::io::Cursor::new(input),
                    output: output.clone(),
                }),
                input: Vec::new(),
                fragment: Vec::new(),
                fragmented: false,
            },
            output,
        )
    }
    #[test]
    fn fragmented_text_allows_ping_and_masks_pong() {
        let (mut ws, out) = socket(vec![
            0x01, 2, b'h', b'e', 0x89, 1, b'!', 0x80, 3, b'l', b'l', b'o',
        ]);
        assert_eq!(ws.poll().unwrap(), Some("hello".into()));
        let pong = out.lock().unwrap();
        assert_eq!(&pong[..2], &[0x8a, 0x81]);
        assert_eq!(pong[6] ^ pong[2], b'!');
    }
    #[test]
    fn rejects_continuation_and_fragmented_oversize() {
        assert!(socket(vec![0x80, 1, b'x']).0.poll().is_err());
        let mut frame = vec![0x01, 126, 0x40, 0x00];
        frame.extend(vec![b'x'; MAX_FRAME]);
        frame.extend([0x80, 1, b'x']);
        assert!(socket(frame).0.poll().is_err());
    }
    #[test]
    fn ping_flood_yields_without_losing_following_text() {
        let mut bytes = Vec::new();
        for _ in 0..200 {
            bytes.extend([0x89, 0]);
        }
        bytes.extend([0x81, 2, b'o', b'k']);
        let (mut ws, _) = socket(bytes);
        assert_eq!(ws.poll().unwrap(), None);
        let mut result = None;
        for _ in 0..16 {
            result = ws.poll().unwrap();
            if result.is_some() {
                break;
            }
        }
        assert_eq!(result, Some("ok".into()));
    }
    #[test]
    fn url_rejects_injection_and_remote_plaintext() {
        for u in [
            "ws://example.org/a",
            "wss://good/a\r\nx:x",
            "wss://user@good/",
            "wss://good/#x",
        ] {
            assert!(Url::parse(u).is_err(), "{u}")
        }
        assert!(Url::parse("ws://[::1]:8765/").is_ok());
    }
    #[test]
    fn frame_boundaries() {
        assert!(parse_frame(&[0x81, 0x80]).is_err());
        assert!(parse_frame(&[0x89, 126, 0, 126]).is_err());
        assert!(parse_frame(&[0x81, 127]).is_err());
        assert!(parse_frame(&[0x81, 3, b'a']).unwrap().is_none());
        assert_eq!(parse_frame(&[0x01, 1, b'a']).unwrap().unwrap().2, 1);
        assert_eq!(parse_frame(&[0x80, 1, b'b']).unwrap().unwrap().2, 0);
        assert_eq!(parse_frame(&[0x89, 1, b'x']).unwrap().unwrap().2, 9);
    }
}
