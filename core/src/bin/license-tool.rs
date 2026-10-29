//! Инструмент владельца проекта (в приложение не входит в виде интерфейса).
//!   license-tool keygen                       — новая пара ключей
//!   license-tool sign <секретный-hex> <ID> [дней=30] — ключ для пользователя
//! Секретный ключ хранить только у себя и никогда не публиковать.

use plugjam_core::license::{hex_decode, hex_encode, now_secs, sign};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("keygen") => {
            let mut sk = [0u8; 32];
            getrandom::getrandom(&mut sk).expect("системный генератор случайных чисел");
            let pk = ed25519_dalek::SigningKey::from_bytes(&sk).verifying_key();
            println!("SECRET (хранить в тайне): {}", hex_encode(&sk));
            println!("PUBLIC (вписать в core/src/license.rs, PUBLIC_KEY_HEX): {}", hex_encode(pk.as_bytes()));
        }
        Some("sign") if a.len() >= 3 => {
            let sk: [u8; 32] = hex_decode(&a[1]).and_then(|v| v.try_into().ok()).unwrap_or_else(|| die("секретный ключ: 64 hex-символа"));
            let days: u64 = a.get(3).map(|d| d.parse().unwrap_or_else(|_| die("дни — число"))).unwrap_or(30);
            let exp = now_secs() + days * 86_400;
            match sign(&sk, a[2].trim(), exp) {
                Ok(k) => println!("{k}"),
                Err(e) => die(&e),
            }
        }
        _ => die("использование: license-tool keygen | license-tool sign <секретный-hex> <ID> [дней]"),
    }
}

fn die(m: &str) -> ! {
    eprintln!("{m}");
    std::process::exit(2)
}
