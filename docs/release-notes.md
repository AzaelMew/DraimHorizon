**DraimHorizon 0.39.22 - cached auctions, seed prediction range and invisible entity visibility**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- **J → Settings → General → Show invisible entities** restores invisible player and mob models locally. Off by default, persisted, with English and Russian labels. Normal depth testing, range and frustum culling apply. Entity metadata, potion timers, server gameplay and other players' views do not change. Invisible armour stands and item frames are left alone.
- Experiments → Ores offers saved 1 chunk, 3x3 and 5x5 SMP seed prediction ranges. Existing preferences retain 1 chunk; 5x5 is labelled heavy. Generation remains cancellable on the background worker with shared terrain/decorating halos. Loaded-chunk highlighting and the existing 256-highlight / 128-block limits are unchanged.
- The custom auction catalogue restores bounded saved lots and item stacks for this server/account immediately and starts a fresh hidden walk on every opening. Search, sorting and paging remain usable; incoming lots do not reorder the list under the pointer.
- Saved lots show their age in English and Russian and always require purchase confirmation. Changed, expired or vanished lots are refused. Only a provably complete scan prunes unseen saved lots; interrupted, blank or partial pages preserve them. Switching to the server view stops the walk.

Combined-build checks must be rerun after integration. Live in-game acceptance is not verified.
