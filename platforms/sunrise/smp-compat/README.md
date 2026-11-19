# Horizon SMP brand adapter — retired

This directory used to hold a native adapter that made Sunrise present the `horizon` brand to the official
Horizon server, because Sunrise's network code sent `vanilla` from a lower crate that no SDK target replaces.
Since 0.41.4 it was folded into the main `.smod` as two mixins (`register-smp-session` on
`sunrise_client_lib::net::Session::poll`, `announce-horizon-brand` on `net::receive_brand`) plus a
rendezvous and an address policy in the mod crate.

**It is out of the package since Nightly `ed3ac19` (2026-11-19).** That Nightly adds `Hooks::client_identity`
and ships `sunrise-official`, whose `HORIZON_IDENTITY` part answers for `play.playhorizon.dev` with the brand
`horizon` and the empty `horizon:payloads` offer (revision 1, no records). On the source route the launcher's
native-mod build enables the `official` feature and calls `sunrise_official::register()` before the game runs.
Keeping the adapter would have announced the brand twice and pinned an internal
(`sunrise_client_lib::net::Session`, whose hash changes with most nightlies), which is what the drop rules in
`platform.toml` and the retired `internals` table used to hold together.

Two differences from the retired adapter, both in the engine's favour:

- the engine also sends the `horizon:payloads` offer, which the adapter did not;
- the engine matches the exact host `play.playhorizon.dev` (one trailing dot ignored, no port or 24454 /
  25565 only), where the adapter matched any `*.playhorizon.dev` name.

**Known gap (upstream, not this mod):** on Nightly `ed3ac19` the launcher's *prebuilt* native route builds only
`sunrise-mod-host`, so `sunrise-official` is never linked and a `.smod` client still announces `vanilla` on
`play.playhorizon.dev`. Tracked upstream as
[sunrise-org/sunrise#3119](https://github.com/sunrise-org/sunrise/issues/3119); no mod-side hook or replaceable
SDK target covers it.

[protocol-audit.md](protocol-audit.md) keeps the server-side notes: what the SMP checks after the brand
(block registry hash, mod list), which `horizon:*` channels Sunrise ignores safely, and the older
`DraimHorizon: Horizon SMP brand <outcome>` log lines that a client running 0.41.x still writes. Those
checks are still not bridged by this mod.
