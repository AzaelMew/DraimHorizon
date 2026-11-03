# Interaction workflows — development tracking

Base: v0.14.0, Horizon f42ef42. Work solo; use GitHub tools and Actions while the local executor is unavailable. Do not publish an incomplete update.

- [x] Inventory search implementation and UTF-8 input model committed; compilation pending.
- [ ] X opens compact waypoint create dialog (name, type, colour).
- [ ] Waypoints: world grouping/filter, click row to edit, trailing delete icon.
- [ ] Map: double LMB deletes nearest own waypoint; double RMB creates; keep dragging.
- [ ] Party: important-player list; every friend automatically important.
- [ ] Incoming translation: transport diagnosis, retries and visible status; outgoing toggle beside chat input.
- [ ] Tab: distinct player cards, clear columns.
- [ ] HUD: separate performance (FPS/TPS/online/playtime) and economy cards.
- [ ] Market/AH: correct Price: lore parsing, 24h movers, exact server button totals/limits, RU client, automatic read-only refresh, themed native browser, explicit transaction confirmation.
- [ ] Full fmt, clippy, tests, actual native check, package validation, release notes/version, publish and digest verification.

Known findings: market lore prefixes prices with Price:; existing quote parser cannot parse them. Movers paper is not ingested. Translation failures remain permanently requested and cannot retry. No implementation has changed main yet.
