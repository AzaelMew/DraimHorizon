# Horizon SMP brand adapter for Sunrise — experimental

This adapter is **part of the main DraimHorizon `.smod`**: `tools/sunrise.py smod` copies its two mixins
(`register-smp-session`, `announce-horizon-brand`) into the package under `src/mixins/smp/`, its shared state
into the mod crate as `mod_draimhorizon::smp_brand`, and the `internals` fingerprints into `sunrise-mod.toml`.
Players install nothing extra. This directory is the adapter's source, its declaration (`sunrise-mod.toml`,
read by the generator and the checkers) and its tests.

It uses **two replacement points already published in the official Sunrise
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
build pin and internal fingerprints must match. See [native replacement mechanics](https://github.com/sunrise-org/sunrise/blob/2d8126336e579777eabf9b2fd8b3d476236a04f2/docs/modding/src/concepts/native-route/7-how-it-works.md).

The earlier direct replacement of `net::brand` was unavailable. Its upstream hook proposal has been removed:
this implementation uses the existing SDK instead.

## Build

There is no separate package any more. Build the main one, which carries the adapter:

```sh
python tools/package_flat.py sunrise          # target/draimhorizons-<version>.smod
python tools/sunrise.py smod-check --fetch    # the SDK's own package check
```

Keep the `[native] sunrise` pin of this directory's `sunrise-mod.toml` equal to `compatibility.json`, so the
checkers below look at the same build as the package.

`Cargo.toml` and `src/lib.rs` here only give `check_helpers.py` a crate to type-check the helpers against;
they are not shipped.

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

## What the server checks after the brand

From Horizon's own server code at the pinned Nightly `9f42064` (`crates/horizon-client/src/lan/conn.rs`, the
configuration phase). The live SMP runs its own server build, so treat this as the public reference, not a
guarantee:

1. **Brand.** The server keeps the last `minecraft:brand` received before the client's known-pack reply. Not
   `horizon` while `horizon-clients-only` is on: disconnect. This adapter covers that step.
2. **Mod list**, only when the server runs native mods with `side = "both"`: it sends `horizon:mod_list` and a ping,
   and disconnects unless the client's list matches. Sunrise does not answer that channel, so such a server would
   name the missing mods. That cannot be passed honestly: the client would need the same mods.
3. **Content registry**, only when the server runs content mods: it sends `horizon:block_registry` and a ping, and
   disconnects unless the client returns the same hash. Echoing the server's hash would only move the failure
   into the game (unknown blocks and items), so the adapter never does it.
4. Announcements Sunrise ignores safely: `horizon:quiet_commands`, `horizon:mod_net`, the far-terrain offer, and in
   play `horizon:recipe_content`, `horizon:recipe_transfer`, attachment sync. Unknown channels from a server that
   did not negotiate Sunrise payloads are passed by, not refused; quiet mod commands fall back to normal commands.

Protocol versions and the core known pack (`minecraft:core`, 26.3) are the same on both clients.

## Reading a test login

Each official-SMP join writes one line to `latest.log`: `DraimHorizon: Horizon SMP brand <outcome>`.

| Outcome or server message | Meaning | Next step |
| --- | --- | --- |
| `Queued`, then you are in | The brand passed and the server needs nothing else | Done |
| `Queued`, then a list of mods (`Missing: …`) | The server requires shared native mods (step 2) | Only the server can relax this |
| `Queued`, then `Content registry mismatch: …` | The server runs content mods (step 3) | Only the server can relax this |
| `Queued`, then `This server is for the Horizon client` or the Sunrise message | The server identifies clients by more than the brand | Send the log; the live server differs from the public code |
| `TimedOut` | `Session::poll` did not run during configuration within 2 s | Send the log; the rendezvous needs another trigger |
| `Unavailable` | The connection had already left configuration or closed | Send the log |
| No line at all | The server did not announce `horizon`, or the address is not under `playhorizon.dev` | Check the address you typed |

## Remaining protocol work

This addresses the initial `horizon-clients-only` brand gate, not the full DraimHorizon port. The nonempty
mod-list codecs differ and custom-content registry matching must remain real. See
[protocol-audit.md](protocol-audit.md). Do not alias every channel prefix or echo an unverified registry hash.
After a launcher build, check the received server-side brand, SMP login, an unrelated server, reconnects,
menus, signed chat, commands and each extension the actual server announces.
