# Chat translation: source detection and language selection

## Comparison

Reference inspected: [horizon-chat-translate runtime.rs at a749a4b](https://github.com/AzaelMew/horizon-chat-translate/blob/a749a4b5502dcaacd120e96c11aa42b2eb6ae2a9/runtime.rs).

| Concern | Reference mod | DraimHorizons before this change | DraimHorizon 0.24.0 |
|---|---|---|---|
| Incoming trigger | Click `[t]` | Automatic for recent eligible chat | Unchanged |
| Source language | Google `sl=auto`; DeepL omits `source_lang` | Providers already auto-detect, but local script filters decide eligibility | Same symmetric eligibility for both targets: letters, not commands |
| Russian target filter | Any alphabetic source | Non-Cyrillic letters, or qualifying mixed text; Cyrillic-only Ukrainian/etc. rejected | No script-based rejection |
| English target filter | Any alphabetic source | Requires a Cyrillic character; Finnish, Polish, Spanish and Japanese rejected | No script-based rejection |
| Destination | Configurable language code | RU/EN tied to UI language; outgoing uses the opposite | Independent Input / Output / Received-chat choices from 20 common languages on a full-screen settings page |
| Providers | Google; optional personal DeepL; configurable order | Google endpoint/mirror; personal or shared-relay DeepL fallback | Existing provider order and relay protocol unchanged |
| Outgoing | `/trans <language> <message>` | Automatic queue and signed chat submission | Existing queue/submission, with the selected outgoing target |

The bug was before the HTTP requests, not in Google/DeepL's source detection. Both providers must get a chance to identify Latin, Cyrillic, CJK and other alphabets. A request in the target language may return unchanged text; language coverage is still bounded by each provider. Punctuation-only text and slash commands remain excluded. No new automatic translation trigger was added.

## Implementation

- `relay_client/src/translation_languages.rs`: 20-language catalog (Google and DeepL codes, RU/EN names) and `SourcePolicy`. The filter reuses `detect_source_language` (the same detector that produces the per-line language badge) instead of a second detector, so there is no extra dependency or vendored code.
- `relay_client/src/translation.rs`: workers take a target plus policy. Skipped (non-common) text returns unchanged without any network request. Explicit input sets Google `sl` and DeepL `source_lang`; shared-relay frames carry it as an optional third element.
- `outgoing.rs`: `TranslationTargets` (schema 1, atomic save, never overwrites an invalid file), layout/hit-testing shared by painting and input, 39 px button.
- `outgoing_view.rs`: right-click opens a dedicated mod screen. The native chat text field is moved aside and restored, so draft, caret and selection survive.
- `chat_view.rs`: received chat uses the Received-chat target and the common-only policy; badges show the target code.
- `relay/`: 1.3.0 accepts the 20 DeepL targets and an optional source.

The filter only skips text the detector positively identifies as an uncommon language; unidentifiable text is translated, not dropped. Short messages are the weak point of auto-detection (and of the common-languages filter). Setting **Chat from** to the chat's language sends it as the explicit source and bypasses detection for received chat.

Limits: language identification of short text is probabilistic; provider quotas and coverage still apply; the UI is localized only in English and Russian.

## Verification

- Workspace tests: 320 passed, 1 ignored (public-provider smoke test); relay tests: 11 passed (in a throwaway venv).
- Mock-HTTP tests cover every catalog language as Google `tl` / DeepL `target_lang` and explicit `sl` / `source_lang`.
- Native client built with the pinned mixins staging. In an isolated game on bot display `:20` with fresh data directories: all 20 languages selected and saved for each of the three fields, draft and caret preserved, Esc cancel, save-failure error with the invalid file untouched, restart persistence, and the compact button at 39 px.
- No live translation request or public chat message was sent; software rendering only.
