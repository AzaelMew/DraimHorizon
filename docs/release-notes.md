- **Whisper translation.** With outgoing translation on, `/msg`, `/tell`, `/w`, `/whisper` (with a player name) and `/r`, `/reply` are translated too. Only the message text is translated; the command and recipient are sent as typed. A translated `/r` goes to whoever whispered last when you pressed Enter, even if someone else whispers while it is being translated. A failed translation sends the original whisper, and other commands are never touched.
- **Polish is detected in automatic mode.** Polish typed without diacritics used to be mistaken for English and skipped, so automatic translation never translated it. It is now recognised by common words and Polish-only letters. Finnish, Dutch, Swedish, Danish, Czech and Turkish are detected the same way.

Also in this release (unpublished 0.45.3):

- Updated the Sunrise package to DEV `ba1f24e` (`dev-2026-11-26-ba1f24e`, Java 26.3). The pinned source hash of `crates/sunrise-client/src/net.rs` did not change, but two APIs the mod uses did, so the adapters were updated.
- Dynamic textures are standalone GPU textures in this build: `HudAtlas` no longer keeps their table or `dynamic_names()`. The world map, Seed Map and minimap are created and updated through `Game.dynamic_textures` and drawn with `HudBatch::dynamic_texture`; the Seed Map's composed pixels now go straight into its texture instead of an atlas patch. Nothing in the map, Seed Map or minimap was redrawn through the atlas, which previously left the minimap on a missing texture and the two maps showing only markers.
- `SystemToast::update` takes a leading `ToastClock`. The shared Baritone toast keeps its Horizon call, and a platform rewrite passes `ToastClock::Real`, so the countdown still follows real time.
- Install the new `.smod` and rebuild native mods after moving the client to that Dev build. A native mod works on exactly one game build.
- Horizon checks and `.hmod` releases remain paused while the horizon-rust repositories are unavailable.
