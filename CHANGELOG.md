# История версий

## 0.29.4

- The Baritone action keys also work while the menu is open on a Baritone tab and no text field is focused (the game does not deliver keys to the mod while a screen is open). Start closes the menu and begins the task; errors are shown in the menu feedback.
- Baritone keys have defaults: Numpad 1 preview, 2 start/resume, 3 pause, 4 stop (changeable in Options > Mods), so a task can be started without first assigning keys. The idle status now says to press the Start key instead of "No destination".
- Mining can collect 13 more targets besides the ores: stone, cobblestone, dirt, sand, gravel, clay, wood, obsidian, glowstone, nether stone, sandstone, end stone, pumpkins and melons. A pickaxe, axe or shovel is accepted; tunnels and dig routes still require a pickaxe.
- Baritone mining messages ask for a suitable tool (pickaxe, axe or shovel) instead of only a pickaxe. Tunnels and dig routes still need a pickaxe.
- Drain fix: water deeper than 12 blocks reported hundreds of cells "without support" and did nothing. With sand or gravel the bot now drops blocks into the surface cell beside a solid block, so any depth fills from the bottom; other blocks keep the bottom-up mode and report that deep water needs sand or gravel. The task ends after several empty scans instead of one, because a sinking block briefly leaves its cell empty.
- New workflow `auto-version.yml`: a same-repository pull request that leaves the version unchanged gets its patch version raised automatically, with a stub CHANGELOG entry and release note from the title (`tools/auto_version.py`, tested in `tools/tests`). Forks are not touched. Commits pushed with the workflow token do not start other workflows; the merge to main runs the full verification.
- Not compiled or tested in game yet beyond the pure modules and CI.

## 0.29.3

- Baritone > Navigation: coordinates are three separate X / Y / Z fields (pasting `x y z` fills all three). The Preview / Go / Resume / Stop buttons are removed from every Baritone tab; use the four configurable keys (preview, start/resume, pause, stop) in Options > Mods. Entered settings now survive closing and reopening the menu.
- Navigation no longer gets stuck: when no walkable route exists or the character stalls, it plans again with permission to break blocks and place a block under a gap, executes the route step by step, re-plans after every route and stops with a visible reason after a bounded number of failures. Vertical pillaring is not supported.
- Mining: uses the same ore table as the Ores experiment. Overlay marks are tried first, then loaded chunks are scanned (radius 24); buried ore is reached by a dug route.
- Tunnel and Bridge spend any suitable building block the player carries (hotbar, off hand or inventory), not a fixed list. Stone-like blocks go first. Shulker boxes, chests and other containers, redstone parts, workstations, TNT, slabs, stairs, ores, storage blocks of metals and gems, and anything that is not a plain solid cube are never placed; the check also uses the game's own block data.
- New Baritone > Drain tab: fills a pond from the bottom up within a radius of 3–12 with blocks from the inventory, sand and gravel included, only on cells that rest on something solid. Walks to the next cell when it is out of reach, stops when no supported cell remains and reports how many were left.
- Tunnel: looks ahead and keeps walking instead of working one block at a time. The vanilla 5-tick destroy delay still bounds the rate per block, even with Efficiency 5 and Haste 2.
- Bridge: no sneaking while walking onto confirmed blocks; blocks from the off hand and the inventory (moved into the hotbar) are used.
- Farm: seeds are found in any inventory area, standing spots are ordered so the aim ray does not hit neighbouring crops, and the status line shows why nothing was sown. The root cause of the old behaviour was not reproduced in game.
- View-layer code was not compiled locally; only the pure modules were tested. Compile the real client with the native-client workflow.

## 0.29.2

- Fix the build: pin `rust-toolchain.toml` to Rust 1.99.1 instead of 1.83.0. The crates use `edition = "2024"` and `rust-version = "1.95"`, which Rust 1.83 cannot parse, so a local `cargo build` through rustup failed before compiling anything.
- Align `.github/workflows/horizon-compatibility.yml` with `release.yml` and `docs/development.md`: install and use Rust 1.99.1 rather than 1.83.0.
- Fix `python3 tools/stage.py` failing with `pinned source hash mismatch: crates/horizon-client/src/game.rs`. The pin already targeted official Nightly `fdc87ae`, but the recorded `game.rs` hash still belonged to the previous Nightly `2ebf00a`, whose `game.rs` upstream has since split into several modules. Refresh it with `tools/compatibility.py --update`; the other nine pinned hashes already matched.
- Checked against `fdc87ae`: all four injection targets that live in `game.rs` still exist, and `screen_over_playing`, used by two adapters, moved to `game/pointer_capture.rs` as `pub(super)`, so it stays reachable from the injected modules. Applying the mixins and compiling the real client are still to be confirmed by CI.
- No runtime behavior, settings or data formats change. Keep the official Nightly pin and the legacy settings namespace.

## 0.29.1 (PR #14 review, unreleased)

- Register configurable Baritone start/resume, pause and stop keys in Horizon's Options > Mods page. Bindings are unassigned by default and use Horizon's existing persistence; ignore releases, obstructed gameplay, modifiers and conflicting gameplay/native-mod bindings. Starting an active job does not restart it.
- Reject action-plan steps beside unknown terrain, including adjacent support and overhead cells. Dig/Bridge routes remain diagnostic only: no unverified world edits or action routes are fed to walking input.
- Clear stale coordinate fallback and show a localized notice when a selected navigation player or waypoint disappears. Adapt the waypoint regression to explicit dropdown selection.
- Restore PR #14 verification, fix the previously failing waypoint test and Clippy errors, and format the author's new planner/UI code.
- Preserve the author's facing-based construction, default floor, compact controls, target dropdowns and bounded recovery. Keep the official Nightly 2ebf00a pin and legacy settings namespace.
- Remaining PR requirements: sequential server-confirmed Dig/Bridge execution, full permissions/tool/resource safety integration and live in-game end-to-end validation. This review build is not a completed autonomous action navigator.

## 0.28.1

- Chat translation no longer touches messages that are already in the target language. Translating into English, lines such as `im at spawn` were sent to the provider and shown back rewritten (`I'm at spawn`). They are now skipped before sending when the local detector identifies the target language, or when plain-ASCII text cannot be identified and the target is English, as before language selection existed.
- A provider result is discarded when the provider reports the text was already in the target language, or when only case, spacing or punctuation changed.
- An explicit **Input** / **Chat from** language is trusted, except that a source equal to the target is never translated. With a non-Latin source such as Russian, text containing none of that script (for example `yea i know`) is left alone instead of being forced through the provider as Russian and rewritten.
- Outgoing messages use the same rule, so an unnecessary translation no longer ends with a "service unavailable" notice.

## 0.28.0

- Add Baritone > Farm: choose wheat, carrots, potatoes or beetroot, radius 3–12, harvest limit or continuous operation, and optional replanting. Harvest only mature crops, count server-confirmed breaks, and plant empty farmland with seeds from the hotbar.
- Add Baritone > Schematics: import Sponge/WorldEdit .schem v2/v3 and Litematica .litematic v4–v7, including signed and multiple regions. Select integer XYZ origin and 0/90/180/270 rotation, preview up to 256 nearby block outlines, copy the material list, and build from hotbar items.
- Decode bounded GZip/NBT imports on an idle worker. Reject path traversal, oversized files/regions, malformed palettes and versions newer than the client. Ignore entities and block-entity NBT.
- Reuse normal movement, attack and use input. Check each dry placement against expected states and confirm all affected cells after the server acknowledgement; pause on conflicts, unavailable material, manual input, refusals or timeouts. Do not demolish occupied schematic cells.
- Keep farm routes level to avoid automatic jump instructions. Preserve pending acknowledgements across pause/resume, restore the original hotbar selection, and discard jobs/import results on account/world changes.
- Keep the official Nightly 2ebf00a pin and existing settings/data namespace.

## 0.27.0

- Add Baritone Tunnel and Bridge modes with cardinal direction, length, preview, pause/resume and stop. Tunnels support widths 1–3, heights 2–3 and optional floor filling.
- Clear tunnel sections from top to bottom using ordinary digging and server acknowledgements. Stop at fluids, unloaded terrain, falling blocks, containers, unbreakable blocks or unsuitable tools.
- Build a one-block-wide floor from suitable full blocks in the hotbar. Approach the edge while sneaking, validate the clicked support face and a dry placement preview, then wait for the server before walking onto the new floor.
- Restore the original hotbar slot after automatic placement while respecting manual slot changes. Keep pending confirmations through pause/resume and cancel construction on world/account changes.
- Keep the official Nightly 2ebf00a pin, existing navigation/following/ore mining and saved user data.

## 0.26.0

- Target the latest official Horizon Nightly 2026-11-07 source 2ebf00a, with refreshed source hashes and native validation.
- Add Baritone > Mining: choose an ore and a 1–64 block limit, preview or mine reachable exposed ore from loaded chunks. Use ordinary attack processing and count only server-confirmed block changes.
- Mining checks the held pickaxe, remaining durability, a free inventory slot, actual reach and entity obstruction. Avoid support under the player, adjacent fluids and falling blocks above the ore; stop on server rejection or timeout.

- Add an experimental native walking navigator in the Baritone tab: enter XYZ or select a waypoint in the current dimension, preview a route, start, pause, resume and stop.
- Calculate bounded A* segments on a background worker using snapshots of loaded server chunks. Render the remaining route and a localized HUD status with distance.
- Walk and jump up one full block using normal movement input. Avoid unknown terrain, hazardous blocks, unsupported shapes and drops greater than one block.
- Follow a nearby loaded player by UUID, maintain a short distance and replan as the player moves. Stop moving immediately when the target disappears; stop following after a 10-second loss and require Resume.
- Replan automatically when terrain changes or movement stalls, with at most three recovery attempts near one location. Wait for landing before planning another segment.
- Pause on manual movement and open screens. Cancel pending work and clear the destination on disconnect, dimension/account/world changes or mod disable.
- This is an independent Rust navigator, not the Java Baritone library. Block placement, tunneling, farming, schematics and flight are not included.

## 0.25.1

- Split experimental settings into Ores and Baritone tabs in Russian and English, including compact layouts.
- Preserve ore settings and the existing consent gate. Switching tabs never enables highlighting.
- Baritone is an informational placeholder; movement automation is not integrated in this build.

## 0.25.0

- Presence uses one-line, direction-colored Lucide status marks before the name. Normal joins/leaves are green/red; friends and important players use cyan/lavender.
- Add a saved Icon and name / Text presence setting. Text mode restores localized joined/left and first-visit labels.
- Merge PR #11: launcher bundle ID is draimhorizon; old settings/data remain under claim_borders. This is a manual reinstall migration, not an in-place launcher update; custom bindings and activation state may reset.
- Retain the official Nightly policy and verified 6560641 source; do not adopt the main snapshot from the older PR.


## 0.24.1

- Target the official Horizon Nightly 2026-11-07 source commit 6560641, instead of Horizon main.
- Resolve and check the published Nightly tag in the compatibility tool and release CI. Keep the tested full commit SHA and source hashes reproducible.
- Document the mandatory official Nightly policy in AGENTS.md; add regression checks for annotated tags, missing tags and avoiding a main fallback.


## 0.24.0

- Show X/Y/Z in one styled minimap coordinate strip; allow editing all three coordinates in the X waypoint editor.
- Use compact + / minus presence events while retaining their original server text.
- Include ordinary messages from friends and important players in the Friends chat view, without moving messages out of other tabs.
- Enable auto reconnect on first use, with a saved toggle, Horizon's countdown, cancellation and bounded backoff. Do not reconnect after manual exit or permanent access/authentication refusals.
- Use top settings tabs, centered language controls and a centered Experiments confirmation with Lucide warning/check icons and soft action colors.
- Require official Lucide UI and documentation icons in AGENTS.md.
- Refresh Horizon compatibility to 6cc93d1.


- Chat translation button is now a compact 39 px direction tag (for example `EN>RU`; `?` means auto-detected input) with no `:...` suffix. The chat input box starts right after it. Left click toggles outgoing translation; right click opens a full-screen **Translation languages** screen.
- Four independent choices — **Input** (language you write in, or Auto detect), **Output** (language sent), **Chat into** (language incoming chat is translated into) and **Chat from** (language incoming chat is written in, or Auto detect) — each from 20 common languages: English, Russian, Finnish, Polish, German, French, Spanish, Italian, Portuguese, Ukrainian, Dutch, Swedish, Danish, Czech, Turkish, Arabic, Hindi, Japanese, Korean, Chinese. An explicit Input or Chat from is sent to Google (`sl`) and DeepL (`source_lang`). Choosing **Chat from** fixes short messages that auto-detection skips; it also bypasses the common-languages filter.
- First start after the update keeps the previous behaviour instead of Auto: RU interface gives `RU>EN`, EN interface gives `EN>RU`, and received chat is translated into the interface language. The result is saved once; existing choices are never overwritten.
- **Auto-translate: common languages only** (on by default) skips text whose detected language is outside those 20, without a network request. The filter reuses the detector that already labels each chat line with its language badge (script plus common-word vocabulary), so what is skipped matches what you see; text it cannot identify is translated.
- The common-languages filter skips a line only when the language detector positively identifies an uncommon language (for example Greek, Hebrew or Thai script). Tiny words, typos, slang and mixed lines (for example `claim команда`) are translated instead of silently dropped.
- Translation into English no longer requires Cyrillic source text; all targets share one eligibility rule.
- Settings are stored atomically in `translation_preferences`; an invalid file is never overwritten, the screen shows an error and keeps your selection. Esc cancels, Enter saves, and the unsent chat draft and caret are preserved.
- Relay 1.3.0 accepts the 20 DeepL targets and an optional validated source language; older clients keep working, but the shared DeepL fallback needs this relay for non-RU/EN targets.

## 0.23.1

- Target current Horizon main c78187d; prevent release with a stale source pin.
- Use official Lucide outlines, a single-line DraimHorizon title and an icon-only settings button.
- Compact waypoint rows and delete own Seed Map waypoints with a double left click.
- Ask once for explicit consent before opening Experiments; declining keeps features unchanged.
- Launcher limitation: GitHub native mod rows still display the stable bundle ID; Horizon Launcher needs display-name support.


## 0.23.0

- Gray original-language badges appear after chat and private messages in normal and compact layouts. Provider detection refines the local estimate, and unknown text uses ??.
- README now explains each feature visually with instructions and real chat examples.
- Expanded AGENTS.md and added a synchronized major/minor/patch version manager. Every tracked change requires a version increment.

## 0.22.7

### English
- Seed Map searches wide views progressively in cached 1024-block tiles instead of dropping whole structure sets or stopping at 512 candidates. The 2 km landscape view is fully covered; progress is shown while new tiles load. POI clustering is world-aligned and type-specific, so panning does not arbitrarily swap a village for a mineshaft icon.

### Русский
- Seed Map последовательно ищет структуры в кешируемых областях по 1024 блока вместо пропуска целых наборов или остановки на 512 кандидатах. Широкий вид с линейкой 2 км покрывается полностью; во время загрузки новых областей показан прогресс. Группировка POI привязана к миру и типу структуры, поэтому перемещение карты не подменяет деревню значком шахты.

## 0.22.6

### English
- Seed Map: click an individual POI to mark it found and dim its icon; click again to undo. Marks persist locally per server/world, account, seed and dimension. Dragging still pans; zoom into clusters before marking individual locations.
- Chat wraps at spaces instead of splitting words that fit on a full line. Explicit newlines are preserved; oversized words/URLs retain a character-wrapping fallback to avoid clipping.
- Localize live claim-list dimensions and observed-owner labels on the claims screen and map; waypoint dimension labels also follow the RU/EN switch. Owner and reporter names remain unchanged.
- Preserve leading colons in chat messages such as `:3` and `:`; repeat filtering no longer merges them with messages lacking those colons.
- Settings → Chat → Appearance: **Compact messages** reduces card padding and gaps without changing font size or multiline spacing. The setting persists; normal spacing remains the default.
- Fullscreen claim fills, outlines and the chunk grid are drawn independently of terrain. Borders retain screen-pixel thickness during fractional panning and in wide viewports.
- Terrain shading reads neighbouring region heights, removing artificial lines on continuous slopes at 256-block boundaries.
- A world-aligned, buffered terrain image is reused during small drags; unchanged results no longer trigger full texture uploads. The buffer preserves the previous visible texel density.
- Views exceeding the detailed-region cache use existing chunk overviews instead of repeatedly decoding and evicting the visible regions. Distant terrain is therefore coarser; saved exploration and its file format are unchanged.

### Русский
- Seed Map: щелчок по отдельной структуре отмечает её найденной и затемняет значок; повторный щелчок снимает отметку. Отметки сохраняются локально отдельно для сервера/мира, аккаунта, сида и измерения. Перетаскивание двигает карту; для отметки отдельной структуры в группе сначала приблизьте карту.
- Чат переносит текст по пробелам, не разрывая слова, помещающиеся в полную строку. Явные переводы строк сохраняются; слишком длинные слова и ссылки переносятся по символам, чтобы избежать обрезания.
- Названия измерений в списке приватов, подписи наблюдений в меню и на карте, а также измерения в метках учитывают переключатель RU/EN. Ники владельцев и наблюдателей не изменяются.
- Сохраняются начальные двоеточия в сообщениях чата, например `:3` и `:`; фильтр повторов больше не объединяет их с сообщениями без этих двоеточий.
- Настройки → Чат → Оформление: «Компактные сообщения» уменьшают вертикальные отступы карточек и промежутки между ними, не меняя размер шрифта и межстрочный интервал. Параметр сохраняется; прежний вид остаётся по умолчанию.
- Заливка приватов, контуры и сетка чанков полноэкранной карты рисуются отдельно от рельефа. Толщина контуров в экранных пикселях сохраняется при дробном смещении и в широких окнах.
- Затенение рельефа использует высоты соседних регионов, убирая искусственные линии на непрерывных склонах через каждые 256 блоков.
- При небольшом перетаскивании переиспользуется привязанный к миру снимок рельефа с запасом по краям; неизменённые результаты больше не загружают текстуру заново. Прежняя плотность видимых пикселей сохранена.
- Если видимая область превышает кеш подробных регионов, используются существующие обзоры чанков вместо постоянного чтения и вытеснения регионов. Дальний рельеф становится грубее; сохранённая карта и формат её файлов не изменены.


## 0.22.5

- Переключатель RU/EN теперь охватывает карту, Seed Map, экономику и торги, инструменты инвентаря, оформление, подсветку руд, окна телепортации и дополнительные подписи чата и relay.
- Описание мода и подписи клавиш в Options → Mods, а также предупреждения инвентаря и оформления обновляются при смене языка.

## 0.22.4

- Relay 1.2.0 поддерживает общий резерв DeepL. Оператор хранит API-ключ на сервере, а игроки используют токен приватного relay.
- Личный ключ DeepL можно сохранить и удалить в меню J → Настройки → Чат → DeepL.
- При сбое одного адреса Google мод использует доступный адрес. Подписи входов, выходов и первого визита переключаются между RU и EN локально.

## 0.22.3

- Меню J переключается между русским и английским. В английском режиме входящие переводятся на EN, исходящие на RU.
- DeepL используется как резерв для Google при наличии локального ключа.
- В «Друзья → Важные» можно сохранять и удалять правила по нику, включая офлайн-игроков.
- 3D-границы приватов выключены по умолчанию. Для включения используйте `/claimborders on` или настройки карты.

## 0.22.2

- Миникарта под крышей, в Незере и в режиме «Пещеры» обновляет срез по текущей высоте игрока с шагом в один блок. Рельеф берётся из загруженных чанков.

## 0.22.1

- `/pay` и другие команды с ником первым аргументом используют штатный выпадающий список Horizon: фильтрация по префиксу, Tab/Shift+Tab, стрелки, мышь и Escape. Имена берутся из текущего списка игроков; сумма перевода и текст сообщений не заменяются.
- Под миникартой дополнительно показана высота Y.
- Из скорборда, HUD, торговых карточек и настроек убраны отменённые дневные лимиты продаж. Старые нулевые/неизвестные лимиты больше не блокируют оценки и подтверждение продажи; проверки свежести цены, предмета, истории обратной продажи и явное подтверждение сохранены.
- Поиск и обычные кнопки сортировки инвентаря рисуются до переносимого предмета и его подсказки. Модальное редактирование сортировки остаётся поверх окна.
- Вместо нарисованных условных пиктограмм на карте и миникарте показываются настоящие головы: проекция частей baked-модели с текущей текстурой конкретного моба из атласа Horizon. Сохраняются форма головы, уши/морда и варианты текстур; ресурсы Minecraft в мод не включаются.

## 0.22.0

- Единые настройки открываются нижней шестерёнкой: основные, связь и друзья, карта/приваты, чат, торги, TAB и Scoreboard. Параметры TAB и Scoreboard отделены от торгов; прежняя вкладка «Опции» удалена.
- Баланс скорборда распознаёт сокращения k/M/B и обновляется в фоне без открытия Торгов. Остаток продаж запрашивается отдельно; при отсутствии серверных данных вместо «— / —» показывается пояснение. В SMP 0.33+ дневные лимиты продаж удалены; отсутствие ответа не считается нулевым или бесконечным лимитом.
- Друзья: заявки с галочкой и крестиком находятся в «С модом», без отдельной вкладки приглашений. Relay и Auto Trust перенесены в настройки, проценты HP убраны.
- Приваты: список известных действующих чанков, поиск по владельцу, фильтры своих/дружественных и чужих. Кеш чужих территорий остаётся последним известным снимком, а не гарантией текущего владения.
- Вкладка чата содержит перевод входящих и исходящих сообщений. Оформление, уведомления и антиспам перенесены в общие настройки; отдельной вкладки «Важные» нет. Эксперименты получили иконку.
- На карте и миникарте мобы показаны небольшими оригинальными иконками голов вместо квадратных точек. Фильтр типов на исследованной карте сохраняется между входами; категории и отдельные типы комбинируются.
- Метки: увеличенные карточки, цветная плитка иконки, отдельная строка координат и больше воздуха между действиями. Торги и серверные карточки стали компактнее.
- Seed Map: города Края без корабля и города с кораблём/элитрами выбираются независимо. Корабль определяется по реальным частям структуры генератора; прежний фильтр городов мигрирует на оба вида.
- Source pin Horizon остаётся f319701; после установки нужна пересборка нативных модов. Игровую проверку ресурсов, масштабов GUI и соответствия прогноза серверной генерации следует выполнить в клиенте.

## 0.21.1

- Исправлено переключение исследованной карты на Y-срез при изменении высоты и освещения. Поверхность теперь режим по умолчанию; старый «Авто» мигрирует в поверхность.
- Пещеры включаются вручную; сканирование и история используют одну каноническую высоту внутри 16-блочной полосы, без пересчёта при каждом изменении Y на блок.
- Поверхность другого измерения больше не подменяется случайным сохранённым пещерным слоем. Подсказка кнопки рельефа показывает текущий режим; пещерный режим подсвечивается.
- Сохранённая история не удаляется; отсутствующая поверхность заполняется при повторном посещении территории.

## 0.21.0

- Быстрые действия в списке друзей: выбрать игрока и нажать «Запрос ТП» или «Обмен» для `/tpa <ник>` и `/trade <ник>`. В компактном интерфейсе кнопка телепорта называется «ТП»; подсказки показывают команды.
- Доступность определяется присутствием на игровом сервере независимо от relay; перед отправкой повторно проверяются UUID и ник. Офлайн-игрокам и себе запросы не отправляются.
- После отправки меню закрывается; сервер управляет принятием запросов, телепортом и обменом. Действия дружбы и авто trust сохранены.

## 0.20.0

- Важные игроки видны во вкладке друзей с цветной звездой и подписью. Это не выдаёт статус взаимной дружбы и доступ к телеметрии.
- Локальная история современного чата: до 500 сообщений отдельно для каждого сервера и профиля, включая ЛС, исходный текст и готовый перевод. Архивные сообщения не создают непрочитанное, уведомления и повторный перевод.
- J → Эксперименты: разрешение подсветки руд, выбор типов и источника. По умолчанию запрещена; O переключает разрешённую подсветку, после входа в мир она всегда выключена.
- Режим загруженных чанков показывает данные клиента в ближайших 3×3 чанках; добытые блоки исчезают. Экспериментальный прогноз сида SMP рассчитывается в фоне для текущего чанка и рисуется разорванными рамками. До 256 ближайших меток в радиусе 128 блоков.
- Прогноз не учитывает добычу, серверные datapack, старую генерацию и изменения мира; точность совпадения руд с сервером требует игровой проверки. Переводчик не изменён.

## 0.19.0

- M открывает исследованную карту; полноэкранный интерфейс с компактными инструментами, вкладками измерений и плавным перемещением/масштабированием готового изображения.
- Цвета верхних текстур блоков текущего ресурс-пака, биомная окраска и усиленное затенение рельефа. Палитра готовится в фоне; сбор цветов ограничен 16 чанками за проход.
- Формат истории v2 сохраняет цвета поверхности; регионы v1 читаются и обновляются без потери исследования при повторном посещении.
- Seed Map показывает одно выбранное измерение. Вместо списка — поиск, категории и плитки фильтров с иконками предметов. Метки используют те же иконки, плотные группы имеют счётчик.
- Полный каталог 34 структур vanilla, включая все варианты, шахты, ископаемые Незера и руины троп. Поиск использует штатные правила размещения, частоты и исключения. Старые фильтры мигрируют.

## 0.18.0

- M открывает Seed Map с тремя независимыми панелями измерений; J — меню настроек. Прежняя стандартная пара мигрирует один раз; прочие пользовательские переназначения сохранены.
- Сид SMP -3292718948513116954, расчёт биомов Y64 и проверенных структур штатным генератором vanilla Java 26.3 в отменяемом фоновом потоке.
- Сохраняемые фильтры биомов, отдельных типов структур и Claims; координаты и подсказки при наведении.
- Приваты, игроки, метки и смерть сохраняются; исследованная карта доступна отдельной кнопкой.
- Независимый зум/перемещение панелей, ограниченный поиск при далёком масштабе, ошибки ресурсов без блокировки интерфейса.

## 0.17.0

- Карта показывает последнюю смерть по данным Horizon. Кнопка «Смерть» открывает её измерение и центрирует карту.
- Современный чат получил вкладки «ЛС», «Пати» и «Торги» со счётчиками непрочитанного. ЛС выделяются и переводятся, `/r` и `/reply` отвечают последнему отправителю. Tab подставляет онлайн-ник в `/tp`, `/msg` и `/trade`.
- Избранное аукциона сохраняет пороги за штуку для каждого сервера и подсвечивает подходящие лоты на открытой странице.
- Зум камеры на Z с колесом, плавным переходом и снижением чувствительности мыши.

## 0.16.0

- F8 и кнопка стандартного вида сохраняют выбор для всех страниц и следующих запусков; аукцион и Market настраиваются независимо. F8 работает и для возврата из стандартного вида.
- Серверные подтверждения выставления и продажи лотов выделяются цветом и закрепляются в блоке важных сообщений; обычный поток чата не вытесняет недавние уведомления.
- Совместимость обновлена до Horizon Nightly 2026-11-05 · `f319701` (native hooks API 409).

## 0.15.2

- Обновлена точная совместимость с Horizon Nightly 2026-11-04 · `67dad0d` вместо `f42ef42`; пересчитаны контрольные суммы исходников.
- horizon-mixins закреплён на `e8cd0aa`, как в лаунчере этого nightly; недоступная прежняя ревизия `10fdf8b` больше не нужна.
- Обновлена проверка настоящего клиента в CI и инструкция по пересборке.

## 0.14.0

- Единые полупрозрачные панели со скруглениями, мягкими тенями и спокойной палитрой для всех экранов мода.
- Контурные иконки навигации, действий, HUD и сортировки без дополнительных игровых ресурсов.
- Плавное появление окон/карточек и подсветка наведения/выбора; интерактивные области остаются на месте.
- Общие настройки оформления на Ctrl+, и через шестерёнку: фон, акцент, эффекты, тени, контраст; предпросмотр, отмена и сохранение.
- Более понятные заголовки и пояснения; локализованные названия обычных предметов в торговле и поиск по кириллице.
- Новые проверки геометрии, анимаций и настроек; прежние данные и pins Horizon/relay совместимы.

## 0.13.0

- Открытие опорного хранилища сквозь рамки, светящиеся рамки и таблички; Shift сохраняет обычное действие, отдельные переключатели.
- Сортировка основных 27 слотов инвентаря и обычных хранилищ, включая шалкеры, воронки, раздатчики, выбрасыватели и грузовые слоты животных. Хотбар, экипировка и служебные слоты исключены.
- Вертикальные кнопки S/F/R/T у обеих областей: сортировка, дополнение стаков, пополнение имеющихся видов, перенос.
- Порядок по ID, имени и количеству; строки/столбцы; разовый режим через Shift/Ctrl/Alt + СКМ.
- Редактор Ctrl+O/E: исключения слотов, видимость, включение, смещения, шаблоны, сброс и отмена.
- Защита виртуальных серверных окон и торговых меню; проверка каждой перестановки штатным движком меню перед отправкой обычных кликов.
- Сохраняется точная совместимость с Horizon f42ef42 и relay 1.1.0.

## 0.12.0

- Совместимость с Horizon f42ef42, новые контрольные суммы точного исходного commit.
- Зафиксировано исправление horizon-mixins из PR #25 для встроенных атрибутов Rust в новом клиенте.
- Изолированный remote workflow: тесты, clippy, стадирование, регрессии инструмента, настоящая компиляция клиента и валидатор пакета.
- Совместимые прежние настройки, ID мода и relay 1.1.0.

## 0.11.0

- TPS в sidebar и Tab: сглаженная оценка по серверным пакетам времени, состояния ожидания/паузы, переключатели отображения.

- Исправлены реальные ответы `/worth` с запятой, дробными ценами, односторонней торговлей и неторгуемыми товарами; автоматические ответы скрываются до штатного и кастомного чата.
- Исправлен голод опроса большого списка товаров; опрос работает только при открытом торговом UI, неподдерживаемые товары исключаются до ручной перепроверки.
- Торги на B: компактное окно, без sidebar, сортировка AH, полноценные условные расчёты за весь лот и по остаткам лимитов. Маленькие подсказки вне инвентаря вместо полосы на весь экран.
- Все налоги удалены из модели, настроек и интерфейса; старые значения больше не восстанавливаются.
- Новый прозрачный Tab с тремя колонками и читаемым пингом; новый sidebar с финансовыми блоками и понятным остатком дневного лимита, без дублирующего HUD.
- Наблюдения чужих приватов через `/claim info`: сохранение, общий обмен, снятие устаревшей отметки при подтверждённом свободном чанке, красные границы и клетки на картах, без требования мода/дружбы у владельца.

## 0.10.0

- DraimHorizons: новое публичное название и имена релизных архивов, совместимый прежний ID и конфигурация.
- Экономика: наблюдаемые цены, история, Market/AH, ручной безопасный обход страниц, целая стоимость лота, лимиты, cooldown покупок, условные оценки прибыли и подтверждаемая продажа руки.
- TAXES отключены: нулевые Market/VAT по умолчанию, старые счета игнорируются, отдельный учёт неизвестной комиссии объявления.
- Расширенные настройки экономики, HUD и серверных подсказок; свои sidebar/Tab с переключателями, друзьями, пингом и страницами.
- Автоматический trust взаимным друзьям с подтверждением сервера и уважением ручного untrust.
- Метки двойным ПКМ на карте и клавишей X; расстояние при наведении на голову друга.
- Карточки trade accept/deny; настраиваемые важные игроки в чате и закреплённые сообщения.

## 0.9.0

- Асинхронный перевод исходящего русского чата на английский: отдельная настройка, сохранение порядка/оригинала, обычная подписанная отправка, исключение команд и возврат к оригиналу при ошибке.
- Постоянный кеш своих и чужих приватных территорий; начальный `/claims` только без сохранённых данных, обновление по успешным claim/unclaim.
- Независимый от пати E2E-обмен списками территорий между всеми участниками мода одного сервера/relay, атомарная сборка страниц, закрепление ключа и сохранение офлайн.
- Карточки TPA с точными серверными командами, кнопками, оставшимся временем и полоской на 60 секунд; защита от повторного/просроченного клика.
- Мобы на миникарте и большой карте: категории, высота и режимы радара.

## 0.8.0

- Постоянное хранилище исследованного рельефа по серверам/аккаунтам, мирам, измерениям и пещерным слоям; восемь переключаемых атласов.
- Фоновая атомарная запись регионов, ограниченный кеш и очередь, обзор для далёкого масштаба, сохранение при выходе и изоляция повреждённых файлов.
- Большая карта с текстурой 512×512, широким видом и нижней панелью управления; тени высот/воды без прежней шахматной яркости.
- Компас, направление игрока и масштабная линейка на обеих картах; G переключает сетку.
- Страница «Память» в Клеймах, настройки теней и записи; старые карты не стираются при переключении.

## 0.7.0

- Автоподключение сохранённого relay и настройка его включения.
- Списки пользователей мода, друзей и приглашений вместо ручного ввода кодов; принятие и отклонение запросов, закрепление ключа выбранного контакта.
- Головы вместо текстовых мировых меток игроков, нативная интерполяция близких сущностей и буфер снимков для дальних.
- Головы игроков и друзей с рамками на картах; цветные ромбы waypoints.
- Распознавание системных событий чата по ключам компонентов независимо от локализации и поддержка русских текстовых объявлений.
- Сортировка верхних слотов сундука на СКМ обычными кликами с проверкой плана и остановкой при изменении содержимого.
- Удалён старый Party / HP HUD. Relay 1.1.0 остаётся совместимым.

## 0.6.0

- Большая карта J и миникарта на штатном движке Horizon: рельеф загруженных чанков, слои приватных участков, игрока, друзей и waypoints.
- Зум к курсору, перетаскивание, выбор измерения, переход к игроку/своим участкам и информация о чанке под указателем.
- Клеймы разделены на карту, 3D-границы, HEX-цвета и сортируемый список; шаг сетки 3D и толщина линий доступны в меню.

- Сохраняемые настройки ширины, высоты, масштаба, угла экрана и сдвигов чата.
- Четыре режима уведомлений, отдельные переключатели входов/выходов и смертей, длительность и прозрачность фона.
- Объединение повторов со счётчиком ×N, режимы подряд/за интервал и игнорирование временного префикса.
- Клик по карточке в открытом чате переключает оригинал и перевод; учитываются масштаб, положение и границы карточки.
- Relay 1.1.0 остаётся совместимым.

## 0.5.0

- Relay 1.1.0: без квот подключений/IP/realm и частоты по умолчанию; начальный каталог не забивает очередь.
- Передача координат до 20 Гц, объединение старых снимков в обе стороны, heartbeat и восстановление после обрыва, интерполяция.
- Список до 64 друзей, компактное меню с боковой навигацией в стиле социальных панелей.
- Карточки чата с головами, отдельные уведомления входов/выходов и смертей, устранение дублирующих объявлений первого входа.
- Фоновый автоперевод на русский через Google с переключателем и сохранением оригинала.
- Полноценный курсор и выделение в полях; исправлена лишняя V при вставке.

## 0.4.0

- Отдельный WebSocket relay: E2E-шифрование, закрепление ключей друзей, взаимные приглашения по коду.
- Настройка URL/токена и управление подключением в меню Party; обмен через `/msg` удалён.
- Готовый сервис с Docker Compose/Caddy и отдельным архивом для своего хоста.
- Меньшие метки фиксированного экранного размера без расширения вдали.
- Интеграционные проверки двух Rust-клиентов, Python relay и запуска контейнера.

## 0.3.0

- Игровое меню M с вкладками Waypoints, Party и Claim Borders; клавиша переназначается.
- Списки, страницы, поля ввода и кнопки заменяют необходимость управления только командами.
- Визуальные настройки границ сохраняются: свои/дружеские территории, цвета, прозрачность, дальность, вертикальный охват.
- Режим сквозь блоки и сетка вокруг игрока для ориентации в пещерах.
- Отдельное подтверждение подписанного обмена в меню; общий переключатель не меняет отдельные флаги видимости.
- Тег и GitHub Release v0.3.0 для обновления через GitHub-источник Horizon Launcher.

## 0.2.0

- Взаимные приглашения в друзья, сохранение по серверу и аккаунту, удаление.
- Клиентский обмен через `/msg` с явным выбором signed/unsigned на каждом подключении.
- Проверка UUID/онлайн-профиля, сессий, порядка кадров и полноты страниц приватов.
- HUD онлайн-друзей с процентом HP; метки и голубые границы друзей.
- Waypoints: создание, название, цвет, удаление, сохранение, расстояние и затухание.
- Исправлены встречные приглашения и транзакционное сохранение дружбы.
- Настройки Codex GPT-6.1 Sol / Medium; Token Optimizer 5.13.63, balanced.

## 0.1.0

- Зелёные границы собственных приватов из ответа Horizon SMP `/claims`.
- Координаты сервера трактуются как начала чанков в блоках: область 16×16.
- Внешний контур смежных чанков без общей внутренней стенки.
- Переключение видимости и ручное обновление через `/claimborders`.
- Изоляция измерений, сброс при переподключении и командах изменения приватов.
- Проверка полноты ответа, координат, размера и срока ожидания.
- Целевая ревизия Horizon: `c9d474d`.
