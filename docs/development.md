# Разработка DraimHorizons

## Устройство

- `mixins/draimhorizon/runtime.rs` — парсер ответа SMP `/claims`, кеш сеанса и построение внешнего контура. Один исходник используется standalone Cargo-библиотекой и внедряемым модулем.
- Остальные `.rs` в bundle — тонкие адаптеры чата, команд, отрисовки и переподключения.
- `horizon-mixin.toml` — native hooks и точечные HEAD-инъекции; точная ревизия Horizon закреплена в `compatibility.json`.
- Для рисования используются штатные `Gizmos` (depth-tested или `on_top`); новый GPU-пайплайн не добавляется.
- `/claims` содержит блоковые начала чанков, не номера чанков. Формат проверен по `horizon-rust/horizon-mods/native/smp/mixins/smp/smp_claims.rs`.

## Проверки

Нужны Git, Python 3.12+, Rust 1.99.1 (rustfmt, clippy). Стадирование использует Linux file locks. `tools/stage.py` скачивает закреплённые исходники в gitignored `.local/`, экспортирует их без изменения исходного checkout и вызывает официальный horizon-mixins:

```bash
cargo fmt --all --check
rustfmt --edition 2024 --check mixins/draimhorizon/*.rs
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 tools/stage.py
```

Можно использовать существующие checkouts:

```bash
python3 tools/stage.py --horizon /path/to/horizon --mixins /path/to/horizon-mixins
```

Horizon checkout должен содержать точный commit; текущая ветка не меняется. Mixins checkout должен быть чистым и находиться на точном закреплённом SHA.

Проверка компиляции настоящего клиента без ресурсов Minecraft:

```bash
export RUSTUP_TOOLCHAIN=1.99.1
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_INCREMENTAL=0
cargo check --manifest-path target/mixins/modded/Cargo.toml --target-dir target/horizon-check \
  --locked -p horizon-client --bin horizon-client --no-default-features
python3 tools/package.py
cargo run --manifest-path target/mixins/modded/Cargo.toml --target-dir target/horizon-check \
  --locked -p cargo-horizon-mod -- check target/draimhorizons-0.16.0.hnmod --json
```

Последняя команда принимает путь относительно текущей рабочей директории. Она выполняет те же проверки архива, что реестр Horizon. Полная игровая сборка и интерактивный тест требуют дополнительных зависимостей, GPU и собственных подготовленных ресурсов Minecraft. Отсутствие игровых ресурсов не блокирует `cargo check` и упаковку исходного `.hnmod`.

Тесты охватывают ответ со скриншота, отрицательные координаты, Unicode-пробелы и цветовые коды, разные измерения, неполные/дублированные/слишком большие ответы, ограниченное ожидание, понижение серверной квоты, размер чанка 16×16, объединённые границы, дальность и сброс состояния.

## Релиз

Workflow `.github/workflows/release.yml` при изменении исходников в `main` запускает проверки, официальное стадирование и проверку пакета; полная компиляция клиента проверяется отдельным обязательным CI workflow; затем публикует GitHub Release для версии из Cargo.toml. Повторный запуск не заменяет существующий релиз. Перед изменениями увеличивайте версию одновременно в Cargo.toml, bundle manifest и runtime VERSION.

Артефакты: `draimhorizons-<version>.hnmod` и его `.sha256`. Пакет содержит только исходники и `horizon-native.json`; одинаковые исходники дают одинаковые байты. Подпись minisign пока не используется. Публикация GitHub Release не добавляет мод автоматически в каталог Horizon: для каталога требуется отдельная заявка и ревью.

## Среда Codex

ECC 2.2.3 подключён нативным плагином `ecc@ecc`; роли находятся в `.codex/agents/`, проектный навык — `.agents/skills/horizon-modding/`. Повторная установка: `bash scripts/bootstrap-ecc.sh`. Хуки ECC требуют отдельного доверия в `/hooks`.

В исходной облачной среде Rust установлен в `/workspace/work/tools/`:

```bash
export CARGO_HOME=/workspace/work/tools/cargo
export RUSTUP_HOME=/workspace/work/tools/rustup
export PATH=/workspace/work/tools/cargo/bin:$PATH
```

Для другой машины эти пути не нужны. Подготовленные ресурсы Minecraft, сохранения, ключи и `target/` никогда не коммитятся.

## Пати, метки и модель Codex

Pure-модули `runtime.rs`, `party.rs`, `transport.rs`, `waypoints.rs` и `overlay.rs` подключены к `src/lib.rs` через `#[path]`. Тонкие адаптеры `client.rs`/`hud.rs` компилируются в реальном клиенте Horizon; `cargo test` отдельно проверяет модели. Для связи не устанавливаются серверные моды и не используются скрытые API приватов: клиент делится только собственным подтверждённым ответом `/claims`.

В `.codex/config.toml` и каждой роли закреплены `model = "gpt-6.1-sol"`, `model_reasoning_effort = "medium"`. Это настройки будущих сессий, не переключатель уже работающего облачного чата.

Token Optimizer 5.13.63 установлен нативным Codex-плагином. Установка для другой среды:

```bash
codex plugin marketplace add alexgreensh/token-optimizer
codex plugin add token-optimizer@alexgreensh-token-optimizer
```

Затем из установленного плагина запусти `measure.py --runtime codex codex-install --project "$PWD" --profile balanced`, перезапусти Codex и проверь доверие к хукам. `.codex/hooks.json` содержит машинные пути и игнорируется Git. Диагностика `codex-doctor` подтверждает регистрацию, но реальное выполнение Stop-хука нужно проверить в следующей сессии. В текущей облачной среде локальные Codex transcript/state DB отсутствуют, поэтому текущая экономия токенов не измерена.

Workflow проверяет feature-ветки и отдаёт `.hnmod` как Actions artifact; публикация Release выполняется только из `main` после успешной проверки.

## UI и проверка обновлений

`menu.rs` — pure-модель состояний, полей и действий; `menu_view.rs` рисует штатный модальный экран и принимает события. `client::service_menu` выполняет действия на игровом потоке, а не под mutex рендера. Escape/смена сервера сбрасывает очередь; подтверждение подписанного обмена не может быть вызвано серверной кликабельной командой. Все параметры визуализации проходят проверку до атомарной записи.

Launcher `src/github_mods.rs` сохраняет `<id>.origin.json` при установке по GitHub-ссылке. Проект из релиза следует `releases/latest`, а `is_update` сравнивает теги. Поэтому в релизе обязательны обычный tag/release и актуальный `.hnmod`; локальная ручная установка файла не даёт лаунчеру сведения о репозитории. Source-only пакет и версия в `horizon-native.json` проверяются штатным `cargo horizon-mod check`.

Chat cards and translation use `chat_feed.rs`, `chat_view.rs` and the bundled relay client translation worker. Native compilation is currently disabled in release CI by repository choice; run it locally against the exact pin before publishing.

Chat geometry and persistent preferences are shared in `chat_preferences.rs`; click hit boxes are stored in final GUI coordinates after scaling. Antispam operates on original text before translation and keeps author identity separate.

Claim cartography: `claim_map.rs` owns settings, projection, claim overlays and sorting; `map_view.rs` owns map UI; `map_backend.rs` is injected as a child of `game::extras` to reuse Horizon terrain scanning and its dynamic atlas slot. Source hashes for `game/extras.rs`, render `minimap.rs` and `hud.rs` are recorded in compatibility.json. The read-only `terrain_bridge.rs` exposes only newly scanned native tiles; `map_atlas_hook.rs` reserves a separate 512² world map slot.

Cartography 0.19: `seed_map.rs` содержит каталог 34 структур vanilla Java 26.3, группы, иконки предметов и геометрию полноэкранной карты. Фильтры schema 2 используют u64, читают schema 1 и распространяют старые групповые флаги на варианты. M открывает карту в режиме и мире, выбранных в прошлый раз (`map_selection`: режим, мир исследованной карты, мир Seed Map; впервые — исследованная карта в мире игрока); Seed Map включается отдельной вкладкой и показывает одно измерение.

`terrain_colors.rs` в одном фоне готовит средние цвета верхних текстур из текущего `RenderAssets` и источники tint через `MeshTables::tint_source`. Трава/листва/вода подкрашиваются биомом загруженного чанка; специальные цвета сохраняются штатными правилами. Ресурс-паки учитываются по generation, старый фон не публикуется после смены поколения. Декомпрессия текстур выполняется вне игрового потока; готовая палитра компактная. `map_backend.rs` читает только уже загруженные блоки и ограничивает захват 16 чанками за проход, с повтором остатка.

`map_history.rs` пишет DHMAP02: к прежним map-color/height/depth добавлены 256 ARGB цветов на чанк. Zero — нет цвета текстуры, применяется прежняя палитра. DHMAP01 читается с прежним размером записи и checksum; контекст и пути сохранения не меняются. Проверки размера, координат, дублей и атомарная запись сохранены; лимит региона 550000 байт. Старые, не загруженные в игру территории перекрашиваются при повторном сканировании, поскольку v1 не содержит состояния блоков/текстур. Тесты проверяют реальное чтение v1, запись v2 и сохранение цвета после перезапуска.

`seed_worker.rs` создаёт штатные генераторы vanilla normal preset для сида SMP. У запроса одно измерение, 256² биомных сэмплов Y64, одна последняя очередь/результат и отдельный кеш каждого измерения. Из `structure_sets.json` собирается вспомогательный `StructureRegistry` только для правил размещения; пустой resolver биомов в нём намеренный: он используется только для `potential_chunk`/`is_structure_chunk`, включая частоту и исключения. Кольцевые позиции и полные `structure_starts` берутся из полноценных генераторов с настоящими ресурсами/биомами. Структуры ищутся в привязанных к миру областях 1024×1024 блока: на область не более 64×64 чанков-кандидатов, без общего отсечения после 512 кандидатов. Кеш содержит до 512 завершённых областей и защищает текущую рабочую область от вытеснения; при превышении охвата показывается явное ограничение. За проход выполняется до восьми новых областей, с остановкой после завершённой области при затрате 30 мс; это не жёсткий дедлайн одного вызова генератора. Промежуточные результаты публикуются последовательно, а следующий проход запускается только для актуального поколения запроса. Отмена проверяется между строками и кандидатами; один старт и инициализация генератора завершаются до следующей проверки. Биомный слой переиспользуется между проходами и при смене только структурных фильтров. `seed_poi::clusters` группирует только одинаковые типы в привязанных к миру ячейках, выбирает стабильную реальную позицию представителя и рисует плотные группы под отдельными POI. Синтетические тесты покрывают охват вида 2 км, работу более чем с 512 маркерами, изоляцию измерений и стабильность группировки; это не заменяет игровую проверку генератора и внешнего вида.

`seed_view.rs` рисует предметные иконки штатным `draw_item`, категорийные плитки и поиск; atlas 256² используется целиком. Координатная репроекция старого снимка делает drag/zoom непрерывным, пока новый снимок рассчитывается. В `map_view.rs` отображённая проекция отдельно от опубликованной текстуры: hit tests меток используют проекцию на экране. Claims, игроки, метки и смерть рисуются отдельно. Никакие новые серверные пакеты или записи игровых чанков не используются. Интерактивная проверка требует игровых ресурсов; синтетический рендер проверяет только CPU-композицию/затенение.

Player faces use float-positioned atlas vertices to avoid GUI pixel snapping. Near-player markers resolve Horizon entities; far-player snapshots use a bounded interpolation deque. The roster is scoped to the game tab and relay realm, with selected contact keys pinned on mutual acceptance (TOFU, not Mojang identity verification).

`inventory_sort.rs` plans bounded chest transactions; `inventory_sort_view.rs` first simulates each plan in a disposable `menu_session`, checks cursor, drops, metadata and untouched player slots, then emits normal `menu_ops`. Server corrections or changed menus stop the operation.

Exploration: `map_history.rs` stores 16×16-chunk regions, including canonical map colour, height, water depth and a separately checksummed overview. Context metadata binds the hash directory to the exact scope, atlas, dimension and cave band. Version/size/checksum/coordinate/duplicate checks isolate bad files; atomic pending-file replacement preserves the previous completed region. Only the 24 most recently used regions remain in RAM; disk regions are not evicted.

`map_worker.rs` owns all disk I/O and terrain composition. At most 2048 coalesced tiles and one latest render request/result wait in each direction; unchanged renders reuse an image. The game thread scans already loaded terrain around the player under the native budget, then applies current claims and icons to the returned image. Panning never changes the scan centre. Normal frames do not wait for disk; `save_for_exit` waits up to three seconds for pending writes. A hard crash may lose the last second of unsaved exploration. No old 0.7.0 terrain can be migrated because it was never stored.

View coordinates remain geographic across rectangular viewports; the full map has a separate atlas texture, while the minimap keeps Horizon’s existing 128² slot. Stored terrain is local, and no relay protocol or source pin changes are needed.

Outgoing chat: `outgoing.rs` orders up to eight pending entries, splits text by packet UTF-16 limits and falls back on timeout; `outgoing_view.rs` intercepts normalized user chat before the original send path, preserves history, and calls `send_signed_chat` directly after translation. Commands bypass the queue. Direction-specific translation workers share the same bounded HTTP parser and never mix target-language caches. Disconnect clears all pending sends. The chat button's right-click opens a full-screen page that persists Input/Output/Received languages (20 common languages) and a common-languages-only auto-translate filter. See [translation comparison and verification](chat-translation.md).

`claim_directory.rs` stores per-owner, key-bound complete claim lists with monotonic persisted revisions; `[DHC1]` pages are independent of DHP party consent and remain E2E encrypted by the existing relay. Receivers validate tab membership, unique peer UUID, key pin, origins/dimensions/counts/revisions and full page completion before saving. Requests for cached lists do not run a game-server command. `claim_memory.rs` bootstraps once when no own record exists, observes server-confirmed claim/unclaim for the original command location/dimension, and exposes offline owners to both map and border rendering. This is owner-reported cached data, not server authority or Mojang identity verification; external changes need manual refresh. New clients learn directly from online owners, not offline relay storage.

`teleport.rs` accepts only the known server request grammar and whitelisted command variants with a Minecraft nickname; `teleport_view.rs` creates at most eight local requests, renders three cards, and drains click-selected commands on the game thread. Only server-sourced messages can create cards; the native chat cursor and exact current viewport are required for a click. Expiry, duplicate-click consumption and reset on disconnect prevent stale requests crossing sessions.

Mob radar reads interpolated loaded living entities only, excluding player/misc types; positions are bounded to 512 nearest marks and never persisted. Colours describe entity categories, not current aggression. Existing source pins and relay service protocol remain compatible.

## Экономика и интерфейс сервера (0.10.0)

`economy.rs` хранит котировки в сотых долях монеты и строит условные оценки без сетевых действий. Лоты используют полную цену объявления; остаток лота не получает выдуманную стоимость. Дневной денежный лимит применяется к выручке до удержаний. TAXES полностью удалены из расчётов, UI и настроек; старые значения в кеше игнорируются. Комиссия выставления AH имеет отдельное состояние «неизвестна».

`economy_client.rs` принимает только серверные сообщения, известные меню Market/Auction house и форматированный sidebar; `economy_scan.rs` допускает только проверенную навигацию, ждёт подтверждения новой страницы и прекращает работу при ручном вводе. Покупки и ставки не автоматизированы. Подтверждение продажи проверяет подпись текущего предмета, свежесть цены и лимитов. Автоматические `/worth` ограничены интервалом и очередью. Дисковая запись котировок выполняется ограниченным worker; live-баланс, дневные остатки и лоты между сеансами не восстанавливаются. Кеш привязан к серверу и аккаунту, неизвестные схемы не перезаписываются.

`server_ui.rs` заменяет sidebar/Tab только при соответствующих настройках; выключение возвращает штатный UI. Bridge внутри scoreboard использует реальные display names и number formats, сортирует строки по серверному порядку. Tab запрашивает скины только показанной страницы. `chat_priority.rs` задаёт точные имена авторов и не доверяет совпадениям в тексте сообщений. Карточки trade используют ту же проверку серверного источника, срока и однократного клика, что TPA.

Двойной ПКМ проецируется через опубликованный кадр карты с его фактическим соотношением сторон; высота читается фоновым worker из исследованной колонки. X использует текущую позицию игрока и подавляет повтор клавиши. Прежний ID `claim_borders` сохранён; файлы релиза имеют префикс `draimhorizons`.


0.11.0: `economy_requests.rs` сопоставляет тихие ответы с недавними запросами и обеспечивает справедливый опрос (старейший запрос первым), `economy_ui.rs` разделяет геометрию компактного окна/карточки и сортировку. Cancellable-инъекция `signed_chat_line` останавливает только наши распознанные тихие ответы после учёта данных, перед добавлением в штатный чат. Клиентские и игровые сообщения не создают наблюдения приватов.

`claim_observations.rs` — отдельный слой `[DHO1]`: не более 8192 проверенных чанков, независимые страницы до четырёх записей и 1024 байт, обновление по времени с сохранением освобождённых чанков как tombstones. Некорректная страница не применяется частично. Серверный ответ принимается только после локального `/claim info` с совпавшими координатами; измерение привязано к запросу на десять секунд. Доставка идёт через существующий шифрованный канал только однозначному peer из списка игроков этого realm, без требования дружбы. Полученное наблюдение не становится объявлением от имени владельца. Кеш изолирован по серверу/аккаунту; запись выполняется ограниченным worker с отказом перезаписывать неизвестную схему. Подтверждённые собственные/объявленные территории имеют приоритет отображения перед наблюдениями.


TPS: HEAD-инъекция `remote::Game::handle_packet` наблюдает только `SetTime`, `TickingState` и `Respawn`. `server_tps.rs` хранит до 64 точек за 30 секунд, вычисляет изменение серверных тиков по монотонному времени, ограничивает значением заявленного tick rate и скрывает замер старше восьми секунд. Это оценка, зависящая от доставки пакетов, а не официальный серверный показатель. Предсказанный `native_clock` и FPS не используются.

## Удалённая проверка Horizon f42ef42

При недоступном executor используйте `.github/workflows/horizon-compatibility.yml` на ветке `feature/horizon-f42ef42`: он проверяет именно закоммиченные source/tool pins и компилирует реальный клиент. Подробности исправления индексатора и неизменяемые SHA — в [horizon-f42ef42.md](horizon-f42ef42.md). Локальных патчей инструмента в финальном пути нет; прежняя политика обычного release CI сохранена.

## Инвентарь и click-through (0.13)

`inventory_sort.rs` — чистые планы по точному ключу компонентов, с явными списками разрешённых слотов. `inventory_preferences.rs` — ограниченная версия схемы настроек и проверка физического типа хранилища. Они включены в standalone тесты. Native-адаптер читает mapping из `menu_session`, симулирует Pickup на копии и проверяет весь снимок меню, курсор и dropped. Только после этого вызывает обычный `menu_ops`; QuickMove и Swap не используются. Максимум 512 шагов за 90 секунд, интервал 150 мс; любое изменение полного снимка останавливает операцию.

Click-through использует `native_interaction_key` на удалённом клиенте и точечную HEAD-инъекцию `use_item_as_player` для одиночной игры. Код адаптера живёт под `game::remote`, чтобы вызывать штатный приватный `remote_use_item_on`. Гостевые серверные действия не перехватываются. Временный forced_hit восстанавливается сразу после вызова. Опорный блок должен быть первым блоком после ровно одной таблички или первой целью за ближайшей рамкой, пересекаться лучом и быть в штатной дальности. Hook `remote_open_screen` фиксирует физическое происхождение окна до потребления use_target; обычный заголовок «Chest» сам по себе не доказывает безопасность plugin GUI.

Ручная проверка в игре перед оценкой визуального результата: сундук 27/54, бочка, эндер-сундук, шалкер (включая попытку перенести другой шалкер), воронка, выбрасыватель, грузовое животное, свой инвентарь и печь. Проверить каждую кнопку в обе стороны, полный приёмник, пустой источник, занятый курсор, компоненты/износ, исключения, неизменность хотбара/брони/крафта, редактор на разных GUI scale. Для click-through проверить шесть граней рамок, glow-рамку, разные таблички, Shift, недоступный контейнер, дальность, стену и ближнюю сущность. На сервере проверить plugin Market/AH, неизвестное виртуальное окно и изменение содержимого вторым игроком. Cargo/native check не заменяет эти интерактивные проверки.

## Визуальные компоненты (0.14)

`ui_style.rs` содержит независимые от игры цвета, геометрию скруглений, оригинальные контурные иконки 12×12, настройки и кривые анимаций. `ui_view.rs` рисует через существующий HudBatch/атлас, без GPU-пайплайна, сторонних ресурсов и подмены шрифта. rounded строится веером неперекрывающихся треугольников; цвет преобразуется ARGB → штатный RGBA. Поверхности учитывают общую плотность и существующую локальную прозрачность. Цвета пользовательских меток/приватов не заменяются.

Наведение/выбор используют экспоненциальное сглаживание с 75 мс, открытие — cubic ease-out 160 мс. После отрисовки изменяется только альфа новых вершин, координаты и hit regions сохраняются. Счётчики времени и игровые данные обновляются отдельно от декоративных эффектов. Кеши анимаций ограничены 2048 элементами наведения и 128 сценами. Настройки `appearance` в штатной папке данных `claim_borders` пишутся через временный файл; ошибка схемы/чтения не разрешает перезаписать исходный файл.

Общий модальный редактор получает ввод раньше остальных адаптеров; парный release перехватывается даже после закрытия редактора на press. Escape отменяет предпросмотр, Tab/Shift+Tab обходят девять элементов, Enter/Space активируют выбор. Ctrl+, не забирает обычные буквы из текстовых полей. Тесты проверяют частотно-независимое сглаживание, выключенные эффекты, альфу, границы маленьких кнопок, сетку иконок и сохранение настроек. Интерактивно проверить наведение, фокус, длинные подписи, шрифты ресурспаков, малый/большой GUI scale и сочетание двух редакторов.

## Horizon Nightly 2026-11-04

Текущий целевой nightly — `67dad0d1a5adca139b426c8114e74a33b376ba64`, horizon-mixins — `e8cd0aa38cffef49cb2e088ef716baa107148c02` (штатный pin лаунчера). Проверка `horizon-compatibility.yml` сверяет этот pin и компилирует настоящий клиент. Подробности — в [horizon-nightly-2026-11-04.md](horizon-nightly-2026-11-04.md); раздел f42ef42 выше сохранён как история предыдущего обновления.


## Аукцион и Horizon Nightly 2026-11-05 (0.16.0)

В версии 0.16.0 использовался source pin `f319701e8c9ab657ff562250b3ba07b2d654e3bc`, native hooks API 409. Инструмент остаётся на штатном pin лаунчера `e8cd0aa38cffef49cb2e088ef716baa107148c02`. Предыдущие разделы nightly сохранены как история; `compatibility.json` определяет текущую цель.

`economy::Preferences` хранит независимые `custom_auction` / `custom_market`; старые настройки получают включённые значения по умолчанию. F8 обрабатывается до проверки активности кастомного UI, чтобы возвращать его из стандартного вида. Переключение отменяет подготовленное действие и подтверждение.

`chat_feed::AuctionNotice` распознаёт подтверждения из `horizon-mods/native/smp/mixins/smp/smp_auction.rs`. Только серверный `push_component` может создать важное уведомление; player/local push не повышает приоритет. Дедупликация различает доверенные уведомления и обычный текст. История ограничена 200 записями; недавние уведомления сохраняются до 180 секунд при вытеснении обычных строк. Время показа блока задаётся существующей настройкой важных сообщений (10–180 секунд).


## Official Nightly policy (0.24.1)

Target the latest official release at https://github.com/horizon-rust/horizon/releases/tag/nightly, not Horizon main. For 0.24.1 this is Nightly 2026-11-07, source `6560641ba51a452f6bc2407baa4eb1b3be517ca8`, native hooks API 411.

Run `python tools/compatibility.py` before work and publication. Refresh with `python tools/compatibility.py --update --horizon PATH`, then apply the pinned mixins and compile the real client. The tool peels the official nightly tag to its source SHA, updates source hashes and records the nightly channel; CI rejects a missing or stale Nightly. `compatibility.json` always holds the exact tested SHA, never a moving branch or tag.
