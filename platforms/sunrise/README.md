# Sunrise platform

DraimHorizon is written once against Horizon's source (`mixins/draimhorizon`). Sunrise is a fork with
renamed crates, a different module layout and its own native hook model, so the Sunrise variant is
derived by `tools/sunrise.py` from the shared sources plus the files here:

- `platform.toml` - injections retargeted or dropped, module destinations, and regex rewrites for the
  engine API differences found by compiling against the pinned Sunrise build.
- `overlay/` - Sunrise-specific source: hook helpers whose arguments differ, and `sunrise_platform.rs`,
  a shim giving `Game` the `native_*` helpers the shared sources call (`on`, `setting`, `mod_screen`, ...).
- `platform.d/` - further rows for `platform.toml`, one file per feature (tables merge, arrays append).
  `[module_override.<id>] to_crate = true` moves a pure shared module into the mod crate, `crate_move` moves a
  pure relay client file there (a rewrite makes the relay re-export it), and `[smod.dependencies]` adds the registry
  crates they use, pinned as the game's lock pins them (add their entries to `smod/Cargo.lock`).
- `overlay/crate/` - code that lives in the `.smod`'s own crate: the Options → Mods entry, key mappings and
  client commands (`mod_entry.rs`), and state that helpers below the game crate need.
- `smod/` - the mod crate entry (`lib.rs`) and the `Cargo.lock` its registry dependencies are pinned with.
- `targets.toml` - frozen hook targets, so a package can be generated without a Sunrise checkout.

## Commands

```sh
python tools/compatibility.py                      # Horizon and Sunrise pins
python tools/package_flat.py sunrise               # target/sunrise/smod/draimhorizon and target/draimhorizons-<v>.smod
python tools/sunrise.py smod-check --fetch         # validate the .smod against the pinned SDK (downloads it)
python tools/sunrise.py freeze --sunrise TREE      # refresh targets.toml after a pin bump
python tools/sunrise.py stage --sunrise PATH --check   # source-level: apply the bundle and compile sunrise-client
```

`package_flat.py sunrise` runs `tools/sunrise.py smod`: it generates the Sunrise bundle from the shared sources and
turns it into the flat native project the Sunrise launcher builds (`sunrise-mod.toml` with `[[native.mixin]]` and
`[[native.module]]` rows, `Cargo.toml`, `src/`). Sunrise compiles native mods against a prebuilt game, so the
generator also:

- inlines the relay client as one game-side module (the launcher refuses `ring`, `rustls` and `ureq` as registry
  dependencies of a mod crate, while game-side code can use the game's own copies); the TLS roots come from the
  mod crate's `webpki-roots` dependency;
- turns every `impl Game` block of a module into an extension trait and imports it where needed (modules are
  compiled outside the game crate, where inherent impls are refused);
- registers no-op listeners for the hooks whose callers Sunrise only runs while some mod listens
  (`screen_render_post`, `screen_char_typed_pre`, `render_hand`, `humanoid_setup_anim`).

To test a build locally: `sunrise-launcher native-build target/sunrise/smod/draimhorizon --instance <test instance>
--trust-native-code`.

## Status

The `.smod` targets the Sunrise Dev build `ba1f24e` (`dev-2026-11-26-ba1f24e`). Client commands bridge the
shared handlers at `run_client_native_command_in_chain`; registered usages carry typed, fail-closed executors.
Local notices use `add_chat_line` with the client source and are captured once without server-message parsing.
Zoom composes `compute_fov`, `player_turn` and `mouse_scrolling`, with smoothing advanced once in
`render_frame_pre`; it yields to a first-person spyglass. Wheel handling follows the held key, independently
of camera queries or the game's tick rate. Trading-panel scissors use final screen coordinates because
Sunrise stores clipping separately from vertices.
The host shim retains the existing data paths and player snapshots. The HUD borrows the extras pass's
local `corners` explicitly, keeping the PvP/mod/registered-layer drawing order.
Seed Map reads random-spread candidate rules from the same resource definitions as the game;
`GeneratorWorker::structure_starts` validates the actual starts, including frequency and exclusions.
The earlier port's Launcher build and
in-game menu and minimap checks were performed on Nightly `520fc48`. Differences from Horizon:

- the minimap is drawn by the mod through `DynamicTextures` and the GUI pass, because the Nightly deleted the
  engine minimap (`minimap` hook, `Minimap` type, terrain scanner and its drawing);
- auto-reconnect is the engine's: Nightly `520fc48` moved it into `sunrise-reconnect`, a platform layer
  `sunrise-official` registers, with the same refusal policy the mod carried. Its switch is the engine's
  Options → Extras row, so the mod's own Auto-reconnect row is Horizon-only;
- the Horizon SMP brand comes from the engine's `sunrise-official` on both routes since Nightly `520fc48`
  ([sunrise-org/sunrise#3133](https://github.com/sunrise-org/sunrise/pull/3133));
- the mod keeps its own data folder under the Sunrise game folder, so settings, waypoints and the explored map
  start empty;
- key labels, the Options → Mods description and its settings switch are English;
- the settings entry is a switch that opens the menu and turns itself off again;
- `native_mod_key` ignores modifier keys, and the Baritone port treats modded fluids as none.
