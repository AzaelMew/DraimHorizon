**DraimHorizon 0.39.12 - Patting, ore picker and seed map labels**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- New **Pat** key (**K**, rebindable): aim at a player within 3 blocks for two gentle pats. Purely cosmetic: no attack, item use or chat.
- Players running this version (only friends, by default) see your arm move and an optional "*name* patted you" line. The pat travels only over the encrypted relay; players without the mod see nothing.
- **J → Settings → General**: who can pat you (friends only by default, everyone or nobody) and the pat notification.
- Older DraimHorizon versions ignore pats; friendship and sharing keep working with them.
- Pat input no longer waits for a release event that chat or a focus change can consume; the next real press works normally, and the client's repeat filtering still prevents held-key repetition.
- Local and remote third-person arm poses stay independent even when a remote player's server entity id is 0.
- Experiments → Ores: one icon tile per ore (redstone dust, iron ingot, ancient debris and so on), toggled directly like the seed map filters.
- Seed map: filter labels are centred and wrapped at spaces instead of cut off; the bottom hint wraps in narrow windows. Label regression tests cover English and Russian at 1080p and 1440p sizes.
- Builds on 0.39.11, preserving farm plots and schematic preview persistence.

Before combining these changes, patting was verified on two clients on a private lab server with a local relay: first- and third-person animation, the remote arm, RU and EN notifications and settings, consent policies, range, walls, cooldown, a stopped relay, a 0.39.11 peer, a dimension change and the mod switched off. The combined build has not been verified in game.
