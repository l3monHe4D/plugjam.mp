//! Протокол запросов SA-MP / open.mp (UDP, порт сервера).
//!
//! Пакет запроса: "SAMP" + 4 байта IP + 2 байта порта (little endian) + код запроса.
//! Коды: 'i' — информация, 'r' — правила, 'c' — игроки. Ответ повторяет эти 11 байт,
//! дальше идёт полезная нагрузка. Пинг считаем по времени ответа на запрос.

use crate::model::{Player, Rule, ServerInfo};
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{net::UdpSocket, sync::Semaphore, task::JoinSet, time::timeout};

pub const DEFAULT_PORT: u16 = 7777;
const HEADER: usize = 11;

// ───────────────────────── адреса ─────────────────────────

/// Разбирает "host", "host:port" или "samp://host:port". Порт по умолчанию — 7777.
pub fn parse_address(input: &str) -> Result<(String, u16), String> {
    let s = input.trim();
    let s = s.strip_prefix("samp://").unwrap_or(s).trim_end_matches('/');
    if s.is_empty() || s.len() > 255 {
        return Err("Пустой или слишком длинный адрес".into());
    }
    let (host, port) = match s.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "Некорректный порт")?),
        None => (s, DEFAULT_PORT),
    };
    if port == 0 {
        return Err("Некорректный порт".into());
    }
    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err("Некорректный адрес".into());
    }
    Ok((host.to_ascii_lowercase(), port))
}

/// Единый вид адреса для хранилища: "host:port" в нижнем регистре.
pub fn normalize_address(input: &str) -> Result<String, String> {
    let (h, p) = parse_address(input)?;
    Ok(format!("{h}:{p}"))
}

// ───────────────────────── разбор пакетов ─────────────────────────

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Reader { b, i: 0 }
    }
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.i.checked_add(n)?;
        let s = self.b.get(self.i..end)?;
        self.i = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        let b = self.bytes(2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        let b = self.bytes(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(self.u32()? as i32)
    }
    fn str_u8(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        Some(decode(self.bytes(n)?))
    }
    fn str_u32(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        Some(decode(self.bytes(n)?))
    }
}

/// Серверы шлют строки в разных кодировках: UTF-8 (open.mp) или ANSI. Если это не UTF-8,
/// читаем как Windows-1251 (самая частая для русскоязычных серверов).
fn decode(b: &[u8]) -> String {
    let s = match std::str::from_utf8(b) {
        Ok(s) => s.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1251.decode(b).0.into_owned(),
    };
    s.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string()
}

pub fn parse_info(payload: &[u8]) -> Option<ServerInfo> {
    let mut r = Reader::new(payload);
    let password = r.u8()? != 0;
    let players = r.u16()? as u32;
    let max_players = r.u16()? as u32;
    let hostname = r.str_u32()?;
    let mode = r.str_u32()?;
    let language = r.str_u32().unwrap_or_default();
    Some(ServerInfo {
        hostname,
        players,
        max_players,
        mode,
        language,
        password,
        ..Default::default()
    })
}

/// Обрезанный пакет не ошибка: возвращаем то, что успели прочитать.
pub fn parse_rules(payload: &[u8]) -> Option<Vec<Rule>> {
    let mut r = Reader::new(payload);
    let count = r.u16()?;
    let mut out = Vec::new();
    for _ in 0..count {
        match (r.str_u8(), r.str_u8()) {
            (Some(name), Some(value)) => out.push(Rule { name, value }),
            _ => break,
        }
    }
    Some(out)
}

pub fn parse_players(payload: &[u8]) -> Option<Vec<Player>> {
    let mut r = Reader::new(payload);
    let count = r.u16()?;
    let mut out = Vec::new();
    for _ in 0..count {
        match (r.str_u8(), r.i32()) {
            (Some(name), Some(score)) => out.push(Player { name, score }),
            _ => break,
        }
    }
    Some(out)
}

// ───────────────────────── сеть ─────────────────────────

fn build_packet(ip: Ipv4Addr, port: u16, op: u8) -> [u8; HEADER] {
    let mut p = [0u8; HEADER];
    p[..4].copy_from_slice(b"SAMP");
    p[4..8].copy_from_slice(&ip.octets());
    p[8] = (port & 0xff) as u8;
    p[9] = (port >> 8) as u8;
    p[10] = op;
    p
}

async fn resolve(host: &str, port: u16) -> Result<SocketAddrV4, String> {
    let mut it = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("DNS: {e}"))?;
    it.find_map(|a| match a {
        SocketAddr::V4(v) => Some(v),
        _ => None,
    })
    .ok_or_else(|| "У хоста нет IPv4-адреса".to_string())
}

/// Один запрос с повторами. Возвращает нагрузку ответа и время ответа в мс.
async fn exchange(addr: &str, op: u8, per_try: Duration, tries: u32) -> Result<(Vec<u8>, u32), String> {
    let (host, port) = parse_address(addr)?;
    let target = resolve(&host, port).await?;
    let req = build_packet(*target.ip(), port, op);
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;
    sock.connect(target).await.map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 8192];

    for _ in 0..tries.max(1) {
        let t0 = Instant::now();
        sock.send(&req).await.map_err(|e| e.to_string())?;
        let deadline = t0 + per_try;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match timeout(left, sock.recv(&mut buf)).await {
                Ok(Ok(n)) => {
                    // чужие и устаревшие пакеты пропускаем
                    if n >= HEADER && buf[..10] == req[..10] && buf[10] == op {
                        return Ok((buf[HEADER..n].to_vec(), t0.elapsed().as_millis() as u32));
                    }
                }
                _ => break,
            }
        }
    }
    Err("Нет ответа".into())
}

pub async fn query_info(addr: &str, per_try: Duration, tries: u32) -> Result<ServerInfo, String> {
    let (payload, ping) = exchange(addr, b'i', per_try, tries).await?;
    let mut info = parse_info(&payload).ok_or("Некорректный ответ сервера")?;
    info.address = addr.to_string();
    info.ping = Some(ping);
    info.online = true;
    Ok(info)
}

pub async fn query_rules(addr: &str, per_try: Duration, tries: u32) -> Result<Vec<Rule>, String> {
    let (payload, _) = exchange(addr, b'r', per_try, tries).await?;
    parse_rules(&payload).ok_or_else(|| "Некорректный ответ сервера".to_string())
}

pub async fn query_players(addr: &str, per_try: Duration, tries: u32) -> Result<Vec<Player>, String> {
    let (payload, _) = exchange(addr, b'c', per_try, tries).await?;
    parse_players(&payload).ok_or_else(|| "Некорректный ответ сервера".to_string())
}

/// Опрашивает список серверов с ограничением параллелизма. Для каждого адреса
/// ровно один вызов `on_result` (при неудаче — `ServerInfo::offline`).
pub async fn query_many<F>(addrs: Vec<String>, limit: usize, on_result: F)
where
    F: Fn(ServerInfo) + Send + Sync + 'static,
{
    let sem = Arc::new(Semaphore::new(limit.max(1)));
    let cb = Arc::new(on_result);
    let mut set = JoinSet::new();
    for a in addrs {
        let (sem, cb) = (sem.clone(), cb.clone());
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.ok();
            let info = query_info(&a, Duration::from_millis(900), 2)
                .await
                .unwrap_or_else(|_| ServerInfo::offline(&a));
            cb(info);
        });
    }
    while set.join_next().await.is_some() {}
}

// ───────────────────────── тесты ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn s32(s: &[u8]) -> Vec<u8> {
        let mut v = (s.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(s);
        v
    }
    fn s8(s: &[u8]) -> Vec<u8> {
        let mut v = vec![s.len() as u8];
        v.extend_from_slice(s);
        v
    }

    #[test]
    fn address_parsing() {
        assert_eq!(parse_address("1.2.3.4:7778").unwrap(), ("1.2.3.4".into(), 7778));
        assert_eq!(parse_address("Example.COM").unwrap(), ("example.com".into(), 7777));
        assert_eq!(parse_address("samp://a.b:1/").unwrap(), ("a.b".into(), 1));
        assert_eq!(normalize_address(" HOST:7777 ").unwrap(), "host:7777");
        for bad in ["", ":7777", "a:0", "a:70000", "a b:7777", "ht/tp:1", "[::1]:7777"] {
            assert!(parse_address(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn packet_layout() {
        let p = build_packet(Ipv4Addr::new(185, 71, 66, 21), 7775, b'i');
        assert_eq!(&p[..4], b"SAMP");
        assert_eq!(&p[4..8], &[185, 71, 66, 21]);
        assert_eq!(u16::from_le_bytes([p[8], p[9]]), 7775);
        assert_eq!(p[10], b'i');
    }

    #[test]
    fn info_parsing_utf8_and_cp1251() {
        let mut p = vec![1u8];
        p.extend(123u16.to_le_bytes());
        p.extend(1000u16.to_le_bytes());
        p.extend(s32("Сервер".as_bytes()));
        // "Мод" в Windows-1251
        p.extend(s32(&[0xCC, 0xEE, 0xE4]));
        p.extend(s32(b"Russian"));
        let i = parse_info(&p).unwrap();
        assert!(i.password);
        assert_eq!((i.players, i.max_players), (123, 1000));
        assert_eq!(i.hostname, "Сервер");
        assert_eq!(i.mode, "Мод");
        assert_eq!(i.language, "Russian");
    }

    #[test]
    fn info_without_language_and_truncated() {
        let mut p = vec![0u8];
        p.extend(1u16.to_le_bytes());
        p.extend(2u16.to_le_bytes());
        p.extend(s32(b"H"));
        p.extend(s32(b"M"));
        assert_eq!(parse_info(&p).unwrap().language, "");
        for cut in 0..p.len() - 4 {
            // любая обрезка до конца режима — None, без паники
            if cut < 5 + 5 + 1 {
                assert!(parse_info(&p[..cut]).is_none(), "cut {cut}");
            }
        }
        // длина строки больше данных
        let mut bad = vec![0u8, 0, 0, 0, 0];
        bad.extend(u32::MAX.to_le_bytes());
        assert!(parse_info(&bad).is_none());
    }

    #[test]
    fn rules_and_players() {
        let mut r = 2u16.to_le_bytes().to_vec();
        r.extend(s8(b"weather"));
        r.extend(s8(b"10"));
        r.extend(s8(b"worldtime"));
        r.extend(s8(b"10:00"));
        let rules = parse_rules(&r).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[1].value, "10:00");
        // заявлено 5 правил, пришло 2 — возвращаем 2
        let mut r2 = 5u16.to_le_bytes().to_vec();
        r2.extend(s8(b"a"));
        r2.extend(s8(b"b"));
        assert_eq!(parse_rules(&r2).unwrap().len(), 1);

        let mut c = 2u16.to_le_bytes().to_vec();
        c.extend(s8(b"Tina_Carter"));
        c.extend((-5i32).to_le_bytes());
        c.extend(s8(b"Bob"));
        c.extend(42i32.to_le_bytes());
        let pl = parse_players(&c).unwrap();
        assert_eq!(pl[0], Player { name: "Tina_Carter".into(), score: -5 });
        assert_eq!(pl[1].score, 42);
        assert!(parse_players(&[1]).is_none());
    }

    #[test]
    fn control_chars_are_stripped() {
        assert_eq!(decode(b"a\nb\0 "), "ab");
    }
}
