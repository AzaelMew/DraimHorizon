**DraimHorizon 0.40.3 - map fixes**

Target: official Horizon Nightly source unchanged.

- Double-click the last death point on the map with the left mouse button to remove it, as with a regular waypoint. A new death shows it again. The game's last-death state and the recovery compass are untouched.
- Farm: the harvest, seeds and bone-meal containers you point at are remembered per world after a restart or reconnect.

Verification: rustfmt check only; the container persistence round-trip test passes; clippy, the full tests and the native build are not yet run.
