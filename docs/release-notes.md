**DraimHorizon 0.41.3 — Horizon & Sunrise (pre-release)**

- **Horizon:** `draimhorizons-0.41.3.hnmod` for Horizon Nightly `9f42064`, with a launcher that still builds `.hnmod`
  mods (0.2.69 or older). Horizon Launcher 0.2.70+ needs a published Horizon mod SDK, which the current Nightly
  does not have yet, so no `.hmod` is attached.
- **New: the client always announces itself as `horizon` on playhorizon.dev.** The official SMP reads one string in
  the configuration phase — the `minecraft:brand` custom payload — and disconnects anyone whose value is not exactly
  `horizon`. Earlier builds sent `vanilla` first and re-announced `horizon` only after the server identified itself.
  Now the first brand is already `horizon` there; every other server still receives the client's own `vanilla`, so
  nothing changes on Paper, vanilla, singleplayer or LAN. This is a policy filter, not authentication: identity is
  still the Microsoft account (online-mode).
- **Sunrise:** `draimhorizons-0.41.3.smod`. In Sunrise Launcher 1.0.1 or newer choose **Mods → Add from GitHub**
  and paste `https://github.com/DraimCiDo/DraimHorizon/releases/tag/v0.41.3`, then rebuild native mods. Sunrise keeps
  its engine's brand exchange; the separate smp-compat mod coordinates the brand there.
- Every asset has a `.sha256` next to it. The relay archive `draimhorizons-relay-0.41.3.tar.gz` carries no secrets.
