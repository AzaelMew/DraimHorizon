# Sunrise platform (experimental)

DraimHorizon is written once against Horizon's source (`mixins/draimhorizon`). Sunrise is a fork with
renamed crates, a different module layout and its own native hook model, so the Sunrise variant is
derived by `tools/sunrise.py` from the shared sources plus the files here:

- `platform.toml` - injections retargeted or dropped, module destinations, and regex rewrites for the
  engine API differences found by compiling against the pinned Sunrise Nightly.
- `overlay/` - Sunrise-specific source: hook helpers whose arguments differ, and `sunrise_platform.rs`,
  a shim giving `Game` the `native_*` helpers the shared sources call (`on`, `setting`, `mod_screen`, ...).
- `smp-compat/` - the separate Horizon SMP brand adapter prototype (see its README).

## Commands

```sh
python tools/compatibility.py                      # Horizon and Sunrise pins
python tools/sunrise.py resolve --sunrise PATH     # where each Horizon hook target lives in Sunrise
python tools/sunrise.py stage --sunrise PATH --check
```

`stage` exports the pinned commit, pastes the include files the mixin tool cannot read, applies the
bundle with horizon-mixins `e59da90` plus Sunrise's patch, and compiles `sunrise-client --lib`.
It needs Rust 1.99.1 and the Linux headers listed in `.github/workflows/sunrise-compatibility.yml`.

## Packages

`python tools/package_flat.py sunrise` writes the flat project the Sunrise launcher installs
(`sunrise-mod.toml`, `Cargo.toml`, `src/`) and packs `draimhorizons-<version>.smod`. Targets come from
`targets.toml` (`python tools/sunrise.py freeze --sunrise <staged tree>` refreshes it), so no Sunrise checkout is
needed. The launcher's own package reader accepts it (`tools/package_check`); its native build is not verified.

## Status

Compiles: 75 of 79 hooks and every module apply to Sunrise Nightly `234db8d` and the modified
`sunrise-client` builds. **Not verified:** a Sunrise launcher/SDK build (native mods there replace
functions in a prebuilt game, which this repository cannot produce), a running client, or a server.
Do not describe the mod as supported on Sunrise until that is done.

Dropped hooks: `commands` (command-name completion), `list` (Options -> Mods entry) and
`menu-options` / `menu-options-click` (its settings buttons). Sunrise registers these through its
`Hooks` trait and `sunrise-mod.toml`, which the shared sources do not yet provide. Behaviour
differences: `native_on` follows Sunrise's mod toggle, `native_mod_key` ignores modifier keys, and
the Baritone port treats modded fluids as none.
