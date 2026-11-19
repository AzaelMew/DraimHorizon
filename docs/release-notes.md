**DraimHorizon 0.42.3 — Sunrise Nightly `ed3ac19`: the mod's own minimap, engine-provided Horizon identity**

Sunrise Nightly `ed3ac19` (2026-11-19) removed the engine minimap and changed several native hook points.
This release moves the Sunrise `.smod` to it. The Horizon `.hmod` carries the same mod code as 0.42.2 and
keeps its pin.

- Install `draimhorizons-0.42.3.hmod` for Horizon or `draimhorizons-0.42.3.smod` for Sunrise; Horizon's mod
  code is unchanged in this version. On Sunrise a hand-copied `.smod` is turned on with the line
  `mod.draimhorizon:true` in `sunrise-mods.txt`.
- The minimap on Sunrise is the mod's own picture now: the Nightly deleted the engine's minimap (hook, terrain
  scanner, drawing, atlas slot) and a mod keeps its own. Tiles, marks, frame, zoom, size and corner behave as
  before; shading and resource-pack terrain colours are the same code as the World map.
- Joining the official Horizon SMP from Sunrise no longer depends on this mod: the Nightly's own
  `sunrise-official` part sends the `horizon` brand and the `horizon:payloads` offer. The mod's brand adapter is
  out of the package. On the launcher's prebuilt native route that layer is not linked yet
  (sunrise-org/sunrise#3119), so a `.smod` client still announces `vanilla` there.
- World map / Seed Map images, Baritone's world tracking, chat, menus and the rest of the port were retargeted to
  the Nightly's new hook points and compiled against the real client source.
