# Checking the flat packages with the launchers' own reader

`tools/package_flat.py` writes the flat packages (`.hmod`, `.smod`). The launchers refuse a package their
reader does not accept, so the check that matters is that reader. `main.rs` here calls it:

- on a project folder: the launcher's gate for the old layout, then `Package::read`, the file selection and
  `Package::pack_directory`;
- on a `.hmod` / `.smod`: `index::inspect(NativeMod, file)`, what the launcher runs on a downloaded release asset.

It is not part of this repository's build. To run it, put it in a checkout of the project whose reader you want
(Horizon: `horizon-rust/horizon`, Sunrise: `sunrise-org/sunrise`) as a workspace crate:

```sh
mkdir -p crates/pkgcheck/src && cp tools/package_check/main.rs crates/pkgcheck/src/main.rs
# crates/pkgcheck/Cargo.toml: [dependencies] horizon-mod-package = { path = "../horizon-mod-package", features = ["index"] }
# (Sunrise: sunrise-mod-package, and use sunrise_mod_package / "smod" in main.rs); add it to the workspace members.
cargo run -p pkgcheck -- /path/to/DraimHorizon/target/flat/horizon
cargo run -p pkgcheck -- /path/to/DraimHorizon/target/draimhorizons-<version>.hmod
```

Checked on Horizon main `c70c5b96` and Sunrise Nightly `234db8d`: the old `mixins/draimhorizon` folder is refused with
"unsupported package path horizon-mixin.toml" (the error players saw), and the flat packages are read, selected and packed.
This checks the package format only. It does not run the launcher's native build.
