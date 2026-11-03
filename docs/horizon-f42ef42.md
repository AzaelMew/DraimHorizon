# Horizon f42ef42 migration (in progress)

Requested source: `f42ef42ef6ceccb3c6a20bea2481d87949a51c81`.
The source hashes in compatibility.json were recomputed on a GitHub runner from that exact commit.
Observed native hooks API: 359. The mod uses an exact revision pin.

The task's managed executor cannot start because its GitHub binding reports `github_connector_not_configured`, while the GitHub app itself can read and write this repository. An isolated workflow on `feature/horizon-f42ef42` provides a working build runner without a new chat or changing release CI policy.

## Current official-tool blocker

The pinned horizon-mixins indexer rejects Horizon's new crate-level `#![recursion_limit = "256"]` as an unexpanded attribute macro:
`E_UNSUPPORTED_MACRO: target module has an unexpanded attribute macro` at the chat hook.
This is a Rust compiler builtin; it does not expand or replace source items.
The current horizon-mixins main (`ddd9700b21997c95d95ac6ee5ecafe3830cf622a`) still has the same builtin-attribute allowlist.

`tools/horizon-mixins-recursion-limit.patch` is a minimal **diagnostic-only** allowlist correction for the pinned tool.
The workflow first runs the unmodified official tool and preserves its actual failure.
Only then may it apply the explicit patch in the runner's disposable tool checkout to expose subsequent Rust compatibility errors.
Horizon source is never rewritten or stripped of attributes.
The final job remains failed if a tool patch was required, and no release package is validated or published on that path.
Do not release or merge the migration until unpatched launcher-compatible staging and native compilation pass.
