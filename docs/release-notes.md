**DraimHorizon 0.41.7 — Sunrise: mouse-bound keys, nickname completion and `/r`**

- Key bindings set to a mouse button (for example `key.mouse.4` for pat) now work on Sunrise. The two
  Sunrise-only hooks that bridge mouse buttons were dropped by the generator; they are part of the
  `.smod` now, forward buttons 1-8 as `key.mouse.*`, and act only while playing.
- Nickname completion for `/tp`, `/msg`, `/tpa`, `/pay` and the other name commands no longer
  disappears on every second letter: the list is built from the text in the box at each keystroke.
- `/r hello` and `/reply hello` are no longer shown red: client commands now declare their arguments.

The generator fails with a clear message if a Sunrise-only hook rule has no target or helper.
Not verified: an in-game test of these three fixes.
