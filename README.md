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
- Запуск игры (`core/src/launch.rs`) — свой, без samp.exe: `gta_sa.exe` стартует приостановленным с `-c -n -h -p [-z]`, в процесс загружается клиентская DLL (`plugjam.dll`, иначе `samp.dll` из папки игры), затем игра возобновляется. Папка игры ищется сама: рядом с приложением (установка в папку GTA), реестр, типовые пути; вручную — «Настройки → Обзор». Нужны gta_sa.exe 1.0 US и клиентская DLL в папке игры.
- Данные: `store.json` в папке данных приложения.

## Установщик (.exe)

Без Windows: залить проект на GitHub → вкладка Actions → «Build Windows installer» → Run workflow.
Готовый `plugjam.mp_0.1.0_x64-setup.exe` скачивается из Artifacts (`plugjam-windows`).

## Подписка Plus (0,99 USDT / 30 дней)

- Лимиты: без подписки 3 папки, с подпиской 10; список сохранённых ников (общий и по серверам) и наборы иконок — только Plus.
- Оплата: USDT в сети TON на кошелёк из `core/src/donate.rs`, в комментарии — ID пользователя (`PJ-XXXX-XXXX-XXXX`, случайный, создаётся при первом запуске из генератора случайных чисел ОС).
- Серверов нет, поэтому ключ — подпись Ed25519 над «ID|срок». Выдача ключа вручную после проверки платежа:
  - `cargo run -p plugjam-core --bin license-tool -- keygen` — своя пара ключей; открытый вписать в `core/src/license.rs` (`PUBLIC_KEY_HEX`), секретный хранить только у себя;
  - `cargo run -p plugjam-core --bin license-tool -- sign <секретный-hex> <ID пользователя> 30` — ключ на 30 дней.
- Исходный код открыт, поэтому проверку в клиенте при желании можно обойти — подписка символическая.
