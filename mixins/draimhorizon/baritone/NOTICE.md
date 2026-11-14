# Baritone navigation (LGPL-3.0)

The files in this directory are **not** MIT. They are a Rust port of
[Baritone](https://github.com/cabaletta/baritone) 1.21.4 by leijurv and the Baritone contributors,
licensed under the **GNU Lesser General Public License v3.0** (with the "no anime" part in
`LICENSE-Part-2.jpg`). They are taken from the port for Horizon by luhnox,
https://github.com/luhnox/horizon-baritone (commit `41e1b72`, also LGPL-3.0), and keep its
file layout, module names (`baritone_*`) and authorship.

- `LICENSE`, `COPYING` and `LICENSE-Part-2.jpg` are the license texts, unchanged.
- The rest of DraimHorizon stays under the MIT license and uses this directory only through
  its public functions.
- Local changes to the upstream files are listed here:
  - `behavior/pathing_behavior.rs`: the desktop notification on a failed search (it ran an
    external command) is not ported.

Not included yet: commands, processes (mine, farm, build, elytra, ...), the schematic and
selection code, the settings screen and the game bridge (`src/game/`).
