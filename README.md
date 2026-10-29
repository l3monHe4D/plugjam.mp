# plugjam.mp

Клиент-наследник SA-MP: Rust + Tauri 2, интерфейс на чистом HTML/CSS/JS (без фреймворков и сборщика).

## Сборка (Windows)

1. Rust (stable) и WebView2 (есть в Windows 10/11).
2. `cargo install tauri-cli --version "^2"`
3. Из корня проекта: `cargo tauri dev` — запуск, `cargo tauri build` — установщик (release: LTO, strip, opt-level "s").
4. Тесты ядра (работают на любой ОС): `cargo test -p plugjam-core`

## Структура

- `core/` — Rust-ядро без Tauri: UDP-запросы SA-MP (`query.rs`), хранилище папок/избранного (`store.rs`), индекс Internet (`browser.rs`), запуск игры (`launch.rs`).
- `src-tauri/` — оболочка Tauri: команды для интерфейса.
- `ui/` — интерфейс: вкладки Favorites / Internet / Top 10, папки, сортировка А–Я / Онлайн.

## Заметки

- Internet берётся из индекса `https://api.open.mp/servers` (поменять адрес — `core/src/browser.rs`).
- Top 10 — по онлайну; записи, где игроков больше слотов, в рейтинг не попадают.
- Запуск игры: `core/src/launch.rs`, сейчас через `samp.exe` с аргументами `-c -n -h -p [-z]`. Проверить на Windows с установленной игрой; при необходимости меняется только эта функция.
- Данные: `store.json` в папке данных приложения.

## Установщик (.exe)

Без Windows: залить проект на GitHub → вкладка Actions → «Build Windows installer» → Run workflow.
Готовый `plugjam.mp_0.1.0_x64-setup.exe` скачивается из Artifacts (`plugjam-windows`).
