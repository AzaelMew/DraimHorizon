# DraimHorizon guide

Use for the owner's `DraimHorizon` repository (https://github.com/DraimCiDo/DraimHorizon).
Facts were read from the repository on 2026-11-22; its own `AGENTS.md` and
`compatibility.json` take precedence over this summary for technical details.

## What it is

A native Rust mod for the Horizon client (Minecraft content), shipped for Horizon
(`.hmod`) and Sunrise (`.smod`) launchers. Bundle ID `draimhorizon`; the settings
namespace stays `claim_borders` (`runtime::SETTINGS_ID`) - never change it with
launcher identity. A separate Python relay lives in `relay/`. Toolchain: Rust pinned
in `rust-toolchain.toml`; Python tools in `tools/`; platform retargeting in
`platforms/horizon` and `platforms/sunrise`; mod code in `mixins/draimhorizon/`.

## Project facts that still apply

- Target the latest official Horizon Nightly resolved to a full commit SHA recorded in
  `compatibility.json`; never silently use main or a stale pin.
- Every completed task that changes tracked content bumps the mod version once
  (`python tools/version.py bump patch|minor`), with a concrete `CHANGELOG.md` entry
  and `docs/release-notes.md`. Do not reuse or move published tags. Include the
  version bump in the plan; it does not authorize release or deployment.
- Keep network, disk scans, and heavy generation off the render/game thread; preserve
  old preferences, archives, waypoints, and explored terrain; never execute translated
  text as a command; keep credentials out of source, logs, and artifacts.
- Use official Lucide icons for interface pictograms; no game assets or extracted
  upstream code in Git.

## Checks (each needs plan approval before running)

`python tools/compatibility.py`, `python tools/version.py check`, `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, relay Python tests, native-client staging with
`tools/sunrise.py`. These download or compile and can be expensive: list the exact
subset in the plan with environment and expected cost. Distinguish unit tests,
native compilation, and live-server checks in reports.

## Playbook overrides for this project

The playbook wins on process: checks only as listed in the approved plan (including
the compatibility check the project asks for before every task); no extra agents or
`.codex` multi-agent roles unless the owner asks; commit and push the task branch and
keep one draft PR automatically; releases, tags, and deployments always need explicit
approval. English for code, commits, and PRs.
