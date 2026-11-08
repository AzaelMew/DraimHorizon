# Experimental navigation

Open **J → Experiments → Baritone** and accept the experimental warning if prompted. Choose **Navigation**, **Mining**, **Tunnel**, **Bridge**, **Farm**, **Drain** or **Schematics**. [Tunnel and bridge controls](construction.md).

1. Enter **X**, **Y** and **Z** in three separate fields (pasting `x y z` fills all three), click **Waypoint** to cycle through saved destinations in your current dimension, or click **Player** to choose a nearby loaded player. The selectors are mutually exclusive and return to coordinate input after the last entry. Editing a coordinate clears both selections.
2. There are no action buttons. Assign the four **Baritone** keys (preview, start/resume, pause, stop) in **Options → Mods → DraimHorizon**; they work from every Baritone tab. See [automation](automation.md#global-automation-keys).
3. Preview draws a route without moving. Start calculates a route and starts walking. Movement keys, jump, sneak or opening a screen pause navigation; Start continues with a fresh route and Stop clears the destination.

With a player selected, **Go** becomes **Follow**. The navigator tracks that player's UUID without requiring the relay, maintains a short distance and recalculates as they move. It stops moving immediately if the player is no longer loaded nearby. It waits up to 10 seconds for them to reappear; after that, press the start key once they are visible again. Manual pause always requires the start key and never restarts on its own.

The line shows the remaining planned segment. The HUD shows status and straight-line distance to the destination. Long routes are calculated in segments as real chunks become available. The camera remains under your control.

This first implementation walks on full blocks, jumps up one block and descends one block. It treats slabs, stairs, doors and other partial shapes as obstacles. It avoids fluids and hazardous blocks; unavailable chunks are never assumed to be empty. Changed blocks or a stalled character trigger replanning, with at most three recovery attempts near one location before stopping. Planning waits for the player to land after a jump. When no walkable route exists, or the character stalls, the navigator plans again with permission to **break** blocks in the way and to **place** a block under a gap (this needs a suitable tool and building blocks in the hotbar, inventory or off hand). Each new route is calculated from the current position; after a few failed attempts it stops with a reason. Vertical pillaring is not supported.

Navigation resets after disconnecting, changing dimensions/worlds/accounts or disabling the mod. It never starts automatically on joining. It does not use Seed Map predictions for movement.

## Ore mining

In **Baritone → Mining**, toggle what to collect in the grid of targets, like the structure filters on the map; several can be selected and the nearest of any of them is mined. Page through the grid with the **Selected … →** button and choose a block limit (1, 8, 16, 32 or 64). Besides the ten highlighted ores the list has stone, cobblestone, dirt, sand, gravel, clay, wood, obsidian, glowstone, nether stone, sandstone, end stone, pumpkins and melons. The preview key searches and displays a route without attacking. Hold a suitable tool (pickaxe for ore and stone, axe for wood and pumpkins, shovel for dirt, sand and gravel), leave a free inventory slot and press the start key. The camera turns toward each target when breaking it; manual camera adjustment, attack/use input, movement or an open screen pauses the task.

The miner uses the same ore table as the **Ores** experiment: ore marks it shows are tried first, then loaded chunks are scanned (radius 24). Buried ore is reached by digging a route to it. Seed Map predictions are not used. Blocks beneath the player's support area, ore beside fluids/unknown terrain, and ore under falling blocks are excluded. It stops when the limit is reached, no accessible ore remains, the tool is unsuitable or nearly broken, the inventory needs a free slot, or the server refuses or times out a break.

The counter measures server-confirmed ore blocks, not dropped items. Item pickup uses ordinary game behavior. Pause/Resume preserves progress within the current session; disconnecting or changing worlds resets the task.

This is an independent native Rust navigator. It does not load the Java Baritone library. Straight tunnels and floor placement are described in the construction guide; [Farming and schematic building](automation.md) have their own controls and limits. Flight automation is not implemented.
