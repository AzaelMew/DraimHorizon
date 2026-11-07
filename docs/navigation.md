# Experimental navigation

Open **J → Experiments → Baritone** and accept the experimental warning if prompted.

1. Enter space-separated **X Y Z**, or click **Waypoint** to cycle through saved destinations in your current dimension. After the last waypoint, the selector returns to coordinate input. Editing XYZ also clears the selected waypoint.
2. **Preview** calculates and draws a route without moving. **Go** calculates a route, closes the menu and starts walking.
3. Movement keys, jump, sneak or opening a screen pause navigation. Use **Resume** to calculate a fresh route from your current position; **Stop** clears the destination and route.

The line shows the remaining planned segment. The HUD shows status and straight-line distance to the destination. Long routes are calculated in segments as real chunks become available. The camera remains under your control.

This first implementation walks on full blocks, jumps up one block and descends one block. It treats slabs, stairs, doors and other partial shapes as obstacles. It avoids fluids and hazardous blocks; unavailable chunks are never assumed to be empty. Changed blocks or a stalled character pause movement. If no walkable route exists, try another destination.

Navigation resets after disconnecting, changing dimensions/worlds/accounts or disabling the mod. It never starts automatically on joining. It does not use Seed Map predictions for movement.

This is an independent native Rust navigator. It does not load the Java Baritone library and currently has no mining, block placement, player following, farming, schematic building or flight automation.
