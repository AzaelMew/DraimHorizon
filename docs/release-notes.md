**DraimHorizons 0.22.6** for **Horizon Nightly 2026-11-05 · f319701** (Java 26.3).

## English

- Seed Map: click an individual POI to toggle a saved **found** mark and dim its icon. Click again to undo; dragging still pans. Zoom in to separate clustered POIs. Marks stay local and are isolated by server/world, account, seed and dimension.
- Settings → Chat → Appearance now has a **Compact messages** toggle. It reduces card padding and the gaps between messages without shrinking the font or changing multiline text spacing.
- The setting persists across restarts. Existing preferences keep normal spacing until compact messages are enabled; resetting chat settings restores normal spacing.
- Fullscreen claim outlines remain complete during fractional panning; terrain shading no longer creates the confirmed 256-block region-boundary stripes.
- Small map drags reuse buffered terrain instead of rebuilding and uploading unchanged images. Broad views use coarser chunk overviews to avoid detailed-region cache thrashing; exploration files are unchanged.

Rebuild native mods in the launcher after updating.

## Русский

- Seed Map: щелчок по отдельной структуре переключает сохраняемую отметку **найдено** и затемняет значок. Повторный щелчок снимает отметку; перетаскивание по-прежнему двигает карту. Для выбора структуры в группе сначала приблизьте карту. Отметки локальные и разделены по серверу/миру, аккаунту, сиду и измерению.
- Настройки → Чат → Оформление: «Компактные сообщения» уменьшают отступы карточек и промежутки между ними без изменения размера шрифта и межстрочного интервала.
- Параметр сохраняется после перезапуска. По умолчанию и после сброса настроек сохраняются обычные отступы.
- Контуры приватов полноэкранной карты сохраняют все стороны при дробном смещении; затенение рельефа больше не создаёт подтверждённые полосы на границах регионов через каждые 256 блоков.
- Небольшое перетаскивание переиспользует снимок рельефа с запасом по краям вместо повторной загрузки неизменённой текстуры. Широкие виды используют более грубые обзоры чанков, избегая постоянного вытеснения подробных регионов; файлы исследованной карты не меняются.

После обновления пересоберите нативные моды в лаунчере.
