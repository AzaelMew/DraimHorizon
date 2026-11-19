**DraimHorizon 0.42.1 — the Horizon `.hmod` builds in Horizon Launcher**

0.42.0's Horizon package stopped in the launcher's native build (`E0603`, private game items). This release fixes
that, and carries the relay's key-exchange crates in the mod crate because the prebuilt game does not link them.

- Install `draimhorizons-0.42.1.hmod` with Horizon Launcher 0.2.70 or newer and rebuild native mods. Target:
  official Horizon Nightly `dd67b76` (the pin moved from `6d765e0`, which Horizon replaced while this fix was in progress). Sunrise keeps `draimhorizons-0.42.1.smod`.
- Verified with the launcher's own native compile on Nightly `6d765e0` (66 functions patched by 76 mixins, no
  errors); on `dd67b76` the SDK function index and the source stage pass, the launcher compile has not been run. Not
  run in a game client. 0.42.0's note that a native build had been checked was wrong: it had only been packed and checked
  against the SDK.
- Still not available on Horizon: the third-person patting arm and the World map / Seed Map images
  (`platforms/horizon/README.md`).
