**DraimHorizon 0.39.19 - farm fixes, auto-sell, Fortune harvesting, villager auto-trade**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- `/farm pos1`, `/farm pos2` and `/farm save` work on servers with their own `/farm`; new `/dfarm` alias.
- No more "Material missing" with seeds in the inventory.
- The farm no longer walks round a crop before harvesting it.
- A farm resumed after a server restart keeps its original square.
- Auto-sell sells the harvest at once with `/sell hand` instead of storing it.
- Villager auto-trade: Shift-click an offer to repeat the trade until the price or the stock runs out.
- The farm status line shows how much was sold and for how much.
- Carrots and potatoes keep a stack for replanting when selling.
- Harvesting prefers a Fortune tool for bigger drops.

Native client compiled against the pinned Nightly. Not verified in game.
