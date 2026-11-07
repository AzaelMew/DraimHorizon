use crate::{Config, Peer};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use curve25519_dalek::montgomery::MontgomeryPoint;
use hkdf::Hkdf;
use ring::{
    rand::{SecureRandom, SystemRandom},
    signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};
type Result<T> = std::result::Result<T, String>;
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn unhex<const N: usize>(s: &str) -> Result<[u8; N]> {
    if s.len() != N * 2
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("invalid lowercase hex".into());
    }
    let mut out = [0; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| "invalid hex")?
    }
    Ok(out)
}
pub(crate) fn random<const N: usize>() -> Result<[u8; N]> {
    let mut out = [0; N];
    SystemRandom::new()
        .fill(&mut out)
        .map_err(|_| "secure random failed")?;
    Ok(out)
}
pub(crate) fn code(ed: &[u8]) -> String {
    hex(&Sha256::digest(ed)[..12])
}
pub(crate) struct Identity {
    signing: Ed25519KeyPair,
    secret: [u8; 32],
    pub ed: String,
    pub x: String,
}
impl Identity {
    pub fn load(path: &Path) -> Result<Self> {
        let mut bytes = [0; 64];
        match OpenOptions::new().read(true).open(path) {
            Ok(mut file) => {
                if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                    return Err("identity is not a regular file".into());
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if file
                        .metadata()
                        .map_err(|e| e.to_string())?
                        .permissions()
                        .mode()
                        & 0o077
                        != 0
                    {
                        return Err("identity permissions must be 0600".into());
                    }
                }
                file.read_exact(&mut bytes)
                    .map_err(|_| "invalid identity file")?;
                let mut extra = [0];
                if file.read(&mut extra).map_err(|e| e.to_string())? != 0 {
                    return Err("invalid identity length".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                bytes = random()?;
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let temporary = path.with_extension(format!("tmp-{}", hex(&random::<8>()?)));
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let result = (|| {
                    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
                    file.write_all(&bytes)
                        .and_then(|()| file.sync_all())
                        .map_err(|e| e.to_string())?;
                    // A hard link atomically installs the complete key without replacing another writer.
                    fs::hard_link(&temporary, path).map_err(|e| e.to_string())?;
                    Ok::<_, String>(())
                })();
                let _ = fs::remove_file(&temporary);
                result?;
            }
            Err(e) => return Err(e.to_string()),
        }
        let signing = Ed25519KeyPair::from_seed_unchecked(&bytes[..32])
            .map_err(|_| "invalid identity seed")?;
        let secret = bytes[32..]
            .try_into()
            .map_err(|_| "invalid identity secret")?;
        let ed = hex(signing.public_key().as_ref());
        let x = hex(&MontgomeryPoint::mul_base_clamped(secret).to_bytes());
        Ok(Self {
            signing,
            secret,
            ed,
            x,
        })
    }
    pub fn sign(&self, bytes: &[u8]) -> String {
        hex(self.signing.sign(bytes).as_ref())
    }
    pub fn certificate(&self, c: &Config) -> Value {
        let data = json!([
            "id1",
            c.realm,
            self.ed,
            self.x,
            format!("{:032x}", c.uuid),
            c.name
        ])
        .to_string();
        json!({"data":data,"sig":self.sign(data.as_bytes())})
    }
    pub fn peer(&self, c: &Config) -> Peer {
        Peer {
            code: code(self.signing.public_key().as_ref()),
            uuid: c.uuid,
            name: c.name.clone(),
        }
    }
}
#[derive(Clone)]
pub(crate) struct Verified {
    pub peer: Peer,
    pub ed: [u8; 32],
    pub x: [u8; 32],
}
pub(crate) fn certificate(value: &Value, realm: &str) -> Result<Verified> {
    let literal = string(&value["data"])?;
    if literal.len() > 1024 {
        return Err("certificate too large".into());
    }
    let parsed: Value = serde_json::from_str(literal).map_err(|_| "invalid certificate JSON")?;
    let data = parsed.as_array().ok_or("invalid certificate")?;
    if data.len() != 6 || data[0] != "id1" || data[1] != realm {
        return Err("certificate realm/version mismatch".into());
    }
    let ed = unhex::<32>(string(&data[2])?)?;
    let x = unhex::<32>(string(&data[3])?)?;
    let uuid = u128::from_str_radix(string(&data[4])?, 16).map_err(|_| "invalid UUID")?;
    if string(&data[4])?.len() != 32 {
        return Err("invalid UUID length".into());
    }
    let name = string(&data[5])?;
    if name.is_empty()
        || name.len() > 16
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("invalid player name".into());
    }
    verify(&ed, literal.as_bytes(), string(&value["sig"])?)?;
    Ok(Verified {
        peer: Peer {
            code: code(&ed),
            uuid,
            name: name.to_owned(),
        },
        ed,
        x,
    })
}
pub(crate) fn string(v: &Value) -> Result<&str> {
    v.as_str().ok_or("expected string".into())
}
pub(crate) fn verify(ed: &[u8; 32], bytes: &[u8], sig: &str) -> Result<()> {
    UnparsedPublicKey::new(&ED25519, ed)
        .verify(bytes, &unhex::<64>(sig)?)
        .map_err(|_| "signature verification failed".into())
}
fn key(
    secret: [u8; 32],
    public: [u8; 32],
    realm: &str,
    sender: &[u8; 32],
    recipient: &[u8; 32],
) -> Result<[u8; 32]> {
    let shared = MontgomeryPoint(public).mul_clamped(secret).to_bytes();
    if shared == [0; 32] {
        return Err("invalid X25519 public key".into());
    }
    let mut info = b"DHRelay-box-v1\0".to_vec();
    info.extend_from_slice(sender);
    info.extend_from_slice(recipient);
    let mut key = [0; 32];
    Hkdf::<Sha256>::new(Some(&unhex::<32>(realm)?), &shared)
        .expand(&info, &mut key)
        .map_err(|_| "HKDF failed")?;
    Ok(key)
}
fn signed_box(aad: &[u8], cipher: &[u8]) -> Vec<u8> {
    let mut out = b"DHRelay-box-v1\0".to_vec();
    out.extend_from_slice(aad);
    out.extend_from_slice(cipher);
    out
}
pub(crate) fn seal(identity: &Identity, realm: &str, to: &Verified, body: &str) -> Result<Value> {
    let ephemeral = random::<32>()?;
    let epk = hex(&MontgomeryPoint::mul_base_clamped(ephemeral).to_bytes());
    let nonce = hex(&random::<12>()?);
    let from = code(identity.signing.public_key().as_ref());
    let sender = unhex::<32>(&identity.ed)?;
    let context = json!(["box1", realm, from, to.peer.code, epk, nonce]);
    let aad = context.to_string();
    let key = key(ephemeral, to.x, realm, &sender, &to.ed)?;
    let cipher = ChaCha20Poly1305::new((&key).into())
        .encrypt(
            Nonce::from_slice(&unhex::<12>(&nonce)?),
            Payload {
                msg: body.as_bytes(),
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| "encryption failed")?;
    Ok(json!([
        "box1",
        realm,
        from,
        to.peer.code,
        epk,
        nonce,
        hex(&cipher),
        identity.sign(&signed_box(aad.as_bytes(), &cipher))
    ]))
}
pub(crate) fn open(
    identity: &Identity,
    realm: &str,
    from: &Verified,
    value: &Value,
) -> Result<(String, String)> {
    let data = value.as_array().ok_or("invalid envelope")?;
    if data.len() != 8
        || data[0] != "box1"
        || data[1] != realm
        || data[2] != from.peer.code
        || data[3] != code(identity.signing.public_key().as_ref())
    {
        return Err("wrong message context/recipient".into());
    }
    let epk = unhex::<32>(string(&data[4])?)?;
    let nonce = unhex::<12>(string(&data[5])?)?;
    let ciphertext = string(&data[6])?;
    if ciphertext.len() > 2080 || ciphertext.len() % 2 != 0 {
        return Err("ciphertext exceeds limit".into());
    }
    let cipher: Vec<_> = ciphertext
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| {
            std::str::from_utf8(chunk)
                .map_err(|_| "invalid cipher")
                .and_then(|s| u8::from_str_radix(s, 16).map_err(|_| "invalid cipher"))
        })
        .collect::<std::result::Result<_, _>>()?;
    let context = Value::Array(data[..6].to_vec());
    let aad = context.to_string();
    verify(
        &from.ed,
        &signed_box(aad.as_bytes(), &cipher),
        string(&data[7])?,
    )?;
    let key = key(
        identity.secret,
        epk,
        realm,
        &from.ed,
        &unhex::<32>(&identity.ed)?,
    )?;
    let clear = ChaCha20Poly1305::new((&key).into())
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &cipher,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| "decryption failed")?;
    let body = String::from_utf8(clear).map_err(|_| "invalid message UTF-8")?;
    if body.len() > 1024 {
        return Err("message too long".into());
    }
    Ok((format!("{}:{}", data[4], data[5]), body))
}
