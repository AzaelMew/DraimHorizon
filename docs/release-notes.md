**DraimHorizon 0.40.3 - Baritone navigation**

Target: official Horizon Nightly source `9f42064` (unchanged).

- Adds the pure core of the Baritone 1.21.4 port (A*, costs, all movements, executors) under `mixins/draimhorizon/baritone/`, LGPL-3.0 with authorship and license texts; the rest stays MIT. Wired into the game through the tab below.
- A Baritone tab appears under Experiments in the J menu (same experiments consent). The Baritone section inside Experiments is renamed Automation.

- The Baritone port is now built into the mod: the Baritone tab has a switch (off by default) and buttons: Go to X/Y/Z, Mine a block, Farm, Explore, Follow, Come, Pause, Resume and Cancel. The `#` commands also work while it is on. Existing navigation, mining and the farm are unchanged.
- Baritone tab: Build and Tunnel buttons, a settings section, and a status line with the task and time left. An engine switch (off by default) lets the farm and mining walk with Baritone's planner; their own logic is unchanged.

Verification: Clippy with warnings denied, cargo fmt and the workspace tests pass. The pinned native client was staged and `cargo check`ed against Nightly 9f42064, and the packaged bundle passes `cargo horizon-mod check`. Nothing was tried in a running game.
