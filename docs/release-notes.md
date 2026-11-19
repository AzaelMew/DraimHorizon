**DraimHorizon 0.42.3 — Sunrise Nightly `520fc48`: the mod's own minimap, engine-owned reconnect and identity**

Sunrise Nightly `520fc48` (2026-11-19) moved the engine minimap out of the client, moved auto-reconnect into the
engine and changed several native hook points. This release moves the Sunrise `.smod` to it. The Horizon `.hmod`
carries the same mod code as 0.42.2 and keeps its pin.

- Install `draimhorizons-0.42.3.hmod` for Horizon or `draimhorizons-0.42.3.smod` for Sunrise; Horizon's mod code
  is unchanged in this version. On Sunrise a hand-copied `.smod` is turned on with the line
  `mod.draimhorizon:true` in `sunrise-mods.txt`.
- The minimap on Sunrise is the mod's own picture: the Nightly deleted the engine's minimap (hook, terrain
  scanner, drawing, atlas slot) and a mod keeps its own. Tiles, marks, frame, zoom, size and corner behave as
  before; shading and resource-pack terrain colours are the same code as the World map.
- Auto-reconnect on Sunrise is the engine's, switched in Options → Extras: the Nightly moved it out of the client
  with the same policy the mod carried — lost connections, server shutdowns and restarts retry; kicks, bans,
  whitelist and session refusals never. The mod's own Auto-reconnect switch is Horizon-only now.
- Joining the official Horizon SMP no longer depends on this mod on either route: `sunrise-official` sends the
  `horizon` brand and the `horizon:payloads` offer, and this Nightly links that layer in the launcher's prebuilt
  build too, so a `.smod` client identifies itself like the shipped client.
- World map / Seed Map images, Baritone's world tracking, chat, menus, schematics and dry placement were retargeted
  to the Nightly's new hook points and compiled against the real client source; a package built for that Nightly by
  Sunrise Launcher 1.1.1 reports no left-out mods.
