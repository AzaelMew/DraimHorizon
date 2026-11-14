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
  - `baritone.rs`: `enabled()` (cached for half a second, `invalidate_enabled()` drops the
    cache) and `run_line()` are added; `hooks/chat.rs`, `hooks/command.rs` and
    `hooks/tick.rs` use it instead of `native_on("baritone")`, which is always true for a mod
    that is not registered. The port runs only with the experiments consent given and the
    switch on the Baritone tab on.
- The module and hook list of upstream's `horizon-mixin.toml` is merged into DraimHorizon's
  with the ids prefixed `bar-` and the source paths under `baritone/`. Not taken: the
  Options > Mods entry and its settings page hooks (`list`, `settings`, `click`, `page_*`).
  The two sort hooks target `horizon_client::game::extras::sorting` (upstream's Nightly had
  them in `tweaks`).
- Upstream's data folder `baritone` (settings, waypoints, cache, schematics) is kept.
- Performance changes: while the port is off its hooks do nothing (`port_on()` in `baritone.rs`: look,
  block-change, interact, toast, beacon; `hooks/tick.rs` calls `off_tick`, which lets go once after the
  port was on instead of running `stop` every tick; `hooks/unload.rs` and `game/unload_stash.rs` keep no
  chunks while off). `block_changed` no longer clones the block name. Sections are timed with `prof!`
  when the profiler is on. The chunk packer budget is 1 ms a tick (`PACK_BUDGET_MS` in
  `behavior/pathing.rs`, upstream 4 and 6) and a search slice is 10 ms (`SEARCH_SLICE` in
  `behavior/pathing_behavior.rs`, upstream 25).
- DraimHorizon's own glue is MIT and lives outside this directory: `baritone_bridge.rs` (walks for the
  farm and mining, the tab's settings and status) and `baritone_walk.rs` (its pure logic). They call
  the port only through `crate::game::baritone::with` and its public API.
- Upstream's `notification.rs` is still built: `#orderpizza` opens a web page and the
  notification settings can run a desktop command, both only on the player's own request.
