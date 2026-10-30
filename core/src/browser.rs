//! Список Internet: общий индекс серверов open.mp (https://api.open.mp/servers).
//! Поля ответа: ip, hn (имя), pc (игроков), pm (слотов), gm (мод), la (язык), pa (пароль), vn (версия).

use crate::model::ServerInfo;
use serde::Deserialize;
use std::{sync::OnceLock, time::Duration};

const URL: &str = "https://api.open.mp/servers";

#[derive(Deserialize, Default)]
#[serde(default)]
struct Raw {
    ip: String,
    hn: String,
    pc: u32,
    pm: u32,
    gm: String,
    la: String,
    pa: bool,
    vn: String,
}

/// Преобразует JSON-массив индекса в список серверов. Мусорные записи отбрасываются.
pub fn parse_list(json: &str) -> Result<Vec<ServerInfo>, String> {
    let raw: Vec<Raw> = serde_json::from_str(json).map_err(|e| format!("Ответ индекса: {e}"))?;
    Ok(raw
        .into_iter()
        .filter(|r| !r.ip.is_empty())
        .map(|r| ServerInfo {
            address: r.ip.trim().to_ascii_lowercase(),
            hostname: r.hn.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string(),
            players: r.pc,
            max_players: r.pm,
            ping: None,
            mode: r.gm,
            language: r.la,
            password: r.pa,
            online: true,
            version: r.vn,
        })
        .collect())
}

pub fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("plugjam.mp/0.1")
            .timeout(Duration::from_secs(12))
            .build()
            .expect("http client")
    })
}

pub async fn fetch_internet() -> Result<Vec<ServerInfo>, String> {
    let text = client()
        .get(URL)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Не удалось получить список серверов: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    parse_list(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_index() {
        let j = r#"[
          {"ip":"1.2.3.4:7777","hn":"A\n","pc":10,"pm":100,"gm":"RP","la":"Russian","pa":true,"vn":"0.3.7-R2"},
          {"ip":"","hn":"no address"},
          {"ip":"HOST.com:7778","hn":"B"}
        ]"#;
        let l = parse_list(j).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].hostname, "A");
        assert!(l[0].password);
        assert_eq!(l[1].address, "host.com:7778");
        assert_eq!(l[1].players, 0);
        assert!(parse_list("{}").is_err());
    }
}
