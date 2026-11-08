**DraimHorizon 0.29.2 — build fix (unreleased)**

Target: official Horizon Nightly source `2ebf00a0281c592f79191e76fce0e7d522c6f425` (Java 26.3).

- Fix the project not building: `rust-toolchain.toml` pinned Rust 1.83.0, but the crates require edition 2024 and Rust 1.95 or newer. The toolchain is now 1.99.1, matching `docs/development.md` and the release workflow.
- The Horizon compatibility workflow now installs and uses Rust 1.99.1 as well.

No gameplay, settings or data changes. Bundle ID remains `draimhorizon`; persisted data remains under `claim_borders`. No installation or publication is implied by this build.
