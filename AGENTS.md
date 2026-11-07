# DraimHorizons agent instructions

Reply to the owner in Russian. Keep the public README and artwork in English.
Finish authorized tasks through verified artifacts and preserve unrelated work.
Use skills and subagents only when explicitly requested by the user.

## Start from current sources

- Check Git status, remote main and release tags. Preserve local changes before updating.
- Read relevant symbols and bounded ranges with rg. Source and compatibility.json take precedence over old prose.
- This is a native Rust mod for Horizon. Bundle ID is claim_borders. Distribution name is DraimHorizons. Artifacts use .hnmod.
- Exact Horizon and horizon-mixins pins live in compatibility.json. Check their actual APIs. Before every task run tools/compatibility.py; if behind main, refresh with --update --horizon PATH, adapt the hooks and compile the real client. Never release with a stale pin. Exact tested SHAs remain reproducible.
- Use the existing toolchain. Keep Windows file operations within checked paths, use -LiteralPath and UTF-8, and preserve system configuration.

## Navigation

| Path | Responsibility |
| --- | --- |
| Cargo.toml / Cargo.lock | Runtime crate and pinned dependencies |
| src/lib.rs | Pure modules shared by tests and native injection |
| mixins/claim_borders/horizon-mixin.toml | Module wiring and hook definitions |
| mixins/claim_borders/runtime.rs | Mod identity, version and verified claim state |
| client.rs / menu.rs / menu_view.rs | Client lifecycle, menu actions and rendering |
| chat_feed.rs / chat_view.rs / chat_archive.rs | Classification, layout and local history |
| relay_client/src/translation.rs / outgoing_view.rs | Background providers and signed outgoing chat |
| claim_map.rs / map_backend.rs / map_history.rs / map_view.rs | Minimap, explored map and persistence |
| seed_map.rs / seed_view.rs | Seed prediction, POIs and found markers |
| economy*.rs / inventory*.rs / waypoint*.rs | Trading, item tools and waypoints |
| relay/ | Independently deployed Python WebSocket service |
| tools/version.py | Synchronized mod version manager |
| tools/stage.py / tools/package.py | Pinned mixin application and bundle packaging |
| tools/package_relay.py | Portable relay archive without secrets |
| .github/workflows/ | Runtime, native-client and publication checks |

Unprefixed Rust paths in the table are relative to mixins/claim_borders/.

## Version every task

Every completed task that changes tracked content must increment the mod version,
including small fixes, README edits, AGENTS edits and build/configuration changes.
Increment once per task and keep that version while fixing its checks.
Combine changes under the highest applicable category.

The format is major.minor.patch.

| Change | Command | Example |
| --- | --- | --- |
| Fix, tuning, documentation or maintenance | python tools/version.py bump patch | 0.23.0 → 0.23.1 |
| New user-facing capability or substantial feature update | python tools/version.py bump minor | 0.23.1 → 0.24.0 |
| Major product release explicitly requested by the owner | python tools/version.py bump major | 0.24.0 → 1.0.0 |

- Start from the current remote release. Never reuse or move a published tag.
- The manager updates Cargo.toml, the root package in Cargo.lock, horizon-mixin.toml and runtime.rs together. Run python tools/version.py check before packaging.
- Add a concrete CHANGELOG.md entry and replace docs/release-notes.md with notes for this task. Preserve historical entries.
- Manifest versions, artifact names, release titles and the v<version> tag must agree.
- Relay service version is separate, in relay/dhrelay/__init__.py. Bump it when deployed relay behavior changes and update its healthcheck tests.
- Bumping a version does not authorize commit, push, release or deployment. Perform those when requested. A release task includes monitoring CI and checking published assets.

## Icons

- All project interface and documentation pictograms must use official Lucide icons: https://lucide.dev/icons/.
- Reuse ui_style::Icon and tools/update_lucide.py; do not draw substitutes or use Unicode icon glyphs. Keep Lucide attribution with the bundle.
- Minecraft item textures, player heads and user images are content, not interface pictograms.

## Implementation contracts

- Reuse existing types, entry points and patterns. Keep changes focused and readable. Add dependencies only for a concrete need.
- Keep network requests, disk scans and expensive generation off the render/game thread. Bound queues, timeouts, caches and geometry.
- Preserve old preferences, chat archives, waypoints and explored terrain. Recheck account/server/dimension identity before applying async results.
- Preserve signed chat and command behavior. Translation changes message text and must never execute a translated message as a command.
- Keep original text, detected source language and translation target separate. A language switch must not show a cached result for the wrong target.
- Cover RU/EN labels, settings, errors and tooltips. Preserve player names, commands, coordinates and user-authored content.
- Reserve badge/control space before wrapping or truncating. Check compact padding, multiline text, minimum GUI size, clipping, fading and click targets.
- Live minimap slices follow exact player height. Saved explored-map bands and Seed Map use their own cache policies.
- Keep 3D claim borders opt-in. Gameplay authority stays on the server.
- Treat sender names and UUID metadata as untrusted. Friendship and telemetry consent stay bound to pinned peer identities.
- Personal DeepL keys stay local. Shared keys stay on the relay. Keep credentials out of source, artifacts, logs, screenshots and diagnostics.
- Shared translation requires relay-token authentication and bounded requests. Translation text reaches the relay and provider. Party message routing remains encrypted.
- Never commit game assets, extracted upstream code, saves, build output, credentials or signing keys.

## Verification and delivery

- Rust changes require python tools/version.py check, cargo fmt --all --check, rustfmt --edition 2024 --check mixins/claim_borders/*.rs, cargo clippy --workspace --all-targets --locked -- -D warnings and cargo test --workspace --locked.
- Add regression tests for meaningful behavior, not implementation duplication.
- Native adapters require applying official pinned mixins and checking the real Horizon client. Validate the final bundle with cargo horizon-mod check <artifact> --json.
- Relay changes require its Python tests and relevant integration checks. Exclude real .env files from deployment archives and use portable archive paths.
- README describes implemented behavior. Inspect rendered artwork for readable text and clipping. Identify diagrams and screenshots accurately.
- Check original command exit codes. Fix failures before publication. Repeat a passed check only after new changes or unresolved evidence.
- Distinguish unit tests, native compilation, provider smoke tests and live SMP checks. Report unavailable or unsuccessful checks.
- During authorized deployments, preserve .env, local Compose ports, TLS and client identities. Verify health and version afterwards.
- Finish with concise results, version, usable artifact/release link, checks and material limitations.
