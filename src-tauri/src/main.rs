#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use plugjam_core::{
    browser, donate,
    launch::{self, GameCheck},
    license,
    model::{Player, Rule, ServerInfo},
    query,
    store::{max_folders, SortMode, Store},
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{ipc::Channel, Manager, State};

const INTERNET_TTL: Duration = Duration::from_secs(45);
/// За один вызов опрашиваем не больше стольких серверов — защита от случайного залпа пакетов.
const MAX_BATCH: usize = 300;

struct AppState {
    store: Mutex<Store>,
    path: PathBuf,
    internet: tokio::sync::Mutex<Option<(Instant, Vec<ServerInfo>)>>,
}

/// Что видит интерфейс: хранилище + вычисленные поля подписки.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    #[serde(flatten)]
    store: Store,
    premium: bool,
    premium_until: Option<u64>,
    max_folders: usize,
    /// Выбранный пак иконок; без подписки всегда classic.
    icon_pack_effective: String,
}

fn view(st: &Store) -> View {
    let until = st.premium_until(license::now_secs());
    View {
        premium: until.is_some(),
        premium_until: until,
        max_folders: max_folders(until.is_some()),
        icon_pack_effective: if until.is_some() { st.icon_pack.clone() } else { "classic".into() },
        store: st.clone(),
    }
}

/// Меняет хранилище под замком, сохраняет файл и возвращает новое состояние целиком.
fn edit(
    s: &AppState,
    f: impl FnOnce(&mut Store) -> Result<(), String>,
) -> Result<View, String> {
    let mut g = s.store.lock().map_err(|_| "Состояние повреждено".to_string())?;
    f(&mut g)?;
    g.save(&s.path)?;
    Ok(view(&g))
}

#[tauri::command]
fn get_state(s: State<'_, AppState>) -> Result<View, String> {
    s.store.lock().map(|g| view(&g)).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings(s: State<'_, AppState>, nickname: String, gta_path: String) -> Result<View, String> {
    edit(&s, |st| st.set_settings(&nickname, &gta_path))
}

#[tauri::command]
fn set_sort(s: State<'_, AppState>, sort: SortMode) -> Result<View, String> {
    edit(&s, |st| {
        st.sort = sort;
        Ok(())
    })
}

#[tauri::command]
fn add_favorite(
    s: State<'_, AppState>,
    address: String,
    name: String,
    folder: Option<u32>,
) -> Result<View, String> {
    edit(&s, |st| st.add_favorite(&address, &name, folder))
}

#[tauri::command]
fn remove_favorite(s: State<'_, AppState>, address: String) -> Result<View, String> {
    edit(&s, |st| {
        st.remove_favorite(&address);
        Ok(())
    })
}

#[tauri::command]
fn move_favorite(s: State<'_, AppState>, address: String, folder: Option<u32>) -> Result<View, String> {
    edit(&s, |st| st.move_favorite(&address, folder))
}

#[tauri::command]
fn create_folder(s: State<'_, AppState>, name: String) -> Result<View, String> {
    edit(&s, |st| st.create_folder(&name).map(|_| ()))
}

#[tauri::command]
fn rename_folder(s: State<'_, AppState>, id: u32, name: String) -> Result<View, String> {
    edit(&s, |st| st.rename_folder(id, &name))
}

#[tauri::command]
fn delete_folder(s: State<'_, AppState>, id: u32) -> Result<View, String> {
    edit(&s, |st| st.delete_folder(id))
}

#[tauri::command]
fn set_names(s: State<'_, AppState>, items: Vec<(String, String)>) -> Result<(), String> {
    edit(&s, |st| {
        st.set_names(&items);
        Ok(())
    })
    .map(|_| ())
}


#[tauri::command]
fn set_theme(s: State<'_, AppState>, theme: String) -> Result<View, String> {
    edit(&s, |st| st.set_theme(&theme))
}

#[tauri::command]
fn set_icon_pack(s: State<'_, AppState>, pack: String) -> Result<View, String> {
    edit(&s, |st| st.set_icon_pack(&pack))
}

#[tauri::command]
fn activate_license(s: State<'_, AppState>, key: String) -> Result<View, String> {
    edit(&s, |st| st.activate(&key, license::now_secs()))
}

#[tauri::command]
fn add_saved_nick(s: State<'_, AppState>, nick: String) -> Result<View, String> {
    edit(&s, |st| st.add_saved_nick(&nick))
}

#[tauri::command]
fn remove_saved_nick(s: State<'_, AppState>, nick: String) -> Result<View, String> {
    edit(&s, |st| {
        st.remove_saved_nick(&nick);
        Ok(())
    })
}

#[tauri::command]
fn set_server_nick(s: State<'_, AppState>, address: String, nick: Option<String>) -> Result<View, String> {
    edit(&s, |st| st.set_server_nick(&address, nick.as_deref()))
}

#[tauri::command]
fn detect_game() -> Option<String> {
    launch::detect_gta()
}

#[tauri::command]
fn check_game(path: String) -> GameCheck {
    launch::check_game(&path)
}

#[tauri::command]
fn get_donate_info(s: State<'_, AppState>) -> Result<donate::Donate, String> {
    let g = s.store.lock().map_err(|e| e.to_string())?;
    Ok(donate::info(&g.user_id))
}

/// Открывает ссылку ton:// (кошелёк) или https:// системой. Команда без оболочки: адрес — один аргумент.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let ok_scheme = url.starts_with("ton://") || url.starts_with("https://");
    let ok_chars = !url.chars().any(|c| c.is_control() || c.is_whitespace() || "\"'<>^|`".contains(c));
    if !ok_scheme || !ok_chars || url.len() > 600 {
        return Err("Недопустимая ссылка".into());
    }
    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", &url]).creation_flags(0x0800_0000);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&url);
        c
    };
    cmd.spawn().map(|_| ()).map_err(|e| format!("Не удалось открыть ссылку: {e}"))
}

/// Опрашивает серверы параллельно; каждый результат уходит в интерфейс сразу, как только пришёл.
#[tauri::command]
async fn query_servers(addresses: Vec<String>, on_result: Channel<ServerInfo>) -> Result<(), String> {
    if addresses.len() > MAX_BATCH {
        return Err(format!("Не больше {MAX_BATCH} серверов за раз"));
    }
    query::query_many(addresses, 48, move |info| {
        let _ = on_result.send(info);
    })
    .await;
    Ok(())
}

#[derive(Serialize)]
struct Details {
    info: ServerInfo,
    rules: Vec<Rule>,
    players: Vec<Player>,
}

#[tauri::command]
async fn server_details(address: String) -> Result<Details, String> {
    query::parse_address(&address)?;
    let t = Duration::from_millis(1000);
    let (info, rules, players) = tokio::join!(
        query::query_info(&address, t, 2),
        query::query_rules(&address, t, 2),
        query::query_players(&address, t, 2)
    );
    Ok(Details {
        info: info.unwrap_or_else(|_| ServerInfo::offline(&address)),
        rules: rules.unwrap_or_default(),
        players: players.unwrap_or_default(),
    })
}

/// Список Internet. Кэш на 45 с: повторные открытия вкладки не бьют по индексу.
#[tauri::command]
async fn fetch_internet(s: State<'_, AppState>, force: bool) -> Result<Vec<ServerInfo>, String> {
    let mut cache = s.internet.lock().await;
    if !force {
        if let Some((at, list)) = cache.as_ref() {
            if at.elapsed() < INTERNET_TTL {
                return Ok(list.clone());
            }
        }
    }
    let list = browser::fetch_internet().await?;
    *cache = Some((Instant::now(), list.clone()));
    Ok(list)
}

#[tauri::command]
async fn connect(s: State<'_, AppState>, address: String, password: Option<String>) -> Result<(), String> {
    let (nick, mut path) = {
        let g = s.store.lock().map_err(|e| e.to_string())?;
        (g.nick_for(&address), g.gta_path.clone())
    };
    // Путь устарел (игру переместили) — ищем заново и запоминаем.
    if !launch::check_game(&path).exe {
        if let Some(found) = launch::detect_gta() {
            path = found.clone();
            let _ = edit(&s, |st| {
                st.gta_path = found;
                Ok(())
            });
        }
    }
    // Запуск ждёт загрузки библиотеки в игру — выносим из потока интерфейса.
    tauri::async_runtime::spawn_blocking(move || launch::launch(&path, &address, &nick, password.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let path = dir.join("store.json");
            let mut st = Store::load(&path);
            // Первый запуск: создаём случайный ID и ищем папку игры без участия пользователя.
            let mut dirty = st.ensure_user_id();
            if !launch::check_game(&st.gta_path).exe {
                if let Some(p) = launch::detect_gta() {
                    st.gta_path = p;
                    dirty = true;
                }
            }
            if dirty {
                let _ = st.save(&path);
            }
            app.manage(AppState {
                store: Mutex::new(st),
                path,
                internet: tokio::sync::Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_settings,
            set_sort,
            add_favorite,
            remove_favorite,
            move_favorite,
            create_folder,
            rename_folder,
            delete_folder,
            set_names,
            query_servers,
            server_details,
            fetch_internet,
            connect,
            set_theme,
            set_icon_pack,
            activate_license,
            add_saved_nick,
            remove_saved_nick,
            set_server_nick,
            detect_game,
            check_game,
            get_donate_info,
            open_url,
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска plugjam.mp");
}
