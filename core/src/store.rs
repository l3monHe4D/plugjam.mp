//! Локальное хранилище: ник, путь к игре, папки и избранные серверы.
//! Один JSON-файл, запись атомарная (временный файл + rename).

use crate::{license, query::normalize_address};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

pub const FREE_FOLDERS: usize = 3;
pub const PLUS_FOLDERS: usize = 10;
const MAX_NICKS: usize = 30;
pub const THEMES: [&str; 3] = ["system", "dark", "light"];
/// Иконки лаунчера (окно и панель задач). Всё, кроме classic, — Plus.
pub const APP_ICONS: [&str; 5] = ["classic", "sunset", "ocean", "violet", "mono"];

pub fn max_folders(premium: bool) -> usize {
    if premium { PLUS_FOLDERS } else { FREE_FOLDERS }
}

const NEED_PLUS: &str = "Нужна подписка Plus";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortMode {
    #[default]
    Name,
    Online,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Folder {
    pub id: u32,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Favorite {
    pub address: String,
    /// Последнее известное имя — чтобы показывать офлайн-серверы.
    pub name: String,
    pub folder: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    pub nickname: String,
    pub gta_path: String,
    pub sort: SortMode,
    pub folders: Vec<Folder>,
    pub favorites: Vec<Favorite>,
    next_folder_id: u32,
    /// Случайный ID установки (создаётся один раз).
    pub user_id: String,
    /// Лицензионный ключ подписки Plus (пусто — бесплатная версия).
    pub license: String,
    pub theme: String,
    pub app_icon: String,
    /// Plus: общий список сохранённых ников.
    pub nicknames: Vec<String>,
    /// Plus: ник для конкретного сервера (адрес -> ник).
    pub server_nicks: BTreeMap<String, String>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            nickname: "Player".into(),
            gta_path: String::new(),
            sort: SortMode::Name,
            folders: vec![],
            favorites: vec![],
            next_folder_id: 1,
            user_id: String::new(),
            license: String::new(),
            theme: "system".into(),
            app_icon: "classic".into(),
            nicknames: vec![],
            server_nicks: BTreeMap::new(),
        }
    }
}

/// Правила ника SA-MP: 3–20 символов из набора `A-Za-z0-9_[]()$@.=`.
pub fn validate_nickname(n: &str) -> Result<(), String> {
    let ok_len = (3..=20).contains(&n.chars().count());
    let ok_chars = n
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_[]()$@.=".contains(c));
    if ok_len && ok_chars {
        Ok(())
    } else {
        Err("Ник: 3–20 символов, латиница, цифры и _ [ ] ( ) $ @ . =".into())
    }
}

impl Store {
    /// Читает файл. Нет файла — настройки по умолчанию. Битый файл сохраняем как .bak.
    pub fn load(path: &Path) -> Store {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
                let _ = fs::rename(path, path.with_extension("bak"));
                Store::default()
            }),
            Err(_) => Store::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    /// Создаёт ID, если его ещё нет. Возвращает true, если хранилище изменилось.
    pub fn ensure_user_id(&mut self) -> bool {
        if license::valid_user_id(&self.user_id) {
            return false;
        }
        self.user_id = license::new_user_id();
        true
    }

    /// Срок окончания подписки (unix-секунды), если ключ действителен на момент `now`.
    pub fn premium_until(&self, now: u64) -> Option<u64> {
        // Только в отладочных сборках (`cargo tauri dev`): PLUGJAM_DEV_PLUS=1 включает Plus для проверки.
        // В релизной сборке этого кода нет.
        #[cfg(debug_assertions)]
        if std::env::var("PLUGJAM_DEV_PLUS").as_deref() == Ok("1") {
            return Some(now + 365 * 86_400);
        }
        if self.license.is_empty() {
            return None;
        }
        license::verify(&self.license, &self.user_id, now).ok()
    }

    pub fn is_premium(&self) -> bool {
        self.premium_until(license::now_secs()).is_some()
    }

    pub fn activate(&mut self, key: &str, now: u64) -> Result<(), String> {
        let exp = license::verify(key, &self.user_id, now)?;
        if let Some(cur) = self.premium_until(now) {
            if exp <= cur {
                return Err("Этот ключ не продлевает текущую подписку".into());
            }
        }
        self.license = key.split_whitespace().collect();
        Ok(())
    }

    pub fn set_theme(&mut self, theme: &str) -> Result<(), String> {
        if !THEMES.contains(&theme) {
            return Err("Неизвестная тема".into());
        }
        self.theme = theme.into();
        Ok(())
    }

    /// Иконки лаунчера кроме «classic» доступны с подпиской.
    pub fn set_app_icon(&mut self, icon: &str) -> Result<(), String> {
        if !APP_ICONS.contains(&icon) {
            return Err("Неизвестная иконка".into());
        }
        if icon != "classic" && !self.is_premium() {
            return Err(NEED_PLUS.into());
        }
        self.app_icon = icon.into();
        Ok(())
    }

    /// Иконка, которая реально применяется: без подписки всегда classic.
    pub fn app_icon_effective(&self) -> &str {
        if self.is_premium() { &self.app_icon } else { "classic" }
    }

    pub fn add_saved_nick(&mut self, nick: &str) -> Result<(), String> {
        if !self.is_premium() {
            return Err(NEED_PLUS.into());
        }
        let n = nick.trim();
        validate_nickname(n)?;
        if !self.nicknames.iter().any(|x| x == n) {
            if self.nicknames.len() >= MAX_NICKS {
                return Err(format!("Не больше {MAX_NICKS} сохранённых ников"));
            }
            self.nicknames.push(n.to_string());
        }
        Ok(())
    }

    pub fn remove_saved_nick(&mut self, nick: &str) {
        self.nicknames.retain(|x| x != nick);
        self.server_nicks.retain(|_, v| v != nick);
    }

    pub fn set_server_nick(&mut self, address: &str, nick: Option<&str>) -> Result<(), String> {
        if !self.is_premium() {
            return Err(NEED_PLUS.into());
        }
        let a = normalize_address(address)?;
        match nick.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => {
                validate_nickname(n)?;
                self.server_nicks.insert(a, n.to_string());
            }
            None => {
                self.server_nicks.remove(&a);
            }
        }
        Ok(())
    }

    /// Ник для подключения: у Plus — индивидуальный для сервера, иначе общий.
    pub fn nick_for(&self, address: &str) -> String {
        if self.is_premium() {
            if let Ok(a) = normalize_address(address) {
                if let Some(n) = self.server_nicks.get(&a) {
                    return n.clone();
                }
            }
        }
        self.nickname.clone()
    }

    pub fn set_settings(&mut self, nickname: &str, gta_path: &str) -> Result<(), String> {
        let nick = nickname.trim();
        validate_nickname(nick)?;
        self.nickname = nick.to_string();
        self.gta_path = gta_path.trim().to_string();
        Ok(())
    }

    fn check_folder(&self, folder: Option<u32>) -> Result<(), String> {
        match folder {
            Some(id) if !self.folders.iter().any(|f| f.id == id) => Err("Папка не найдена".into()),
            _ => Ok(()),
        }
    }

    /// Добавляет сервер. Повтор не создаёт дубликат, а просто перекладывает в нужную папку.
    pub fn add_favorite(&mut self, address: &str, name: &str, folder: Option<u32>) -> Result<(), String> {
        let address = normalize_address(address)?;
        self.check_folder(folder)?;
        let name: String = name.chars().take(120).collect();
        match self.favorites.iter_mut().find(|f| f.address == address) {
            Some(f) => {
                f.folder = folder;
                if !name.is_empty() {
                    f.name = name;
                }
            }
            None => self.favorites.push(Favorite { address, name, folder }),
        }
        Ok(())
    }

    pub fn remove_favorite(&mut self, address: &str) {
        let a = address.trim().to_ascii_lowercase();
        self.favorites.retain(|f| f.address != a);
    }

    pub fn move_favorite(&mut self, address: &str, folder: Option<u32>) -> Result<(), String> {
        self.check_folder(folder)?;
        let a = address.trim().to_ascii_lowercase();
        let f = self
            .favorites
            .iter_mut()
            .find(|f| f.address == a)
            .ok_or("Сервера нет в избранном")?;
        f.folder = folder;
        Ok(())
    }

    pub fn set_names(&mut self, items: &[(String, String)]) {
        for (addr, name) in items {
            let a = addr.trim().to_ascii_lowercase();
            if let Some(f) = self.favorites.iter_mut().find(|f| f.address == a) {
                f.name = name.chars().take(120).collect();
            }
        }
    }

    fn clean_folder_name(&self, name: &str, except: Option<u32>) -> Result<String, String> {
        let n = name.trim();
        if n.is_empty() || n.chars().count() > 32 {
            return Err("Имя папки: от 1 до 32 символов".into());
        }
        let lower = n.to_lowercase();
        if self
            .folders
            .iter()
            .any(|f| Some(f.id) != except && f.name.to_lowercase() == lower)
        {
            return Err("Папка с таким именем уже есть".into());
        }
        Ok(n.to_string())
    }

    pub fn create_folder(&mut self, name: &str) -> Result<u32, String> {
        let name = self.clean_folder_name(name, None)?;
        let limit = max_folders(self.is_premium());
        if self.folders.len() >= limit {
            return Err(if limit == FREE_FOLDERS {
                format!("Без подписки — до {FREE_FOLDERS} папок. С Plus — до {PLUS_FOLDERS}")
            } else {
                format!("Достигнут предел: {PLUS_FOLDERS} папок")
            });
        }
        let id = self.next_folder_id;
        self.next_folder_id += 1;
        self.folders.push(Folder { id, name });
        Ok(id)
    }

    pub fn rename_folder(&mut self, id: u32, name: &str) -> Result<(), String> {
        let name = self.clean_folder_name(name, Some(id))?;
        self.folders
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or("Папка не найдена")?
            .name = name;
        Ok(())
    }

    /// Удаляет папку; серверы из неё остаются в избранном без папки.
    pub fn delete_folder(&mut self, id: u32) -> Result<(), String> {
        self.check_folder(Some(id))?;
        self.folders.retain(|f| f.id != id);
        for f in self.favorites.iter_mut().filter(|f| f.folder == Some(id)) {
            f.folder = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("plugjam-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("store.json")
    }

    #[test]
    fn nickname_rules() {
        for ok in ["Tina_Carter", "abc", "[CL]Name.1", "a".repeat(20).as_str()] {
            assert!(validate_nickname(ok).is_ok(), "{ok}");
        }
        for bad in ["ab", "", "Имя_Ник", "has space", "a".repeat(21).as_str(), "x/y"] {
            assert!(validate_nickname(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn favorites_dedupe_and_folders() {
        let mut s = Store::default();
        let fid = s.create_folder("RP").unwrap();
        s.add_favorite("Host.com:7777", "Srv", None).unwrap();
        s.add_favorite("host.com", "", Some(fid)).unwrap(); // тот же адрес
        assert_eq!(s.favorites.len(), 1);
        assert_eq!(s.favorites[0].folder, Some(fid));
        assert_eq!(s.favorites[0].name, "Srv");
        assert!(s.add_favorite("a.b:1", "x", Some(99)).is_err());
        assert!(s.add_favorite("bad addr", "x", None).is_err());

        s.delete_folder(fid).unwrap();
        assert_eq!(s.favorites[0].folder, None);
        assert!(s.delete_folder(fid).is_err());
        s.remove_favorite("HOST.com:7777");
        assert!(s.favorites.is_empty());
    }

    #[test]
    fn folder_names() {
        let mut s = Store::default();
        let a = s.create_folder("Alpha").unwrap();
        let b = s.create_folder("Beta").unwrap();
        assert_ne!(a, b);
        assert!(s.create_folder("alpha").is_err());
        assert!(s.create_folder("  ").is_err());
        assert!(s.create_folder(&"x".repeat(33)).is_err());
        assert!(s.rename_folder(b, "ALPHA").is_err());
        s.rename_folder(b, "Gamma").unwrap();
        s.rename_folder(a, "Alpha").unwrap(); // то же имя у самой себя — можно
        // id не переиспользуются после удаления
        s.delete_folder(b).unwrap();
        assert!(s.create_folder("Delta").unwrap() > b);
    }

    #[test]
    fn move_and_names() {
        let mut s = Store::default();
        let f = s.create_folder("F").unwrap();
        s.add_favorite("a.b:1", "old", None).unwrap();
        s.move_favorite("A.B:1", Some(f)).unwrap();
        assert_eq!(s.favorites[0].folder, Some(f));
        assert!(s.move_favorite("zzz:1", None).is_err());
        s.set_names(&[("a.b:1".into(), "new".into())]);
        assert_eq!(s.favorites[0].name, "new");
    }

    #[test]
    fn save_load_roundtrip_and_corrupt() {
        let p = tmp("rt");
        let mut s = Store::default();
        s.set_settings("Tina_Carter", "C:\\Games\\GTA").unwrap();
        s.sort = SortMode::Online;
        let f = s.create_folder("RP").unwrap();
        s.add_favorite("1.2.3.4:7777", "S", Some(f)).unwrap();
        s.save(&p).unwrap();
        assert_eq!(Store::load(&p), s);

        fs::write(&p, "{ not json").unwrap();
        assert_eq!(Store::load(&p), Store::default());
        assert!(p.with_extension("bak").exists());
        assert_eq!(Store::load(&tmp("missing")), Store::default());
    }

    #[test]
    fn old_file_with_missing_fields_loads() {
        let p = tmp("old");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, r#"{"nickname":"Abc"}"#).unwrap();
        let s = Store::load(&p);
        assert_eq!(s.nickname, "Abc");
        assert_eq!(s.sort, SortMode::Name);
    }

    /// Делает хранилище подписчиком: ключ подписан тестовым секретом на `secs` вперёд.
    fn make_plus(s: &mut Store, secs: u64) -> String {
        s.ensure_user_id();
        let key = license::sign(&license::TEST_SECRET, &s.user_id, license::now_secs() + secs).unwrap();
        s.activate(&key, license::now_secs()).unwrap();
        key
    }

    #[test]
    fn free_folder_limit_is_three() {
        let mut s = Store::default();
        for n in ["A", "B", "C"] {
            s.create_folder(n).unwrap();
        }
        let e = s.create_folder("D").unwrap_err();
        assert!(e.contains("до 3"), "{e}");
        assert_eq!(max_folders(false), 3);
        assert_eq!(max_folders(true), 10);
        // удалить и создать снова можно
        let id = s.folders[0].id;
        s.delete_folder(id).unwrap();
        assert!(s.create_folder("D").is_ok());
    }

    #[test]
    fn free_user_cannot_use_plus_features() {
        let mut s = Store::default();
        assert!(s.add_saved_nick("Nick_1").unwrap_err().contains("подписка"));
        assert!(s.set_server_nick("a.b:1", Some("Nick_1")).is_err());
        assert!(s.set_app_icon("sunset").is_err());
        assert!(s.set_app_icon("classic").is_ok());
        assert!(s.set_app_icon("nope").is_err());
        assert!(s.set_theme("light").is_ok());
        assert!(s.set_theme("pink").is_err());
        s.nickname = "Main_Nick".into();
        assert_eq!(s.nick_for("a.b:1"), "Main_Nick");
        assert!(s.activate("garbage", 1).is_err());
        assert!(!s.is_premium());
    }

    #[test]
    fn user_id_created_once() {
        let mut s = Store::default();
        assert!(s.ensure_user_id());
        let id = s.user_id.clone();
        assert!(license::valid_user_id(&id));
        assert!(!s.ensure_user_id());
        assert_eq!(s.user_id, id);
    }

    #[test]
    fn plus_unlocks_ten_folders_nicks_and_launcher_icon() {
        let mut s = Store::default();
        make_plus(&mut s, 3600);
        assert!(s.is_premium());
        for i in 0..10 {
            s.create_folder(&format!("F{i}")).unwrap();
        }
        assert!(s.create_folder("F10").unwrap_err().contains("10"));

        s.add_saved_nick("Nick_1").unwrap();
        s.add_saved_nick("Nick_1").unwrap(); // без дубликата
        s.add_saved_nick("Nick_2").unwrap();
        assert_eq!(s.nicknames.len(), 2);
        assert!(s.add_saved_nick("x").is_err());

        s.nickname = "Main_Nick".into();
        s.set_server_nick("HOST.com", Some("Nick_2")).unwrap();
        assert_eq!(s.nick_for("host.com:7777"), "Nick_2");
        assert_eq!(s.nick_for("other:7777"), "Main_Nick");
        s.set_app_icon("ocean").unwrap();
        assert_eq!(s.app_icon_effective(), "ocean");

        // удаление ника из списка снимает его и с серверов
        s.remove_saved_nick("Nick_2");
        assert_eq!(s.nick_for("host.com:7777"), "Main_Nick");
        s.set_server_nick("host.com:7777", None).unwrap();
    }

    #[test]
    fn expired_plus_falls_back_but_keeps_data() {
        let mut s = Store::default();
        let key = make_plus(&mut s, 3600);
        for i in 0..5 {
            s.create_folder(&format!("F{i}")).unwrap();
        }
        s.add_saved_nick("Nick_1").unwrap();
        s.set_server_nick("a.b:1", Some("Nick_1")).unwrap();
        // «через два часа» подписка кончилась
        assert!(s.premium_until(license::now_secs() + 7200).is_none());
        assert!(s.premium_until(license::now_secs()).is_some());
        s.set_app_icon("violet").unwrap();
        // повтор того же ключа срок не продлевает
        assert!(s.activate(&key, license::now_secs()).unwrap_err().contains("не продлевает"));
        // ключ другого пользователя не подходит
        let mut other = Store::default();
        other.ensure_user_id();
        assert!(other.activate(&key, license::now_secs()).unwrap_err().contains("другого"));
        // продление новым ключом
        let longer = license::sign(&license::TEST_SECRET, &s.user_id, license::now_secs() + 9999).unwrap();
        s.activate(&longer, license::now_secs()).unwrap();
        assert!(s.premium_until(license::now_secs() + 7200).is_some());
    }
}
