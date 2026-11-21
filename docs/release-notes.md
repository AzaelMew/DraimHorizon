# DraimHorizon 0.44.1

## Storage sorter

Automation gets a third mode, **Sorting** (also `/dsort`). It works like a copper golem for a whole storage room:

- Mark the storage zone with **Corner 1** / **Corner 2** (up to 128×128 blocks, 24 levels), aim at an input container and press **Input** (up to four), and optionally set an **Overflow** container.
- Each item goes to the container whose **item frame** shows it; otherwise to an unframed container already holding it, then to the overflow, and with **Empty ones** on to an empty unframed container. Anything else stays in the input.
- **Sort now** empties the inputs once, **Watch** checks them every 30 seconds, **Survey** opens containers not seen yet to learn what they hold.
- Frames and containers are indexed while you are near the storage and saved per world, so a large storage is not reopened every round. Only items taken from an input are carried; your own inventory stays put.

Copper chests now count as containers for the sorter, the farm and mining trips.

The Horizon package now targets the official Nightly `8f83e5c`; the Sunrise package targets Nightly `9425651`.
