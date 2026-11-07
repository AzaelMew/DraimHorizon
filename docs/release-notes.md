**DraimHorizon 0.24.0** for Horizon main 1d63e0a (Java 26.3).

- Chat translation button is now a compact 39 px direction tag (for example `EN>RU`; `?` means auto-detected input) with no `:...` suffix. The chat input box starts right after it. Left click toggles outgoing translation; right click opens a full-screen **Translation languages** screen.
- Four independent choices — **Input** (language you write in, or Auto detect), **Output** (language sent), **Chat into** (language incoming chat is translated into) and **Chat from** (language incoming chat is written in, or Auto detect) — each from 20 common languages: English, Russian, Finnish, Polish, German, French, Spanish, Italian, Portuguese, Ukrainian, Dutch, Swedish, Danish, Czech, Turkish, Arabic, Hindi, Japanese, Korean, Chinese. An explicit Input or Chat from is sent to Google (`sl`) and DeepL (`source_lang`). Choosing **Chat from** fixes short messages that auto-detection skips; it also bypasses the common-languages filter.
- First start after the update keeps the previous behaviour instead of Auto: RU interface gives `RU>EN`, EN interface gives `EN>RU`, and received chat is translated into the interface language. The result is saved once; existing choices are never overwritten.
- **Auto-translate: common languages only** (on by default) skips text whose detected language is outside those 20, without a network request. Detection is a bundled MIT copy of whatlang 0.18; short or mixed text is approximate.
- The common-languages filter now skips a line only when detection of an uncommon language is reliable. Tiny words, typos, slang and mixed Russian/English lines (for example `claim команда`) are translated instead of silently dropped; Cyrillic neighbours of Russian/Ukrainian are accepted.
- Translation into English no longer requires Cyrillic source text; all targets share one eligibility rule.
- Settings are stored atomically in `translation_preferences`; an invalid file is never overwritten, the screen shows an error and keeps your selection. Esc cancels, Enter saves, and the unsent chat draft and caret are preserved.
- Relay 1.3.0 accepts the 20 DeepL targets and an optional validated source language; older clients keep working, but the shared DeepL fallback needs this relay for non-RU/EN targets.
- Update the Horizon pin to main 1d63e0a.

Rebuild native mods in the launcher and restart the client. Deploy relay 1.3.0 before using non-RU/EN targets through the shared DeepL relay.
