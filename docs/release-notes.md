**DraimHorizon 0.26.0** for official Horizon Nightly 2026-11-07 - 6560641 (Java 26.3).

- J > Experiments > Baritone: enter X Y Z or select a saved waypoint in the current dimension.
- Preview a route without moving, or Go to start walking. Pause, Resume and Stop are available in the same tab.
- A bounded background planner uses loaded server chunks. The world line shows the current segment; the HUD shows status and straight-line distance.
- Manual movement and open screens pause navigation. Route changes and stalls also pause; reconnecting or changing worlds clears the destination.
- First walking implementation: full blocks, one-block jumps/descents. Partial shapes, fluids and hazards are obstacles. Mining, placement, following, farming, building and flight are not included.

This is an independent native navigator, not an integration of the Java Baritone library. See docs/navigation.md for controls and limitations.

The bundle ID remains draimhorizon; settings and saved data remain under claim_borders. Users upgrading from claim_borders must reinstall the mod once.
