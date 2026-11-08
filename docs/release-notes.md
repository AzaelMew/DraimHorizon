**DraimHorizon 0.29.1 — PR #14 review build (unreleased)**

Target: official Horizon Nightly source `2ebf00a0281c592f79191e76fce0e7d522c6f425` (Java 26.3).

- Configurable **Baritone: start / resume**, **pause** and **stop** bindings in **Options > Mods > DraimHorizon**. Unassigned by default; Horizon saves custom bindings. They apply outside menus, not while typing, and ignore conflicting gameplay/native-mod keys. Start uses the last preview or paused task and does not restart an active job.
- Action-route preflight rejects unknown neighboring terrain and overhead cells.
- Losing a selected player/waypoint clears stale coordinate fallback and asks for a new destination.
- Re-enable PR verification, update the waypoint-selection test, fix Clippy failures and format the new code.
- Preserve upstream 0.28.1's chat translation safeguards: skip text already in the target language, ignore cosmetic provider rewrites, and reject an explicit non-Latin source when its script is absent.

Retains the PR author's facing-based tunnel/bridge direction, default-on tunnel floor, compact settings, paginated player/waypoint selection and bounded walking recovery.

**Not complete:** Dig/Bridge navigation plans are diagnostic only. Sequential world-action execution, live permission/tool/resource checks and server-confirmed replanning are still required. This is not a finished autonomous action navigator and has not been validated end-to-end in a live game.

Bundle ID remains `draimhorizon`; persisted data remains under `claim_borders`. No installation or publication is implied by this review build.
