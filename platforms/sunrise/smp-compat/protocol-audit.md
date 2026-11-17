# Horizon / Sunrise connection audit

Compared sources: official Horizon Nightly `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0`
and official Sunrise Nightly `234db8df6210165318b2b604dea50275357bfaeb`. This is a source
audit, not confirmation of the official SMP's deployed version or successful live login.

## Initial configuration

Horizon `lan/conn.rs` records the last `minecraft:brand` received before the client's known-pack
reply. With `horizon-clients-only`, any value other than `horizon` is disconnected there.
Sunrise `net.rs::run` first sends `vanilla`; its configuration loop announces `sunrise` only
after the server says `sunrise`. Therefore it retains `vanilla` against a Horizon server.

The proposed brand replacement runs before the first configuration packet is sent. Replacing
a later play-phase packet is too late. The public `client_packet` / `client_packet_send` hooks
exclude configuration and the engine's brand handling. Application raw payload registration
cannot claim the engine-owned `minecraft:` namespace.

The SDK currently indexes two functions in `sunrise_client_lib::net`: `Session::poll` and
`receive_brand`. It does not expose the proposed `brand` target. The source path under
`crates/sunrise-client/` does not make it part of the game crate. Run `verify_sdk.py` against
the actual index to reproduce this prerequisite failure.

## Extension compatibility

| Horizon channel | Sunrise counterpart | Result on these pins |
| --- | --- | --- |
| `horizon:quiet_commands` | `sunrise:quiet_commands` | The codec implementation is identical after namespace replacement (the remaining source difference is a documentation path). Announcement and routing still need an adapter. Normal chat commands remain the fallback. |
| `horizon:mod_list` | `sunrise:mod_list` | Incompatible nonempty payloads. Horizon writes count, then each id/version, a side byte and optional SHA256. Sunrise writes only both-side id/version pairs. Never alias the channel without a codec bridge and verified local identities. |
| `horizon:block_registry` | `sunrise:block_registry` | Both use a 32-byte identity challenge. Base block-name/property hashing agrees structurally, but Sunrise adds block-entity/custom-stat registries and validity mappings; custom-component/default hash domain tags also use different namespaces. Equality must be computed from local state, never assumed from matching lengths or echoed from the server. |
| `horizon:menu_extra` | `sunrise:menu_extra` | Horizon matches Sunrise's **legacy** layout: window, mandatory XYZ, byte array. Sunrise revision 1 adds a presence boolean before optional XYZ. Use the legacy decoder on Horizon messages; do not insert the new layout into an unnegotiated stream. |
| `horizon:mod_net` | No `sunrise-protocol/src/mod_net.rs` | Horizon's legacy native-message transport has no same-named implementation on this Sunrise pin. Sunrise application payload negotiation is a different contract. |
| `horizon:attachment_sync` | `sunrise:attachment_sync` | Holder/value structure is similar, but limits and negotiated channel/version rules differ. Not established as interchangeable. |
| `horizon:lod` | `sunrise:lod` | Optional far-terrain extension. Full codec and runtime route interoperability has not been verified; leave it unnegotiated until tested. |

These are engine-owned messages. Registering an application payload or changing the displayed
server brand does not automatically connect them to Sunrise's engine handlers.

## Minimum successful connection vs. full support

On Horizon, a content-registry check is conditional on `sync.content_identity`, and the mod-list
exchange depends on server mod declarations. A plain server can therefore pass configuration
after the correct brand without negotiating every optional extension. Whether the official SMP
uses custom content or additional extensions must be checked against its deployed build/logs.

For a server requiring both-side content mods, retain real missing-mod / registry mismatch
refusals until local content and the wire codec are verified. An empty mod list is not a valid
substitute for required mods.

Next runtime validation after the SDK hook exists: use the launcher replacement pipeline, inspect
the server's received brand, complete configuration with the actual SMP, then test vanilla menus,
signed chat, commands, reconnects and any extensions the server actually announces. The loopback
brand test only proves the helper's packet bytes satisfy a brand gate; it runs neither client.

## Source references

- [Horizon configuration and gate](https://github.com/horizon-rust/horizon/blob/9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0/crates/horizon-client/src/lan/conn.rs)
- [Sunrise connection and configuration](https://github.com/sunrise-org/sunrise/blob/234db8df6210165318b2b604dea50275357bfaeb/crates/sunrise-client/src/net.rs)
- [Horizon mod-list codec](https://github.com/horizon-rust/horizon/blob/9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0/crates/horizon-protocol/src/mod_list.rs)
- [Sunrise mod-list codec](https://github.com/sunrise-org/sunrise/blob/234db8df6210165318b2b604dea50275357bfaeb/crates/sunrise-protocol/src/mod_list.rs)
- [Horizon registry identity](https://github.com/horizon-rust/horizon/blob/9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0/crates/horizon-client/src/content_identity.rs)
- [Sunrise registry identity](https://github.com/sunrise-org/sunrise/blob/234db8df6210165318b2b604dea50275357bfaeb/crates/sunrise-client/src/content_identity.rs)
- [Horizon menu extras](https://github.com/horizon-rust/horizon/blob/9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0/crates/horizon-protocol/src/menu_extra.rs)
- [Sunrise menu extras](https://github.com/sunrise-org/sunrise/blob/234db8df6210165318b2b604dea50275357bfaeb/crates/sunrise-protocol/src/menu_extra.rs)
