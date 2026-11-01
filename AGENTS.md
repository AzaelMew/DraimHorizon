# HorizonMod

This repository is the workspace for a future mod for https://github.com/horizon-rust/horizon.
Communicate with the user in Russian unless they request another language.
The mod feature set and content/native format have not been chosen. Do not invent a product scaffold.

## Sources and workflows

- Read `.agents/skills/horizon-modding/SKILL.md` for Horizon work.
- ECC is installed as the native Codex plugin `ecc@ecc`; do not also run the legacy sync or duplicate its skills here.
- Use ECC `rust-patterns`, `rust-testing`, `tdd-workflow`, `security-review`, and `verification-loop` when relevant. Load only the skills needed for the task.
- `.ecc/upstream-lock.json` records the ECC installation and the Horizon revision inspected during setup. Treat that Horizon revision as the initial reference, not a guarantee of compatibility with future releases.
- Project roles live in `.codex/agents/`: explorer, reviewer, docs_researcher, rust_developer. Delegate independent tasks when useful and supported by the current session; avoid overlapping writes. Higher-priority session restrictions take precedence.

## Implementation

- Check upstream docs and actual source before selecting a hook, manifest key, dependency or native build command.
- Content mods use `.hmod` and content API 0.2; native Rust mods use horizon-mixins bundles and `.hnmod`. Fabric/NeoForge Java code does not execute in Horizon.
- Prefer stable native hook points; record the minimum hook API actually used. Arbitrary source mixins must pin an exact Horizon revision.
- Keep gameplay mutations on the authoritative server; consider multiplayer and persistence when relevant.
- Use idiomatic Rust, explicit errors, small changes and justified dependencies. Do not add a Cargo project until the mod format requires one.
- Never commit prepared Minecraft resources, decompiled game sources, credentials, player saves, build outputs or signing private keys.

## Verification

- For a Rust crate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and relevant `cargo test` targets.
- For a content mod: `cargo horizon-mod build`, then `cargo horizon-mod check <artifact>`.
- For a native mod: follow the pinned template and horizon-mixins build process; validate the resulting `.hnmod` with `cargo horizon-mod check <artifact>`. Content `build` does not build native bundles.
- Report unavailable tools/resources and checks that were not run. Setup-only changes do not require a game build.
- Do not publish releases or registry submissions without an explicit request.
