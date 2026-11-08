# Farming and schematic building

Open **J → Experiments → Baritone** and accept the experimental warning. These are native Rust features, not the Java Baritone library. Manual movement, attack/use, camera adjustments during interaction, or opening a screen pauses the relevant action. Use **Resume** after correcting a problem; **Stop** clears the job. Jobs are session-only and reset when the server, account or dimension changes.

## Global automation keys

In **Options → Mods → DraimHorizon**, assign **Baritone: preview**, **Baritone: start / resume**, **Baritone: pause** and **Baritone: stop**. They replace the former on-screen buttons in all tabs. By default they are **Numpad 1** (preview), **Numpad 2** (start / resume), **Numpad 3** (pause) and **Numpad 4** (stop); the numeric keypad is unused by the game. Change them in the same page, for example on a laptop without a keypad. Horizon saves them with the other native-mod bindings. The keys also work while the menu is open on a Baritone tab (unless you are typing in a field); Start then closes the menu and begins the task. Choose and configure a tab, then press start; press preview first to see the route without acting. Pressing Start again while a task is active does not restart it.

Bindings are ignored while a screen or text editor is open, the window is unfocused, modifiers are held, or experimental consent is absent. A binding shared with another automation action, an enabled native mod or a gameplay key does not trigger automation; choose a unique key. Native key repeats and key releases do not start actions.

Navigation executes dig and bridge routes step by step: it breaks blocks in the way and places a block under a gap, re-planning after each route and giving up with a visible reason after a bounded number of failures.

## With horizon-baritone

[horizon-baritone](https://github.com/luhnox/horizon-baritone) is a separate native mod, a Rust port of Baritone 1.21.4 (LGPL-3.0). Install it in the launcher's native mods next to DraimHorizon; both are built into the same Horizon Nightly, so use the Nightly both support. When it is on, the Baritone tabs hand their task to it instead of the built-in drivers:

| Tab | Baritone command |
| --- | --- |
| Navigation (coordinates or waypoint) | `goto x y z`; the preview key sets `goal x y z` |
| Navigation (player) | `follow player <name>` |
| Mining | `mine <limit> <blocks of every selected target>` |
| Tunnel (without Floor) | `tunnel <height> <width> <length>` in the facing direction |
| Farm | `farm <radius>` (Baritone farms every crop it knows; the crop choice is ignored) |
| Schematics (rotation 0°) | the file is copied to Baritone's `schematics` folder, then `build <file> x y z` |

Pause sends `pause`, Start after a pause sends `resume`, Stop sends `cancel`. The status line shows `Baritone · <command>`. Bridge, Drain, Tunnel with Floor and rotated schematics keep the built-in drivers. Commands go through Horizon's native command hook as `/baritone <line>`: if no mod runs them, nothing is sent to the server and the tab reports that Baritone did not accept the command. Baritone's own settings (`#set ...`, for example `allowPlace`, `allowBreak`, `replantCrops`) apply.

## Drain

Open **Baritone → Drain**, choose a radius (3, 6, 9 or 12), carry building blocks (hotbar, off hand or inventory) and press the start key.

- **With sand or gravel** (used first when you have them) any depth works. The bot drops the block into the surface water cell next to a solid block, for example the edge of the shore or a bridge you stand on. The block sinks and fills the column from the bottom; the bot repeats it until the column reaches the surface, then moves on along the new surface. Each drop waits about 1.5 s for the block to sink.
- **With other blocks** (stone, dirt, …) only cells resting on something solid are filled, bottom-up, so the floor must be within reach (about 4 blocks below you). For deeper water the task stops and asks for sand or gravel.

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
