**DraimHorizon 0.26.0** for official Horizon Nightly 2026-11-07 - 2ebf00a (Java 26.3).

- J > Experiments > Baritone: enter X Y Z, select a saved waypoint or follow a nearby loaded player.
- Preview a route without moving, or Go to start walking. Pause, Resume and Stop are available in the same tab.
- A bounded background planner uses loaded server chunks. The world line shows the current segment; the HUD shows status and straight-line distance.
- Following maintains a short distance and updates the goal by player UUID. Losing sight stops movement immediately; after 10 seconds following stops until Resume.
- Manual movement and open screens pause navigation. Terrain changes and stalls trigger bounded replanning; reconnecting or changing worlds clears the destination.
- Baritone > Mining: select an ore and a 1–64 block limit; preview or mine reachable exposed ore from loaded chunks. Uses ordinary block-breaking input and server-confirmed counts, with tool, inventory and terrain checks.
- First walking implementation: full blocks, one-block jumps/descents. Partial shapes, fluids and hazards are obstacles. Placement, tunneling, farming, building and flight are not included.

This is an independent native navigator, not an integration of the Java Baritone library. See docs/navigation.md for controls and limitations.

The bundle ID remains draimhorizon; settings and saved data remain under claim_borders. Users upgrading from claim_borders must reinstall the mod once.
