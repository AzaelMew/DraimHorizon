**DraimHorizon 0.24.1** for official Horizon Nightly 2026-11-07 - 6560641 (Java 26.3).

- Pin the official Nightly source commit 6560641ba51a452f6bc2407baa4eb1b3be517ca8, replacing the main-branch snapshot used in 0.24.0.
- Compatibility tooling and CI now resolve the official nightly tag, including annotated tags, and reject a missing or outdated Nightly pin. There is no fallback to main.
- AGENTS.md requires the latest official Nightly for all future work and releases.

Includes the translation-language features, chat/UI fixes and auto reconnect from 0.24.0. The shared DeepL relay still needs service 1.3.0 for all new language/source settings.
