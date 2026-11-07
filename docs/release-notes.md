**DraimHorizons 0.22.7** for **Horizon Nightly 2026-11-05 · f319701** (Java 26.3).

## English

- Fix missing Seed Map POIs at the **2 km landscape scale**: structure searches now fill cached world-aligned tiles progressively instead of dropping whole structure sets or stopping at a shared 512-candidate cutoff.
- Known POIs remain available while new tiles load. Wait for **Searching POIs…** to finish in a cold area; extremely distant views still show an explicit search-limit warning.
- Clusters combine only matching structure types and use stable real-world representatives, so panning does not arbitrarily replace a village icon with a mineshaft icon. Dense clusters draw underneath individual POIs.
- Existing found marks, click/drag handling, world/account/seed/dimension isolation, and English/Russian controls remain supported. Source pins and saved exploration/found-marker formats are unchanged.

Rebuild native mods in the launcher after updating. The new 2 km rendering still needs in-game acceptance; synthetic coverage tests do not establish live generator completeness or visual quality.

## Русский

- Исправлено исчезновение POI Seed Map на **широком виде с линейкой 2 км**: поиск последовательно заполняет привязанные к миру кешируемые области вместо пропуска целых наборов структур или остановки на общих 512 кандидатах.
- Известные POI сохраняются на карте во время загрузки новых областей. В новой области дождитесь завершения надписи **Поиск POI…**; на чрезвычайно далёком масштабе ограничение поиска явно отмечается.
- Группируются только одинаковые типы структур с устойчивым выбором реального представителя, поэтому перемещение карты не подменяет значок деревни значком шахты. Плотные группы рисуются под отдельными POI.
- Сохранены отметки найденных мест, различение щелчка и перетаскивания, разделение по миру/аккаунту/сиду/измерению и переключатель RU/EN. Source pins и форматы сохранённой карты и отметок не изменены.

После обновления пересоберите нативные моды в лаунчере. Новому виду 2 км ещё требуется игровая проверка: синтетические тесты охвата не подтверждают полноту живого генератора или качество изображения.
