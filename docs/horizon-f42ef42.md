# Horizon f42ef42 compatibility and remote builds

Horizon: `f42ef42ef6ceccb3c6a20bea2481d87949a51c81` (observed native hooks API 359).
horizon-mixins: `10fdf8bda39d1e336c0f13f802db1041fbe41abf`.

The task executor reports `github_connector_not_configured` even though the GitHub app can read/write the repository.
An isolated workflow on `feature/horizon-f42ef42` now supplies a working build runner in the same conversation.
The normal release workflow's choice to skip native compilation is preserved; the compatibility workflow performs it explicitly before release.

## Required indexer correction

Horizon f42ef42 adds `#![recursion_limit = "256"]` to its client library.
The previous mixin tool incorrectly treated this Rust compiler builtin as an unexpanded attribute macro and rejected all hooks below it with `E_UNSUPPORTED_MACRO`.
The pinned public [horizon-mixins PR #25](https://github.com/horizon-rust/horizon-mixins/pull/25) distinguishes compiler builtins and known tool attributes from expanding macros, with unit/integration coverage.
Its immutable head is readable and fetchable from the canonical tool repository; it is not yet merged upstream.

The final check uses that exact tool commit with no local patches or rewritten Horizon sources.
Runtime tests, strict clippy, tool regression tests, official plan/apply, real native-client compilation and package validation must all pass.
Caches only speed up recompilation; a failed command propagates through every log pipeline.
The managed executor itself has not been repaired; GitHub Actions is the verified alternative build route.
