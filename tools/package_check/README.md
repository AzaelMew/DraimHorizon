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


`internals.rs` is a second, Sunrise-specific probe. Compile it as the `main.rs` of an isolated Cargo
project with `sunrise-mod-package = { path = "/path/to/pinned/sunrise/crates/sunrise-mod-package", default-features = false }`.
Run its binary with the generated native project folder and SDK `internals.json` as two arguments.
It calls the same public `internals::read` / `internals::check` implementation the launcher uses.
This catches `shape_only` declarations that a package-format check or a Rust metadata type-check alone
cannot catch. It does not build the native replacement library or connect to a server.
