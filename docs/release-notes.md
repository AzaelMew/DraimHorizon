**DraimHorizon 0.28.1** for official Horizon Nightly 2026-11-07 — 2ebf00a (Java 26.3).

- Chat translation no longer touches messages that are already in the target language. Translating into English, lines such as `im at spawn` were sent to the provider and shown back rewritten (`I'm at spawn`). They are now skipped before sending when the local detector identifies the target language, or when plain-ASCII text cannot be identified and the target is English, as before language selection existed.
- A provider result is discarded when the provider reports the text was already in the target language, or when only case, spacing or punctuation changed.
- An explicit **Input** / **Chat from** language is trusted, except that a source equal to the target is never translated. With a non-Latin source such as Russian, text containing none of that script (for example `yea i know`) is left alone instead of being forced through the provider as Russian and rewritten.
- Outgoing messages use the same rule, so an unnecessary translation no longer ends with a "service unavailable" notice.

Update by reinstalling from GitHub in Horizon Launcher and rebuild native mods.
