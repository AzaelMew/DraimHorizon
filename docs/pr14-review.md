# PR #14 review and follow-up corrections

Repository: https://github.com/DraimCiDo/DraimHorizon

Original reviewed PR head: `b7a31c4b375f5f2b5780d86523aaa73bab439cb9` (42 commits, six changed files relative to the PR base). This document distinguishes the original findings from the follow-up source corrections. The PR remains a draft; these changes do not authorize merging, releasing or installing the mod.

## Verdict

**Keep the PR in draft.** The current implementation is a useful construction/UI/recovery increment, not a completed autonomous Dig/Bridge navigator. The author's continuation comment correctly identifies this distinction; the PR description is older than the dropdown and action-planner commits.

## Author's implemented changes

| Commit group | Implemented behavior | Assessment |
| --- | --- | --- |
| `7243f1b` through `aa66f94`, plus floor assertion fixes | Derive tunnel/bridge direction from Minecraft yaw, enable tunnel floor by default, add regressions | Preserve. The native adapter sets direction when starting the construction job; this is not merely a UI label change. |
| `6322414` through `f437e3a`, including intermediate compact-layout corrections | Vertical/compact construction controls and geometry tests | Preserve final corrected geometry rather than resurrecting an earlier layout. Geometry tests are not live visual acceptance. |
| `5d9cd58`, `7318855`, `3c91cba` | Bounded recovery from missing walking routes and planning timeouts | Preserve. Mining timeouts retain their separate behavior; failed preview does not start walking. |
| `caefde2` through `576492f` | Direct player/waypoint dropdowns, paging/wheel support, dismissal, identity reconciliation and empty states | Implemented beyond the outdated PR description. One older waypoint test still assumed opening the selector selected its first entry; corrected in the follow-up. |
| `6759629` through `b7a31c4` | `Walk`/`Dig`/`Bridge` preflight, weighted bounded A*, worker fallback for absent/incomplete walking routes, diagnostic status, separate head/feet digs | Planning only. The adapter stores the action plan and explicitly does not submit it to walking input. No executor has been connected. |
| `e37011f` | Disable verification specifically for this PR branch | Removed in the follow-up. A draft should not silently bypass its verification gates. |

## Findings and corrections

### P1 — action execution remains missing (not implemented by this review)

`mixins/draimhorizon/navigation_view.rs:157`, `:1011`, `:1280` calculate, retain and describe an action plan without executing its nodes/actions. Repeated walking replans cannot create a missing support block or clear an obstruction. Successful search/model tests therefore do not satisfy the PR's world-action requirement.

The necessary implementation is a separate, sequential executor: approach a safe standing position, re-read live blocks, act on one block, wait for the matching server acknowledgement and validated world state, then replan. A two-high obstruction must be cleared in separate head/feet operations. Never feed edit-requiring nodes directly to walking input.

### P1 — action plans do not establish mining/placement permission or resources (remaining)

The action-search callback receives `Cell`, not the full live construction/mining safety facts. A full collision cell is not proof of a breakable/allowed block. Falling-block facts, block entities, tool suitability/durability, materials and server territory rules are not established by this search. Do not enable execution merely because A* returns a complete plan. Reuse the existing mining send/ack and construction placement gates; retain pending acknowledgement state across pause/cancel transitions.

### P1 — CI intentionally skipped this branch (fixed in the follow-up)

Removed `verify.if` from `.github/workflows/release.yml:25`. GitHub reported both `verify` and dependent `release` as skipped at the original reviewed head. The restored workflow must validate the updated PR head; local results are not a substitute for remote CI. The release job still correctly excludes pull requests.

### P2 — edits adjacent to unknown terrain passed preflight (fixed)

`mixins/draimhorizon/navigation.rs:145` now rejects `Unknown` as well as `Hazard` throughout the checked neighborhood, including overhead and adjacent support cells. A new regression first failed by producing `Some(Dig { ... })` beside an unknown block, then passed after the fix. Coverage includes digging and bridging.

This strengthens conservative preflight; it does not turn the planner into a complete live safety/permissions system.

### P2 — vanished selection silently fell back to old coordinates (fixed)

`mixins/draimhorizon/menu.rs:779` cleared a missing selected UUID/waypoint ID but retained coordinates entered before that selection. A later Start could therefore navigate to an unrelated old destination. Reconciliation now clears that stale coordinate fallback and shows an EN/RU notice requiring a fresh destination. The regression covers both target kinds and both languages and was observed failing before the fix.

### P2 — global configurable controls were absent (implemented)

`mixins/draimhorizon/navigation.rs:9`, `navigation_view.rs:21`, `client.rs:14`, and `menu_open_hook.rs` register and handle **start/resume**, **pause** and **stop** through Horizon's native key system. Options > Mods supplies the editable controls and existing binding persistence.

- Unassigned by default; no gameplay key is stolen.
- Start uses the last preview or paused task; pressing it while active does not restart that job.
- Ignore releases, native key repeats, unfocused/obstructed gameplay, modifiers and absent experimental consent.
- Ignore duplicate automation bindings and conflicts with gameplay/enabled native-mod bindings.
- Keep EN/RU labels and existing consent/world/server-confirmation entry points.

Pure event-selection tests were observed red then green. Native code was compile-checked. Live typing, rebinding persistence and actual input gestures have not been exercised in the game.

### P2 — the unverified head failed its own gates (fixed in the follow-up)

Baseline `cargo test --workspace --locked` stopped with **289 passed, one failed** in `navigation_waypoint_selection_and_manual_edit_are_separate_targets`. The test now explicitly selects a dropdown row before starting, matching the author's newer direct-selection UX.

Formatting also failed, and strict Clippy identified one collapsible conditional and two test initializations. Those issues are corrected without disabling lints or changing the final layout.

## Specification status

| Requirement | Status after follow-up corrections |
| --- | --- |
| Construction direction from facing; default floor | Existing implementation retained |
| Compact/regular construction settings | Existing implementation retained; model tests pass |
| Direct waypoint/player list selection and list lifecycle | Existing implementation retained; stale-target fallback fixed |
| Bounded no-path/worker-timeout recovery | Existing implementation retained |
| Configurable global start/pause/stop | Implemented, pure tests + native compile; live acceptance pending |
| Action-aware planning | Existing diagnostic planner retained; unknown-neighbor check strengthened |
| Sequential Dig/Bridge execution and confirmed replanning | **Not implemented** |
| Full live permissions, tool/resource/hazard/cancellation integration for that executor | **Not complete** |
| Restore verification | Branch-specific bypass removed; current head CI must be checked separately |
| Native packaging/validation | Passed locally |
| In-game end-to-end validation | **Not performed** |

The action search is currently level-only; a future executor/planner integration must also define how ordinary step-up/down walking connects to edited sections, partial routes, changed targets and bounded no-progress recovery.

## Initial review-build verification

The following results were recorded for the initial patched review build before integrating upstream main. Final integrated-source verification is recorded separately; refer to the current PR checks for remote CI status.

- `python tools/version.py check`: synchronized **0.29.0** across four version entry points.
- `python tools/compatibility.py`: official Nightly pin `2ebf00a0281c592f79191e76fce0e7d522c6f425` accepted.
- `cargo fmt --all --check`: passed.
- `rustfmt --edition 2024 --check mixins/draimhorizon/*.rs`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: **325 passed, one ignored** (293 runtime, 4 schematic integration, 7 translation integration, 21 relay-client tests). The ignored test was not run.
- Python tooling unit tests: **6 passed**.
- Official mixin staging: **50 injections, 76 modules, status ok**.
- Real pinned Horizon `cargo check -p horizon-client --bin horizon-client --no-default-features --locked`: passed; the injected client emitted **19 dead-code warnings**. This is compile-checking, not a built/running game or live SMP validation.
- Native bundle validation: **`ok: true`**, ID `draimhorizon`, version `0.29.0`, Java `26.3`.
- `git diff --check`: passed.

No relay server deployment/provider/network end-to-end tests were run; relay behavior was not changed. No claim is made that GitHub CI or live game acceptance passed.

## Delivery and integration cautions

- Test bundles use `target/draimhorizons-0.29.0.hnmod`; they are not published releases.
- Version bumped once to **0.29.0** for the new global-control capability; changelog, release notes and control documentation updated.
- Upstream main's independent **0.28.1** translation fix (`e2abab1`) must be preserved when integrating this branch. Integrate main with a merge rather than rewriting the author's commit history, retain version **0.29.0**, and verify the combined source.
- This bundle is an **unreleased review build**, not approval to merge or an installation into the user's game.
