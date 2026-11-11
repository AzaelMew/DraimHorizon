**DraimHorizon 0.39.8 - Schematic ghost blocks**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- Schematic previews draw each missing block as a translucent ghost of the real block, facing the way it is to be placed; wrong blocks stay red.
- Ghosts hide behind terrain, are built near the player section by section within a per-tick budget and rebuild only what changed.
- Ghost rendering follows Horizonmatica by keksi2k (MIT License).

Native client compiled against the pinned Nightly. Not verified in game.
