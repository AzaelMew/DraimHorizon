# Horizon SMP brand adapter for Sunrise — prototype

This separate client mod selects the protocol-encoded `horizon` brand for `playhorizon.dev`
and its subdomains. Other destinations retain the engine's original packet. The policy has no
shared state, and does not change account authentication, UUIDs or chat signatures.

**The current official Sunrise SDK cannot load this adapter.** The native entry point type-checks,
but the SDK does not expose a replaceable brand function. This directory is a reviewable prototype,
not a supported install or a completed DraimHorizon port.

## Required Sunrise change

`upstream-brand-hook.patch` is a minimal proposal against official Nightly
`234db8df6210165318b2b604dea50275357bfaeb`. It passes the connection address to the existing private
`brand` function and annotates that function with Sunrise's `replaceable` marker. Without a mod,
the function still returns the same `vanilla` packet. The actual module belongs to
`sunrise_client_lib`, despite its source living under `crates/sunrise-client/src/`.

Sunrise documents that a native mixin may replace a function below the game crate only when it
has that marker: [native mixins](https://github.com/sunrise-org/sunrise/blob/234db8df6210165318b2b604dea50275357bfaeb/docs/modding/src/concepts/native-route/2-mixins.md).
This uses the documented native route; the public trait API has no client brand setter.

The patch must be reviewed and built into a new Sunrise SDK. Then update `compatibility.json`
and this project's `[native]` pins to that official build, verify the function is in its
`functions.json`, and build through the launcher:

```sh
sunrise-launcher native-build platforms/sunrise/smp-compat --instance "My Instance" --trust-native-code
```

This command is for the future SDK with the hook, not the current pin. For IDE use, fetch the API
sources into `.sunrise/sdk/api/` with `cargo sunrise-mod`, as described in the
[native mod guide](https://github.com/sunrise-org/sunrise/blob/234db8df6210165318b2b604dea50275357bfaeb/docs/modding/src/start/first-mod.md).

## Scope and verification

Check the actual SDK before attempting a native build:

```sh
python platforms/sunrise/smp-compat/verify_sdk.py --sdk /path/to/sdk.json --functions /path/to/functions.json --source /path/to/sunrise
```

The command checks both build pins, the mixin-tool revision, exact target ownership,
source-file/function byte hashes and the required address parameter. It exits 2 with
`SDK_TARGET_UNAVAILABLE` on the current official SDK. An `ok: true` result on a future
SDK only clears these prerequisites: `runtime_verified` remains false.

Run the standalone checks without game resources or account credentials:

```sh
python -m unittest discover -s platforms/sunrise/smp-compat/tests
rustc --edition 2024 --test platforms/sunrise/smp-compat/tests/brand_gate.rs -o target/sunrise-brand-gate-tests
target/sunrise-brand-gate-tests
```

The Rust test uses the real mixin helper and a loopback socket with a configuration brand
gate. It supplies the framing itself, so it does not test Sunrise's network writer,
authentication, native function replacement or live SMP login. Create `target/` first if
the repository has not been built yet. See [protocol-audit.md](protocol-audit.md) for the
channel and codec differences that remain after brand selection.

- Address policy and concurrent connection isolation are covered by the repository's Rust tests.
- The empty Hooks entry point type-checks against the published SDK metadata with Rust 1.99.1.
- The upstream patch applies cleanly to the pinned official source.
- The official SDK index confirms the missing replacement point; no working launcher replacement
  or live SMP login is claimed. The cloud launcher also encounters a TLS trust failure downloading
  its releases; standalone SDK downloads succeed and their published hashes were verified.

This addresses the `horizon-clients-only` brand gate only. It does not translate `horizon:*`
extension channels or prove matching content registries. Those must be tested separately against
the actual SMP. Do not echo a server's content hash without verifying local content.

After the required SDK release, test SMP login, an unrelated server, parallel clients, reconnects,
custom content, menus and commands. Check the received brand in the SMP server log.
