**DraimHorizon 0.39.22 - cached auction browsing and selectable seed prediction range**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- Experiments → Ores offers saved 1 chunk, 3x3 and 5x5 SMP seed prediction ranges. Existing settings default to 1 chunk; 5x5 is labelled heavy. Generation stays cancellable on a background worker, sharing terrain and decorating halos across the selected square.
- English and Russian range controls and result status are updated. Loaded-chunk highlighting and the existing 256-highlight / 128-block limits are unchanged.
- The custom auction catalogue shows the lots saved for this server and account immediately and walks the auction again on every open; search, sort and paging work while it loads and do not stop the walk.
- Saved lots (with item stacks for names, enchantments and icons) are an unverified snapshot: dimmed, with the data's age shown in English and Russian; expired lots are dropped; a click never buys a saved lot without the confirmation.
- Every new opening of the catalogue starts a fresh walk (no 15-second suppression); one opening starts only one.
- Only a walk that proves it reached the end (the server's lot count matches, the last page is a settled listing page) removes lots it did not see; blank or half-filled pages, limits, closing, disconnects and purchases keep the saved lots.
- Lots that arrive during the walk join at the end instead of moving the ones being read, and a purchase is refused if the lot disappeared, expired or changed price or seller since it was drawn.
- The server's own pages are no longer drawn or clickable under the catalogue between page changes.

The combined source requires the pinned Nightly. Live in-game appearance, auction interaction and server ore accuracy have not been verified for the combined build.
