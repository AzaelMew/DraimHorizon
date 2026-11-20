**DraimHorizon 0.42.3 (Sunrise only) — Nightly `520fc48`: the mod's own minimap, engine-owned reconnect and identity**

This pre-release carries only the Sunrise package, `draimhorizons-0.42.3.smod`. The Horizon `.hmod` stays at
0.42.2's release: Horizon's pinned Nightly was not refreshed for this batch, so nothing Horizon is validated or
attached here.

Sunrise Nightly `520fc48` (2026-11-19) moved the engine minimap out of the client, moved auto-reconnect into the
engine and changed several native hook points. This release moves the Sunrise `.smod` to it.

- Put `draimhorizons-0.42.3.smod` in the game folder's `sunrise-native-mods/` and add the line
  `mod.draimhorizon:true` to `sunrise-mods.txt`, or install it from this release link with the launcher's
  **Mods → Add from GitHub**. It needs Sunrise Nightly `520fc48` (2026-11-19, 10:36 UTC) exactly; the launcher
  rebuilds native mods when it moves to that Nightly.
- The minimap is the mod's own picture: the Nightly deleted the engine's minimap (hook, terrain scanner,
  drawing, atlas slot) and a mod keeps its own. Tiles, marks, frame, zoom, size and corner behave as before;
  shading and resource-pack terrain colours are the same code as the World map.
- Auto-reconnect is the engine's now, switched in Options → Extras: the Nightly moved it out of the client with
  the same policy the mod carried — lost connections, server shutdowns and restarts retry; kicks, bans,
  whitelist and session refusals never. The mod's own Auto-reconnect switch is no longer on Sunrise.
- Joining the official Horizon SMP no longer depends on this mod on either route: `sunrise-official` sends the
  `horizon` brand and the `horizon:payloads` offer, and this Nightly links that layer in the launcher's prebuilt
  build too, so a `.smod` client identifies itself like the shipped client.
- World map / Seed Map images, Baritone's world tracking, chat, menus, schematics and dry placement were retargeted
  to the Nightly's new hook points and compiled against the real client source; a package built for that Nightly by
  Sunrise Launcher 1.1.1 reports no left-out mods.
