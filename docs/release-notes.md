**DraimHorizon 0.40.4 — Auto Tools held-tool trigger and Auto Eat "Eat to heal"**

Target: official Horizon Nightly source `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0`.

- New optional **Trigger only when holding tool** toggle under Auto Tools (J → Experiments → Helpers; Russian: «Только с инструментом в руке»). It is off by default, so existing setups behave as before, and it does not turn Auto Tools on by itself.
- When on, Auto Tools switches hotbar slots only while your main hand holds a pickaxe, axe or shovel. Swords, food, hoes, shears, an empty hand and off-hand tools do not trigger it, and switching stops immediately if you select one of them mid-attack.
- Only the item category is checked: the held tool need not suit the block or be fresh. Automated mining, construction, farming and durability protection are unchanged.

- New optional **Eat to heal** toggle under Auto Eat (J → Experiments → Helpers; Russian: «Есть для лечения»), off by default. With Auto Eat on and your health below your actual maximum, an allowed safe food may be eaten even if it overfills hunger: a food that fits is still preferred, otherwise the smallest one is used. It never eats at full hunger and does nothing at full health, with the toggle off, or with invalid health.
- This only lets Auto Eat waste hunger while you are injured. It is not a custom HP target, not instant healing, and does not touch health directly: recovery still comes from the server's normal regeneration. Allowed-food list, risky-food exclusions, modified food components, interruption by attacks/use and farm supply checks are unchanged.
