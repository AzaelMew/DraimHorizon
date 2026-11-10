**DraimHorizon 0.39.4 — Unreleased UI update**

Base: upstream main / release **0.39.3** (`50dcce711b1f0e0d0e4e3574f43298bd76a9cfa7`).
Horizon target: official Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

This is one unreleased update. The earlier local-only 0.40.x/0.41.0 test labels have been consolidated into 0.39.4; subsequent fixes and test rebuilds keep this version until publication or an upstream base change. No published tag is reused.

- Fixed overlapping text in Experiments → Baritone → Mining. Storage/custom-block actions, task status/auto-eat, and action-key hints have distinct footer rows. Compact hints reserve space for Settings/language; wide layouts use the full content width. Schematic footer actions use the same separation.
- Preserved all current upstream 0.39.3 gameplay additions alongside the UI work, including Human look, arbitrary mining blocks, farm levels/bindings and schematic previews. Helper paging derives its size from actual helper rows so new helpers are not hidden.
- Includes the broader UI update: dedicated Trading navigation with grouped preferences, responsive trading details and scrolling, grouped inventory tools, clearer map/filter controls, waypoint-list confirmation, readable appearance/helper pages, and translation-role explanations.
- The J menu groups preferences by feature: Map and claims (claims list, Map, Borders, Colors, Storage), Chat (style, notifications, anti-spam, DeepL, translation) and Settings (General, Network, Player list, Sidebar).
- Steppers are single rows with a value cell and − / + buttons; choices show their value and a chevron; switch rows no longer look selected; text inputs are inset; single-page settings hide paging. Map tools say Zoom in/Zoom out and use locate/skull icons; seed-map status text no longer covers the tools.
- Translation languages is a centred card with My messages / Chat messages pairs and all languages visible at once.
- The auction is a catalogue of every lot, loaded in the background when it opens; one pager, search and sort across all lots, and `/ah buy <number>` purchases behind a confirmation that can be switched off (Ask before buying at the auction). Market screens keep one pager across server pages. Windows are compact by default with an optional large size.
- Single player no longer sends automatic `/claims` and `/bal`, which raised Horizon's unknown-command prompt.
- The explored map stays sharp and relief-shaded when zoomed out: wide views use per-region overviews (2/4/8 blocks per texel) instead of one colour per chunk. They are built in the background, centre first, and the saved map format is unchanged.
- Boolean settings use switches; numeric values have units and read-only value cells; corner choices use names. Chat original-message badges follow the selected UI language. Russian remains the default, with English counterparts.
- Saved settings, archives, maps, waypoints and server-authoritative gameplay behavior are preserved. Local delivery replaces the installed bundle after checks, without restarting the game or publishing a release.

Regression coverage includes footer separation in Russian/English and compact/wide layouts, alongside the navigation and persistence checks. Menus, settings sections, the explored and seed maps were checked in a real native client (single player, both languages); the auction catalogue, its walk, search, sort, refresh and confirmation dialog, and the market screens were checked on play.playhorizon.dev at 1080p and 1440p without buying, selling or bidding; `/ah buy` itself was not run by the tests; live claim refreshes were not exercised.
