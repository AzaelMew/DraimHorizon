**DraimHorizon 0.27.0** for official Horizon Nightly 2026-11-07 - 2ebf00a (Java 26.3).

- J > Experiments > Baritone now includes Tunnel and Bridge alongside Navigation and Mining.
- Tunnel: choose cardinal direction, length, width 1–3, height 2–3 and optional floor filling. Clears top-down, confirms breaks and advances section by section.
- Bridge: creates a one-block-wide level floor from suitable hotbar blocks. Uses sneak movement at the edge, actual support faces and a dry placement check; waits for the server before advancing.
- Pause/Resume preserves distance and in-flight acknowledgements. Manual slot changes remain authoritative; automatic material selection restores the previous slot afterwards.
- Stops at fluids, unavailable terrain, falling blocks, protected shapes, server refusals, missing tools/materials or timeouts. World/account changes clear tasks.

Includes the navigation, player following and exposed-ore mining from 0.26.0. Farming, arbitrary schematic building and flight are not included.

See docs/construction.md and docs/navigation.md for controls and limits. The bundle ID remains draimhorizon; existing settings/data remain under claim_borders.
