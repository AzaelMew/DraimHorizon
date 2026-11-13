**DraimHorizon 0.40.1 - lighter custom chat and sidebar**

Target: official Horizon Nightly source `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0` (unchanged). Performance maintenance; appearance and behavior are intended to be identical.

- Chat reuses wrapped rows until the text, translation, width, language, presentation flags or font change (compared structurally, bounded). Fading, entrance animation and scrolling still run every frame. Layout is skipped when no card fits, and drawing stops at the first faded card.
- Chat shares one history snapshot that is rebuilt only when the feed or translation target changes, instead of copying up to 500 entries each frame.
- Chat heads and their per-frame roster refresh are unchanged (optimization deferred).
- Translation scheduling is unchanged in behavior (verified frame for frame against the old loop): settled lines are not re-sent, a full queue is retried on the next render, failures keep their backoff.
- The sidebar no longer holds the economy lock while drawing and reuses clipped labels and static decoration geometry; live values are drawn every frame.

Verification: 550 tests passed (one ignored), Clippy, formatting, the pinned native client compile-check and bundle validation passed. Isolated live before/after runs used the same Nightly, local world, camera and 500-message fixture. With both custom widgets enabled, CPU preparation fell from 1.424 to 1.031 ms/frame and total CPU time from 1.824 to 1.434 ms/frame. Average FPS stayed effectively unchanged (183.66 versus 184.07); these are CPU-overhead improvements, not a demonstrated FPS increase. Sidebar-only timings were mixed and do not establish a live speedup.

Inspected live chat crops matched pixel-for-pixel for stable focused, scrolled, GUI-scale-3, Russian-label and original-toggle cases. Tab filtering, expiry and reopening retained history also worked. This covers the tested local cases, not every server, skin or translation-provider scenario; live provider requests were not exercised.
