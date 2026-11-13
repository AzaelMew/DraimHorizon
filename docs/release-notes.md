**DraimHorizon 0.40.0 - friends survive a relay key change**

Target: official Horizon Nightly source `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0` (unchanged).

- A friend who reinstalls the mod, moves to a new PC or resets the data folder gets a new relay key. Their frames used to be dropped until both sides removed and re-added each other. Now the key is re-pinned automatically when the relay lists exactly one identity for the friend's UUID, the old key is no longer connected, and the same account and nickname are on the game server's player list. The session restarts and a chat notice (English and Russian) tells you.
- If the friend lost the friendship entirely, the mod invites them once after three unanswered handshakes; they only accept. A friend who forgot you and invites again from the key you already pinned is relinked without a click.
- Ambiguous cases change nothing: two keys for one account, a friend who is not on the server, or a malformed code. The row reads "Key changed · waiting to verify the identity".
- Security trade-off: relay UUIDs and nicknames are self-claimed. The new key is trusted without an out-of-band code comparison, protected by the token-gated private relay, the server player-list check and a visible notice. Remove the friend if the notice was not expected.

Verification: Rust formatting, Clippy with warnings denied, the workspace tests, official mixin staging, the pinned native client compile-check and native bundle validation pass. Live in-game acceptance with two real clients is not verified.
