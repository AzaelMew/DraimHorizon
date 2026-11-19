# DraimHorizons

![DraimHorizons — native tools for Horizon SMP](docs/features.svg)

[Download the latest release](https://github.com/DraimCiDo/DraimHorizon/releases/latest) · [Changelog](CHANGELOG.md) · [Relay setup](relay/README.md) · [Navigation](docs/navigation.md) · [Tunnels and bridges](docs/construction.md) · [Farming and schematics](docs/automation.md)

**Install** — Take the files from a release, not from the repository source tree. Each release carries one package per client, built from the same source: `draimhorizons-<version>.hmod` for Horizon and `draimhorizons-<version>.smod` for Sunrise.

| Client | Launcher | Install |
| --- | --- | --- |
| **Horizon** | Horizon Launcher 0.2.70 or newer | **Mods → Add from GitHub** with the release link, or `horizon-launcher mods add-github <release link> --trust-native-code`. Then rebuild native mods (`horizon-launcher native-build`): the launcher builds the mod against the Mod SDK that Horizon publishes with each Nightly. |
| **Sunrise** | Sunrise Launcher 1.0.1 or newer | `sunrise-launcher mods add-github <release link> --trust-native-code`, or **Mods → Add from GitHub** where your launcher has it, or the **Discover** tab if the mod is listed there. Then rebuild native mods. |

A release link looks like `https://github.com/DraimCiDo/DraimHorizon/releases/tag/v0.42.2`. The old `.hnmod` is no longer produced: current launchers refuse it.

**Sunrise, a file you copied in by hand.** Put `draimhorizons-<version>.smod` in the game folder's `sunrise-native-mods/` and add the line `mod.draimhorizon:true` to `sunrise-mods.txt`, beside `options.txt`, then restart. Until then the game's Options → Mods says "Not loaded: turn it on in the launcher". The launcher's own toggle stays locked for hand-copied files because it cannot check where they came from; a mod installed with `mods add-github` or from Discover has a working toggle. To turn a hand-copied mod off, remove that line or the file.

Target clients are the official Horizon Nightly `dd67b76` and Sunrise Nightly `2d81263`, both Java 26.3. A native mod is built for exactly one Nightly, so after a client update install the release made for it (the release notes name the Nightly). Press **J** in game for the mod menu.

**Upgrading from before 0.25.0:** remove `claim_borders` in the launcher, install this repository again, then rebuild native mods. The new entry is `draimhorizon`. Keep the existing data folder; settings, waypoints and history reuse it. Reapply custom key bindings if needed. Do not keep both mod IDs installed.

## Clients

| Client | Status | Pinned official Nightly | How it is checked |
| --- | --- | --- | --- |
| Horizon | Supported, with the gaps listed below. Install the release's `.hmod` (0.42.2 or newer; 0.42.0 and 0.42.1 do not build in the launcher). | `dd67b76` (`horizon_revision`) | On every push to `main` and before each release: the `.hmod` is validated against the SDK of that Nightly (`DRAIM_TARGET=horizon python tools/sunrise.py smod-check --fetch`), and the bundle is applied to that exact source and the real client compiles. The launcher's own native compile is run by hand (`horizon-launcher native-build --version <Nightly> --server-output <new folder> <project>`); it passed on Nightly `6d765e0`. Not yet confirmed in a game client. |
| Sunrise | Supported. Install the release's `.smod`. | `2d81263` (`sunrise.revision`) | Each release's `.smod` is validated against the SDK of that Nightly (`python tools/sunrise.py smod-check --fetch`), the bundle is applied to that source and the client compiles; the package builds and loads in the Sunrise launcher. |

Both pins live in `compatibility.json`. CI compares them every day with the Nightly tag each project publishes
(`python tools/compatibility.py --strict-sunrise`); a newer Nightly turns the check red until the pin is refreshed
and the client is compiled again. Nightlies come often, so a release can be one Nightly behind for a short time.

**Not available on Horizon yet.**
- The third-person patting arm (see Patting below): other players do not see your arm move. Your own view and the
  rest of patting work as described. Horizon's own `BodyPose` hook is the intended route.
- The World map and Seed Map images are not drawn: the Horizon HUD atlas no longer has a slot for mod images and
  runtime sprites stop at 256 px. The rest of the map code (explored area, minimap, claims, waypoints) runs.

Both are listed in [platforms/horizon](platforms/horizon/README.md) with the reason and the route to fix them.

**On Sunrise** the mod keeps its own data folder (settings, waypoints and explored map start empty), its key labels and
Options text are English, and the settings entry in Options → Mods is a switch that opens the menu. See [platforms/sunrise](platforms/sunrise/README.md).

Since Sunrise Nightly `ed3ac19` the client itself sends the brand `horizon` and the empty `horizon:payloads`
offer to playhorizon.dev (`sunrise-official`'s identity hook), so the mod no longer carries its own brand
adapter; Horizon's later login checks (block registry, mod list) are still not bridged. One upstream gap is
open for that nightly: the launcher's *prebuilt* native route does not link `sunrise-official`, so a `.smod`
client still announces `vanilla` until [sunrise-org/sunrise#3119](https://github.com/sunrise-org/sunrise/issues/3119)
ships. Notes and the server-side audit stay in
[platforms/sunrise/smp-compat](platforms/sunrise/smp-compat/README.md).

**Building from source.** The mod is written once in `mixins/draimhorizon`; `tools/sunrise.py` derives each client's
project from it and `platforms/<client>` (`python tools/package_flat.py horizon|sunrise`). See
[docs/development.md](docs/development.md) for the checks.

## World

The World map and Seed Map images are not drawn on Horizon yet (see Clients above).

![Explored map and live minimap instructions](docs/showcase/maps.svg)
![Seed Map instructions](docs/showcase/seed-map.svg)

## Chat and people

![Chat features and original-language badges](docs/showcase/chat.svg)

Real in-game chat examples, with normal and compact spacing. Sample player text keeps its original language.

![Normal chat in game](docs/showcase/chat-normal.png)
![Compact chat in game](docs/showcase/chat-compact.png)

![Translation directions and DeepL setup](docs/showcase/translation.svg)
![Friends, Important players and waypoints](docs/showcase/friends.svg)

**Patting** — Aim at a player within 3 blocks and press **K** (rebindable under Options → Mods → Settings) for two gentle pats. Your own arm moves at once. A player running DraimHorizon on the same relay sees your arm move too (Sunrise; not yet on Horizon), plus an optional "*name* patted you" line. **J → Settings → General** sets who may pat you (friends only by default, everyone or nobody) and turns that line on or off. Patting is cosmetic only: it sends no attack, item use or chat. Players without the mod see nothing.

**Friends keep their place** — If a friend's relay key changes (reinstall, new PC, reset data folder) the mod re-pins the new key automatically when the relay shows a single identity for that account, the old key is gone and the same nickname is on the game server's player list. You get a chat notice; remove the friend if it was unexpected. A friend who lost the friendship is invited once and only has to accept. Ambiguous cases are left untouched and shown as "Key changed".

**Important players** — **J → Friends → Important** adds a player by exact name, offline included, and gives them a colour. Press the colour chip beside the **Colour** field for a full picker: a saturation/brightness square, a hue bar, a live `#RRGGBB` readout and a preview, plus eight presets and Done / Cancel [Esc]. Dragging previews the colour and **Done** stores it; a saved player is recoloured then, while a new name keeps the colour until **Save**. You can also click one of the eight swatches or type a `#RRGGBB` value, and the colour paints that player's name in chat while their messages stay visible with global chat off. Friends are always important and need no rule.

**Show invisible entities** — After accepting experimental features, enable **J → QOL → Survival → Show invisible entities** to see invisible players and mobs locally. Off by default. This can violate server rules and lead to a ban; experimental consent does not grant server permission. Models use normal depth testing and view range, not a through-wall overlay. Potion timers, server gameplay and other players' views do not change; the client cannot reveal entities the server does not send. Invisible armour stands and item frames are left alone.

## Items and trading

![Auction catalogue with search and purchase confirmation](docs/showcase/trading.svg)
![Inventory search, sorting and click-through](docs/showcase/inventory.svg)

## Baritone

Every Baritone job shares one route planner. A task waits instead of stopping when it has nothing to do, keeps running when you take damage, and resumes after a restart when Auto Resume is on.

![Shared route planner: slabs, doors, ladders, gap jumps, drops, elytra and swimming](docs/showcase/navigation.svg)
![Mining trees, ores and blocks under water](docs/showcase/mining.svg)
![Farm field split into crop quadrants with harvest, seed and bone meal chests](docs/showcase/farm.svg)
![Tunnel, bridge and drain jobs](docs/showcase/construction.svg)
![Schematic ghost blocks and preview editor keys](docs/showcase/schematics.svg)

## QOL and interface

**Auto Eat** — Open **J → QOL → Survival → Allowed foods** and select food-item tiles, just like mining targets. Only selected foods may be eaten; the list starts empty. Auto Eat waits until a food's entire hunger value fits (steak restores 8 points, so it waits until hunger is 12/20 or lower). Smaller selected snacks can be eaten sooner. Saturation does not affect this check; risky and reserved foods remain excluded.

![Auto Eat, Auto Totem, Auto Fill, Auto Tools, Auto Click, Auto Resume, Human Look and shulker peek](docs/showcase/helpers.svg)
![Zoom, interface customization and opt-in features](docs/showcase/customize.svg)

**Shared DeepL** — Add `RELAY_DEEPL_API_KEY` to the private relay's server environment. Friends configure its address and access token under **J → Settings → Network**. [Operator instructions](relay/README.md#общий-резервный-перевод-deepl)

**Compatibility** — Friends' telemetry requires the relay and both clients to use the mod. Seed Map predicts generation and can differ from modified or previously generated terrain. Baritone acts only through normal client input; the server stays the authority and can refuse any action. Player-supplied text remains unchanged by interface localization.
