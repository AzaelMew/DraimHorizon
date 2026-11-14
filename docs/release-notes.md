**DraimHorizon 0.40.3 — Auto Eat food filter and no-waste meals**

Target: official Horizon Nightly source `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0`.

- Open **J → Experiments → Helpers → Allowed foods**. Toggle food-item icon tiles, like the mining target filter; selected tiles are highlighted, hover text shows the food name and hunger restored, and pages keep all foods reachable in small menus. Controls are available in English and Russian.
- Only explicitly selected safe foods may be eaten. The list starts empty, including when upgrading: select foods before relying on Auto Eat. Risky, teleporting and reserved foods remain excluded.
- A food is eaten only when its entire hunger value fits: steak (+8) waits until 12/20 hunger or lower, bread (+5) until 15/20 or lower. Smaller selected snacks no longer wait for the old fixed 14/20 threshold. There is no wasteful fallback or starvation exception. Saturation is not part of this check.
- Pending meals recheck the actual stack and current allowlist. Farm food-availability checks use the same list, without mistaking food that merely needs more hunger room for missing supplies.

Verification: 565 workspace tests pass (one ignored); version checks, formatting and strict Clippy pass. Official pinned mixins apply, the real native client compiles and builds optimized, and the source bundle validates with `ok: true`. In an isolated local Survival client, steak stayed untouched at 13/20 and consumed one at 12/20 to reach 20/20; unselected bread stayed untouched at 15/20, then consumed one after selection to reach 20/20. English/Russian icon tiles, hover hunger values, reduced-window layout and selection persistence across a full process restart were inspected. No live SMP acceptance or restart of the owner's client was performed.
