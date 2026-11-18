# Sunrise platform

DraimHorizon is written once against Horizon's source (`mixins/draimhorizon`). Sunrise is a fork with
renamed crates, a different module layout and its own native hook model, so the Sunrise variant is
derived by `tools/sunrise.py` from the shared sources plus the files here:

- `platform.toml` - injections retargeted or dropped, module destinations, and regex rewrites for the
  engine API differences found by compiling against the pinned Sunrise Nightly.
- `overlay/` - Sunrise-specific source: hook helpers whose arguments differ, and `sunrise_platform.rs`,
  a shim giving `Game` the `native_*` helpers the shared sources call (`on`, `setting`, `mod_screen`, ...).
- `platform.d/` - further rows for `platform.toml`, one file per feature (tables merge, arrays append).
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

The `.smod` builds in Sunrise Launcher 1.0.1 against Nightly `2d81263` and runs: HUD, minimap, J menu, the world
and seed map images (also after a resource reload), waypoints, chat badges and translation, ores, inventory tools,
schematics, Baritone commands, the Options → Mods entry with key bindings and command completion were checked in
game. Differences from Horizon:

- the mod keeps its own data folder under the Sunrise game folder, so settings, waypoints and the explored map
  start empty;
- key labels, the Options → Mods description and its settings switch are English;
- the settings entry is a switch that opens the menu and turns itself off again;
- `native_mod_key` ignores modifier keys, and the Baritone port treats modded fluids as none.
