# Experimental navigation

Open **J → Experiments → Baritone** and accept the experimental warning if prompted. Choose **Navigation** or **Mining**.

1. Enter space-separated **X Y Z**, click **Waypoint** to cycle through saved destinations in your current dimension, or click **Player** to choose a nearby loaded player. The selectors are mutually exclusive and return to coordinate input after the last entry. Editing XYZ clears both selections. Hover a selector to read its full name.
2. **Preview** calculates and draws a route without moving. **Go** calculates a route, closes the menu and starts walking.
3. Movement keys, jump, sneak or opening a screen pause navigation. Use **Resume** to calculate a fresh route from your current position; **Stop** clears the destination and route.

With a player selected, **Go** becomes **Follow**. The navigator tracks that player's UUID without requiring the relay, maintains a short distance and recalculates as they move. It stops moving immediately if the player is no longer loaded nearby. It waits up to 10 seconds for them to reappear; after that, press **Resume** once they are visible again. Manual pause always requires Resume and never restarts on its own.

The line shows the remaining planned segment. The HUD shows status and straight-line distance to the destination. Long routes are calculated in segments as real chunks become available. The camera remains under your control.

This first implementation walks on full blocks, jumps up one block and descends one block. It treats slabs, stairs, doors and other partial shapes as obstacles. It avoids fluids and hazardous blocks; unavailable chunks are never assumed to be empty. Changed blocks or a stalled character trigger replanning, with at most three recovery attempts near one location before stopping. Planning waits for the player to land after a jump. If no walkable route exists, try another destination.

Navigation resets after disconnecting, changing dimensions/worlds/accounts or disabling the mod. It never starts automatically on joining. It does not use Seed Map predictions for movement.

## Ore mining

In **Baritone → Mining**, choose an ore and a block limit (1, 8, 16, 32 or 64). **Preview** searches and displays a route without attacking. Hold a suitable pickaxe, leave a free inventory slot and click **Mine** to start. The camera turns toward each target when breaking it; manual camera adjustment, attack/use input, movement or an open screen pauses the task.

The miner looks for reachable exposed ore within 16 blocks horizontally and 12 vertically in loaded chunks. It does not tunnel through other blocks or use Seed Map predictions. Blocks beneath the player's support area, ore beside fluids/unknown terrain, and ore under falling blocks are excluded. It stops when the limit is reached, no accessible ore remains, the tool is unsuitable or nearly broken, the inventory needs a free slot, or the server refuses or times out a break.

The counter measures server-confirmed ore blocks, not dropped items. Item pickup uses ordinary game behavior. Pause/Resume preserves progress within the current session; disconnecting or changing worlds resets the task.

This is an independent native Rust navigator. It does not load the Java Baritone library and currently has no block placement, tunneling, farming, schematic building or flight automation.
