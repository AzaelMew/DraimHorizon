**DraimHorizon 0.40.6 - Ores experiment: live portal highlight and icons**

Target: official Horizon Nightly source `9f42064` (unchanged).

- The highlight reads all loaded chunks in range and keeps its marks while the next scan runs, so it follows you while flying. Chests, Ender chests and portals are capped so chests cannot hide portals.
- The Chests, Ender chest, End portal and Nether portal tiles show a chest, an Ender chest, a portal frame and obsidian.

Verification: Clippy with warnings denied, cargo fmt and the workspace tests pass. A running game and the native client build were not checked.
