**DraimHorizon 0.41.1 — flat packages for the current launchers (pre-release)**

Horizon and Sunrise launchers no longer install `.hnmod` / `horizon-mixin.toml` mods ("Old format (.hnmod): ask its
author to republish it as .hmod"). This release ships the flat packages they read:

- `draimhorizons-0.41.1.hmod` for Horizon Launcher and `draimhorizons-0.41.1.smod` for Sunrise Launcher, with the
  legacy `draimhorizons-0.41.1.hnmod` for old launchers. Install from the release link, not from the repository
  link: the repository's own source tree still has the old layout and the launcher refuses it.
- The packages pin Horizon Nightly `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0` and Sunrise Nightly
  `234db8df6210165318b2b604dea50275357bfaeb`.

Checked with the launchers' own package readers: the packages are read, selected and packed. **Not verified:** the
launchers' native build (SDK of the pinned build, `native-lock` for the relay client's dependencies), a running
client or a server. Sunrise support remains experimental; see the README.

A separate experimental Horizon SMP brand adapter for Sunrise lives in `platforms/sunrise/smp-compat`; it is not part of these packages.
