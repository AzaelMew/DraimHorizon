# Horizon platform

DraimHorizon is written once against an older Horizon source (`mixins/draimhorizon`). The current official
Nightly moved its game code into areas (`.game-area`), split the hook points into typed events and builds native
mods from a flat project against a prebuilt game, so the Horizon package is derived like the Sunrise one:

```sh
python tools/package_flat.py horizon                       # target/horizon/smod/draimhorizon and target/draimhorizons-<v>.hmod
DRAIM_TARGET=horizon python tools/sunrise.py smod-check --fetch   # every hook against the SDK's function index
DRAIM_TARGET=horizon python tools/sunrise.py stage --sunrise TREE --check   # apply the bundle, compile horizon-client
DRAIM_TARGET=horizon python tools/sunrise.py freeze --sunrise TREE          # refresh targets.toml after a pin bump
```

- `platform.toml`, `platform.d/` - retargeted or dropped injections and the rewrites the Nightly's API needs.
- `overlay/` - hook helpers whose arguments differ, the `Game` host-call shim and the code of the mod crate.
- `smod/` - the mod crate entry (`lib.rs`) and the lock file its registry dependencies are pinned with.
- `targets.toml` - frozen hook targets, so a package can be generated without a Horizon checkout.

## Launcher build

The launcher's patched crate provides the game modules and their re-exports needed by the hooked
functions, including `api_values`, `minecarts`, `execution_role`, and `entity_culling`.
The relay's key-exchange crates live in the mod crate
(`HORIZON_CRYPTO` in `tools/sunrise.py`). `smod/Cargo.lock` is the game's `Cargo.lock` pruned to the mod crate:
copy the game's lock next to a copy of the project without its `horizon-mod-api` dependency and run
`cargo metadata`. Check with `horizon-launcher native-build --version <Nightly> --server-output <new folder> <project>`.

## Not ported yet

- `pat-arm`: the third-person patting arm. `mobs::anim::setup` is a lower-crate function the SDK does not make
  replaceable; Horizon's own `BodyPose` hook (`horizon_hooks::players`) is the intended route.
- World map and Seed Map images (`world-map-atlas`): the HUD atlas has no slot for mod images and runtime sprites
  stop at 256 px, so the two map images are not drawn. The rest of the map code runs.
