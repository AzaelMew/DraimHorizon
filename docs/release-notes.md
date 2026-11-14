**DraimHorizon 0.40.3 — Auto Eat food filter and no-waste meals**

Target: official Horizon Nightly source `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0`.

- Open **J → Experiments → Helpers → Allowed foods**. Toggle food-item icon tiles, like the mining target filter; selected tiles are highlighted, hover text shows the food name and hunger restored, and pages keep all foods reachable in small menus. Controls are available in English and Russian.
- Only explicitly selected safe foods may be eaten. The list starts empty, including when upgrading: select foods before relying on Auto Eat. Risky, teleporting and reserved foods remain excluded.
- A food is eaten only when its entire hunger value fits: steak (+8) waits until 12/20 hunger or lower, bread (+5) until 15/20 or lower. Smaller selected snacks no longer wait for the old fixed 14/20 threshold. There is no wasteful fallback or starvation exception. Saturation is not part of this check.
- Pending meals recheck the actual stack and current allowlist. Farm food-availability checks use the same list, without mistaking food that merely needs more hunger room for missing supplies.
- Review correction: hunger values come from the actual stack's food component using Horizon's own consumption resolver. Overridden bread restoring 8 points waits until 12/20 rather than using bread's default 5; a removed or zero-nutrition food component is not a meal candidate or usable farm food supply. Pending meals re-resolve components even when the item id is unchanged.

Native review regressions: four tests using real Horizon `ItemStack` food overrides/removals fail with the previous default-only resolution and pass with stack-aware resolution. Run after staging: `cargo test --manifest-path target/mixins/modded/Cargo.toml --profile fast -p horizon-client --lib --locked auto_eat_stack_tests`.

Review-correction checks: 565 workspace tests pass (one ignored), plus the four native regressions above; version consistency, formatting, strict workspace Clippy, official mixin staging, real-client compilation and final bundle validation pass. No new live modified-food test was performed; the native regressions exercise real stack components and the production nutrition helper.

Initial feature verification (before the review correction): 565 workspace tests passed (one ignored), along with formatting, strict Clippy, native compilation/build and bundle validation. In an isolated local Survival client, steak stayed untouched at 13/20 and consumed one at 12/20 to reach 20/20; unselected bread stayed untouched at 15/20, then consumed one after selection to reach 20/20. English/Russian tiles and selection persistence after a process restart were inspected. These historical live checks did not exercise modified food components; no live SMP acceptance is claimed.
