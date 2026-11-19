**DraimHorizon 0.42.0 — Horizon Nightly `6d765e0` and the `.hmod` package**

Horizon's official Nightly changed how native mods are built, so the previous Horizon route stopped working and
every Horizon check on `main` was red. This release moves Horizon to the route Sunrise already uses.

- Install `draimhorizons-0.42.0.hmod` with Horizon Launcher 0.2.70 or newer, then rebuild native mods. The old
  `.hnmod` is not produced any more (current launchers refuse it). Sunrise keeps `draimhorizons-0.42.0.smod`.
- Target: official Horizon Nightly `6d765e0` (Mod SDK `sdk-6d765e0…`), horizon-mixins `e59da90`.
- Checked against that exact Nightly: the package passes the launcher's reader and a native build against the SDK,
  and the modified client compiles. It has not been run in game.
- Not available on Horizon yet: the third-person patting arm, and the World map / Seed Map images (the rest of the
  map features work). Both are listed in `platforms/horizon/README.md`.
