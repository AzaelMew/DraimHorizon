**DraimHorizon 0.40.3 - Baritone navigation core (step 1)**

Target: official Horizon Nightly source unchanged.

- Adds the pure core of the Baritone 1.21.4 port (A*, costs, all movements, executors) under `mixins/draimhorizon/baritone/`, LGPL-3.0 with authorship and license texts; the rest stays MIT. Tested in the test crate only; not wired into the game yet, so nothing changes in play.
- A Baritone tab appears under Experiments in the J menu (same experiments consent). The Baritone section inside Experiments is renamed Automation.

Verification: Clippy with warnings denied, cargo fmt and the workspace tests pass. The pinned native client was staged and `cargo check`ed successfully; nothing was tried in a running game.
