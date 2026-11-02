# Разработка Claim Borders

## Устройство

- `mixins/claim_borders/runtime.rs` — парсер ответа SMP `/claims`, кеш сеанса и построение внешнего контура. Один исходник используется standalone Cargo-библиотекой и внедряемым модулем.
- Остальные `.rs` в bundle — тонкие адаптеры чата, команд, отрисовки и переподключения.
- `horizon-mixin.toml` — native hooks и точечные HEAD-инъекции; точная ревизия Horizon закреплена в `compatibility.json`.
- Для рисования используются штатные `Gizmos` (depth-tested или `on_top`); новый GPU-пайплайн не добавляется.
- `/claims` содержит блоковые начала чанков, не номера чанков. Формат проверен по `horizon-rust/horizon-mods/native/smp/mixins/smp/smp_claims.rs`.

## Проверки

Нужны Git, Python 3.12+, Rust 1.99.1 (rustfmt, clippy). Стадирование использует Linux file locks. `tools/stage.py` скачивает закреплённые исходники в gitignored `.local/`, экспортирует их без изменения исходного checkout и вызывает официальный horizon-mixins:

```bash
cargo fmt --all --check
rustfmt --edition 2024 --check mixins/claim_borders/*.rs
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
  --locked -p cargo-horizon-mod -- check target/claim_borders-0.6.0.hnmod --json
```

Последняя команда принимает путь относительно текущей рабочей директории. Она выполняет те же проверки архива, что реестр Horizon. Полная игровая сборка и интерактивный тест требуют дополнительных зависимостей, GPU и собственных подготовленных ресурсов Minecraft. Отсутствие игровых ресурсов не блокирует `cargo check` и упаковку исходного `.hnmod`.

Тесты охватывают ответ со скриншота, отрицательные координаты, Unicode-пробелы и цветовые коды, разные измерения, неполные/дублированные/слишком большие ответы, ограниченное ожидание, понижение серверной квоты, размер чанка 16×16, объединённые границы, дальность и сброс состояния.

## Релиз

Workflow `.github/workflows/release.yml` при изменении исходников в `main` запускает проверки, официальное стадирование, `cargo check` настоящего клиента и проверку пакета; затем публикует GitHub Release для версии из Cargo.toml. Повторный запуск не заменяет существующий релиз. Перед изменениями увеличивайте версию одновременно в Cargo.toml, bundle manifest и runtime VERSION.

Артефакты: `claim_borders-<version>.hnmod` и его `.sha256`. Пакет содержит только исходники и `horizon-native.json`; одинаковые исходники дают одинаковые байты. Подпись minisign пока не используется. Публикация GitHub Release не добавляет мод автоматически в каталог Horizon: для каталога требуется отдельная заявка и ревью.

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

Claim cartography: `claim_map.rs` owns settings, projection, claim overlays and sorting; `map_view.rs` owns map UI; `map_backend.rs` is injected as a child of `game::extras` to reuse Horizon terrain scanning and its dynamic atlas slot. Extra source hash for `game/extras.rs` is recorded in compatibility.json. Terrain is limited to loaded chunks and is not persisted.
