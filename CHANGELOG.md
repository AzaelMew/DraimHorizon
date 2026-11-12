# История версий

## 0.39.19

- Farm plot commands work on servers with their own `/farm` command. The server's suggestions took the Enter key and the line went to the server, so `/farm pos1`, `/farm pos2` and `/farm save` did nothing. The mod now also catches the line in the native command hook, and a new `/dfarm` alias (offered by completion) never clashes with a server command.
- The farm no longer stops with "Material missing (hotbar, off hand, inventory)" with seeds in the inventory. Each move of seeds from the inventory to the hotbar counted against a limit of 24 for the whole job, so after a while no more seeds were brought over. The limit now counts only moves in a row that bring nothing.
- The farm no longer walks round a crop it is about to harvest. It aimed at the middle of the block, above the top of short ripe crops (carrots, potatoes, beetroot), so the ray met a neighbouring crop or the farmland and the farm went to the next spot round the crop. It now aims low in the crop and tries other points of it from where it stands before it moves.
- A farm resumed after a server restart keeps the square it was started on. Without marked plots it took the square round wherever the bot was when the server went down.
- Farm auto-sell sells the harvest at once instead of storing it. Between two crops, once a stack of 32 has gathered or there is nothing to harvest, the crops' yield is sold with `/sell hand` and not carried to the harvest chest. When the command does not work, selling stops for five minutes and the harvest goes to the chest.
- Villager auto-trade: Shift-click an offer in a villager's trading window to repeat the trade for as long as it goes (the price from the whole inventory, until the villager's stock runs out). Each round is the game's own: the offer is selected, then the result is taken with a shift-click. It stops when nothing more is paid, the offer sells out or the window closes.
- The farm's status line at the top shows what it sold: how many items (counted from the hand going down after each `/sell hand`) and, when the server sends its "Sold … for …" messages, the money they brought since the job started, e.g. "Farm: 242 · sold 640 (+1 280.00)".
- Selling keeps the seeds of crops that are their own seed: carrots and potatoes keep a stack (64) for sowing; whole stacks above it are sold, the smallest first. Before, auto-sell could sell the carrots the farm needed to replant.
- The farm harvests with the item with the most Fortune when there is one (selected, from the hotbar, or swapped in from the inventory), so crops drop more. Breaking a crop wears no tool. The seed fetch no longer swaps a Fortune tool out of the hotbar, and the Fortune swap prefers a slot without seeds, so the two never trade places at every crop.

## 0.39.18

- Routes that break and place blocks no longer stand still. A block the route's dig could not take (the miner's safety checks) left the dig "searching" for good, so the route waited until its time ran out; it now ends at once and the route is planned again. A full inventory no longer stops a route's dig (the drop stays on the ground) nor starts a storage trip in the middle of the route. A step that makes no progress is planned again after 20 seconds (45 before).
- Elytra: the jump presses that spread them were taken for the player's own keys, and the task paused in mid-air. Falling to spread them now does nothing else that tick, so the glide goes on to the landing without a pause.
- Jumps across gaps: the walker backs up to the rear of the block it jumps from, sprints at the gap and jumps at the very edge; it jumped a quarter of a block in, from a standstill, and fell short of three-block gaps. The keys follow the head as on any step, so the run goes the right way whatever it faces.
- Drops and jumps are preferred to long ways round: each block of drop past three costs a step and a half (it was four steps), within the safe height; a jump across a gap costs less too. Stairs right on the way are still taken over a drop.
- Mining is faster and goes on longer. A block that stays gone for 0.7 s counts as broken without waiting for the server's answer (which came late on a busy server, and the miner stood for five seconds); no answer at all is a refusal after 2.5 s (5 s). The job ends only after 32 blocks in a row it cannot reach, not 32 over its whole length (tree tops out of reach made it say no ore was found with logs all round). Logs with a side open to the air are mined before ones buried in foliage.
- Farm: in a marked plot all plain soil (dirt, grass, path) with air above is tilled; a new plot had no farmland to go by and was tilled only a little at each pass. Each part of the field gets only its own crop (another crop no longer stands in, which sowed the field with whatever seeds were at hand). A trip for seeds that fails counts as finding none for a while, so the farm stops asking for them again and again. The field is looked at again a fifth of a second after a crop is done (half a second before).

## 0.39.17

- Large `.litematic` (and `.schem`) files are read: up to 256 blocks to a side, 4 194 304 cells and 262 144 blocks that are not air (it was 128, 64³ and 32 768, and the preview of a larger build stayed empty). A file that cannot be read now says why, in the previews list and on the preview panel, and Refresh in the Schematics tab reads it again; it used to be dropped without a word.
- Schematic previews are shared with friends. The player's shown previews of the world (the five placed last) are listed to every friend online over the encrypted relay as `[DHS1]` frames, again when they are moved, turned, hidden or removed and when a friend comes online; a friend without the file asks for it and gets it in pieces (600 bytes a frame, four a second, files up to 384 KiB), checked against its hash and kept by content in `shared-schematics/`, so the same file is fetched once. Friends see the preview (ghost blocks and box) where the owner placed it, in the same dimension, and list it under the previews of the world with who placed it and how much of the file has come; they can show or hide it on their own screen (kept across sessions), and only the owner moves, turns, hides or removes it. Frames are taken only from friends (pinned relay identity, its only session), pieces only from the friend who listed the file, and a friend's previews go when the friend goes offline. Files are read, written and looked for on threads of their own.
- The previews of the world are paged in the Schematics tab when they do not fit (they were cut off).

## 0.39.16

- Falling into water with elytra on no longer leaves the player unable to swim. The server keeps the glide flag after a fall into water (vanilla ends a glide on the ground, not in a liquid), and the mod's handling of a glide with no route took the movement keys away whenever the flag was set, with or without a task running; only taking the elytra off gave them back. It now applies only while a task is running and the player is really gliding through the air, never in water or lava.
- Mining with elytra on: the miner no longer waits as if in the air while it swims under water with the glide flag still set.

## 0.39.15

- Seed Map found marks are shared with friends. A structure marked as looted (or unmarked) on the Seed Map now shows the same way on every friend's Seed Map, and hovering it says who marked it ("Found by Alex"). Marks are sent over the existing end-to-end encrypted relay as a new `[DHF1]` frame (up to six structures a frame, well under 1 KiB) to every friend online (one frame a second, so a relay with a message quota is not exceeded), again when they change and when a friend comes online; they are accepted only from friends (the friend's pinned relay identity and its only session, in the server's player list) and only for the same seed. The friend's name comes from the relay, never from the frame, and friends' marks are not passed on to others.
- The newer setting of a structure wins, so a mark and a later unmark by different friends end the same everywhere; an unmark is kept (as not found) so it reaches friends too, and redelivered older frames change nothing. Times from a friend more than five minutes ahead of this clock are refused, and a frame is checked whole before any of it is taken.
- Found marks are loaded when the world is joined (not only when the Seed Map opens), so friends' marks arrive while the map is closed; friends' changes are saved within three seconds and on leaving the world. The file gains the state, time and author of each mark (schema 2); existing files (schema 1) are read as the player's own marks.

## 0.39.14

- The full map (M) opens on the selection made last: explored terrain or the Seed Map, and the world chosen in each (overworld, Nether, End). It used to open on explored terrain in the player's world every time. The choice is kept in the settings, so it holds after a restart too. A world other than the player's is shown at the player's place carried over to it (a Nether block for eight overworld ones; the middle of the End). Opening the map on a point (from a waypoint) still shows that point.

## 0.39.13

- **Protect glow berry vines** now also blocks right-click berry harvesting, including held use and either hand. Sneak bypasses both harvesting and breaking protection. The existing saved toggle is reused; entity interactions and other blocks remain unchanged. Updated English and Russian hints.

## 0.39.12

- Optional **J → Settings → General → Protect glow berry vines** prevents initial and held punches from breaking either cave-vine section, with or without berries. Sneak to deliberately break them; right-click harvesting and entity attacks are unchanged. Off by default; saved in the legacy settings namespace. English and Russian controls are included.

- Patting fixes: releasing the Pat key in chat or while unfocused no longer consumes the next press. Local and remote third-person poses use model join identities as well as server entity ids, so a valid remote player with entity id 0 animates independently of the local model.
- Experiments → Ores: ten directly toggled ore icon tiles replace the ore type selector and visibility checkbox; saved ore choices are unchanged.
- Seed map: structure and biome filter labels are centred and wrap at spaces onto up to three rows; the bottom hint wraps in narrow windows. Regression tests cover label wrapping and English/Russian chip sizing.
- Patting: aim at another player within 3 blocks (its hit box, no block in between) and press the new **Pat** key (**K** by default, rebindable under Options → Mods → Settings) for two gentle pats over 0.8 seconds, with a 1-second cooldown. The hand comes down on top of the other player's head. One press is one gesture; holding the key repeats nothing. The first-person hand (empty or holding an item, mirrored for a left main hand) and the player's own third-person model raise the arm; nothing is attacked, used, mined or placed and no chat is sent. It does nothing while a screen, chat or dialog is open, while dead or spectating, or while an item is in use. Aiming at no player in reach shows "No player in reach" above the hotbar.
- The pat reaches the other player only over the existing end-to-end encrypted relay, as a new `[DHT1]` message (schema 1: target UUID, event id, send time, dimension). Older clients ignore it, so friendship, telemetry and claim sharing keep working with them. The sender is the authenticated relay identity matched to the server's player list, never a name in the message; a pat for someone else, from another dimension, more than 30 seconds off the recipient's clock, a repeated event id, a patter not loaded within 6 blocks, or more than one pat per sender in 0.7 s (eight from everyone in 4 s) is dropped. Accepted pats raise the patter's arm on the recipient's screen (at most eight at once) and show "*name* patted you" above the hotbar. Pats in flight are forgotten on a change of server, account or dimension.
- **J → Settings → General**: **Who can pat you** — friends only (default; the friend's pinned relay identity must match), everyone (still needs a relay identity that matches the player list and is the only one for that UUID) or nobody — and **Pat notification** (on by default). Both are saved in the mod's settings, in Russian and English.
- Tests: key edges, cooldown, two-pat timing, bounded poses, expiry, independent remote arms, consent per policy, spoofed or ambiguous identities, wrong target, self, replay, clock window, burst limits, dimension and distance, malformed and newer-schema bodies, bounded inbox and stale-world events.

## 0.39.11

- Farm plots: the player marks a field by hand and it is kept. Stand on one corner and type `/farm pos1`, on the opposite corner (higher or lower is fine) `/farm pos2`; the corners and the box between them are drawn in yellow. `/farm save [name]` keeps it as a plot of this world (server and dimension), `/farm clear` drops the marks, `/farm list` lists the plots in chat. Only these subcommands are the mod's; any other `/farm` command still goes to the server.
- The Farm tab lists the plots of the world with **Farm: on/off**, **Show/Hide** and **Del.** (paged when they do not fit); shown plots are outlined in the world, green when farmed, grey when not.
- The farm works every enabled plot of the world it is in, wherever the player stands when it starts, so after a reconnect it finds its field again (it used to take the player's position at that moment as the middle of the field, and worked the wrong place or nothing). Each plot is shared between the selected crops like the square field, every level between the two corners counts (plus a block above and below), turning plots on or off applies while the farm runs, walking back goes to a place to stand in the middle of the nearest plot, and drops and the composter are looked for by the plots. Without enabled plots the farm works the square around the start as before. A plot has a side of at most 64 and at most 12 levels; the enabled plots of a world together at most 16 384 cells (the farm reads them twice a second on the game thread).
- The farm has no harvest limit any more: the Limit control is gone and the farm works until it is stopped.
- Schematic previews are kept per world across restarts again: they were keyed by a value that counted up at every resource reload and started again with the game. Previews kept the old way are read under the new key.


## 0.39.10

- Route planning is several times faster. Each search keeps the cells it reads in a box around the start, so a cell is read from the world once instead of by every move that passes it (the planner tried 98 moves from each cell, each looking at a dozen cells), and its tables use a fast hasher for block positions. On generated hilly terrain with trees, ponds and walls (`cargo run --release --example route_bench`): a 25-block walk 2.2 ms to 0.5 ms, an 80-block one 11 ms to 5.5 ms, a 140-block one 65 ms to 14 ms, and a goal that cannot be reached (which uses the whole search) 950 ms to about 160-250 ms; the whole set 1089 ms to under 200 ms, with 20 to 80 times fewer reads. On equal estimates the deeper cell is tried first.
- The miner finds its next block faster when the nearest ones are out of reach: after two places to stand that cannot be walked to, every place in reach is found in one search and the rest are looked up in it, instead of a whole failed search for each (six walled-in blocks: 12.8 ms to 2 ms; up to the 900 ms limit before on real ground).
- The planner thread reads blocks faster: the chunk read last is kept, and block states are looked up with the fast hasher.
- Tasks wait less: the next route is asked for 100 ms after the last one (it was 350 ms, standing still between legs; a retry after a failed plan keeps 350 ms, so the bounded retries still give the world time to load), a followed player who moved is re-planned for after 250 ms (500 ms), and a tool swap is allowed every 250 ms (600 ms, at every change between pickaxe and shovel in a tunnel). A planner still holding a cancelled query is asked again on a following tick instead of pausing the task with "Planner busy".
- Tests: the cell cache answers as the world does inside and outside its box (each cell inside read once); exploring reaches exactly the places a search reaches, by walkable steps.

## 0.39.9

- Human look no longer gets in Baritone's way. A walker with the human look stood still at corners until it counted as stuck: its head already looked round the corner along the way ahead while the cell it had to finish lay a little behind it, and it waited to turn round for a turn that never came. Finishing a step now walks the last bit in any direction; turning round first is kept for a way on that really lies behind.
- The player moving the mouse is now read from the game's own look input (mouse or controller) instead of guessed from the camera having moved. A turn made by a task (the human look, opening a door, the storage trip, the pillar, a glide) is never taken for the mouse, so mining and placing (tunnel, bridge, drain, farm, schematics) no longer pause after such a turn. Moving the mouse hands the camera to the player for three seconds in every task: mining, placing, opening and the storage trip wait to aim and go on afterwards (placing, a door being opened, breaking a shulker box on the storage trip and breaking a mined block keep their time limits held back meanwhile); nothing is paused for it. The docs no longer say that moving the camera pauses a task.
- With the human look switched off (or the mouse holding the camera) gliding on elytra is still steered (it is flown by the look), and swimming to a block or along the route under water goes every way instead of waiting for the head to come round.
- Lava: a place to stand diagonally next to lava (or another hazard) is no longer taken; a corner of lava was as easy to slip into as its side.
- New scenario tests from the test checklist: lava walked round, a three-block drop taken straight off, levels 32 blocks up and down found, a goal 120 blocks away round a long wall found in one search, iron doors not opened, and a tick-by-tick walking simulation (open field, round a wall, a one-wide zigzag corridor) that arrives without circling with and without the human look.

## 0.39.8

- Schematic previews show real blocks: every block still to be placed is drawn where it goes as a translucent ghost of that very block (its own model and texture, turned the way the preview is turned, so stairs, logs and doors face the right way; chests, signs and other block entities included) instead of a blue square, and a block standing there wrongly as a red box. Ghosts are drawn in a pass of their own after the world, hidden behind terrain, only their near faces, and without the faces between two ghost cubes.
- Ghosts are built within 64 blocks of the player, one 16x16x16 section at a time, nearest first, within 3 ms per tick; a section is rebuilt only when its blocks change (one placed, a chunk loaded) or the preview is moved, turned, shown or hidden. The world's depth is kept for the pass only while there are ghosts. Hidden with F1 and while the mod is switched off.
- The ghost rendering follows Horizonmatica by keksi2k (MIT License); its license is in `notices/LICENSE-horizonmatica`.

## 0.39.7

- README rewritten to cover every feature, grouped into World, Chat and people, Items and trading, Baritone, and Helpers and interface. The target Nightly is now given as `fdc87ae` (it said `2ebf00a`).
- New plain illustration cards: the shared route planner (slabs, doors, ladders, gap jumps, drops, elytra, swimming), mining (trees, pillars, under water), the farm (crop quadrants, harvest/seed/bone meal chests, composter), tunnel, bridge and drain, schematics (hologram and preview editor keys) and the helpers. The trading card now shows the auction catalogue and the purchase confirmation.

## 0.39.6

- Flight routing on elytra: a glide is flown toward the landing (the first point of the route below, or the goal while the route is planned again), turning aside from walls and blocks in the way (six blocks of open air ahead are looked for at the body's height and the block below, in turns up to all the way round), circling over the landing while still high to lose height, holding a descent that ends at the landing on the way, and levelling out low over it so that it touches down softly. The pitch follows the look quickly while gliding. Gliding never pauses a task any more (it did as soon as the route had no point at hand, and after landing far off the line).
- Damage, a shove or a moment in the air pause nothing: being knocked off the route (even forty blocks) or landing off the line plans again from where the walker is (only a jump of more than 64 blocks, a teleport, pauses); tunnel and bridge wait for the ground instead of pausing when knocked into the air or gliding; mining waits in the air instead of pausing.

## 0.39.5

- Mining under water: the route is planned from the water too (the planner waited for the feet to touch the ground, which never happens while swimming, so nothing was ever planned), a route answer from a moment ago is not a reason to pause there, and a mining job does not swim off to the shore while it looks for the next block; it swims to the block and mines it.
- Partial blocks: a step is reached when the walker is across it and on its level, a slab, a bed or a trapdoor lifting the feet up to a block. It used to wait for the exact height, stood still on a slab or a trapdoor and planned again after a couple of seconds.
- Following and walking: a route answered while the walker had moved on (after a jump, following a moving player) is planned again from where it stands instead of pausing the task.
- Elytra: worn with durability to spare, any drop the route needs is taken, however high (the planner looks down the whole column for the ground, and such drops cost little more than walking): the walker spreads them while falling (a jump press in the air) and glides toward where the route goes, looking a little down, until it lands.
- Doors, gates and trapdoors when held up: if the walker makes no progress for a moment, a closed door, fence gate or wooden trapdoor under its feet, at them or one step toward the next point is opened (a player followed through a door that shut behind them, a trapdoor under the feet over a ladder).
- Snow layers: the body stands on one layer less than snow looks; up to five layers are walked over, six and seven are stood on like a block, eight are a block.
- Drops of three blocks and more: a step down the edge counted as being off the route (its landing is more than 2.6 blocks from where the walker stands), so the walker stood and planned again and again; the distance is now taken from the step under way (this also covers jumps across gaps).
- Straight down (into a ladder shaft, through an opened trapdoor): the walker lets go and falls instead of walking on and stepping off the hole; a trapdoor under the feet over a ladder is opened when it holds the walker up.
- Doors, gates and trapdoors opened on the way are closed behind also when the walk ends right after them (a gate used to stay open).
- Mining no longer pauses itself: its smooth turn to the next block was taken for the player moving the camera (logs left uncut, the job paused at a tree).
- Mining storage: an inventory without a single free slot also sends the miner to the shulker box or the storage (drops of other kinds, sticks and saplings, could not be picked up and the miner went back and forth).
- Tunnel and bridge are not paused by water or a moment in the air at the edge; the walker swims the player out.
- Human look: a steady glance down at the way ahead (more on the way down, less on the way up) that changes slowly, instead of the exact angle to each next cell, which made the head nod at every step.
- Remembered routes work for walks to coordinates and waypoints too, not only for farm and storage walks.
- Doors and gates: one right in front of the walker, the way it goes, is opened at once even if the route did not plan through it; when no way is found and a shut door, gate or trapdoor stands beside, it is opened and the route planned again; the click aims at where the block really is (a door's panel at the side of its cell, a gate's bar).
- Navigation goals given as a block (the ground under the feet, a waypoint saved on it) are walked to where one stands there, up to three blocks up or down; the search reaches up and down as far as the goal is, with no fixed limit, and wide searches look 160 blocks around (legs of up to 140 blocks).
- Mining no longer pauses when the walk's look turns the head as it reaches a block, nor for the task's own presses (breaking a shulker box, a click on a door).
- Tunnel and bridge: vines under the passage are covered by the floor and vines in it walked through (the tunnel stopped with "no floor"); passage jobs step in every direction again instead of standing still to turn, which held the tunnel up; the walk's look does not turn the head while a block is placed, and the gradual turn to a block is only for big turns (the bridge used to spin before each block).
- Hanging vines are climbed up or down, not walked along over a drop; turning round before walking back is used only while the head really turns (human look on, the mouse not holding the camera).
- Schematics tab: buttons for everything the keys do. Place preview (at XYZ, or where the player stands when XYZ is empty), Build, Show/Hide and Delete preview; the editor row moves the selected preview a block along X, Y and Z, turns it, brings it here and turns the camera to it; the schematics folder is listed on the tab (click a file to choose it, paged) and read again whenever the tab opens; the previews of this world are listed beside it.
- Water is no longer a hazard like lava in the shared planner: a place right by the water can be stood on (a shore, the rim of a pool, the edge of a pit in the sea used to be off limits, and the walker stood "stuck"), a walk starts from the water and climbs out along its route, and swimming follows the route instead of only heading for the nearest dry ground. Lava and the other hazards are still kept away from; digging still leaves blocks beside water alone.
- Elytra: over the landing while still high, the glide circles round it to lose height, and low over it levels out, so it does not dive into the ground; a narrow shaft still gives no room to glide.
- Farm: trampled beds are tilled first (they waited behind every ripe crop and were never reached), and dirt in a bed next to farmland on two sides or more counts as trampled too, also when it was trampled before the farm started.
- Farm: the bone meal a composter pops out is picked up (dropped items are collected around the composter and the containers too, not only on the field).
- Near the goal (the last step and a half) the walker steps in any direction instead of turning round first: with the human look on it overshot, turned and circled round its goal, or stopped by a chest as stuck. Places to work from are never on top of a chest, barrel or other container.
- Running jumps across gaps: the planner takes a jump of two to four blocks straight on (gaps of one to three blocks, level or one block down, with the air above the gap clear), dearer than walking round. The walker runs at it with the sprint key and jumps at the edge. Farm level routes never jump.

## 0.39.4

- One unreleased UI update on top of upstream 0.39.3. Local test iterations share this version; earlier unpublished 0.40.x/0.41.0 labels have been consolidated, not released.
- Trading is a dedicated navigation destination with grouped preferences and a workspace entry; compact navigation uses a labelled Section chooser. Settings retain readable switches, numeric readouts and named corner choices in Russian and English.
- The trading workspace supports full-width compact details, bounded detail scrolling, Back to list and responsive sorting. Inventory tools are grouped into Sorting, Buttons and Safety with explicit directional controls. Appearance, Helpers and translation-language dialogs retain readable controls and clear Save/Cancel paths.
- Map/filter controls name their states and provide clear closing actions. Waypoint-list deletion asks for confirmation; auction/Market confirmations use consistent frames and action-specific labels. Original-message badges follow the UI language without changing message data.
- Fixed overlapping Baritone footer content: mining storage/custom-block controls, status/auto-eat and action-key hints occupy distinct reserved rows. Compact key hints reserve space for Settings/language controls; schematic footer actions use the same separation.
- Integrated current upstream 0.39.3 gameplay changes, preserving its Human look helper, arbitrary mining blocks, farm options and schematic previews alongside the UI work. Helper pagination derives its row count from the available helpers rather than a fixed count.
- Reorganized the J menu so each feature keeps its preferences: **Map and claims** holds the claims list plus Map, Borders, Colors and Storage; **Chat** holds style, notifications, anti-spam, DeepL and the translation switches; **Settings** keeps General, Network, Player list and Sidebar. The map's Settings button opens Map and claims → Map.
- Paired `Name − value` / `Name + value` buttons are now one stepper row with a value cell and − / + buttons. Choices show the value on the right with a chevron; switch rows no longer look selected; text inputs are inset; arrow paging buttons use Lucide chevrons; single-page settings hide paging. Helper descriptions start with a capital letter.
- Explored/Seed map tools read "Zoom in"/"Zoom out", use Lucide locate and skull icons for Center and Death, and the tool column is wide enough for "Terrain: Surface". Seed-map status text no longer overlaps the tools. Settings → Network no longer shows a Back button that led to General. The claims list explains an empty list.
- Translation languages (right-click the chat's language button) is a centred card: **My messages** (I write in → Send as) and **Chat messages** (Chat is in → Show as), every language visible at once with a check on the chosen one, a clearer "Translate chat only from common languages" switch and Cancel/Save. Paging appears only on very small GUIs.
- **Auction catalogue.** Opening the auction walks every page of All listings in the background (about 200 ms after each page arrives) and shows only the catalogue: every lot in one list, searchable by item, seller or #number and sortable by unit price, lot price, ending soon or newest, with one pager. Clicking a lot asks for confirmation (item, seller, time left, price, balance after, and that `/ah buy <number>` buys at once with no undo); **Ask before buying at the auction** (Trading → General, or the switch inside the confirmation) turns the question off so a click buys immediately, except lots taking bids and purchases the known balance cannot cover; auction-style lots point to the server view. Right-click still sets a price alert. My listings and Mail open the server's pages, Refresh walks again, F8 shows the server window. Reopening within 15 s reuses the last walk.
- Clicking the catalogue's own controls (search, sort, pager, price alert) no longer stops the background walk. The trading workspace opens again after its market/auction scan instead of leaving the auction window open, and the catalogue's close button returns to the game.
- Market screens use one pager that continues across server pages; market items show their unit price and 24-hour trend. Auction and market windows are compact by default, with a "Large auction and market windows" switch under Trading → General for GUI scale 2 on 1080p/1440p.
- Single player no longer sends automatic `/claims` and `/bal` requests to Horizon's own server, which only raised the unknown-command confirmation.
- Explored map no longer turns blurry when zoomed out (past about 200 m on the scale bar). Wide views used one averaged colour per chunk once more than 18 saved regions were visible; they now sample shaded per-region overviews at 2, 4 or 8 blocks per texel, built on the map worker within a 40 ms budget per redraw (centre first, chunk colours as placeholders until done), kept in memory up to 32 MB and updated as terrain is explored. Saved map files are unchanged.
- Added regression coverage for navigation, settings reachability, modal geometry, language choices, detail scrolling and Baritone footer separation. Offline layout previews are not in-game screenshots; live SMP interaction requires separate verification.

## 0.39.3

- Routing for every Baritone task (walk, follow, mining, tunnel, bridge, farm, drain, schematics): steps across corners when both sides of the corner are open, so open ground is crossed diagonally instead of in a staircase.
- Partial blocks are judged by their real shape instead of being obstacles: up to 0.6 high (bottom slab, carpet, bed, snow layer, stonecutter) is stepped over without a jump; about a block high and covering nearly the whole cell (stairs, top slab, chest, composter, soul sand, cauldron, hopper) is stood on; fences, walls, panes, bars and fence gates are a block and a half high and never stood on.
- Wooden doors and fence gates on a route are opened by hand when reached (the walker stops, aims and clicks, up to five tries) and the ones it opened are closed behind it; iron doors are not. Ladders and vines are climbed up (jump key) and down, and left at the top onto what they lean on.
- Far walks: each leg ends on the ground nearest to the height the walk is heading for, not on the highest block in the column, which led bots onto roofs and trees above the farm. Partial routes favour the goal's height too.
- Wide search: a goal more than 24 blocks off or more than three blocks higher or lower is planned over 96 blocks around (up to 150,000 cells, 0.7 s on the planner thread) instead of 32, so a second floor whose only way down is a slope forty blocks away is planned in one go. Far walks take legs of up to 72 blocks.
- Remembered routes: a walk that arrives (to the farm, a storage, the safe spot) is kept as the cells it went through, per world, up to 32 routes; a later walk between the same places that starts near the route follows it point by point instead of searching again, in either direction. If the remembered way is blocked, it plans afresh.
- Walking like a person, for every task: the head turns toward where the walk goes (a few blocks ahead along the route, a little down, with a slight sway), easing in at up to 22 degrees a tick, so corners are taken in a curve; a way on that lies behind is turned to first instead of being walked to backwards or sideways; between two plans (a leg ends, a route is planned again) the walker keeps going straight for up to half a second when the cell ahead is firm instead of freezing. Aiming at a door, a chest, a crop or a block to mine turns the head over a few ticks too, instead of snapping to it.
- Mining, tall trees: a log inside its crown counts as reachable (the leaves are cut on the way); a 2x2 trunk is also cut from inside, standing under the logs; and when the rest of a tall trunk hangs more than five blocks up with nowhere to stand, the miner builds a column of dirt, netherrack or cobblestone under itself up to four blocks under the lowest log, cuts what it reaches, and digs the column away again from the top. Before, a big spruce stopped at "Finding ore…" once its lower logs were cut.
- Human look switch (Experiments → Helpers, on by default). Moving the mouse lends the camera to the player for three seconds after it stops, without pausing the task; then the bot looks where it goes again.
- Tunnel, bridge and drain move the way the walker does: they turn the head toward where they go and turn round instead of walking backwards (only fine positioning on an edge may step back or aside). Placing a block turns the head to it over a few ticks before the exact aim, in every task (bridge, tunnel floor, drain, schematics, sowing).
- In the water, every task swims: jump held to stay at the surface, toward the nearest dry ground, head turned that way; a tunnel or bridge job waits on dry ground instead of pausing. Lava stays a hazard the planner walks around.
- Mining: the column under the miner is built from any plain building block, earth and stone first and the mined block too (the same tool takes it down), and the right tool is taken to dig it away.
- Mining any block: a new "Own block" tile and button on the Mining tab. Aim at any block of the world and press the button to add its kind (again to take it out, at nothing to clear the list, up to 16 kinds, kept in the settings); the tile then mines all of them. A "Prismarine" tile mines prismarine, prismarine bricks, dark prismarine, sea lanterns and sponges.
- Mining under water: blocks with water beside them are targets (water no longer counts as a hazard beside a block; lava still does). The miner swims to the block (up with jump, down with sneak, head turned to it), holds still once it is in reach and mines it. When the air runs low (under 90 of 300) it swims straight up and waits at the surface until it is full again, then dives back. With Water Breathing (a potion of 3 or 8 minutes, or Conduit Power) it does not go up at all; and in the water during any task, when Water Breathing is missing or has under ten seconds left, a potion of Water Breathing from the inventory is drunk (into the hand, then back to the slot that was held), so the bot does not need to surface. The glass bottles drunk potions leave behind (one per slot) are put together into one stack while a task runs.
- Mining with shulker boxes: when the inventory fills up and a shulker box with room is carried, it is put down beside the miner, filled with what was mined, broken with the right tool and picked up again, and mining goes on where it was; without one, the storage container is used as before.
- Farm: "Levels: flat / ±2 / ±3 / ±4 / ±6 / ±8" for fields on several levels; beds that far above or below the start belong to the field (flat: one block of unevenness). The composter is looked for by the containers first, then by the field.
- Trips to the safe spot and back to the beds may break blocks and bridge gaps when there is no way on foot, like every other walk.
- Schematic previews: the preview key on the Schematics tab places the chosen file as a hologram in the world (its box, and near the player every block still to be placed in blue and every block standing there wrongly in red; what is built is not drawn). Previews are kept per world in the settings across sessions, several per world, listed on the Schematics tab with Show/Hide and Delete; clicking one selects it. The selected one is moved and turned with keys while a panel on the screen shows its file, coordinates, turn and the keys: arrow keys move it with the camera (forward, back, left, right), Page Up raises it (with Shift lowers it), Home turns it a quarter, End moves it to the player, Insert shows or hides it, Delete removes it (all rebindable in Options → Mods). Start builds the selected preview as it stands; stopping the bot leaves the preview in place.
- Schematics: a running build is kept in the settings (server, file, origin, rotation) until it is finished or stopped with the stop key, and is loaded and carried on again after a server restart or a restart of the game (placed blocks are skipped).
- A far walk no longer goes back and forth: a leg that would end where the leg before the last one ended is skipped for another direction.
- Farm: the start is remembered with its height. A bot above the field (on a roof, a tree) no longer counts as being at the beds; it walks back down. Dropped items are only chased at the field's level.
- Farm: with several crops selected the field is shared fairly (two: halves, three: thirds, four: the four corners of the square), each part sown with its own crop; thin lines on the ground show the parts. Before, the first crop with seeds at hand took every empty bed.
- Farm menu: under the container buttons the menu lists what each is bound to, with coordinates (harvest, seeds, meal, safe spot, composter), and each bound block has a coloured frame in the world (orange, green, white, blue, brown).
- Farm: a meal or seeds container found empty is not visited again for 15 minutes; the farm sows, harvests and idles as usual meanwhile instead of walking to the empty chest every minute.
- Farm: composter. With bone meal on and a composter within the field or eight blocks around, spare wheat and beetroot seeds go into it while the meal stock is under 64, and the bone meal it makes is taken out and picked up. When meal runs out and no seeds are spare, two stacks are fetched from the seeds container to compost.
- Drops: one rule for every task, in the shared planner. Three blocks are always stepped off (no damage). When nothing else leads down (a roof, a cliff above the farm) a longer drop is taken if its fall damage is at most three points and leaves more than ten: six blocks without protection. Feather Falling (three a level) and Protection (one a level) on the armour cut the damage by 4 % a point (at most 80 %), so Feather Falling IV boots allow eight blocks and full protection twelve. Each block beyond three costs as much as four steps, so stairs or a slope nearby are taken first. Eating on its own makes up the points.
- Wooden trapdoors: walked over when shut on the floor; opened when they are a hatch at head height, shut at the top of their cell, or in the way of a climb up or down a ladder; passed when open; closed again behind like doors. Iron trapdoors are not opened. The cell over the top of a ladder holds the body, so a hatch over a ladder shaft is climbed through.
- Farm: every farmland block of the field is remembered; one trampled to dirt (a jump, a fall, a mob) is tilled again with any hoe in the inventory and sown on the next look.
- Farm: a hungry farmer (food bar at seven shanks or less) with nothing to eat bakes up to three loaves of bread from three wheat each at a crafting table within eight blocks; eating on its own then eats them.
- Farm: the harvest is taken away when the field is idle, when two slots or fewer are free, when twelve stacks have gathered, or when three or more have waited four minutes; not only when the inventory is full, and not after every crop.

## 0.39.2

- Farm: on the way to unload, a bot that carries wheat looks for a crafting table nearby (eight blocks, twice a second). If it finds one it goes there first and crafts hay bales, then carries on with the unload; if not, the wheat goes into the container as it is. A table that cannot be used is not tried again for 90 seconds, so it never holds the unload back.
- Farm: **Auto sell**, a switch on the Farm tab (off by default). When the harvest container is full (the unload trip moves nothing any more), the bot sells the harvest instead of giving up: each stack of what the chosen crops yield (harvest and hay bales, never seeds) is swapped into the selected hotbar slot and `/sell hand` is sent, one stack after another, until none is left. The market pays for crops, so a full chest no longer stops the income. If a stack does not shrink after three commands (no such command, nothing is bought), the selling stops with a message and starts again after the next failed unload. Selling needs chat commands enabled in the chat settings.

## 0.39.1

- Farm: the spare seeds now reach the seeds container. The unload trip to the harvest container reset the "unload" flag, so the follow-up trip to the seeds container only happened once three or fewer inventory slots were free, which a few stacks of seeds never cause. The flag stays set after the harvest trip when seeds are left, so the seeds trip follows it.
- Farm: hay bales. When wheat is farmed and a crafting table stands within eight blocks of the bot, a stack of nine or more wheat is crafted into hay bales before the unload (the table is opened like a container, wheat goes one per cell into the 3x3 grid, up to four bales a round, shift-click on the result), and the bales go to the harvest container with the rest. There is no switch for it; without a table nothing changes.

## 0.39.0

- A bot task picks itself up again after the server restarts and the client reconnects. The last task you start (walk to a point or a waypoint, follow a player, mining, tunnel, bridge, farm, drain) is remembered. If it is still running when the connection goes (or was running in the last five seconds, since a lost connection pauses it a moment before the reset), it is marked, and once you are back on the same server, in the world and with the chunks around you loaded for six seconds, it is started again as if you had pressed Start. It tries every five seconds, up to six times, and gives up after fifteen minutes. A task you stop yourself (Stop), one that had finished and one that was only a preview do not come back; a different server leaves it waiting. A switch "Auto Resume: on/off" in Experiments > Helpers turns it off (on by default).
- Schematic builds are not resumed (they need the file loaded again).
- Farm: three containers instead of one: **Harvest**, **Seeds** and **Meal** (bone meal), set from three buttons to the right of the crop icons (aim at a chest, barrel or shulker box and press; press elsewhere to clear). Without a seeds container everything goes to the harvest one, as before.
- Farm: carrots and potatoes are both a seed and a crop, so after a harvest each stack beyond the one kept for replanting goes to whichever of the harvest and the seeds container has had less so far: about half and half. Wheat goes to the harvest container, its seeds to the seeds container.
- Farm: bone meal. With "Bone meal: on" (a new switch on the Farm tab), when nothing is ripe or empty the farm feeds the crops that are still growing: it holds the bone meal and clicks the crop, up to eight doses until it ripens, then goes on. It fetches meal from its container when the stock is under eight.
- Farm: bone blocks. If the Meal container holds bone blocks, the farm takes the stack it needs (never more than 64 blocks) and crafts bone meal in the inventory (one block makes nine; never more than 64 meal), so the meal container can hold blocks to save room.
- Farm: a safe point you set. A fourth button beside the containers, "Spot": stand where you want the idle farm to wait (a nook, behind a wall) and press it; pressed again at the same place it is cleared. After six seconds with nothing to do (waiting for crops, no seeds, no light) the farm walks to the point and waits there; the next ripe or empty bed, or a crop to feed, sends it back. With no point set the farm stays where it is.
- Farm and mining: a container 80 or 800 blocks away is reached, and the way back. The planner only sees 32 blocks around its start, so a walk to a farther goal is now taken in legs of up to 24 blocks, each ending on a standable cell toward the goal. A leg that has no way (a cliff, a river, a wall) is tried toward another end (turned 35, 70 or 110 degrees to either side, then at a shorter reach of 14 blocks) before the walk gives up, and the legs have a budget so that it cannot go round in circles. The time allowed for a storage trip grows with its length (two blocks a second, at least 90 seconds). This is why the farm stood still after a trip to a distant storage: the way back to the beds was longer than the planner can see. It helps the mining storage trips too.
- Farm: Pause and Start again no longer ends in "Idle" with a blank reason while crops are ripe. The old target, the idle flag, a pending trip and the walk home are cleared, and the first look at the beds comes at once.
- Walking: a player who touches or walks past the bot no longer holds it for a couple of seconds. After a few ticks without progress with a body (a player, a mob) within a block, it steps away from it instead of pushing into it, and the stall does not count as stuck; being shoved more than 2.6 blocks off the route plans the route again from where it stands (before it paused the walk); only a jump of eight blocks or more (a teleport) is still a pause. A walker that makes no progress (it was stuck on a half block) now jumps every half second and then steps aside, instead of pushing on until it counts as stuck.
- New tests: the carrot/potato split, the bone meal and bone block plan and the crafting amount, the ends of a leg, a shove off the route, jumping and stepping aside when there is no progress.
- Not verified in game.

## 0.38.3

- Auto Click, in Experiments > Helpers: presses a mouse button again and again at a pace you choose. A switch ("Auto Click: on/off"), the button (left or right, "Button: left"/"right") and the rate (1, 2, 3, 4, 5, 6, 8, 10, 12, 15 or 20 clicks a second; the last one holds the button down). It works while you play by hand: with a screen open (inventory, chat, menu) or a task running (walking, mining, a build, the farm, drain) it does nothing, and it presses on top of whatever you hold yourself. It is applied last, after the tasks have read the buttons, so its presses are not taken for you taking the wheel. A key can switch it too: "DraimHorizons: auto click" in Controls, unbound until you bind one. The press count follows the rate whatever the frame rate (tested at 4, 7 and 16 ms frames); a stall is not paid back with a burst. Pure pace logic in `autoclick.rs` with tests.
- Drain: falling into the water swims back to shore again. A regression from 0.35: when the bot hung over the edge to place a block and slipped off, the first tick in the air (not on the ground yet) paused the whole drain job, so by the time it was in the water the swim-back found no running job. Slipping off the edge no longer pauses a drain job, and the swim-back also takes the player out of the water when a drain job is paused.
- Not verified in game.

## 0.38.2

- Mining no longer goes idle when the player stands where the planner sees no floor: on an edge with the middle of the body over a gap, in the air after a jump or a fall, at the lip of a pit. The planner refused to start from such a spot, so every route read "no route" (31 of 31 in the screenshot). It now plans from the nearest place that can be stood on: the cell itself, one up, down to the three-block drop limit, then the four neighbours. The real position is still the one the answer is checked against. This covers the mining search, the dig route and the walk.
- New test for the start finder (flat ground, a jump, an edge, nothing firm anywhere).
- Not verified in game.

## 0.38.1

- Mining with nothing open around: standing in sandstone with sand overhead, the planner digs a stair up through the rock to the sand. A new test pins it (sand seven blocks up, rock everywhere else: a route of at least five steps that breaks blocks). 
- Mining no longer stops for good after a fixed number of dug routes: the budget (24 routes, 3 failures) now counts only those without a mined block in between, and a confirmed break resets it. A long job in a desert, which needs many routes one after the other, goes on; a route that keeps failing still ends it.
- After a storage trip the miner walks back to where it stopped before it goes on (0.38.0), and the farm walks back to its beds when it is far from them (0.36.1), so a distant storage is not a problem for either.
- Not verified in game.

## 0.38.0

- Mining has a storage, like the farm: a chest, barrel or shulker box set from the Mining tab ("Storage: ..." under the targets; aim at the container and press it, press again elsewhere to clear). It is the same storage the farm uses.
- When the inventory is full (every slot taken and every stack of the mined kind at its limit), the miner no longer just waits: it walks to the storage (a spot within reach with a clear line to it, another spot if the chest is hidden from the first), opens it, shift-clicks everything it does not need across and walks back to where it stopped, then goes on mining by itself. It keeps tools and armour (anything that wears), food, a totem and one stack of the mined block to build with; the rest, sand and gravel included, goes into the container.
- If the container is full or cannot be reached, the miner stands idle ("Idle: free an inventory slot") and tries again after one to two minutes. Without a storage set, nothing changes.
- The walk to the storage starts the navigator over, which would drop the mining job; the job is put aside for the trip and put back after it.
- New pure `deposit_mined` plan in `farm_storage.rs` with a test.
- Not verified in game.

## 0.37.13

- Walking: the route planner now steps off drops of up to three blocks (three blocks cost no health); before it only managed one. Dunes and pits are full of drops of two and three blocks, so on a ridge between pits every walk ended at once: "seen/open 131/65, no route 32" while sand lay all around. This also helps every other walking task. The longer drops are refused as before.
- Mining: before any search, a block of the kind within reach (4.2 blocks of the eye) and in plain sight is dug at once from where the player stands. No route is needed for it, and none can exist on a ridge. Walking, and digging a way, come only when nothing is in reach.
- New tests: a walk down a three-block drop to the sand; the planner refuses drops over three blocks as before.
- Not verified in game.

## 0.37.12

- Mining: "seen: 483, unsafe 32/0/0" meant that the 32 nearest blocks of sand were all buried (the ones under the player's feet), so none could be mined from outside and the open ones farther away were never looked at. The search now looks only at blocks with an open side (the buried ones are dug to by the route planner) and counts how many it saw and how many are open: "Finding ore… · seen/open: 483/57 · ...".
- Mining digs a way when it cannot walk: the "dig and place blocks to get there" route is allowed with any digging tool in the inventory (the right one is taken for each block) instead of only with a pickaxe and a free slot, so a buried or unreachable block is reached by breaking through and building up, as the tunnel and the bridge do.
- "Inventory full" for mining now means that every slot is taken and every stack of the mined kind is at its limit: sand laid one by one across the inventory still has room, so the miner goes on picking it up. The same rule applies when a mining job is started or resumed.
- Not verified in game.

## 0.37.11

- Mining goes on block after block: right after a break is confirmed, the job looks for the next block of the chosen kind that is within reach (4.2 blocks of the eye), in plain sight and safe to break, takes the one nearest to the block just broken (the next of a vein, the next of a column of sand, the next log) and hits it at once, with no stop to pick up drops, no search and no step. Only when nothing is left in reach does it pick up the drops around and search for the next place, as before. In a desert this means sand is dug without a pause for as long as there is sand within reach, then the player walks on to the next spot.
- Targets that sight does not confirm are not chained: the existing check (aim, the block really under the crosshair, grass and leaves cleared) still decides, and a block that cannot be hit is left and another is taken.
- New pure `next_in_reach` in `mining.rs` with a test.
- Not verified in game.

## 0.37.10

- Mining sand and gravel (a desert): the search threw out any block with sand or gravel above it, and in a desert or under a dune wall that is nearly every block, so it found nothing and sat on "Finding ore… 0". Sand and gravel are now mined with more of the same above them (the block above only drops into the gap); other ore with a falling block above it is still left alone. Digging to buried sand or gravel goes through more of it.
- The search looks at the blocks next to the player first (cubes of 3, 6, 10 and 16 blocks), looks at up to 32 candidates, and a slow scan keeps what it found instead of giving up with nothing.
- When blocks of the chosen kind are around but none can be mined, the status line says why: "Finding ore… · seen: 41 · unsafe/no footing/no route: 0/3/38". If it ever stays at "Finding ore…" again, those three numbers say which step to fix.
- New test: sand in a desert, in a dune wall, and ore with a falling block above it.
- Not verified in game.

## 0.37.9

- /shop is redirected to /market: when you type /shop (or click it in chat), the mod runs /market instead, so the one Market view and its prices are used. Typed with arguments (/shop something) it goes to the server unchanged. The menu title "Shop" / "Магазин" is still read as the Market as in 0.37.8.
- Not verified in game.

## 0.37.8

- /shop is the same menu as /market on the server: a menu titled "Shop" or "Магазин" is now read and shown as the Market (the custom Market view, prices and the hover note) the way "Market" is. The mod still asks for the market with /market itself.
- Not verified in game (the title the server gives /shop was assumed to be "Shop" or "Market").

## 0.37.7

- Waypoints tab: the list used the old 96 px rail width, so its "World" bar and rows slid under the rail; it now uses the shared rail width and starts on the same left edge as the title.
- Waypoint editor and the quick waypoint (X): a new pop-up on the shared pop-up frame (shadow, thin border, icon and title over a rule). Every field has a caption above it (Name, Type, Colour (HEX), Coordinates); the colour swatches are bigger and the chosen one is ringed; X, Y and Z are three equal boxes with the letter over each, and the world's name is on the right. Coordinates are whole blocks ("-1898"), not "-1897.59938...".
- Appearance pop-up (Settings > General): the same frame, the title and the line under it no longer overlap, and the on/off rows are real switches.
- Every button whose caption is "Name: on/off" and wide enough (120 px) is drawn with a switch in any window that uses the shared button, not only in the J menu; the match is case-insensitive.
- Coordinates under the minimap: dim axis letters with bright values, in a chip with the accent stripe. In the Nether a second row gives the overworld position (x and z times 8), with a red "Nether" and a green "Overworld" tag.
- Not verified in game.

## 0.37.6

- Shulker box tooltip: only the name and the grid of items now. The game's own list of the contents ("6 x Iron Ingot ... and 20 more...") is gone, since the grid shows the same thing. Lore that is not that list stays. The tooltip is drawn by the mod in the game's own tooltip style (box, text and the dye colour) for a shulker box that holds something; empty boxes and every other item keep the plain game tooltip. Recipe book and trading hovers still show the game's list.
- Not verified in game.

## 0.37.5

- J menu shell, rebuilt on one grid: the rail is drawn as often as it is laid out (the selected tab no longer sticks out past the rail, a regression of 0.37.2: the rail background stayed 96 px while the buttons were 112), the window has rounded corners, a soft shadow and a thin border, the rail is an inset plate, and the line about the tab sits under the title on the same left edge.
- Market item page: the figures of the item (1 h, 24 h, volume, base price) are tiles with a green or red stripe for up and down, in two or three columns instead of loose lines; "coins" is translated to "монет" everywhere in the Market text.
- Auction lot page: a lot's "#7833 1 Wooden Pickaxe" subtitle reads "Lot #7833 · ×1"; the duplicate Back button is gone (one button per kind).
- Translation: a failed translation service no longer writes a "[Translation] Service unavailable" line into the chat on every message; the translation button turns red for a minute and its tooltip says the original text was sent.
- Not verified in game.

## 0.37.4

- Auction lots (F8 view): a fixed-price lot card now shows the price per unit on the right in gold (green when it is within your favorite threshold), the lot price below it, and the seller with the time left (1d 20h) on the second line, instead of one long line cut off at the seller's name. Bidding lots keep the old card.
- Auction: a sort button next to the search field cycles the order of the page: the server's order, unit price (cheapest first), lot price (cheapest first). It sorts what the server shows on the current page.
- Not verified in game.

## 0.37.3

- Market and Auction windows (the F8 custom view): the search is a real input field with a magnifier icon, an accent outline while you type and a "Search by name…" hint when empty (it used to be a flat "Search:" button). The balance sits in a pill, a thin divider separates the header, and the two cramped bottom lines became one row: the status on the left and the "Standard view · F8" switch on the right. The page arrows moved down a little to make room.
- Not verified in game.

## 0.37.2

- Interface: every "Name: on/off" row in the J menu (the map, chat, HUD and TAB settings, the Helpers tab and so on) is now drawn as a name on the left and a real switch on the right (accent colour when on, grey when off) instead of "on"/"off" inside the text. Narrow buttons (under 120 px) keep the text. New pure `switch_label` in `ui_style.rs` with a test for both languages.
- Not verified in game.

## 0.37.1

- Interface, first step: the Economy window grows with the screen (up to 640 x 360 instead of 520 x 300), and its Overview shows balance, priced items and auction lots as three cards. Every tab and button that had no icon now has one (Auction, Market, Plan, Check hand, and the settings tabs General, Connection, Map, TAB, HUD, and the map sub-tabs), all from the Lucide set. The sidebar of the J menu is wider (112 instead of 96 px) so "Experiments" is no longer cut off.
- Not verified in game.

## 0.37.0

- New Experiments > Helpers tab (RU "Вспомогательные") with four switches, working while you play by hand, not only in tasks:
  - Auto Eat: eats on its own when the food bar is low (on by default, as before). It no longer needs a task to be running; by hand it waits while the attack or use button is held and gives way if one is pressed during a meal.
  - Auto Totem: when the off hand is empty (the totem was used), a totem from the inventory goes there.
  - Auto Fill: when the last item of a hotbar stack is used up, the same item from the inventory goes into the same slot (a block you build with, a tool that broke). Nothing is refilled behind an open screen, so a stack you move out by hand stays out.
  - Auto Tools: while you hold the attack button, the hotbar tool that fits the block under the crosshair is selected (axe for wood, pickaxe for stone, and so on); the held one stays if it fits. Tools only from the hotbar, bedrock and tasks are left alone.
  - Totem, Fill and Tools are off until turned on. All swaps are spaced 350 ms apart so the server answers each one.
- Not verified in game.

## 0.36.3

- Drain: faster filling. A column took about 0.8 s per block (a dropped block was waited for until it sank, then the next one went in). A cell that just received a falling block is now left alone for 450 ms and the next scan follows after 70 ms (was 800 and 120), and the wait-and-look poll is 50 ms. A pond should fill roughly twice as fast; the server still has to acknowledge every placement. If the server turns a placement down because the block has not sunk yet, the cell is retried later; tell me the timings if it does.
- Not verified in game.

## 0.36.2

- Drain: a block was placed too far away. The reach was measured to the middle of the cell, but the click lands on the face of the block under it, up to a block further, so the server could refuse it. A cell is now chosen only within 3.3 blocks of the eye (was 3.7) and a face is clicked only when the real click point is within 4.1 blocks; otherwise the player walks closer first. The same click limit applies to farm and schematic placements.
- Drain lines and grid: it stood on the line it had built and said "no firm block to click on at the edge" with cells left. A sand block counted as firm only after being watched for 1.5 s without a gap of a second, and a job that is waiting looks only every 1 to 2.5 s, so the count never got there. The count now restarts only when the cell was seen empty (or not looked at for 20 s), so the line it stands on is firm and it goes on along the line by itself.
- Not verified in game.

## 0.36.1

- Shulker box preview (like the ShulkerPeek mod; not a Baritone feature): hover a shulker box that holds something, in the inventory, a chest, the creative tabs, the recipe book or chat, and its tooltip shows the contents as the familiar 9x3 grid with the items and their counts under the name and lore. The tooltip takes the colour of the box's dye (cyan box, teal tooltip; a black box gets a neutral grey). Empty boxes keep the plain tooltip. It is drawn through the game's tooltip component and colour hooks, so it works in every screen that shows an item tooltip. A switch in J > Settings > General ("Shulker preview") turns it off; on by default. New pure `shulker_peek.rs` with tests.
- Farm: when the player is far from the beds, or the chunks they are in are not loaded, the field read as empty from a distance and the job went to "waiting for ripe crops". It now walks back to the beds first (status "walking back to the beds"); if there is no way back it idles and asks again instead of waiting for crops that are not there.
- Not verified in game.

## 0.36.0

- A task no longer pauses when you open the inventory, the chat, the mod's own menu or the map. Before, any open screen paused navigation, mining, tunnel, bridge, drain, farm and schematics (and a click inside the inventory counted as taking the wheel). Now walking goes on as usual (the movement hook is not stopped by a screen) and the jobs keep their state; the things the game does not take behind a screen (a hit, a placed block, a use click, eating) simply wait and carry on the moment the screen is closed. Escape's pause menu, the title screen, signs and books still pause, as does your own movement, jump, sneak or attack input.
- Not verified in game.

## 0.35.2

- Drain: "No firm block to click on at the edge" ended the job as finished-with-a-pause while water was still left. It no longer ends it: the job waits, keeps saying why ("no firm block to click on at the edge, waiting (stand on a block by the water)") and looks again every couple of seconds. Only when no water cell is left is it done.
- Drain: a sand or gravel block now counts as something to stand or hang on when it rests on a solid block that cannot fall (as before) or has stayed where it is for 1.5 s. The 0.35.0 rule (support only) could not tell that a sand line you had built was firm if nothing solid could be found below it, and then no cell had an anchor at all, which is what stopped the job at the very start.
- Not verified in game.

## 0.35.1

- Removed the instruction requiring Russian replies to the owner from AGENTS.md. Public README and artwork remain English.

## 0.35.0

- Idle mode everywhere: a job no longer stops because something is missing or went wrong; it stands idle, says why in the status line ("Idle: ...") and looks again, until you stop it yourself. Mining waits for a suitable tool or a free inventory slot, gives up a block that does not break after 30 s or that the server refuses three times and goes on to the next, and keeps asking the planner when it is slow. Tunnel and Bridge stopped for any reason except your own pause (no blocks, no floor, a chunk not loaded, protected or hazardous block, no face to place on, a server refusal, a timeout) wait 3 s and try again by themselves. Drain waits for sand or gravel, for a route to a cell, and leaves a cell it cannot place. Farm and schematics leave a refused cell for a while, wait for a route, a free slot or the blocks, and a trip to the storage that fails no longer pauses the job.
- A progress watchdog for Farm, Drain and schematics: a job that does nothing for 20 s without saying why (a cell it cannot place, a route it cannot walk) is shaken loose; the cell is left for 10 s and the status says "got stuck, looking for another target".
- Drain: the "stays on one column" rule could hold the bot on a column it could not reach for good, which looked like "it just stops" with the status still "Running". It now waits only for a block that has just sunk (under a second) and leaves any cell given up on.
- Drain: sand and gravel only count as something to stand or hang on when they rest on a solid block that cannot fall. A block dropped into the surface is solid for a moment and then sinks; the bot used to stand on, or hang over, such a block and fall into the water with it. Routes along a laid line, the edge hang and the swim back to dry ground use the same rule.
- Tunnel and mining: "Server refused the break" - the tool change had to reach the server before the first hit (the server works out the break time with the item it believes is in hand and refused a block the client broke too fast); now the slot change gets a tick first. A refused block is also broken again up to three times; an ore block refused three times in a row is left alone, a tunnel or dig route stops only after three.
- Not verified in game.

## 0.34.6

- Farm: the items a harvest drops (and anything else lying in or around the farm) are picked up. Between two crops the bot walks to the nearest item that is still on the ground within the farm's radius plus three blocks and waits for the pickup; an item it cannot reach or take is given up on after a few seconds. Before, a bed's drops stayed on the ground unless the bot happened to walk over them.
- Farm: beds at other heights are found. The scan looked at one layer only (the height the bot started at), so a raised bed or a terrace was invisible; it now looks from two blocks below the start to two above.
- Farm: when no route to a bed or to the storage chest exists, the bot no longer pauses the job. It first tries a second pass over the same spots that may break and place blocks to get there (only after the plain walking route failed everywhere), and if that fails too it idles ("no route to the bed, looking again" / "no route to the storage, carrying on with the crops") and tries again later.
- Not verified in game.

## 0.34.5

- Farm: it can jump again. Routes among the crops were kept level (no jumps) to spare the farmland, which also stopped the bot from climbing a raised bed to reach the crops in its middle. Jumping up onto farmland is harmless (only landing on it from a drop tramples it), so the farm walks normally, still without breaking or placing blocks on the way.
- Farm storage: a chest in sight of the farm bed was not opened because crops stood in the line of the aim ray. The click now tries the middle, the top and the four sides of the chest; the spots the bot walks to for it are limited to those with a clear line to the chest (checked block by block, plants count); and when nothing of the chest is in sight from where it stands, it moves to another spot (up to three tries) instead of clicking at the same place.
- Half blocks in passages: slabs, carpets, snow layers and pressure plates (anything at most half a block high) are walked through by Navigation and the farm instead of being obstacles, and stairs and top slabs count as blocks to climb. Tunnel and Bridge treat a half block as something solid to stand on or to break. The shape is read from the block's real outline, new pure `navigation::low_shape` and `classify_shape` with tests.
- Not verified in game.

## 0.34.4

- Tools: a tool is kept in reserve before it breaks, now in proportion to its durability (about half a percent: 10 points on a netherite tool, 7 on a diamond one, at least 3) instead of a flat 10, and a fresh tool (over a fifth of its durability left) is used before a worn one, so two pickaxes (hotbar and inventory) share the wear and the job only stops when every suitable one is nearly worn out.
- Mining and Tunnel: blocks that need no tool (dirt, sand, gravel, leaves) are broken by hand when none of the carried tools is the right kind for them, instead of stopping with "A suitable tool is required".
- Tunnel and Bridge: when no face of a neighbouring block can be clicked to place a floor block, other points of each face are tried (the middle, then four offsets and two corners) before giving up, and at the edge the job waits 1.5 s before stopping with "No face to place a block on", which was stopping it on a dropped item or a late block update.
- Not verified in game.

## 0.34.3

- Mining: blocks at the foot and the top of a tree (or any block hidden behind grass, ferns, flowers or leaves) were skipped for good, because the aim ray met the plant first and the miner took that for an unreachable block, so only the clear middle of a trunk was cut. Now it breaks the plant or leaf block in the way first (not counted as a mined block, at most six per block, never torches, crops or anything of value) and then goes back to the block it wanted.
- Mining: the standing spots reach one block further up a trunk (up to five blocks below the log).
- Not verified in game. Logs far above the ground still need climbing, which is not implemented.

## 0.34.2

- Drain: it now places only sand, red sand and gravel. Any other block (it used to fall back to the cheapest building block when no sand was carried) would have been left standing in the water; with none of the three in the inventory the job says "Sand or gravel is required".
- Drain: one column at a time. The bot stays on a column, dropping the next block as soon as the last one has sunk (about 0.8 s instead of 1.4 s), until its surface cell is solid, and only then moves to the nearest next column, instead of hopping between columns while each one sinks.
- Not verified in game.

## 0.34.1

- Farm: the way to the storage chest is no longer restricted to one level. Walking among the crops stays level (no jumping onto farmland), but a trip to the chest uses the normal route, which climbs steps and goes around obstacles, so a chest on another height or well away from the beds can be reached (long routes are planned in segments as chunks load). The route still never breaks or places blocks.
- Farm: harvesting used to reset the walking mode to the unrestricted one while a crop was being broken, which could let the bot jump or dig on the way; harvest routes are now level and never break or place blocks.
- Not verified in game.

## 0.34.0

- Drain patterns: the Drain tab has a **Pattern** button (fill, lines, grid) and a **Spacing** button (3 to 8). Lines lay a row of blocks on the water surface every N blocks along the x axis, starting with the row the player stands on; the grid adds columns, so the water is cut into squares (spacing 3 leaves 2x2 squares). One block per surface cell, taken from the inventory (a block that stays put is preferred over sand and gravel). The job grows along its own blocks: it plans a route over the solid cells already laid to the spot next to the next cell, walks out along them sneaking (so a one-wide line is never walked off), hangs over the edge and places. A cell with no such route is left for a few seconds and the job goes on. New pure `draining::in_pattern` and `draining::flat_path` with tests.
- Farm: waiting for ripe crops, for light or for seeds is shown as "Idle" and the job carries on by itself, instead of looking paused. Farm routes are level and never jump (jumping onto farmland tramples it).
- Not verified in game.

## 0.33.0

- Mining and Tunnel find tools on their own: a pickaxe, axe or shovel anywhere in the hotbar or inventory counts (before, only the tool already in hand did, so both reported "hold a tool"). The right one for the block is put in hand, swapped up from the inventory when needed, and the old slot is not touched afterwards.
- Mining no longer stops with "No reachable ore found": it keeps searching every couple of seconds while blocks load and the player moves.
- Auto-eat: while a task runs and the food bar is at 7 shanks or below, the best fitting safe food (no rotten flesh, spider eye, raw chicken, golden apples...) is taken from the hotbar or inventory, eaten, and the old slot restored. A switch beside the status line of every Baritone tab ("Eat: on/off") turns it off; on by default.
- Bridge: width 1-3 next to the length (the floor is laid column by column).
- Farm: several crops can be picked at once (the icons toggle; one always stays selected). Each ripe crop is harvested and seeds of any selected crop are planted. With no seeds on hand and none in the storage the job idles ("Idle: no seeds, waiting for crops to ripen") and goes on when crops ripen, instead of pausing. The storage deposit keeps a stack of seeds per selected crop.
- Farm and Drain: the two glowing corner cubes that looked like stray holograms are replaced by an outline of the working area on the ground and a cube on the block being worked on.
- Drain: when the player falls into the water together with the block, the job swims back to the nearest dry ground and carries on instead of pausing.
- Coming back to the window no longer pauses a task: the click that focuses the window, and keys still held, are ignored for 1.5 s.
- Not verified in game.

## 0.32.0

- Farm storage: a **Storage** button on the Farm tab takes the chest, trapped chest, barrel or shulker box under the crosshair as the farm's storage (aim at it, open the menu, press the button; aiming at anything else while one is set clears it). It is your container, not the nearest one, and is remembered for the current world. When the inventory is nearly full, or no ripe crops are left, the farm walks to the container, opens it, shift-clicks the harvest into it (keeping a stack of seeds for replanting) and closes it. When the seeds run out it takes up to two stacks of seeds from the container instead of stopping. A container that is full, that cannot be opened (three tries) or that holds no seeds ends the job with a message. New pure `farm_storage.rs` with tests; furnaces, hoppers and other non-storage containers are not accepted.
- Not compiled against the game's own server and not verified in game.

## 0.31.0

- horizon-baritone bridge removed: every Baritone tab runs on the built-in drivers. The `baritone_bridge` module, its hook wiring and the documentation section are gone.
- Mining: no block limit any more; pick what to farm and start/stop with the action keys. After each broken block the miner now walks to the items it dropped and waits for them to be picked up (items it cannot reach or take are given up on after a few seconds) before looking for the next block. The nearest block is always taken next, so a vein or a whole trunk is worked through; standing spots up to four blocks below a log let it reach high up a trunk. The status line shows the running count and "Collecting drops".
- Navigation: blocks with no collision (ferns, grass, flowers, saplings, crops, torches, rails, redstone parts) are walked through like air. A bridge no longer stops in a fern.
- Bridge and Tunnel: whatever stands in the way is broken (bridge included), falling blocks above no longer stop a tunnel, a slab or stair under the path counts as floor, and the pause reasons are specific (cannot break this block, no floor, no face to place on) instead of "check the passage". Only unbreakable blocks (bedrock, chests) still stop the job.
- Drain: far faster (a placement every ~0.1-0.2 s instead of ~0.4-1.9 s) and a cell that is refused or unreachable is skipped for a few seconds instead of stopping the whole task; the end message counts skipped cells.
- Farm: crops need light 9 or more (sky or block light), so in a dark cave the server refuses every seed. The farm now skips unlit cells and says "No light on the farmland: crops need a light source (level 9+)" instead of failing with "Server refused". Also: a seed placement was declared refused when the server's acknowledgement arrived before the block update, and the click could be made before the new hotbar slot reached the server. The slot change now gets one tick to arrive and the block is checked again for 0.9 s before "Server refused".
- Farm: the crop is picked from four item icons (wheat, carrot, potato, beetroot; the name is the tooltip) instead of a button that cycles through them.
- A running task keeps working when the window is minimised or loses focus: "pause when the window loses focus" is switched off in memory (never saved) while a task runs and restored afterwards.
- TAB: with 80 or more players the list is split into four columns.
- Not verified in game.

## 0.30.0

- Player list (TAB): compact and redrawn. Rows are 16 px instead of 22, cells 168 px instead of 230, so more players fit on screen and more columns open on a normal window. The "PLAYER / ms" header rows are gone; the title carries the online count and a coloured TPS badge. Ping is shown as four signal bars that fill with connection quality, with the milliseconds beside them when the cell is wide enough. Rounded rows, a thin accent line, smaller faces; friends keep the green tint, the check mark and the distance.
- Sidebar: the two plain text cards are now a header with an accent bar and icon rows: FPS (gauge), TPS (activity), Online (people), Playtime (clock), Balance (coins) and the richest players, with muted names, right-aligned coloured values (FPS and TPS turn green, amber or red) and zebra striping. Three new official Lucide icons (`gauge`, `activity`, `coins`) added through `tools/update_lucide.py`; existing icon geometry is unchanged.
- Existing settings (columns, opacity, TPS, ping, distance, sidebar scale, width, corner and opacity) keep working.
- Not verified in game; layout checked only by compiling the real client.

## 0.29.7

- Mining: the target grid shows item icons (diamond, ingots, redstone, logs, stone, …) instead of text chips. The name appears as a tooltip on hover, in Russian or English; the selected state is unchanged. Tiles are 22 px squares, so far more fit in a row and the grid pages less. New `mining::icon` with a test that every target has an icon.
- horizon-baritone bridge: when that mod is installed and on but does not accept the command (for example it is built for another Nightly), the tab now runs the built-in driver and says so in chat. Before, the tab stopped with "Baritone did not accept the command", which left Navigation, Mining, Tunnel, Farm and Schematics dead whenever the mod was outdated.
- Not compiled against the real client and not verified in game.

## 0.29.6

- Drain fix: with sand or gravel the first cell next to the shore was never filled ("Осушение: 0 · Нет доступной грани … луч прицела упирается в соседний блок"). The only block to click on was the shore's wall facing the water, which cannot be hit from inland: the aim ray meets the block's top first, and the walking spots exclude every cell next to water. The drain now walks over the edge of the shore block, sneaking (the same hang as when bridging), until that wall is in the ray, and places from there. Each cell gets at most 3.5 s for this before the next spot is tried. New pure `draining::edge_stand` with a test.
- Drain is a built-in driver and does not use the horizon-baritone bridge, so it does not depend on that mod being updated to the current Nightly.
- Not compiled against the real client and not verified in game: the view layer builds only in the native-client workflow, which was not run here.

## 0.29.5

- Bridge to horizon-baritone (luhnox/horizon-baritone, a native port of Baritone 1.21.4): when that mod is installed and on, Navigation, Mining, Tunnel (without Floor), Farm and unrotated Schematics hand their task to it as `goto` / `goal` / `follow player` / `mine` / `tunnel` / `farm` / `build` through Horizon's native command hook, and the action keys send `pause` / `resume` / `cancel`. A command no mod ran never reaches the server. Bridge, Drain, Floor and rotated schematics keep the built-in drivers. New pure module `baritone_bridge.rs` with tests.
- Mining: the target button became a grid of toggles, like the structure filters on the map. Several targets can be selected at once; the miner takes the nearest of any of them and uses the Ores overlay marks of every selected ore as hints. The grid pages on small panels.
- Schematics: the file button did nothing when the folder was empty. It now creates and opens the schematic folder in the system file manager (and copies the path); with files present it opens a list to pick from, with an entry to open the folder. The bottom button opens the folder too.
- Farm: seeds were aimed at a point above the farmland's 15/16-high top, so the ray went past it onto the next crop or block and nothing was sown. The aim now targets the real top face; when the client's dry run cannot model seed planting, the click is sent and the server's answer confirms the crop.
- Drain fix: water deeper than 12 blocks reported hundreds of cells "without support" and did nothing. With sand or gravel the bot now drops blocks into the surface cell beside a solid block, so any depth fills from the bottom; other blocks keep the bottom-up mode and report that deep water needs sand or gravel. The task ends after several empty scans instead of one, because a sinking block briefly leaves its cell empty.
- Not compiled locally beyond the pure modules; not verified in game.

## 0.29.4

- The Baritone action keys also work while the menu is open on a Baritone tab and no text field is focused (the game does not deliver keys to the mod while a screen is open). Start closes the menu and begins the task; errors are shown in the menu feedback.
- Baritone keys have defaults: Numpad 1 preview, 2 start/resume, 3 pause, 4 stop (changeable in Options > Mods), so a task can be started without first assigning keys. The idle status now says to press the Start key instead of "No destination".
- Mining can collect 13 more targets besides the ores: stone, cobblestone, dirt, sand, gravel, clay, wood, obsidian, glowstone, nether stone, sandstone, end stone, pumpkins and melons. A pickaxe, axe or shovel is accepted; tunnels and dig routes still require a pickaxe.
- Baritone mining messages ask for a suitable tool (pickaxe, axe or shovel) instead of only a pickaxe. Tunnels and dig routes still need a pickaxe.
- New workflow `auto-version.yml`: a same-repository pull request that leaves the version unchanged gets its patch version raised automatically, with a stub CHANGELOG entry and release note from the title (`tools/auto_version.py`, tested in `tools/tests`). Forks are not touched. Commits pushed with the workflow token do not start other workflows; the merge to main runs the full verification.
- Not compiled or tested in game yet beyond the pure modules and CI.

## 0.29.3

- Baritone > Navigation: coordinates are three separate X / Y / Z fields (pasting `x y z` fills all three). The Preview / Go / Resume / Stop buttons are removed from every Baritone tab; use the four configurable keys (preview, start/resume, pause, stop) in Options > Mods. Entered settings now survive closing and reopening the menu.
- Navigation no longer gets stuck: when no walkable route exists or the character stalls, it plans again with permission to break blocks and place a block under a gap, executes the route step by step, re-plans after every route and stops with a visible reason after a bounded number of failures. Vertical pillaring is not supported.
- Mining: uses the same ore table as the Ores experiment. Overlay marks are tried first, then loaded chunks are scanned (radius 24); buried ore is reached by a dug route.
- Tunnel and Bridge spend any suitable building block the player carries (hotbar, off hand or inventory), not a fixed list. Stone-like blocks go first. Shulker boxes, chests and other containers, redstone parts, workstations, TNT, slabs, stairs, ores, storage blocks of metals and gems, and anything that is not a plain solid cube are never placed; the check also uses the game's own block data.
- New Baritone > Drain tab: fills a pond from the bottom up within a radius of 3–12 with blocks from the inventory, sand and gravel included, only on cells that rest on something solid. Walks to the next cell when it is out of reach, stops when no supported cell remains and reports how many were left.
- Tunnel: looks ahead and keeps walking instead of working one block at a time. The vanilla 5-tick destroy delay still bounds the rate per block, even with Efficiency 5 and Haste 2.
- Bridge: no sneaking while walking onto confirmed blocks; blocks from the off hand and the inventory (moved into the hotbar) are used.
- Farm: seeds are found in any inventory area, standing spots are ordered so the aim ray does not hit neighbouring crops, and the status line shows why nothing was sown. The root cause of the old behaviour was not reproduced in game.
- View-layer code was not compiled locally; only the pure modules were tested. Compile the real client with the native-client workflow.

## 0.29.2

- Fix the build: pin `rust-toolchain.toml` to Rust 1.99.1 instead of 1.83.0. The crates use `edition = "2024"` and `rust-version = "1.95"`, which Rust 1.83 cannot parse, so a local `cargo build` through rustup failed before compiling anything.
- Align `.github/workflows/horizon-compatibility.yml` with `release.yml` and `docs/development.md`: install and use Rust 1.99.1 rather than 1.83.0.
- Fix `python3 tools/stage.py` failing with `pinned source hash mismatch: crates/horizon-client/src/game.rs`. The pin already targeted official Nightly `fdc87ae`, but the recorded `game.rs` hash still belonged to the previous Nightly `2ebf00a`, whose `game.rs` upstream has since split into several modules. Refresh it with `tools/compatibility.py --update`; the other nine pinned hashes already matched.
- Checked against `fdc87ae`: all four injection targets that live in `game.rs` still exist, and `screen_over_playing`, used by two adapters, moved to `game/pointer_capture.rs` as `pub(super)`, so it stays reachable from the injected modules. Applying the mixins and compiling the real client are still to be confirmed by CI.
- No runtime behavior, settings or data formats change. Keep the official Nightly pin and the legacy settings namespace.

## 0.29.1 (PR #14 review, unreleased)

- Register configurable Baritone start/resume, pause and stop keys in Horizon's Options > Mods page. Bindings are unassigned by default and use Horizon's existing persistence; ignore releases, obstructed gameplay, modifiers and conflicting gameplay/native-mod bindings. Starting an active job does not restart it.
- Reject action-plan steps beside unknown terrain, including adjacent support and overhead cells. Dig/Bridge routes remain diagnostic only: no unverified world edits or action routes are fed to walking input.
- Clear stale coordinate fallback and show a localized notice when a selected navigation player or waypoint disappears. Adapt the waypoint regression to explicit dropdown selection.
- Restore PR #14 verification, fix the previously failing waypoint test and Clippy errors, and format the author's new planner/UI code.
- Preserve the author's facing-based construction, default floor, compact controls, target dropdowns and bounded recovery. Keep the official Nightly 2ebf00a pin and legacy settings namespace.
- Remaining PR requirements: sequential server-confirmed Dig/Bridge execution, full permissions/tool/resource safety integration and live in-game end-to-end validation. This review build is not a completed autonomous action navigator.

## 0.28.1

- Chat translation no longer touches messages that are already in the target language. Translating into English, lines such as `im at spawn` were sent to the provider and shown back rewritten (`I'm at spawn`). They are now skipped before sending when the local detector identifies the target language, or when plain-ASCII text cannot be identified and the target is English, as before language selection existed.
- A provider result is discarded when the provider reports the text was already in the target language, or when only case, spacing or punctuation changed.
- An explicit **Input** / **Chat from** language is trusted, except that a source equal to the target is never translated. With a non-Latin source such as Russian, text containing none of that script (for example `yea i know`) is left alone instead of being forced through the provider as Russian and rewritten.
- Outgoing messages use the same rule, so an unnecessary translation no longer ends with a "service unavailable" notice.

## 0.28.0

- Add Baritone > Farm: choose wheat, carrots, potatoes or beetroot, radius 3–12, harvest limit or continuous operation, and optional replanting. Harvest only mature crops, count server-confirmed breaks, and plant empty farmland with seeds from the hotbar.
- Add Baritone > Schematics: import Sponge/WorldEdit .schem v2/v3 and Litematica .litematic v4–v7, including signed and multiple regions. Select integer XYZ origin and 0/90/180/270 rotation, preview up to 256 nearby block outlines, copy the material list, and build from hotbar items.
- Decode bounded GZip/NBT imports on an idle worker. Reject path traversal, oversized files/regions, malformed palettes and versions newer than the client. Ignore entities and block-entity NBT.
- Reuse normal movement, attack and use input. Check each dry placement against expected states and confirm all affected cells after the server acknowledgement; pause on conflicts, unavailable material, manual input, refusals or timeouts. Do not demolish occupied schematic cells.
- Keep farm routes level to avoid automatic jump instructions. Preserve pending acknowledgements across pause/resume, restore the original hotbar selection, and discard jobs/import results on account/world changes.
- Keep the official Nightly 2ebf00a pin and existing settings/data namespace.

## 0.27.0

- Add Baritone Tunnel and Bridge modes with cardinal direction, length, preview, pause/resume and stop. Tunnels support widths 1–3, heights 2–3 and optional floor filling.
- Clear tunnel sections from top to bottom using ordinary digging and server acknowledgements. Stop at fluids, unloaded terrain, falling blocks, containers, unbreakable blocks or unsuitable tools.
- Build a one-block-wide floor from suitable full blocks in the hotbar. Approach the edge while sneaking, validate the clicked support face and a dry placement preview, then wait for the server before walking onto the new floor.
- Restore the original hotbar slot after automatic placement while respecting manual slot changes. Keep pending confirmations through pause/resume and cancel construction on world/account changes.
- Keep the official Nightly 2ebf00a pin, existing navigation/following/ore mining and saved user data.

## 0.26.0

- Target the latest official Horizon Nightly 2026-11-07 source 2ebf00a, with refreshed source hashes and native validation.
- Add Baritone > Mining: choose an ore and a 1–64 block limit, preview or mine reachable exposed ore from loaded chunks. Use ordinary attack processing and count only server-confirmed block changes.
- Mining checks the held pickaxe, remaining durability, a free inventory slot, actual reach and entity obstruction. Avoid support under the player, adjacent fluids and falling blocks above the ore; stop on server rejection or timeout.

- Add an experimental native walking navigator in the Baritone tab: enter XYZ or select a waypoint in the current dimension, preview a route, start, pause, resume and stop.
- Calculate bounded A* segments on a background worker using snapshots of loaded server chunks. Render the remaining route and a localized HUD status with distance.
- Walk and jump up one full block using normal movement input. Avoid unknown terrain, hazardous blocks, unsupported shapes and drops greater than one block.
- Follow a nearby loaded player by UUID, maintain a short distance and replan as the player moves. Stop moving immediately when the target disappears; stop following after a 10-second loss and require Resume.
- Replan automatically when terrain changes or movement stalls, with at most three recovery attempts near one location. Wait for landing before planning another segment.
- Pause on manual movement and open screens. Cancel pending work and clear the destination on disconnect, dimension/account/world changes or mod disable.
- This is an independent Rust navigator, not the Java Baritone library. Block placement, tunneling, farming, schematics and flight are not included.

## 0.25.1

- Split experimental settings into Ores and Baritone tabs in Russian and English, including compact layouts.
- Preserve ore settings and the existing consent gate. Switching tabs never enables highlighting.
- Baritone is an informational placeholder; movement automation is not integrated in this build.

## 0.25.0

- Presence uses one-line, direction-colored Lucide status marks before the name. Normal joins/leaves are green/red; friends and important players use cyan/lavender.
- Add a saved Icon and name / Text presence setting. Text mode restores localized joined/left and first-visit labels.
- Merge PR #11: launcher bundle ID is draimhorizon; old settings/data remain under claim_borders. This is a manual reinstall migration, not an in-place launcher update; custom bindings and activation state may reset.
- Retain the official Nightly policy and verified 6560641 source; do not adopt the main snapshot from the older PR.


## 0.24.1

- Target the official Horizon Nightly 2026-11-07 source commit 6560641, instead of Horizon main.
- Resolve and check the published Nightly tag in the compatibility tool and release CI. Keep the tested full commit SHA and source hashes reproducible.
- Document the mandatory official Nightly policy in AGENTS.md; add regression checks for annotated tags, missing tags and avoiding a main fallback.


## 0.24.0

- Show X/Y/Z in one styled minimap coordinate strip; allow editing all three coordinates in the X waypoint editor.
- Use compact + / minus presence events while retaining their original server text.
- Include ordinary messages from friends and important players in the Friends chat view, without moving messages out of other tabs.
- Enable auto reconnect on first use, with a saved toggle, Horizon's countdown, cancellation and bounded backoff. Do not reconnect after manual exit or permanent access/authentication refusals.
- Use top settings tabs, centered language controls and a centered Experiments confirmation with Lucide warning/check icons and soft action colors.
- Require official Lucide UI and documentation icons in AGENTS.md.
- Refresh Horizon compatibility to 6cc93d1.


- Chat translation button is now a compact 39 px direction tag (for example `EN>RU`; `?` means auto-detected input) with no `:...` suffix. The chat input box starts right after it. Left click toggles outgoing translation; right click opens a full-screen **Translation languages** screen.
- Four independent choices — **Input** (language you write in, or Auto detect), **Output** (language sent), **Chat into** (language incoming chat is translated into) and **Chat from** (language incoming chat is written in, or Auto detect) — each from 20 common languages: English, Russian, Finnish, Polish, German, French, Spanish, Italian, Portuguese, Ukrainian, Dutch, Swedish, Danish, Czech, Turkish, Arabic, Hindi, Japanese, Korean, Chinese. An explicit Input or Chat from is sent to Google (`sl`) and DeepL (`source_lang`). Choosing **Chat from** fixes short messages that auto-detection skips; it also bypasses the common-languages filter.
- First start after the update keeps the previous behaviour instead of Auto: RU interface gives `RU>EN`, EN interface gives `EN>RU`, and received chat is translated into the interface language. The result is saved once; existing choices are never overwritten.
- **Auto-translate: common languages only** (on by default) skips text whose detected language is outside those 20, without a network request. The filter reuses the detector that already labels each chat line with its language badge (script plus common-word vocabulary), so what is skipped matches what you see; text it cannot identify is translated.
- The common-languages filter skips a line only when the language detector positively identifies an uncommon language (for example Greek, Hebrew or Thai script). Tiny words, typos, slang and mixed lines (for example `claim команда`) are translated instead of silently dropped.
- Translation into English no longer requires Cyrillic source text; all targets share one eligibility rule.
- Settings are stored atomically in `translation_preferences`; an invalid file is never overwritten, the screen shows an error and keeps your selection. Esc cancels, Enter saves, and the unsent chat draft and caret are preserved.
- Relay 1.3.0 accepts the 20 DeepL targets and an optional validated source language; older clients keep working, but the shared DeepL fallback needs this relay for non-RU/EN targets.

## 0.23.1

- Target current Horizon main c78187d; prevent release with a stale source pin.
- Use official Lucide outlines, a single-line DraimHorizon title and an icon-only settings button.
- Compact waypoint rows and delete own Seed Map waypoints with a double left click.
- Ask once for explicit consent before opening Experiments; declining keeps features unchanged.
- Launcher limitation: GitHub native mod rows still display the stable bundle ID; Horizon Launcher needs display-name support.


## 0.23.0

- Gray original-language badges appear after chat and private messages in normal and compact layouts. Provider detection refines the local estimate, and unknown text uses ??.
- README now explains each feature visually with instructions and real chat examples.
- Expanded AGENTS.md and added a synchronized major/minor/patch version manager. Every tracked change requires a version increment.

## 0.22.7

### English
- Seed Map searches wide views progressively in cached 1024-block tiles instead of dropping whole structure sets or stopping at 512 candidates. The 2 km landscape view is fully covered; progress is shown while new tiles load. POI clustering is world-aligned and type-specific, so panning does not arbitrarily swap a village for a mineshaft icon.

### Русский
- Seed Map последовательно ищет структуры в кешируемых областях по 1024 блока вместо пропуска целых наборов или остановки на 512 кандидатах. Широкий вид с линейкой 2 км покрывается полностью; во время загрузки новых областей показан прогресс. Группировка POI привязана к миру и типу структуры, поэтому перемещение карты не подменяет деревню значком шахты.

## 0.22.6

### English
- Seed Map: click an individual POI to mark it found and dim its icon; click again to undo. Marks persist locally per server/world, account, seed and dimension. Dragging still pans; zoom into clusters before marking individual locations.
- Chat wraps at spaces instead of splitting words that fit on a full line. Explicit newlines are preserved; oversized words/URLs retain a character-wrapping fallback to avoid clipping.
- Localize live claim-list dimensions and observed-owner labels on the claims screen and map; waypoint dimension labels also follow the RU/EN switch. Owner and reporter names remain unchanged.
- Preserve leading colons in chat messages such as `:3` and `:`; repeat filtering no longer merges them with messages lacking those colons.
- Settings → Chat → Appearance: **Compact messages** reduces card padding and gaps without changing font size or multiline spacing. The setting persists; normal spacing remains the default.
- Fullscreen claim fills, outlines and the chunk grid are drawn independently of terrain. Borders retain screen-pixel thickness during fractional panning and in wide viewports.
- Terrain shading reads neighbouring region heights, removing artificial lines on continuous slopes at 256-block boundaries.
- A world-aligned, buffered terrain image is reused during small drags; unchanged results no longer trigger full texture uploads. The buffer preserves the previous visible texel density.
- Views exceeding the detailed-region cache use existing chunk overviews instead of repeatedly decoding and evicting the visible regions. Distant terrain is therefore coarser; saved exploration and its file format are unchanged.

### Русский
- Seed Map: щелчок по отдельной структуре отмечает её найденной и затемняет значок; повторный щелчок снимает отметку. Отметки сохраняются локально отдельно для сервера/мира, аккаунта, сида и измерения. Перетаскивание двигает карту; для отметки отдельной структуры в группе сначала приблизьте карту.
- Чат переносит текст по пробелам, не разрывая слова, помещающиеся в полную строку. Явные переводы строк сохраняются; слишком длинные слова и ссылки переносятся по символам, чтобы избежать обрезания.
- Названия измерений в списке приватов, подписи наблюдений в меню и на карте, а также измерения в метках учитывают переключатель RU/EN. Ники владельцев и наблюдателей не изменяются.
- Сохраняются начальные двоеточия в сообщениях чата, например `:3` и `:`; фильтр повторов больше не объединяет их с сообщениями без этих двоеточий.
- Настройки → Чат → Оформление: «Компактные сообщения» уменьшают вертикальные отступы карточек и промежутки между ними, не меняя размер шрифта и межстрочный интервал. Параметр сохраняется; прежний вид остаётся по умолчанию.
- Заливка приватов, контуры и сетка чанков полноэкранной карты рисуются отдельно от рельефа. Толщина контуров в экранных пикселях сохраняется при дробном смещении и в широких окнах.
- Затенение рельефа использует высоты соседних регионов, убирая искусственные линии на непрерывных склонах через каждые 256 блоков.
- При небольшом перетаскивании переиспользуется привязанный к миру снимок рельефа с запасом по краям; неизменённые результаты больше не загружают текстуру заново. Прежняя плотность видимых пикселей сохранена.
- Если видимая область превышает кеш подробных регионов, используются существующие обзоры чанков вместо постоянного чтения и вытеснения регионов. Дальний рельеф становится грубее; сохранённая карта и формат её файлов не изменены.


## 0.22.5

- Переключатель RU/EN теперь охватывает карту, Seed Map, экономику и торги, инструменты инвентаря, оформление, подсветку руд, окна телепортации и дополнительные подписи чата и relay.
- Описание мода и подписи клавиш в Options → Mods, а также предупреждения инвентаря и оформления обновляются при смене языка.

## 0.22.4

- Relay 1.2.0 поддерживает общий резерв DeepL. Оператор хранит API-ключ на сервере, а игроки используют токен приватного relay.
- Личный ключ DeepL можно сохранить и удалить в меню J → Настройки → Чат → DeepL.
- При сбое одного адреса Google мод использует доступный адрес. Подписи входов, выходов и первого визита переключаются между RU и EN локально.

## 0.22.3

- Меню J переключается между русским и английским. В английском режиме входящие переводятся на EN, исходящие на RU.
- DeepL используется как резерв для Google при наличии локального ключа.
- В «Друзья → Важные» можно сохранять и удалять правила по нику, включая офлайн-игроков.
- 3D-границы приватов выключены по умолчанию. Для включения используйте `/claimborders on` или настройки карты.

## 0.22.2

- Миникарта под крышей, в Незере и в режиме «Пещеры» обновляет срез по текущей высоте игрока с шагом в один блок. Рельеф берётся из загруженных чанков.

## 0.22.1

- `/pay` и другие команды с ником первым аргументом используют штатный выпадающий список Horizon: фильтрация по префиксу, Tab/Shift+Tab, стрелки, мышь и Escape. Имена берутся из текущего списка игроков; сумма перевода и текст сообщений не заменяются.
- Под миникартой дополнительно показана высота Y.
- Из скорборда, HUD, торговых карточек и настроек убраны отменённые дневные лимиты продаж. Старые нулевые/неизвестные лимиты больше не блокируют оценки и подтверждение продажи; проверки свежести цены, предмета, истории обратной продажи и явное подтверждение сохранены.
- Поиск и обычные кнопки сортировки инвентаря рисуются до переносимого предмета и его подсказки. Модальное редактирование сортировки остаётся поверх окна.
- Вместо нарисованных условных пиктограмм на карте и миникарте показываются настоящие головы: проекция частей baked-модели с текущей текстурой конкретного моба из атласа Horizon. Сохраняются форма головы, уши/морда и варианты текстур; ресурсы Minecraft в мод не включаются.

## 0.22.0

- Единые настройки открываются нижней шестерёнкой: основные, связь и друзья, карта/приваты, чат, торги, TAB и Scoreboard. Параметры TAB и Scoreboard отделены от торгов; прежняя вкладка «Опции» удалена.
- Баланс скорборда распознаёт сокращения k/M/B и обновляется в фоне без открытия Торгов. Остаток продаж запрашивается отдельно; при отсутствии серверных данных вместо «— / —» показывается пояснение. В SMP 0.33+ дневные лимиты продаж удалены; отсутствие ответа не считается нулевым или бесконечным лимитом.
- Друзья: заявки с галочкой и крестиком находятся в «С модом», без отдельной вкладки приглашений. Relay и Auto Trust перенесены в настройки, проценты HP убраны.
- Приваты: список известных действующих чанков, поиск по владельцу, фильтры своих/дружественных и чужих. Кеш чужих территорий остаётся последним известным снимком, а не гарантией текущего владения.
- Вкладка чата содержит перевод входящих и исходящих сообщений. Оформление, уведомления и антиспам перенесены в общие настройки; отдельной вкладки «Важные» нет. Эксперименты получили иконку.
- На карте и миникарте мобы показаны небольшими оригинальными иконками голов вместо квадратных точек. Фильтр типов на исследованной карте сохраняется между входами; категории и отдельные типы комбинируются.
- Метки: увеличенные карточки, цветная плитка иконки, отдельная строка координат и больше воздуха между действиями. Торги и серверные карточки стали компактнее.
- Seed Map: города Края без корабля и города с кораблём/элитрами выбираются независимо. Корабль определяется по реальным частям структуры генератора; прежний фильтр городов мигрирует на оба вида.
- Source pin Horizon остаётся f319701; после установки нужна пересборка нативных модов. Игровую проверку ресурсов, масштабов GUI и соответствия прогноза серверной генерации следует выполнить в клиенте.

## 0.21.1

- Исправлено переключение исследованной карты на Y-срез при изменении высоты и освещения. Поверхность теперь режим по умолчанию; старый «Авто» мигрирует в поверхность.
- Пещеры включаются вручную; сканирование и история используют одну каноническую высоту внутри 16-блочной полосы, без пересчёта при каждом изменении Y на блок.
- Поверхность другого измерения больше не подменяется случайным сохранённым пещерным слоем. Подсказка кнопки рельефа показывает текущий режим; пещерный режим подсвечивается.
- Сохранённая история не удаляется; отсутствующая поверхность заполняется при повторном посещении территории.

## 0.21.0

- Быстрые действия в списке друзей: выбрать игрока и нажать «Запрос ТП» или «Обмен» для `/tpa <ник>` и `/trade <ник>`. В компактном интерфейсе кнопка телепорта называется «ТП»; подсказки показывают команды.
- Доступность определяется присутствием на игровом сервере независимо от relay; перед отправкой повторно проверяются UUID и ник. Офлайн-игрокам и себе запросы не отправляются.
- После отправки меню закрывается; сервер управляет принятием запросов, телепортом и обменом. Действия дружбы и авто trust сохранены.

## 0.20.0

- Важные игроки видны во вкладке друзей с цветной звездой и подписью. Это не выдаёт статус взаимной дружбы и доступ к телеметрии.
- Локальная история современного чата: до 500 сообщений отдельно для каждого сервера и профиля, включая ЛС, исходный текст и готовый перевод. Архивные сообщения не создают непрочитанное, уведомления и повторный перевод.
- J → Эксперименты: разрешение подсветки руд, выбор типов и источника. По умолчанию запрещена; O переключает разрешённую подсветку, после входа в мир она всегда выключена.
- Режим загруженных чанков показывает данные клиента в ближайших 3×3 чанках; добытые блоки исчезают. Экспериментальный прогноз сида SMP рассчитывается в фоне для текущего чанка и рисуется разорванными рамками. До 256 ближайших меток в радиусе 128 блоков.
- Прогноз не учитывает добычу, серверные datapack, старую генерацию и изменения мира; точность совпадения руд с сервером требует игровой проверки. Переводчик не изменён.

## 0.19.0

- M открывает исследованную карту; полноэкранный интерфейс с компактными инструментами, вкладками измерений и плавным перемещением/масштабированием готового изображения.
- Цвета верхних текстур блоков текущего ресурс-пака, биомная окраска и усиленное затенение рельефа. Палитра готовится в фоне; сбор цветов ограничен 16 чанками за проход.
- Формат истории v2 сохраняет цвета поверхности; регионы v1 читаются и обновляются без потери исследования при повторном посещении.
- Seed Map показывает одно выбранное измерение. Вместо списка — поиск, категории и плитки фильтров с иконками предметов. Метки используют те же иконки, плотные группы имеют счётчик.
- Полный каталог 34 структур vanilla, включая все варианты, шахты, ископаемые Незера и руины троп. Поиск использует штатные правила размещения, частоты и исключения. Старые фильтры мигрируют.

## 0.18.0

- M открывает Seed Map с тремя независимыми панелями измерений; J — меню настроек. Прежняя стандартная пара мигрирует один раз; прочие пользовательские переназначения сохранены.
- Сид SMP -3292718948513116954, расчёт биомов Y64 и проверенных структур штатным генератором vanilla Java 26.3 в отменяемом фоновом потоке.
- Сохраняемые фильтры биомов, отдельных типов структур и Claims; координаты и подсказки при наведении.
- Приваты, игроки, метки и смерть сохраняются; исследованная карта доступна отдельной кнопкой.
- Независимый зум/перемещение панелей, ограниченный поиск при далёком масштабе, ошибки ресурсов без блокировки интерфейса.

## 0.17.0

- Карта показывает последнюю смерть по данным Horizon. Кнопка «Смерть» открывает её измерение и центрирует карту.
- Современный чат получил вкладки «ЛС», «Пати» и «Торги» со счётчиками непрочитанного. ЛС выделяются и переводятся, `/r` и `/reply` отвечают последнему отправителю. Tab подставляет онлайн-ник в `/tp`, `/msg` и `/trade`.
- Избранное аукциона сохраняет пороги за штуку для каждого сервера и подсвечивает подходящие лоты на открытой странице.
- Зум камеры на Z с колесом, плавным переходом и снижением чувствительности мыши.

## 0.16.0

- F8 и кнопка стандартного вида сохраняют выбор для всех страниц и следующих запусков; аукцион и Market настраиваются независимо. F8 работает и для возврата из стандартного вида.
- Серверные подтверждения выставления и продажи лотов выделяются цветом и закрепляются в блоке важных сообщений; обычный поток чата не вытесняет недавние уведомления.
- Совместимость обновлена до Horizon Nightly 2026-11-05 · `f319701` (native hooks API 409).

## 0.15.2

- Обновлена точная совместимость с Horizon Nightly 2026-11-04 · `67dad0d` вместо `f42ef42`; пересчитаны контрольные суммы исходников.
- horizon-mixins закреплён на `e8cd0aa`, как в лаунчере этого nightly; недоступная прежняя ревизия `10fdf8b` больше не нужна.
- Обновлена проверка настоящего клиента в CI и инструкция по пересборке.

## 0.14.0

- Единые полупрозрачные панели со скруглениями, мягкими тенями и спокойной палитрой для всех экранов мода.
- Контурные иконки навигации, действий, HUD и сортировки без дополнительных игровых ресурсов.
- Плавное появление окон/карточек и подсветка наведения/выбора; интерактивные области остаются на месте.
- Общие настройки оформления на Ctrl+, и через шестерёнку: фон, акцент, эффекты, тени, контраст; предпросмотр, отмена и сохранение.
- Более понятные заголовки и пояснения; локализованные названия обычных предметов в торговле и поиск по кириллице.
- Новые проверки геометрии, анимаций и настроек; прежние данные и pins Horizon/relay совместимы.

## 0.13.0

- Открытие опорного хранилища сквозь рамки, светящиеся рамки и таблички; Shift сохраняет обычное действие, отдельные переключатели.
- Сортировка основных 27 слотов инвентаря и обычных хранилищ, включая шалкеры, воронки, раздатчики, выбрасыватели и грузовые слоты животных. Хотбар, экипировка и служебные слоты исключены.
- Вертикальные кнопки S/F/R/T у обеих областей: сортировка, дополнение стаков, пополнение имеющихся видов, перенос.
- Порядок по ID, имени и количеству; строки/столбцы; разовый режим через Shift/Ctrl/Alt + СКМ.
- Редактор Ctrl+O/E: исключения слотов, видимость, включение, смещения, шаблоны, сброс и отмена.
- Защита виртуальных серверных окон и торговых меню; проверка каждой перестановки штатным движком меню перед отправкой обычных кликов.
- Сохраняется точная совместимость с Horizon f42ef42 и relay 1.1.0.

## 0.12.0

- Совместимость с Horizon f42ef42, новые контрольные суммы точного исходного commit.
- Зафиксировано исправление horizon-mixins из PR #25 для встроенных атрибутов Rust в новом клиенте.
- Изолированный remote workflow: тесты, clippy, стадирование, регрессии инструмента, настоящая компиляция клиента и валидатор пакета.
- Совместимые прежние настройки, ID мода и relay 1.1.0.

## 0.11.0

- TPS в sidebar и Tab: сглаженная оценка по серверным пакетам времени, состояния ожидания/паузы, переключатели отображения.

- Исправлены реальные ответы `/worth` с запятой, дробными ценами, односторонней торговлей и неторгуемыми товарами; автоматические ответы скрываются до штатного и кастомного чата.
- Исправлен голод опроса большого списка товаров; опрос работает только при открытом торговом UI, неподдерживаемые товары исключаются до ручной перепроверки.
- Торги на B: компактное окно, без sidebar, сортировка AH, полноценные условные расчёты за весь лот и по остаткам лимитов. Маленькие подсказки вне инвентаря вместо полосы на весь экран.
- Все налоги удалены из модели, настроек и интерфейса; старые значения больше не восстанавливаются.
- Новый прозрачный Tab с тремя колонками и читаемым пингом; новый sidebar с финансовыми блоками и понятным остатком дневного лимита, без дублирующего HUD.
- Наблюдения чужих приватов через `/claim info`: сохранение, общий обмен, снятие устаревшей отметки при подтверждённом свободном чанке, красные границы и клетки на картах, без требования мода/дружбы у владельца.

## 0.10.0

- DraimHorizons: новое публичное название и имена релизных архивов, совместимый прежний ID и конфигурация.
- Экономика: наблюдаемые цены, история, Market/AH, ручной безопасный обход страниц, целая стоимость лота, лимиты, cooldown покупок, условные оценки прибыли и подтверждаемая продажа руки.
- TAXES отключены: нулевые Market/VAT по умолчанию, старые счета игнорируются, отдельный учёт неизвестной комиссии объявления.
- Расширенные настройки экономики, HUD и серверных подсказок; свои sidebar/Tab с переключателями, друзьями, пингом и страницами.
- Автоматический trust взаимным друзьям с подтверждением сервера и уважением ручного untrust.
- Метки двойным ПКМ на карте и клавишей X; расстояние при наведении на голову друга.
- Карточки trade accept/deny; настраиваемые важные игроки в чате и закреплённые сообщения.

## 0.9.0

- Асинхронный перевод исходящего русского чата на английский: отдельная настройка, сохранение порядка/оригинала, обычная подписанная отправка, исключение команд и возврат к оригиналу при ошибке.
- Постоянный кеш своих и чужих приватных территорий; начальный `/claims` только без сохранённых данных, обновление по успешным claim/unclaim.
- Независимый от пати E2E-обмен списками территорий между всеми участниками мода одного сервера/relay, атомарная сборка страниц, закрепление ключа и сохранение офлайн.
- Карточки TPA с точными серверными командами, кнопками, оставшимся временем и полоской на 60 секунд; защита от повторного/просроченного клика.
- Мобы на миникарте и большой карте: категории, высота и режимы радара.

## 0.8.0

- Постоянное хранилище исследованного рельефа по серверам/аккаунтам, мирам, измерениям и пещерным слоям; восемь переключаемых атласов.
- Фоновая атомарная запись регионов, ограниченный кеш и очередь, обзор для далёкого масштаба, сохранение при выходе и изоляция повреждённых файлов.
- Большая карта с текстурой 512×512, широким видом и нижней панелью управления; тени высот/воды без прежней шахматной яркости.
- Компас, направление игрока и масштабная линейка на обеих картах; G переключает сетку.
- Страница «Память» в Клеймах, настройки теней и записи; старые карты не стираются при переключении.

## 0.7.0

- Автоподключение сохранённого relay и настройка его включения.
- Списки пользователей мода, друзей и приглашений вместо ручного ввода кодов; принятие и отклонение запросов, закрепление ключа выбранного контакта.
- Головы вместо текстовых мировых меток игроков, нативная интерполяция близких сущностей и буфер снимков для дальних.
- Головы игроков и друзей с рамками на картах; цветные ромбы waypoints.
- Распознавание системных событий чата по ключам компонентов независимо от локализации и поддержка русских текстовых объявлений.
- Сортировка верхних слотов сундука на СКМ обычными кликами с проверкой плана и остановкой при изменении содержимого.
- Удалён старый Party / HP HUD. Relay 1.1.0 остаётся совместимым.

## 0.6.0

- Большая карта J и миникарта на штатном движке Horizon: рельеф загруженных чанков, слои приватных участков, игрока, друзей и waypoints.
- Зум к курсору, перетаскивание, выбор измерения, переход к игроку/своим участкам и информация о чанке под указателем.
- Клеймы разделены на карту, 3D-границы, HEX-цвета и сортируемый список; шаг сетки 3D и толщина линий доступны в меню.

- Сохраняемые настройки ширины, высоты, масштаба, угла экрана и сдвигов чата.
- Четыре режима уведомлений, отдельные переключатели входов/выходов и смертей, длительность и прозрачность фона.
- Объединение повторов со счётчиком ×N, режимы подряд/за интервал и игнорирование временного префикса.
- Клик по карточке в открытом чате переключает оригинал и перевод; учитываются масштаб, положение и границы карточки.
- Relay 1.1.0 остаётся совместимым.

## 0.5.0

- Relay 1.1.0: без квот подключений/IP/realm и частоты по умолчанию; начальный каталог не забивает очередь.
- Передача координат до 20 Гц, объединение старых снимков в обе стороны, heartbeat и восстановление после обрыва, интерполяция.
- Список до 64 друзей, компактное меню с боковой навигацией в стиле социальных панелей.
- Карточки чата с головами, отдельные уведомления входов/выходов и смертей, устранение дублирующих объявлений первого входа.
- Фоновый автоперевод на русский через Google с переключателем и сохранением оригинала.
- Полноценный курсор и выделение в полях; исправлена лишняя V при вставке.

## 0.4.0

- Отдельный WebSocket relay: E2E-шифрование, закрепление ключей друзей, взаимные приглашения по коду.
- Настройка URL/токена и управление подключением в меню Party; обмен через `/msg` удалён.
- Готовый сервис с Docker Compose/Caddy и отдельным архивом для своего хоста.
- Меньшие метки фиксированного экранного размера без расширения вдали.
- Интеграционные проверки двух Rust-клиентов, Python relay и запуска контейнера.

## 0.3.0

- Игровое меню M с вкладками Waypoints, Party и Claim Borders; клавиша переназначается.
- Списки, страницы, поля ввода и кнопки заменяют необходимость управления только командами.
- Визуальные настройки границ сохраняются: свои/дружеские территории, цвета, прозрачность, дальность, вертикальный охват.
- Режим сквозь блоки и сетка вокруг игрока для ориентации в пещерах.
- Отдельное подтверждение подписанного обмена в меню; общий переключатель не меняет отдельные флаги видимости.
- Тег и GitHub Release v0.3.0 для обновления через GitHub-источник Horizon Launcher.

## 0.2.0

- Взаимные приглашения в друзья, сохранение по серверу и аккаунту, удаление.
- Клиентский обмен через `/msg` с явным выбором signed/unsigned на каждом подключении.
- Проверка UUID/онлайн-профиля, сессий, порядка кадров и полноты страниц приватов.
- HUD онлайн-друзей с процентом HP; метки и голубые границы друзей.
- Waypoints: создание, название, цвет, удаление, сохранение, расстояние и затухание.
- Исправлены встречные приглашения и транзакционное сохранение дружбы.
- Настройки Codex GPT-6.1 Sol / Medium; Token Optimizer 5.13.63, balanced.

## 0.1.0

- Зелёные границы собственных приватов из ответа Horizon SMP `/claims`.
- Координаты сервера трактуются как начала чанков в блоках: область 16×16.
- Внешний контур смежных чанков без общей внутренней стенки.
- Переключение видимости и ручное обновление через `/claimborders`.
- Изоляция измерений, сброс при переподключении и командах изменения приватов.
- Проверка полноты ответа, координат, размера и срока ожидания.
- Целевая ревизия Horizon: `c9d474d`.
