**DraimHorizon 0.39.2 - Farm: auto sell**

Target: official Horizon Nightly source `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`.

- New switch "Auto sell" on the Farm tab (off by default): once the harvest container is full, the farm puts each stack of harvest or hay bales in the hand and sends `/sell hand` until none is left.
- Seeds are never sold; selling stops with a message if the command does nothing.
- On the way to unload, wheat is crafted into hay bales if a crafting table is close by; otherwise it is put into the container as it is.
- Builds on 0.39.1.

Not verified in game. No installation or publication is implied.
