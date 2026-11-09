# Farming and schematic building

Open **J → Experiments → Baritone** and accept the experimental warning. These are native Rust features, not the Java Baritone library. Manual movement, attack/use, camera adjustments during interaction, or opening a screen pauses the relevant action. Use **Resume** after correcting a problem; **Stop** clears the job. Jobs are session-only and reset when the server, account or dimension changes.

## Global automation keys

In **Options → Mods → DraimHorizon**, assign **Baritone: preview**, **Baritone: start / resume**, **Baritone: pause** and **Baritone: stop**. They replace the former on-screen buttons in all tabs. By default they are **Numpad 1** (preview), **Numpad 2** (start / resume), **Numpad 3** (pause) and **Numpad 4** (stop); the numeric keypad is unused by the game. Change them in the same page, for example on a laptop without a keypad. Horizon saves them with the other native-mod bindings. The keys also work while the menu is open on a Baritone tab (unless you are typing in a field); Start then closes the menu and begins the task. Choose and configure a tab, then press start; press preview first to see the route without acting. Pressing Start again while a task is active does not restart it.

A running task is not paused by opening the inventory, the chat, the mod's menu or the map: it keeps walking, and a hit, a placed block or a bite waits for the screen to close. Escape's pause menu, signs, books and your own movement, jump, sneak or attack input still pause it.

Bindings are ignored while a screen or text editor is open, the window is unfocused, modifiers are held, or experimental consent is absent. A binding shared with another automation action, an enabled native mod or a gameplay key does not trigger automation; choose a unique key. Native key repeats and key releases do not start actions.

Navigation executes dig and bridge routes step by step: it breaks blocks in the way and places a block under a gap, re-planning after each route and giving up with a visible reason after a bounded number of failures.

### Eating

While a task runs and the food bar is at 7 shanks or below, the mod eats on its own: it takes the best fitting safe food from the hotbar or inventory (never rotten flesh, spider eyes, raw chicken, golden apples or similar), holds the use button until the bar rises and puts the previous slot back. The **Eat** switch next to the status line turns it off; it is on by default.

### Mining storage

The Mining tab has the same **Storage** button: aim at a chest, barrel or shulker box and press it (it is the same container as the farm's, set per world). When the inventory is full, meaning every slot is taken and every stack of the mined kind is at its limit, the miner walks to the storage, opens it, shift-clicks across everything it does not need and walks back to where it stopped. It keeps tools and armour (anything that wears), food, a totem and one stack of the mined block for building; sand laid one by one across the inventory does not count as full. If the container is full or unreachable the miner idles and tries again after one to two minutes.

### Farm containers

On the Farm tab, pick the crops from the icons (several at once; one always stays selected). To the right of the icons are three buttons, one per container: **Harvest**, **Seeds** and **Meal** (bone meal). To set one, aim the crosshair at a chest, trapped chest, barrel or shulker box and press its button (press it with the crosshair on anything else to clear it). The containers belong to the current world and are never simply the nearest ones.

- **Harvest** takes the crops (wheat, beetroot, and half of the carrots and potatoes).
- **Seeds** takes the spare seeds and the other half of the carrots and potatoes, which are both a seed and a crop, so each stack goes to whichever container has had less. When the farm runs out of seeds it takes them from here. With no seeds container set, everything goes to the harvest one, as before.
- **Meal** holds bone meal and bone blocks. With "Bone meal: on", crops that are still growing are fed bone meal by a click on each (up to eight doses, until it ripens); the farm fetches meal when the stock is under eight. If there is only bone meal in the container, it takes up to 64; if there are bone blocks, it takes the stack it needs (never more than 64 blocks) and crafts them into meal in the inventory (a block makes nine), never past 64 meal, so the inventory is not littered.

A trip to a container walks there in legs of up to 24 blocks, so a container 40 blocks or more from the beds is reached; it opens it, moves things with shift-clicks, closes it and walks back to the beds. A farm that goes idle (waiting for crops to ripen, no seeds, no light) walks to the safest cell near the beds after six idle seconds: lit, with as few open sides as possible and a roof if there is one, not on farmland. It leaves as soon as something ripens, can be sown or can be fed. Pause and Start again picks up from a clean slate and looks at the beds at once. A player who walks into the bot is stepped around, and a shove off the route plans it again instead of stopping it.

## Idle instead of stopping

A job does not stop because something is missing or went wrong. It stays up, shows "Idle: <reason>" in the status line and looks again after a few seconds: Mining waits for a suitable tool or a free slot, Tunnel and Bridge wait 3 s and try again, Drain waits for sand or gravel and for a route, Farm waits for ripe crops, light, seeds or a route, schematics for blocks or a route. Only your own stop key ends a job. A progress watchdog moves a job that does nothing for 20 s on to another target.

## Shulker box preview

Not a Baritone feature. Hover a shulker box that holds something (inventory, chest, creative tabs, recipe book, chat) and its tooltip shows the contents as a 9x3 grid, tinted with the dye colour of the box. Switch it off in J > Settings > General ("Shulker preview").

## Drain

Open **Baritone → Drain**, choose a radius (3, 6, 9 or 12), carry sand, red sand or gravel (hotbar, off hand or inventory) and press the start key. Drain places only those blocks, never an ordinary block that would be left standing in the water.

- The bot drops the block into the surface water cell next to a solid block, for example the edge of the shore or a pier you stand on. The block sinks and fills the column from the bottom; the bot stays on that column, dropping the next block when the last one has sunk, until the column reaches the surface, and only then moves on to the next column. Each drop waits under a second for the block to sink.
- The wall of the shore facing the water cannot be clicked from inland, so the bot walks to the edge of the shore block and sneaks over it, like when bridging, before dropping the block. If it falls into the water, it swims back to dry ground and goes on.

**Patterns.** The **Pattern** button switches between a fill (the default), lines and a grid, and **Spacing** sets the distance between the lines (3 to 8). Lines run along the x axis every N blocks, starting with the row you stand on; the grid adds columns, so the water is cut into squares. A pattern grows along its own blocks: the bot walks out along the line it has laid, sneaking, and places the next column from its end.

Stand on a block next to the water, not in it. If the next cell is out of reach the bot walks to a standable spot nearby; if none exists it stops with a message. It finishes after several empty scans in a row and reports how many cells were filled. Water and seagrass are replaced; kelp is not removed. The task stops at 4000 placements.

## Farm

Choose **Wheat**, **Carrots**, **Potatoes** or **Beetroot**, a radius (3, 6, 9 or 12), a harvest limit (16, 32, 64 or 0 for continuous operation), and **Replant**. Stand at crop height beside prepared farmland. **Preview** marks the area without acting; **Start** closes the menu and works within the square centered on that starting position, at that fixed height.

Only fully grown crops of the selected kind are harvested. The limit counts server-confirmed harvested plants, not drops. With Replant enabled, keep the matching seeds/carrots/potatoes in the inventory before harvesting; empty farmland in the area is planted too. Leave a free inventory slot. Item pickup uses normal Minecraft behavior. When no targets are ready, the farm waits for growth.

The farm uses level walking routes through loaded, safe cells. Crops obstruct route planning, so provide walkways within normal interaction reach. It does not till soil, add water, use bone meal, replenish inventory, or automate other crops.

## Schematics

1. Click the file button (or **Open folder**): with no files yet it creates the schematic folder, opens it in the system file manager and copies its path. Put `.schem` or `.litematic` files directly in that folder (`claim_borders/schematics` under Horizon's mod data directory). [Small authored examples](../examples/schematics/) are available in both formats.
2. Click the file button again: a list of the files opens under it (8 per page, mouse wheel or arrows to page, last entry opens the folder); click a file to select it. Enter integer **X Y Z** and choose rotation. Origin means the file's stored origin; Sponge offsets and Litematica region positions are retained. Rotation is clockwise around that origin when viewed from above.
3. **Preview** loads the file without moving or placing. The next 256 schematic cells within 96 blocks are shown as outlines. **Materials → clipboard** copies the total block requirements; double slabs count twice, and upper door/bed halves do not add a second item. This is a total, not an inventory deficit report.
4. Keep the required items in the hotbar, off hand or inventory. **Build** reloads the selected file and starts from its lowest layer. **Resume** starts an already loaded preview or continues a paused job.

Matching blocks are skipped. Incorrect occupied cells pause the job and show their coordinates; clear them yourself and Resume. The builder approaches loaded safe standing positions, turns toward an available face, checks ordinary placement prediction and waits for the server acknowledgement before advancing. Orientation and secondary cells of multi-block placements must match the file. A face/state the normal placement driver cannot reproduce pauses the job rather than substituting another block.

This first builder does not remove blocks, clear air volumes, create scaffolding, fly, fetch supplies, copy inventories, spawn entities, or restore block-entity NBT (containers, signs, etc.). Block-entity blocks needing placement are unsupported. Water/fluid placement and unavailable/older block states can require manual work. It is intended for reachable builds from prepared terrain, not unattended construction of arbitrary structures.

## Import limits and formats

- GZip input up to 8 MiB, decoded NBT up to 32 MiB; one pending import at a time.
- At most 128 blocks per dimension, 262,144 total volume and 32,768 non-air cells; Litematica allows up to 16 regions. Region offsets are limited to ±4096.
- Sponge/WorldEdit **v2 and v3** palettes/VarInts; Litematica **v4–v7** palettes and continuous packed states, including negative region sizes and overlapping identical cells.
- Newer Minecraft data versions, conflicting overlaps, malformed NBT/palettes and paths outside the schematic directory are rejected. No DataFixer migration is performed.
- Files are decoded off the game/render thread. Results are applied only to the originating server/account/dimension/resource generation.

Format references: [Sponge specification](https://github.com/SpongePowered/Schematic-Specification), [Litematica implementation](https://github.com/maruohon/litematica). Test fixtures and demo schematics are authored by this project; regenerate them with `python tools/make_schematic_fixtures.py`.
