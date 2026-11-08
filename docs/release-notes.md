**DraimHorizon 0.29.6 - Drain from the shore edge (unreleased)**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- Drain no longer stops at zero with "no reachable face" when filling deep water with sand or gravel from the shore: it walks over the shore block's edge, sneaking, and clicks the wall that faces the water.
- Drain uses its own driver, so it works even when horizon-baritone is not updated for the current Nightly.

Not compiled against the real client and not verified in game. No installation or publication is implied.
