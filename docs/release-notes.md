**DraimHorizon 0.39.22 - show invisible entities**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- **J → Settings → General → Show invisible entities** restores invisible player and mob models locally. Off by default; persisted using the existing settings namespace. English and Russian controls are included.
- Normal depth testing, view range and frustum culling still apply. This cannot reveal entities withheld by a server or change what another player sees.
- Entity metadata and potion timers are not changed. Turning the switch or the whole mod off restores normal rendering immediately. Invisible armour stands and item frames are not living entities and are left alone.

Verification: Rust formatting, Clippy, workspace tests, official mixin staging, native client compile-check, a native renderer regression using authored geometry, and bundle validation. Live in-game acceptance is not verified.
