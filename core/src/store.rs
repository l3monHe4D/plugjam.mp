//! Локальное хранилище: ник, путь к игре, папки и избранные серверы.
//! Один JSON-файл, запись атомарная (временный файл + rename).

use crate::query::normalize_address;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

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
}
