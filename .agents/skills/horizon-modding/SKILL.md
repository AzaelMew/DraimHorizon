---
name: horizon-modding
description: Develop and review Horizon content or native Rust mods; verify manifests, hook APIs, packaging and multiplayer behavior against pinned upstream source.
---

# Horizon mod development

Read `.ecc/upstream-lock.json` and `docs/development.md` first. The current pinned source checkout in the initial cloud environment is `/workspace/work/horizon-target`; if absent, fetch the recorded revision into a gitignored `.local/horizon` directory. Never assume the setup machine path exists on another machine.

1. Establish the requested behavior, client/server scope and target Horizon version. Use data-driven content for blocks, items, recipes and assets when sufficient; choose native Rust only when code is needed. A mod can require both components.
2. Read upstream `docs/mods.md`, `docs/native-mods.md`, and the relevant implementation at the selected revision. For native work also inspect the official horizon-native-mod-template and horizon-mixins sources before creating a bundle.
3. Content: `horizon-mod.toml`, API `0.2`, `registries` capability, namespace-scoped `assets/` and `data/`; `cargo horizon-mod new` creates content templates only. Unknown manifest fields are rejected.
4. Native: source bundle with `horizon-mixin.toml` and Rust helpers. Distribution `.hnmod` has `horizon-native.json`. Check exact hook signatures and required minimum API in source. At the current pinned reference, `NATIVE_HOOKS_API` is 80; this is an observation, not a requirement for every mod. Stable-hook-only bundles may declare a minimum hooks API; arbitrary source mixins need exact revision compatibility.
5. Keep server authority, world save consistency, mod namespaces and compatibility in view. Do not distribute Mojang code or assets.
6. Use ECC Rust and verification skills as needed. Test behavior and meaningful failures; build and check the actual artifact when tooling and owned game resources are available. State any missing prerequisites.

Primary references:
- https://github.com/horizon-rust/horizon/blob/c9d474dd9116e84c243f8992d75c11314df56f06/docs/mods.md
- https://github.com/horizon-rust/horizon/blob/c9d474dd9116e84c243f8992d75c11314df56f06/docs/native-mods.md
- https://github.com/horizon-rust/horizon-native-mod-template
- https://github.com/horizon-rust/horizon-mixins
