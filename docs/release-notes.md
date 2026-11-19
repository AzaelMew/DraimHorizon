**DraimHorizon 0.42.2 — Horizon `.hmod` that builds in Horizon Launcher (0.42.1 fixes, CI repair)**

This release carries everything 0.42.1 intended: the Horizon `.hmod` builds in Horizon Launcher (launcher compile
checked on Nightly `6d765e0`, 66 functions patched by 76 mixins), the relay's key-exchange crates live in the mod
crate, and the Horizon pin is the official Nightly `dd67b76`. 0.42.1 was not published because its CI run restored a
stale build cache; 0.42.2 fixes the cache key only.

- Install `draimhorizons-0.42.2.hmod` (Horizon Launcher 0.2.70 or newer) or `draimhorizons-0.42.2.smod` (Sunrise).
  On Sunrise a hand-copied `.smod` is turned on with the line `mod.draimhorizon:true` in `sunrise-mods.txt`.
- Not run in a game client on Horizon. Not available there yet: the third-person patting arm and the World map /
  Seed Map images (`platforms/horizon/README.md`).
