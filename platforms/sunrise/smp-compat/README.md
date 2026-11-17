# Horizon SMP brand adapter for Sunrise — experimental

This separate native mod uses **two replacement points already published in the official Sunrise
SDK**: `sunrise_client_lib::net::Session::poll` and `sunrise_client_lib::net::receive_brand`.
It requires no client source changes, new SDK hook, executable patch or replacement of authentication.
It has not yet been verified in a running client or against the live SMP.

## How it works

Sunrise sends its ordinary initial `vanilla` brand. When the server announces `horizon`, this mod
queues a subsequent `minecraft:brand = horizon` **before the network thread continues to known-pack
acknowledgement**. Horizon's configuration gate uses the last brand received before that acknowledgement.
The existing writer performs framing, compression and encryption as usual.

- The poll replacement registers a callback for this session's shared event-channel identity.
- The receiving replacement waits up to two seconds for that registration, on the network thread only.
- Only addresses under `playhorizon.dev` select the sender. Other connections retain engine behavior.
- The callback holds weak references, rechecks configuration under the writer lock and uses the existing
  bounded writer queue. It is attempted once per registration. A missing, closed, full or wrong-phase
  writer is not reported as a successful send.
- The registry holds at most 64 entries, expires them after 30 seconds and distinguishes session serials.
  No game-thread network wait, disk scan, unsafe memory operation or custom authentication is introduced.

The SDK exposes the native layouts used by these helpers; their fingerprints are declared in the manifest.
This is Sunrise's documented trusted **native route**, not the public trait-only payload API. Both the
build pin and internal fingerprints must match. See [native replacement mechanics](https://github.com/sunrise-org/sunrise/blob/234db8df6210165318b2b604dea50275357bfaeb/docs/modding/src/concepts/native-route/7-how-it-works.md).

The earlier direct replacement of `net::brand` was unavailable. Its upstream hook proposal has been removed:
this implementation uses the existing SDK instead.

## Build

The development directory also contains research tools and tests, which Sunrise's package fence does not
accept as package assets. Stage a fresh, clean package first, then use a prepared instance of the pinned
Nightly with its normal launcher:

```sh
python platforms/sunrise/smp-compat/stage_project.py target/sunrise-smp-compat
sunrise-launcher native-build target/sunrise-smp-compat --instance "My Instance" --trust-native-code
```

Run from the repository root. Choose a new output directory for subsequent stages; the stager deliberately
refuses to overwrite an existing directory. It copies only Cargo.toml, sunrise-mod.toml and src/. The native
project uses the launcher's SDK/Rust, not a rebuilt game. Its required manifest capability is `registries`,
as in Sunrise's native template; this adapter declares no content registries.

## Verify

```sh
python platforms/sunrise/smp-compat/verify_sdk.py --sdk /path/to/sdk.json --functions /path/to/functions.json --internals /path/to/internals.json --source /path/to/sunrise
python platforms/sunrise/smp-compat/check_helpers.py --sdk /path/to/sdk.json --api /path/to/sdk/api --engine /path/to/sdk/engine --source /path/to/sunrise --rustc /path/to/rustc
python -m unittest discover -s platforms/sunrise/smp-compat/tests
rustc --edition 2024 --test platforms/sunrise/smp-compat/tests/handshake.rs -o target/sunrise-handshake-tests
rustc --edition 2024 --test platforms/sunrise/smp-compat/tests/brand_gate.rs -o target/sunrise-brand-gate-tests
target/sunrise-handshake-tests
target/sunrise-brand-gate-tests
```

Create target/ first if needed. SDK preflight checks both build IDs, mixin pin, exact replacement signatures,
internal fingerprints, source file hashes and function byte spans. It **passes on the current official SDK**;
`runtime_verified` deliberately remains false.

Verification completed here: SDK preflight, the native entry point and both helpers type-checked against the
published engine/API metadata with Rust 1.99.1; nine Python tests, six handshake synchronization tests and
four brand-policy/loopback tests. The loopback test supplies its own framing and does not run either client.

Full launcher build needs a prepared Minecraft 26.3 installation; the isolated cloud install has none and
stopped at that prerequisite. Source discovery (`--sync-internals`) additionally hit an existing
`E_INVALID_PATH` for `../../creatures/biome_facts.rs`; no client source was edited to work around it.
Runtime replacement, live SMP entry, in-connection reconfiguration and extension
interoperability remain unverified. The two-second rendezvous also requires the connection to be polled during
configuration; confirm that with the running client.

## Remaining protocol work

This addresses the initial `horizon-clients-only` brand gate, not the full DraimHorizon port. The nonempty
mod-list codecs differ and custom-content registry matching must remain real. See
[protocol-audit.md](protocol-audit.md). Do not alias every channel prefix or echo an unverified registry hash.
After a launcher build, check the received server-side brand, SMP login, an unrelated server, reconnects,
menus, signed chat, commands and each extension the actual server announces.
