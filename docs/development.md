# Разработка мода Horizon

## Что настроено

- Нативный плагин `ecc@ecc` версии 2.2.3: общая библиотека навыков, конфигурация MCP и SessionStart-хук. Скрипт установки также устанавливает runtime-зависимости хуков через `npm ci --omit=dev --ignore-scripts --no-bin-links`; доверие к хукам остаётся отдельным действием в Codex.
- Роли `explorer`, `reviewer`, `docs_researcher`, `rust_developer` адаптированы из проектной конфигурации ECC; модель наследуется из сессии.
- Локальный навык `horizon-modding` и корневой `AGENTS.md` задают контекст проекта.
- Источники и точные ревизии записаны в `.ecc/upstream-lock.json`.

MCP Chrome DevTools поставляется ECC; его запуск требует `npx` и подходящего браузера. Готовность браузера этой установкой не проверялась. Claude-агенты и Claude-хуки не равнозначны ролям и хукам Codex.

## Исходники и API

При первоначальной настройке исходники лежат в `/workspace/work/ecc` и `/workspace/work/horizon`, рабочий проект — `/workspace/HorizonMod`. Это пути текущей среды. Для другой среды используйте gitignored `.local/` и ревизии lock-файла.

У Horizon два формата:

| Формат | Содержимое | Инструменты |
| --- | --- | --- |
| `.hmod` | Данные и ресурсы, API 0.2 | `cargo horizon-mod new/build/check` |
| `.hnmod` | Нативный Rust, horizon-mixins | Официальный native-шаблон и сборка нативного bundle; `cargo horizon-mod check` проверяет пакет |

На исследованной ревизии `NATIVE_HOOKS_API = 153` в `crates/horizon-client/src/game/native_hooks.rs`. Выбирайте минимальный API используемых хуков, а не автоматически 153. Для произвольных source-mixins нужна точная ревизия Horizon. Java-код Fabric/NeoForge в Horizon не исполняется.

Источники: upstream `docs/mods.md`, `docs/native-mods.md`, `crates/cargo-horizon-mod/`, `crates/horizon-mods/`; для native — https://github.com/horizon-rust/horizon-native-mod-template и https://github.com/horizon-rust/horizon-mixins.

## Перед реализацией

Определите функции мода, сторону исполнения и целевую версию Horizon. После выбора формата создайте соответствующий шаблон. Rust/Cargo и сама игра не входят в установку ECC: в исходной среде Rust/Cargo отсутствуют. Для текущего `cargo-horizon-mod` зависимость `horizon-mods` требует Rust 1.95, хотя общий README Horizon указывает более низкую версию; сверяйтесь с Cargo-манифестами целевой ревизии.

После установки подходящего Rust toolchain CLI можно поставить из зафиксированных исходников:

```bash
cargo install --path /path/to/pinned/horizon/crates/cargo-horizon-mod --locked
```

Для сборки полной игры могут потребоваться дополнительные системные библиотеки, JDK и собственная копия Minecraft. В рамках настройки ECC игра и мод не собирались. Не коммитьте подготовленные ресурсы или декомпилированный код.
