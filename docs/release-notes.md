**DraimHorizons 0.22.4** for **Horizon Nightly 2026-11-05 · f319701** (Java 26.3).

- A private relay can provide a shared DeepL fallback. Keep the DeepL key on the server and give friends only the relay address and access token.
- Personal DeepL keys can be added or removed under J → Settings → Chat → DeepL.
- Google endpoint failover is faster. Join, leave and first-visit labels follow the selected language locally.

Update the relay to 1.2.0 and set `RELAY_DEEPL_API_KEY` in its server environment to enable shared translation. [Relay setup](https://github.com/DraimCiDo/DraimHorizon/blob/v0.22.4/relay/README.md)

Rebuild native mods in the launcher after updating.
