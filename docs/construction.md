# Tunnels and bridges

Open **J → Experiments → Baritone** and choose **Tunnel** or **Bridge**. Both modes use actual loaded terrain and ordinary client actions; the server decides whether a break or placement succeeds.

## Tunnel

- Hold a suitable pickaxe with durability remaining and keep an inventory slot free.
- Choose North, East, South or West; a length of 1, 8, 16, 32 or 64; width 1–3; and height 2–3.
- **Floor** enables filling missing floor blocks (hotbar, off hand or inventory; any plain building block, stone-like ones first). Leave it off to stop at gaps. Width two extends one column to the left of the central lane.
- The preview key shows the direction and length without acting; the start key clears each section from top to bottom, confirms each break with the server, then advances.

The floor under the player is not excavated. Containers, unbreakable blocks, falling blocks above the cut, liquids and unavailable chunks stop the task. Unsupported partial block shapes also stop progress.

## Bridge and floor placement

Bridge creates a one-block-wide, level passage in the selected direction. It fills gaps and walks across existing solid floor; it does not clear walls. Choose a length, keep suitable blocks in the hotbar, off hand or inventory (inventory blocks are moved into the hotbar automatically) and press the start key. The character keeps walking on confirmed blocks and sneaks only while placing.

Allowed materials are cobblestone, cobbled deepslate, stone, deepslate, dirt, netherrack, andesite, granite, diorite and stone bricks. Sand, gravel, containers and interactive or hazardous blocks are not selected.

The driver selects a material slot temporarily. It checks a real support face, reach, entity obstruction and the client's dry placement result. Where needed, it approaches the platform edge while sneaking. It waits for the server acknowledgement before walking onto the placed floor. Without a usable support face, materials or confirmation it stops.

Automatic slot selection is restored afterwards if the player has not chosen another slot manually. Changing slots manually during placement pauses the task and preserves the player's choice.

## Pause and reset

Movement, jump, sneak, attack/use or opening a screen during placement pauses construction. Moving the camera hands it to you for three seconds: placement waits, with its time limits held back, and goes on. **Resume** preserves completed distance and waits for in-flight confirmations. After moving away from the passage, start a new task instead of resuming from a distant position. **Stop** cancels the task. Reconnecting or changing worlds/accounts clears it.

The first implementation uses full blocks and a straight level passage. It does not build arbitrary schematics, stairs, towers, farms or fluid-clearing structures. Item pickup remains the ordinary game's behavior.

## Materials

Tunnel floors and bridges accept any plain, solid building block you carry: stone types, dirt, planks, logs, bricks, sandstone, terracotta, concrete and similar. Cheap, plentiful blocks are used first. Shulker boxes, chests and other containers, redstone parts, workstations, TNT, slabs, stairs, walls, ores, storage blocks of metals and gems, obsidian, sand, gravel and concrete powder are never placed.
