# Experimental navigation

Open **J → Experiments → Baritone** and accept the experimental warning if prompted.

1. Enter space-separated **X Y Z**, click **Waypoint** to cycle through saved destinations in your current dimension, or click **Player** to choose a nearby loaded player. The selectors are mutually exclusive and return to coordinate input after the last entry. Editing XYZ clears both selections. Hover a selector to read its full name.
2. **Preview** calculates and draws a route without moving. **Go** calculates a route, closes the menu and starts walking.
3. Movement keys, jump, sneak or opening a screen pause navigation. Use **Resume** to calculate a fresh route from your current position; **Stop** clears the destination and route.

With a player selected, **Go** becomes **Follow**. The navigator tracks that player's UUID without requiring the relay, maintains a short distance and recalculates as they move. It stops moving immediately if the player is no longer loaded nearby. It waits up to 10 seconds for them to reappear; after that, press **Resume** once they are visible again. Manual pause always requires Resume and never restarts on its own.

The line shows the remaining planned segment. The HUD shows status and straight-line distance to the destination. Long routes are calculated in segments as real chunks become available. The camera remains under your control.

This first implementation walks on full blocks, jumps up one block and descends one block. It treats slabs, stairs, doors and other partial shapes as obstacles. It avoids fluids and hazardous blocks; unavailable chunks are never assumed to be empty. Changed blocks or a stalled character trigger replanning, with at most three recovery attempts near one location before stopping. Planning waits for the player to land after a jump. If no walkable route exists, try another destination.

Navigation resets after disconnecting, changing dimensions/worlds/accounts or disabling the mod. It never starts automatically on joining. It does not use Seed Map predictions for movement.

This is an independent native Rust navigator. It does not load the Java Baritone library and currently has no mining, block placement, farming, schematic building or flight automation.
