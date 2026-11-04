# Horizon Nightly 2026-11-04

DraimHorizons 0.15.2 предназначен для [Nightly 2026-11-04 · 67dad0d](https://github.com/horizon-rust/horizon/releases/tag/nightly). Тег `nightly` перемещается; неизменяемый источник — `nightly-source/2026-11-04-67dad0d`, полный SHA `67dad0d1a5adca139b426c8114e74a33b376ba64`.

`compatibility.json` и bundle manifest закрепляют именно этот commit. Контрольные суммы девяти используемых исходных файлов пересчитаны по нему. `NATIVE_HOOKS_API` по-прежнему 359; мод использует также произвольные source mixins, поэтому точный pin обязателен.

## Инструмент

В этом nightly `launcher/src/native_mods.rs::MIXINS_REV` равен `e8cd0aa38cffef49cb2e088ef716baa107148c02`. Используется тот же официальный commit, без локальных патчей. Он содержит исправления crate-level и известных tool attributes. Старый pin `10fdf8bda39d1e336c0f13f802db1041fbe41abf` не доступен при клонировании upstream.

Изменения Horizon после f42ef42 затрагивают очистку draw-команд при переходе к панораме и движение мобов в лианах/на лестницах. Точки внедрения и native-адаптеры мода проверяются официальным plan/apply и компиляцией настоящего клиента.

## Проверки

Локально выполнены:

- `cargo fmt --all --check` и `rustfmt --edition 2024 --check mixins/claim_borders/*.rs`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- `cargo test --workspace --locked`: 168 тестов прошли, один сетевой тест провайдера перевода оставлен ignored.
- Тесты `horizon-mixins-core` и обе регрессии индексатора из compatibility CI.
- `python3 tools/stage.py`: официальный plan/apply, 41 инъекция и 54 модуля.
- `cargo check --manifest-path target/mixins/modded/Cargo.toml --target-dir target/horizon-check --locked -p horizon-client --bin horizon-client --no-default-features`: успешно. Единственное предупреждение — существующий неиспользуемый вариант `Icon::Clock` в UI мода.

`python3 tools/package.py` и штатный `cargo-horizon-mod check target/draimhorizons-0.15.2.hnmod --json` завершились успешно (`ok: true`). Архив: 274486 байт, SHA-256 `b19253114ecef81267f77cd2b6503cff2cd07c42615b553957e1cba7204074e3`.

 Интерактивный запуск в игре требует собственных ресурсов Minecraft и не выполнялся. Relay не менялся, его отдельные Python/E2E/container-проверки повторно не запускались.

## Установка

Установите `draimhorizons-0.15.2.hnmod` и пересоберите нативные моды в лаунчере. Обновление по GitHub станет доступно после отдельной публикации релиза. Relay обновлять не требуется.
