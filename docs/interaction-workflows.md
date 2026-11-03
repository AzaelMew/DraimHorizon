# Interaction workflows — 0.15.0

Target: Horizon f42ef42, API 359. Development and publication use main as requested.

Implemented:
- Inventory search, UTF-8 input and localized matching.
- X waypoint editor; persisted kinds with schema-1 migration; world filter and row editing/deletion.
- Double-click map creation/deletion, drag preservation, marker icons.
- Party important-player list; all friends automatically important.
- Incoming translation retries/status; real provider greeting smoke; outgoing input toggle.
- Player cards and separate performance/economy HUD.
- Live Market/AH interface, exact server action confirmation, dynamic lore prices/volume/movers, automatic bounded navigation and localized item labels.

Publication requires strict formatting/lint/tests, native compilation and package validation. The release workflow waits for successful native validation of the same commit.

Manual game checks still required: GUI scales, drag vs double click, localized resource packs, chat caret/completions, storage search focus, changed server prices, partial/closed-market scans and transaction rejection. No interactive game is available in the connector-only environment.
