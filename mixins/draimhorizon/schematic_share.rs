// Schematic previews shared with friends: the owner's shown previews of a world are listed to
// every friend over the relay (`[DHS1]` frames), and a friend without the file asks for it and
// gets it in pieces. Only the owner moves, turns, hides or removes a preview; a friend only shows
// or hides it on their own screen. Pure data: frames, checks and the putting together of a file.
use serde_json::{Value, json};
pub const PREFIX: &str = "[DHS1] ";
/// Previews of one owner listed at most (the most recent shown ones).
pub const MAX_LISTED: usize = 5;
/// The largest file shared (as it is on disk, compressed).
pub const MAX_FILE: usize = 384 * 1024;
/// Bytes of the file a piece carries (base64 in the frame keeps it under the relay's 1 KiB).
pub const PIECE: usize = 600;
/// One shared preview as the owner has it placed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    /// The owner's number for it (stays while it is moved or turned).
    pub id: u32,
    /// The file name, shown to friends.
    pub name: String,
    pub dimension: String,
    pub origin: [i32; 3],
    pub rotation: u8,
    /// The file's content: its hash and its size in bytes.
    pub hash: u64,
    pub size: u32,
}
impl Listed {
    /// "schem" or "litematic": how the file is read.
    pub fn extension(&self) -> &'static str {
        if self.name.to_ascii_lowercase().ends_with(".schem") {
            "schem"
        } else {
            "litematic"
        }
    }
    /// The name the file is kept under by a friend: its content, so the same file is fetched once.
    pub fn cache_name(&self) -> String {
        format!("{:016x}.{}", self.hash, self.extension())
    }
    pub fn pieces(&self) -> u32 {
        (self.size as usize).div_ceil(PIECE) as u32
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// The owner's shown previews (all of them: one missing from it is gone).
    List(Vec<Listed>),
    /// A friend asks for a file, from the piece `from` on.
    Want { hash: u64, from: u32 },
    /// A piece of a file.
    Piece {
        hash: u64,
        index: u32,
        count: u32,
        data: Vec<u8>,
    },
}
fn dimension_ok(d: &str) -> bool {
    !d.is_empty()
        && d.len() <= 64
        && d.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_:/.-".contains(&b))
}
fn name_ok(n: &str) -> bool {
    let lower = n.to_ascii_lowercase();
    !n.is_empty()
        && n.chars().count() <= 48
        && !n.chars().any(|c| c.is_control() || matches!(c, '/' | '\\'))
        && (lower.ends_with(".schem") || lower.ends_with(".litematic"))
}
fn hex(hash: u64) -> String {
    format!("{hash:016x}")
}
fn unhex(v: &Value) -> Option<u64> {
    let s = v.as_str()?;
    (s.len() == 16)
        .then(|| u64::from_str_radix(s, 16).ok())
        .flatten()
}
impl Frame {
    pub fn encode(&self) -> String {
        let body = match self {
            Frame::List(items) => json!([
                "l",
                items
                    .iter()
                    .map(|p| json!([
                        p.id,
                        p.name,
                        p.dimension,
                        p.origin[0],
                        p.origin[1],
                        p.origin[2],
                        p.rotation,
                        hex(p.hash),
                        p.size
                    ]))
                    .collect::<Vec<_>>()
            ]),
            Frame::Want { hash, from } => json!(["w", hex(*hash), from]),
            Frame::Piece {
                hash,
                index,
                count,
                data,
            } => json!(["p", hex(*hash), index, count, base64(data)]),
        };
        format!("{PREFIX}{body}")
    }
    /// A frame from a friend, checked whole.
    pub fn decode(body: &str) -> Result<Self, String> {
        if body.len() > 1024 {
            return Err("schematic frame too large".into());
        }
        let v: Value =
            serde_json::from_str(body.strip_prefix(PREFIX).ok_or("not a schematic frame")?)
                .map_err(|_| "invalid schematic frame JSON")?;
        let parts = v.as_array().ok_or("invalid schematic frame")?;
        let int = |v: &Value| v.as_i64().and_then(|v| i32::try_from(v).ok());
        let small = |v: &Value| v.as_u64().and_then(|v| u32::try_from(v).ok());
        match (parts.first().and_then(Value::as_str), parts.len()) {
            (Some("l"), 2) => {
                let rows = parts[1]
                    .as_array()
                    .filter(|r| r.len() <= MAX_LISTED)
                    .ok_or("invalid preview list")?;
                let mut items = Vec::new();
                for row in rows {
                    let r = row
                        .as_array()
                        .filter(|r| r.len() == 9)
                        .ok_or("invalid listed preview")?;
                    let item = Listed {
                        id: small(&r[0]).ok_or("invalid preview id")?,
                        name: r[1]
                            .as_str()
                            .filter(|n| name_ok(n))
                            .ok_or("invalid preview name")?
                            .to_owned(),
                        dimension: r[2]
                            .as_str()
                            .filter(|d| dimension_ok(d))
                            .ok_or("invalid preview dimension")?
                            .to_owned(),
                        origin: [
                            int(&r[3]).ok_or("invalid preview origin")?,
                            int(&r[4]).ok_or("invalid preview origin")?,
                            int(&r[5]).ok_or("invalid preview origin")?,
                        ],
                        rotation: small(&r[6])
                            .filter(|t| *t < 4)
                            .ok_or("invalid preview rotation")?
                            as u8,
                        hash: unhex(&r[7]).ok_or("invalid preview hash")?,
                        size: small(&r[8])
                            .filter(|s| *s > 0 && *s as usize <= MAX_FILE)
                            .ok_or("invalid preview size")?,
                    };
                    if item.origin[0].unsigned_abs() > 30_000_000
                        || item.origin[2].unsigned_abs() > 30_000_000
                        || item.origin[1].unsigned_abs() > 4096
                    {
                        return Err("preview outside the world".into());
                    }
                    if items.iter().any(|i: &Listed| i.id == item.id) {
                        return Err("duplicate preview id".into());
                    }
                    items.push(item);
                }
                Ok(Frame::List(items))
            }
            (Some("w"), 3) => Ok(Frame::Want {
                hash: unhex(&parts[1]).ok_or("invalid file hash")?,
                from: small(&parts[2])
                    .filter(|f| (*f as usize) < MAX_FILE.div_ceil(PIECE))
                    .ok_or("invalid piece")?,
            }),
            (Some("p"), 5) => {
                let count = small(&parts[3])
                    .filter(|c| *c > 0 && (*c as usize) <= MAX_FILE.div_ceil(PIECE))
                    .ok_or("invalid piece count")?;
                let index = small(&parts[2])
                    .filter(|i| *i < count)
                    .ok_or("invalid piece index")?;
                let data = unbase64(parts[4].as_str().ok_or("invalid piece data")?)
                    .filter(|d| !d.is_empty() && d.len() <= PIECE)
                    .ok_or("invalid piece data")?;
                Ok(Frame::Piece {
                    hash: unhex(&parts[1]).ok_or("invalid file hash")?,
                    index,
                    count,
                    data,
                })
            }
            _ => Err("unknown schematic frame".into()),
        }
    }
}
/// The piece frames of a file, from piece `from` on.
pub fn pieces(bytes: &[u8], hash: u64, from: u32) -> impl Iterator<Item = Frame> + '_ {
    let count = bytes.len().div_ceil(PIECE) as u32;
    (from..count).map(move |index| {
        let start = index as usize * PIECE;
        Frame::Piece {
            hash,
            index,
            count,
            data: bytes[start..(start + PIECE).min(bytes.len())].to_vec(),
        }
    })
}
/// A file being put together from its pieces; done when every piece has come and the content
/// has the hash and size it was listed with.
#[derive(Debug)]
pub struct Download {
    pub hash: u64,
    size: usize,
    pieces: Vec<Option<Vec<u8>>>,
}
impl Download {
    pub fn new(item: &Listed) -> Self {
        Self {
            hash: item.hash,
            size: item.size as usize,
            pieces: vec![None; item.pieces() as usize],
        }
    }
    /// The first piece still missing (to ask for), none when all have come.
    pub fn next_missing(&self) -> Option<u32> {
        self.pieces
            .iter()
            .position(Option::is_none)
            .map(|i| i as u32)
    }
    /// Takes a piece; the whole file once it is complete and checks out. A file that does not
    /// check out is started again.
    pub fn add(&mut self, index: u32, count: u32, data: Vec<u8>) -> Option<Vec<u8>> {
        if count as usize != self.pieces.len() {
            return None;
        }
        let expected = if index as usize + 1 == self.pieces.len() {
            self.size - PIECE * (self.pieces.len() - 1)
        } else {
            PIECE
        };
        if data.len() != expected {
            return None;
        }
        *self.pieces.get_mut(index as usize)? = Some(data);
        if self.next_missing().is_some() {
            return None;
        }
        let bytes: Vec<u8> = self.pieces.iter().flatten().flatten().copied().collect();
        if bytes.len() == self.size && content_hash(&bytes) == self.hash {
            Some(bytes)
        } else {
            self.pieces.iter_mut().for_each(|p| *p = None);
            None
        }
    }
}
/// The hash a shared file is known by.
pub fn content_hash(bytes: &[u8]) -> u64 {
    super::claim_borders_map_history::hash(bytes)
}
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
fn unbase64(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let value = |c: u8| ALPHABET.iter().position(|a| *a == c).map(|v| v as u32);
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (i, quad) in bytes.chunks(4).enumerate() {
        let last = i + 1 == bytes.len() / 4;
        let pad = quad.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for (j, c) in quad.iter().enumerate() {
            let v = if j >= 4 - pad { 0 } else { value(*c)? };
            n = n << 6 | v;
        }
        let decoded = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&decoded[..3 - pad]);
    }
    Some(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: u32, size: u32, hash: u64) -> Listed {
        Listed {
            id,
            name: format!("Дом {id}.litematic"),
            dimension: "minecraft:overworld".into(),
            origin: [-1200, 64, 3400],
            rotation: 3,
            hash,
            size,
        }
    }
    #[test]
    fn frames_are_read_back_and_stay_under_the_relay_size() {
        let list = Frame::List(
            (0..MAX_LISTED as u32)
                .map(|i| item(i, MAX_FILE as u32, u64::MAX - u64::from(i)))
                .collect(),
        );
        let body = list.encode();
        assert!(body.len() <= 1024, "{}", body.len());
        assert_eq!(Frame::decode(&body).unwrap(), list);
        let want = Frame::Want { hash: 42, from: 7 };
        assert_eq!(Frame::decode(&want.encode()).unwrap(), want);
        let bytes: Vec<u8> = (0..PIECE as u32 * 2 + 17)
            .map(|i| (i * 7 % 256) as u8)
            .collect();
        for piece in pieces(&bytes, 9, 0) {
            let body = piece.encode();
            assert!(body.len() <= 1024, "{}", body.len());
            assert_eq!(Frame::decode(&body).unwrap(), piece);
        }
        assert_eq!(pieces(&bytes, 9, 2).count(), 1);
    }
    #[test]
    fn bad_frames_are_refused() {
        let good = item(1, 100, 5);
        let mut bad = Vec::new();
        for change in [
            |p: &mut Listed| p.name = "../evil.litematic".into(),
            |p: &mut Listed| p.name = "notes.txt".into(),
            |p: &mut Listed| p.dimension = "Bad Dim".into(),
            |p: &mut Listed| p.origin[1] = 9000,
            |p: &mut Listed| p.size = 0,
            |p: &mut Listed| p.size = MAX_FILE as u32 + 1,
        ] {
            let mut p = good.clone();
            change(&mut p);
            bad.push(Frame::List(vec![p]).encode());
        }
        bad.push(Frame::List(vec![good.clone(), good.clone()]).encode());
        bad.push(Frame::List(vec![good.clone(); MAX_LISTED + 1]).encode());
        bad.push(format!(r#"{PREFIX}["l",[[1,"a.litematic","minecraft:overworld",0,0,0,4,"0000000000000005",10]]]"#));
        bad.push(format!(r#"{PREFIX}["w","05",0]"#));
        bad.push(format!(r#"{PREFIX}["p","0000000000000005",3,3,"AAAA"]"#));
        bad.push(format!(r#"{PREFIX}["p","0000000000000005",0,1,"A==="]"#));
        bad.push(format!(r#"{PREFIX}["x"]"#));
        bad.push("[DHF1] [\"l\",[]]".to_string());
        for body in bad {
            assert!(Frame::decode(&body).is_err(), "{body}");
        }
    }
    #[test]
    fn a_file_is_put_together_from_its_pieces_in_any_order_and_checked() {
        let bytes: Vec<u8> = (0..PIECE * 3 + 5).map(|i| (i % 251) as u8).collect();
        let listed = item(1, bytes.len() as u32, content_hash(&bytes));
        let mut download = Download::new(&listed);
        let mut frames: Vec<Frame> = pieces(&bytes, listed.hash, 0).collect();
        frames.reverse();
        let mut done = None;
        for frame in frames {
            assert!(done.is_none());
            if let Frame::Piece {
                index, count, data, ..
            } = frame
            {
                done = download.add(index, count, data);
            }
        }
        assert_eq!(done.as_deref(), Some(bytes.as_slice()));
        // A piece with the wrong length or count is not taken; a file with another content is
        // started again.
        let mut download = Download::new(&listed);
        assert!(download.add(0, 4, vec![1; PIECE - 1]).is_none());
        assert!(download.add(0, 5, vec![1; PIECE]).is_none());
        assert_eq!(download.next_missing(), Some(0));
        for (i, frame) in pieces(&bytes, listed.hash, 0).enumerate() {
            if let Frame::Piece {
                index,
                count,
                mut data,
                ..
            } = frame
            {
                if i == 0 {
                    data[0] ^= 1;
                }
                assert!(download.add(index, count, data).is_none());
            }
        }
        assert_eq!(download.next_missing(), Some(0));
        assert_eq!(
            listed.cache_name(),
            format!("{:016x}.litematic", listed.hash)
        );
    }
    #[test]
    fn base64_round_trips() {
        for len in 0..10 {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 200) as u8).collect();
            assert_eq!(unbase64(&base64(&data)).unwrap(), data);
        }
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert!(unbase64("TW=u").is_none());
        assert!(unbase64("TWF").is_none());
    }
}
