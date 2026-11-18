**DraimHorizon 0.41.4 — Horizon & Sunrise (pre-release)**

- **Sunrise: joining playhorizon.dev without a second mod.** The official SMP currently admits only Horizon clients:
  it checks the `minecraft:brand` sent during login. The `.smod` now carries the brand adapter that was a separate
  mod: when the server announces itself as Horizon, the client sends `horizon` before the login continues. Every
  other server still sees Sunrise unchanged. **Experimental:** it compiles against the Sunrise SDK of Nightly
  `2d81263`, but it has not yet been confirmed on the live server, and Horizon-specific server channels (block
  registry check, mod list) are not bridged yet. If the server disconnects you after login, please report the message.
- **Sunrise:** `draimhorizons-0.41.4.smod`. In Sunrise Launcher 1.0.1 or newer choose **Mods → Add from GitHub**
  and paste `https://github.com/DraimCiDo/DraimHorizon/releases/tag/v0.41.4`, then rebuild native mods. If you
  installed the separate SMP compatibility mod, remove it.
- **Horizon:** `draimhorizons-0.41.4.hnmod` for Horizon Nightly `9f42064`, with a launcher that still builds `.hnmod`
  mods (0.2.69 or older). Horizon Launcher 0.2.70+ needs a published Horizon mod SDK, which the current Nightly
  does not have yet, so no `.hmod` is attached. Horizon behavior is unchanged from 0.41.3.
- Every asset has a `.sha256` next to it. The relay archive `draimhorizons-relay-0.41.4.tar.gz` carries no secrets.
