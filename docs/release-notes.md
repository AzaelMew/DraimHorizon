**DraimHorizon 0.39.10 - Faster Baritone**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- Route planning several times faster (a 140-block route 65 ms to 14 ms; a whole benchmark set 1089 ms to under 200 ms).
- The miner finds reachable blocks quickly even when the nearest are out of reach.
- Shorter waits between route legs (100 ms), when following (250 ms) and between tool swaps (250 ms); no pause when the planner is briefly busy.
- Builds on 0.39.9.

Native client compiled against the pinned Nightly. Not verified in game.
