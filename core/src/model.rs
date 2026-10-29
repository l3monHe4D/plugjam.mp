use serde::{Deserialize, Serialize};

/// Сведения о сервере. Один и тот же тип для UDP-запроса и для списка Internet.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// "host:port" в том виде, в каком адрес передал интерфейс (ключ для сопоставления).
    pub address: String,
    pub hostname: String,
    pub players: u32,
    pub max_players: u32,
    /// Задержка в мс; None, если сервер не ответил.
    pub ping: Option<u32>,
    pub mode: String,
    pub language: String,
    pub password: bool,
    pub online: bool,
    pub version: String,
}

impl ServerInfo {
    /// Результат для сервера, который не ответил. Имя пустое: интерфейс оставит прежнее.
    pub fn offline(address: &str) -> Self {
        ServerInfo {
            address: address.to_string(),
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Player {
    pub name: String,
    pub score: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rule {
    pub name: String,
    pub value: String,
}
