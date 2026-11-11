**DraimHorizon 0.39.13 - Protect glow berry harvesting**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- **J → Settings → General → Protect glow berry vines** now prevents both accidental breaking and right-click berry harvesting on either cave-vine section.
- Protection applies to held clicks and either hand. Sneak while clicking to deliberately harvest or break a vine.
- Reuses the existing saved setting (off by default); entity interactions and other blocks remain unchanged. Disabling the mod disables protection.
- Updated English and Russian setting hints.

Verification: workspace regression tests and native-client compilation are checked separately from in-game behavior; this change has not been tested in game.
