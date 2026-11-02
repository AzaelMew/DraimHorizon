# DHRelay v1 wire contract

UTF-8 JSON arrays in WebSocket text messages, maximum frame/message 16384 bytes.
Path `/v1/relay`; `/healthz` is HTTP GET. `wss://` in production; `ws://` only for loopback development.
Authorization header `Bearer <access_token>` when operator sets RELAY_ACCESS_TOKEN.
No compression required. Ping/pong and close follow RFC6455. Max opaque packet 8192 UTF-8 bytes;
cert string <=1024 bytes, plaintext party body <=1024 bytes. No offline queue on the relay.

## Identity and authentication

Persistent client Ed25519 signing seed32 and X25519 static secret32, both secure random.
Code is `hex(SHA256(ed25519_public_key)[0:12])`, 24 lowercase hex characters.
Realm is hex SHA256 of the normalized Minecraft server address, 64 lowercase hex.
Normalize trimmed lowercase hostname, omit trailing `:25565`; bracketed IPv6 preserved.
Identity certificate is the literal compact JSON string of:

`["id1", realm, ed_public_hex64, x_public_hex64, minecraft_uuid_hex32, account_name]`

Account name is ASCII `[A-Za-z0-9_]{1,16}`; UUID is CLAIMED metadata, not proof of Mojang ownership.
The certificate is self-signed Ed25519 over its EXACT UTF-8 bytes. First pairing must use the
other player's code through an external trusted channel; do not silently trust name/UUID.
Client validates every peer certificate signature and recalculates its code before using it.
Parent mod additionally matches UUID/name to the current game tab and requires reciprocal acceptance.

1. Server -> `["challenge", nonce_hex64]` (32 secure random bytes, one use, expires in 10s).
2. Client -> `["auth", cert_string, cert_signature_hex128, challenge_signature_hex128]`.
3. challenge signature covers `b"DHRelay-auth-v1\0" + nonce_bytes + cert_utf8`.
4. Server checks optional access token, all certificate fields and signatures, realm limits;
   then sends `["ready", own_code]`.
5. Server sends one `["peer", cert_string, cert_signature_hex128]` per OTHER peer in the same realm.
   New peer broadcasts same peer frame to existing realm connections. No cross-realm discovery.
6. Disconnection broadcasts `["gone", code]` to that realm. Duplicate live identity is rejected;
   cleanup must never remove a replacement connection accidentally.

Client exposes Connected{code}, Peers(Vec<Peer>), Message{peer,body}, Disconnected{reason},
and Notice{message}. Peer {code:String, uuid:u128, name:String}; cryptographic keys remain internal.

## Routing

Client -> `["box", recipient_code, opaque_string]`.
Server -> recipient `["box", authenticated_sender_code, opaque_string]`.
Recipient must currently be in the same realm; otherwise `["error", "recipient_offline"]`.
Limits:256 global clients,64/realm,8/IP; token-bucket 10messages/s burst20, bounded queues128.
Relay never parses opaque data and never logs it, tokens, certificates or health/coordinates.

## End-to-end box (inside opaque_string)

Compact JSON string:

`["box1", realm, sender_code, recipient_code, ephemeral_x25519_public_hex64, nonce_hex24, ciphertext_hex, signature_hex128]`

AAD is compact JSON UTF-8 of the FIRST SIX array fields (all ASCII strings).
Generate a fresh X25519 ephemeral secret per packet; shared secret = X25519(ephemeral secret,
recipient's certificate static X25519 public key), rejecting all-zero shared secret.
HKDF-SHA256 uses salt=32decoded realm bytes and info=`b"DHRelay-box-v1\0" + sender_ed_public32 + recipient_ed_public32`;
output32 is a ChaCha20-Poly1305 key. Random96-bit nonce; encrypt plaintext UTF-8 party body with AAD.
Ciphertext includes the authentication tag. Sign Ed25519 over
`b"DHRelay-box-v1\0" + aad_bytes + ciphertext_bytes`.
Recipient verifies sender/recipient/realm, known peer cert, signature, size and replay nonce before
accepting AEAD plaintext. Bounded replay LRU >=1024 packets per worker connection. Party protocol
nonces/sequences provide independent session/replay checks after decryption.

Private keys are local-only, atomic 0600 storage on Unix; never sent to relay. URL/token settings
must not print secrets. Certificate validation must not be disabled. Handshakes and reconnect backoff
run on a worker thread with bounded queues, never on the render/game thread.
