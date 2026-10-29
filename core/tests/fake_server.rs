//! Интеграционные тесты: поддельный SA-MP сервер на локальном UDP-порту.

use plugjam_core::query::{query_info, query_many, query_players, query_rules};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::net::UdpSocket;

fn s32(s: &str) -> Vec<u8> {
    let mut v = (s.len() as u32).to_le_bytes().to_vec();
    v.extend_from_slice(s.as_bytes());
    v
}
fn s8(s: &str) -> Vec<u8> {
    let mut v = vec![s.len() as u8];
    v.extend_from_slice(s.as_bytes());
    v
}

/// Отвечает на 'i', 'r', 'c'. Перед настоящим ответом шлёт «мусорный» пакет —
/// клиент обязан его пропустить.
async fn spawn_server(drop_first: bool) -> String {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = sock.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        let mut first = drop_first;
        loop {
            let Ok((n, from)) = sock.recv_from(&mut buf).await else { return };
            if n < 11 || &buf[..4] != b"SAMP" {
                continue;
            }
            if first {
                first = false; // потеряли первый пакет — клиент должен повторить
                continue;
            }
            let _ = sock.send_to(b"SAMP\x01\x02\x03\x04\x01\x00z", from).await; // мусор
            let mut out = buf[..11].to_vec();
            match buf[10] {
                b'i' => {
                    out.push(0);
                    out.extend(7u16.to_le_bytes());
                    out.extend(50u16.to_le_bytes());
                    out.extend(s32("Test Server"));
                    out.extend(s32("RP"));
                    out.extend(s32("English"));
                }
                b'r' => {
                    out.extend(1u16.to_le_bytes());
                    out.extend(s8("weather"));
                    out.extend(s8("10"));
                }
                b'c' => {
                    out.extend(1u16.to_le_bytes());
                    out.extend(s8("Tina_Carter"));
                    out.extend(9i32.to_le_bytes());
                }
                _ => continue,
            }
            let _ = sock.send_to(&out, from).await;
        }
    });
    addr
}

const T: Duration = Duration::from_millis(500);

#[tokio::test]
async fn info_rules_players() {
    let a = spawn_server(false).await;
    let i = query_info(&a, T, 1).await.unwrap();
    assert_eq!(i.hostname, "Test Server");
    assert_eq!((i.players, i.max_players), (7, 50));
    assert_eq!(i.mode, "RP");
    assert!(i.online && i.ping.is_some());
    assert_eq!(i.address, a);

    let r = query_rules(&a, T, 1).await.unwrap();
    assert_eq!((r[0].name.as_str(), r[0].value.as_str()), ("weather", "10"));
    let p = query_players(&a, T, 1).await.unwrap();
    assert_eq!((p[0].name.as_str(), p[0].score), ("Tina_Carter", 9));
}

#[tokio::test]
async fn retry_after_lost_packet() {
    let a = spawn_server(true).await;
    assert!(query_info(&a, Duration::from_millis(300), 1).await.is_err());
    let a = spawn_server(true).await;
    let i = query_info(&a, Duration::from_millis(300), 2).await.unwrap();
    assert!(i.online);
}

#[tokio::test]
async fn silent_port_times_out_quickly() {
    let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap(); // слушает, но молчит
    let a = dead.local_addr().unwrap().to_string();
    let t = Instant::now();
    assert!(query_info(&a, Duration::from_millis(200), 2).await.is_err());
    assert!(t.elapsed() < Duration::from_millis(900));
}

#[tokio::test]
async fn many_servers_one_result_each() {
    let mut addrs = vec![];
    for _ in 0..12 {
        addrs.push(spawn_server(false).await);
    }
    addrs.push("127.0.0.1:1".to_string()); // никто не слушает
    addrs.push("not a host".to_string()); // некорректный адрес
    let got = Arc::new(Mutex::new(vec![]));
    let g = got.clone();
    let t = Instant::now();
    query_many(addrs.clone(), 4, move |i| g.lock().unwrap().push(i)).await;
    let got = got.lock().unwrap();
    assert_eq!(got.len(), addrs.len());
    assert_eq!(got.iter().filter(|i| i.online).count(), 12);
    for a in &addrs {
        assert_eq!(got.iter().filter(|i| &i.address == a).count(), 1, "{a}");
    }
    assert!(t.elapsed() < Duration::from_secs(4));
}
