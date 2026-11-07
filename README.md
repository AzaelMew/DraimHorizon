# DraimHorizons

![DraimHorizons — native tools for Horizon SMP](docs/features.svg)

[Download the latest release](https://github.com/DraimCiDo/DraimHorizon/releases/latest) · [Changelog](CHANGELOG.md) · [Relay setup](relay/README.md)

**Install** — In Horizon Launcher, choose **Install from GitHub**, enter `https://github.com/DraimCiDo/DraimHorizon`, then rebuild native mods. Target client is Horizon Nightly `f319701` for Java 26.3.

![Explored map and live minimap instructions](docs/showcase/maps.svg)
![Seed Map instructions](docs/showcase/seed-map.svg)
![Chat features and original-language badges](docs/showcase/chat.svg)

Real in-game chat examples, with normal and compact spacing. Sample player text keeps its original language.

![Normal chat in game](docs/showcase/chat-normal.png)
![Compact chat in game](docs/showcase/chat-compact.png)

![Translation directions and DeepL setup](docs/showcase/translation.svg)
![Friends, Important players and waypoints](docs/showcase/friends.svg)
![Trading and economy tools](docs/showcase/trading.svg)
![Inventory search, sorting and click-through](docs/showcase/inventory.svg)
![Zoom, interface customization and opt-in features](docs/showcase/customize.svg)

**Shared DeepL** — Add `RELAY_DEEPL_API_KEY` to the private relay's server environment. Friends configure its address and access token under **J → Settings → Connections and friends**. [Operator instructions](relay/README.md#общий-резервный-перевод-deepl)

**Compatibility** — Friends' telemetry requires the relay and both clients to use the mod. Seed Map predicts generation and can differ from modified or previously generated terrain. Player-supplied text remains unchanged by interface localization.
