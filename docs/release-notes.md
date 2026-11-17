**DraimHorizon 0.42.0 — Experimental Sunrise SMP adapter**

The Horizon target remains official Nightly `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0`.

A separate native Sunrise project in `platforms/sunrise/smp-compat` now uses two existing replacement
points in official Sunrise SDK `234db8df6210165318b2b604dea50275357bfaeb`: `Session::poll` and
`receive_brand`. It queues `minecraft:brand = horizon` before the known-pack acknowledgement only
for `playhorizon.dev` and its subdomains, through the connection's existing writer.

This route needs no client source edits, new SDK hook or rebuilt game. The previously proposed
upstream patch has been removed. SDK preflight and native helper type checks pass on the current SDK;
synchronization and loopback checks are separate from runtime validation.

Still unverified: complete launcher build (the isolated installation lacks prepared Minecraft 26.3
resources), live SMP entry, reconfiguration and Horizon extension interoperability. The launcher's
source discovery also reports an existing relative-module-path error. Full DraimHorizon porting remains
in progress; do not advertise Sunrise support yet. See the adapter README for staging and validation.

- Experimental Sunrise port (source level): the generated bundle applies 75 of 79 hooks and 214 modules to the pinned Sunrise Nightly and the modified `sunrise-client` compiles. Not run in a Sunrise launcher or on a server; command completion and the Options → Mods entry are not ported.
