#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use plugjam_core::{
    browser, launch,
    model::{Player, Rule, ServerInfo},
    query,
    store::{SortMode, Store},
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

/// Меняет хранилище под замком, сохраняет файл и возвращает новое состояние целиком.
fn edit(
    s: &AppState,
    f: impl FnOnce(&mut Store) -> Result<(), String>,
) -> Result<Store, String> {
    let mut g = s.store.lock().map_err(|_| "Состояние повреждено".to_string())?;
    f(&mut g)?;
    g.save(&s.path)?;
    Ok(g.clone())
}

#[tauri::command]
fn get_state(s: State<'_, AppState>) -> Result<Store, String> {
    s.store.lock().map(|g| g.clone()).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings(s: State<'_, AppState>, nickname: String, gta_path: String) -> Result<Store, String> {
    edit(&s, |st| st.set_settings(&nickname, &gta_path))
}

#[tauri::command]
fn set_sort(s: State<'_, AppState>, sort: SortMode) -> Result<Store, String> {
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
) -> Result<Store, String> {
    edit(&s, |st| st.add_favorite(&address, &name, folder))
}

#[tauri::command]
fn remove_favorite(s: State<'_, AppState>, address: String) -> Result<Store, String> {
    edit(&s, |st| {
        st.remove_favorite(&address);
        Ok(())
    })
}

#[tauri::command]
fn move_favorite(s: State<'_, AppState>, address: String, folder: Option<u32>) -> Result<Store, String> {
    edit(&s, |st| st.move_favorite(&address, folder))
}

#[tauri::command]
fn create_folder(s: State<'_, AppState>, name: String) -> Result<Store, String> {
    edit(&s, |st| st.create_folder(&name).map(|_| ()))
}

#[tauri::command]
fn rename_folder(s: State<'_, AppState>, id: u32, name: String) -> Result<Store, String> {
    edit(&s, |st| st.rename_folder(id, &name))
}

#[tauri::command]
fn delete_folder(s: State<'_, AppState>, id: u32) -> Result<Store, String> {
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
fn connect(s: State<'_, AppState>, address: String, password: Option<String>) -> Result<(), String> {
    let (nick, path) = {
        let g = s.store.lock().map_err(|e| e.to_string())?;
        (g.nickname.clone(), g.gta_path.clone())
    };
    launch::launch(&path, &address, &nick, password.as_deref())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let path = dir.join("store.json");
            app.manage(AppState {
                store: Mutex::new(Store::load(&path)),
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
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска plugjam.mp");
}
