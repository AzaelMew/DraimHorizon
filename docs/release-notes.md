**DraimHorizon 0.41.2 — Horizon & Sunrise (pre-release)**

This pre-release replaces 0.41.1 with the build that was actually tested: the 0.41.1 assets were
packed from an earlier commit, before the Sunrise `.smod` work, so its `.smod` was the previous flat
bundle. `v0.41.1` stays published but is superseded. No gameplay behavior changed.

- **Sunrise:** `draimhorizons-0.41.2.smod`. In Sunrise Launcher 1.0.1 or newer choose **Mods → Add from GitHub**
  and paste `https://github.com/DraimCiDo/DraimHorizon/releases/tag/v0.41.2`, then rebuild native mods. Built and
  checked in game on Sunrise Nightly `2d81263`. The mod keeps its own data folder on Sunrise, so settings and
  waypoints start empty; key labels and the Options → Mods text are English.
- **Horizon:** `draimhorizons-0.41.2.hnmod` for Horizon Nightly `9f42064`, with a launcher that still builds `.hnmod`
  mods (0.2.69 or older). Horizon Launcher 0.2.70+ needs a published Horizon mod SDK, which the current Nightly
  does not have yet, so no `.hmod` is attached.
- Every asset has a `.sha256` next to it. The relay archive `draimhorizons-relay-0.41.2.tar.gz` carries no secrets.
