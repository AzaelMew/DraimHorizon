**DraimHorizon 0.25.0** for official Horizon Nightly 2026-11-07 - 6560641 (Java 26.3).

**Upgrade requires a reinstall:** remove `claim_borders` in Horizon Launcher, install `https://github.com/DraimCiDo/DraimHorizon` again, then rebuild native mods. The new bundle ID is `draimhorizon`; the launcher cannot update the old ID into the new one. Do not keep both versions installed. Keep the data folder: settings, waypoints, friends and history reuse `claim_borders`. Reapply custom key bindings and activation preferences if needed.

- Single-line presence cards with official Lucide + / minus marks before player names.
- Green joins, red leaves; cyan joins and lavender leaves for friends and important players.
- Settings > Chat > Notifications > Presence view switches between Icon and name and the localized Text view.
- Includes PR #11 with the identity/storage separation, checked against the official Nightly rather than main.

The shared DeepL service requirements from 0.24.0 are unchanged (service 1.3.0 for all new language/source settings).
