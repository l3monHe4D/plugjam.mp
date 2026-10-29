//! Подписка «Plus» без сервера: лицензионный ключ — подпись Ed25519 над строкой «ID|срок».
//! Владелец проекта выдаёт ключ после оплаты (инструмент `license-tool`), приложение
//! проверяет подпись встроенным открытым ключом. Закрытый ключ в приложении не хранится.
//!
//! Формат ключа: `PJ1.<base64url(payload)>.<base64url(подпись)>`, payload = `PJ-XXXX-XXXX-XXXX|<unix-секунды>`.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// Открытый ключ владельца (hex). Заменяется на свой: `license-tool keygen`.
pub const PUBLIC_KEY_HEX: &str = "e84913ee1fa7f006618fa9f5959e134de2f7aacfc5d5e1de03384d0becd60efa";

const DOMAIN: &[u8] = b"plugjam-license-v1:";
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Уникальный ID установки: 60 бит из системного генератора случайных чисел ОС.
pub fn new_user_id() -> String {
    let mut b = [0u8; 12];
    if getrandom::getrandom(&mut b).is_err() {
        // ОС не дала случайность — крайне редкий случай; подмешиваем время и адрес стека
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(1);
        let x = &b as *const _ as usize as u128;
        let mut v = t ^ (x << 64) ^ x;
        for byte in b.iter_mut() {
            v = v.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            *byte = (v >> 56) as u8;
        }
    }
    let c: Vec<char> = b.iter().map(|x| ALPHABET[(*x & 31) as usize] as char).collect();
    format!(
        "PJ-{}-{}-{}",
        c[0..4].iter().collect::<String>(),
        c[4..8].iter().collect::<String>(),
        c[8..12].iter().collect::<String>()
    )
}

pub fn valid_user_id(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    p.len() == 4
        && p[0] == "PJ"
        && p[1..].iter().all(|g| g.len() == 4 && g.bytes().all(|c| ALPHABET.contains(&c)))
}

pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

pub fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Подписывает ключ для пользователя до момента `exp` (unix-секунды).
pub fn sign(secret: &[u8; 32], user_id: &str, exp: u64) -> Result<String, String> {
    if !valid_user_id(user_id) {
        return Err("Некорректный ID пользователя".into());
    }
    let payload = format!("{user_id}|{exp}");
    let sig = SigningKey::from_bytes(secret).sign(&[DOMAIN, payload.as_bytes()].concat());
    Ok(format!("PJ1.{}.{}", B64.encode(payload.as_bytes()), B64.encode(sig.to_bytes())))
}

/// Проверяет ключ открытым ключом `public`. Возвращает срок окончания (unix-секунды).
pub fn verify_with(public: &[u8; 32], key: &str, user_id: &str, now: u64) -> Result<u64, String> {
    let bad = || "Ключ не подходит".to_string();
    let key: String = key.split_whitespace().collect();
    let mut it = key.split('.');
    let (v, p, s) = (it.next(), it.next(), it.next());
    if v != Some("PJ1") || it.next().is_some() {
        return Err(bad());
    }
    let payload = B64.decode(p.ok_or_else(bad)?).map_err(|_| bad())?;
    let sig: [u8; 64] = B64.decode(s.ok_or_else(bad)?).map_err(|_| bad())?.try_into().map_err(|_| bad())?;
    VerifyingKey::from_bytes(public)
        .map_err(|_| bad())?
        .verify(&[DOMAIN, &payload].concat(), &Signature::from_bytes(&sig))
        .map_err(|_| bad())?;
    let text = String::from_utf8(payload).map_err(|_| bad())?;
    let (id, exp) = text.split_once('|').ok_or_else(bad)?;
    if id != user_id {
        return Err("Ключ выдан для другого ID".into());
    }
    let exp: u64 = exp.parse().map_err(|_| bad())?;
    if exp <= now {
        return Err("Срок ключа истёк".into());
    }
    Ok(exp)
}

#[cfg(not(test))]
fn public_key() -> Option<[u8; 32]> {
    hex_decode(PUBLIC_KEY_HEX).and_then(|v| v.try_into().ok())
}

/// В тестах подпись проверяется тестовым ключом, чтобы можно было проверять сценарии Plus.
#[cfg(test)]
pub const TEST_SECRET: [u8; 32] = [7; 32];
#[cfg(test)]
fn public_key() -> Option<[u8; 32]> {
    Some(SigningKey::from_bytes(&TEST_SECRET).verifying_key().to_bytes())
}

pub fn verify(key: &str, user_id: &str, now: u64) -> Result<u64, String> {
    let pk = public_key().ok_or("В приложении не задан открытый ключ")?;
    verify_with(&pk, key, user_id, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SK: [u8; 32] = [7; 32];
    fn pk() -> [u8; 32] {
        SigningKey::from_bytes(&SK).verifying_key().to_bytes()
    }
    const ID: &str = "PJ-ABCD-EFGH-JKMN";

    #[test]
    fn user_ids_are_random_and_valid() {
        let a = new_user_id();
        let b = new_user_id();
        assert!(valid_user_id(&a), "{a}");
        assert_ne!(a, b);
        assert!(!valid_user_id("PJ-ABCD-EFGH"));
        assert!(!valid_user_id("PJ-ABCD-EFGH-JKMI")); // I не из алфавита
    }

    #[test]
    fn sign_verify_roundtrip() {
        let k = sign(&SK, ID, 2_000).unwrap();
        assert_eq!(verify_with(&pk(), &k, ID, 1_000), Ok(2_000));
        // пробелы и переносы при вставке не мешают
        let spaced = format!("  {}\n{} ", &k[..20], &k[20..]);
        assert_eq!(verify_with(&pk(), &spaced, ID, 1_000), Ok(2_000));
    }

    #[test]
    fn rejects_wrong_user_expired_tampered() {
        let k = sign(&SK, ID, 2_000).unwrap();
        assert!(verify_with(&pk(), &k, "PJ-AAAA-AAAA-AAAA", 1_000).unwrap_err().contains("другого"));
        assert!(verify_with(&pk(), &k, ID, 2_000).unwrap_err().contains("истёк"));
        // подмена срока в payload
        let parts: Vec<&str> = k.split('.').collect();
        let forged = format!("PJ1.{}.{}", B64.encode(format!("{ID}|9999999999")), parts[2]);
        assert!(verify_with(&pk(), &forged, ID, 1_000).is_err());
        // ключ другого владельца
        let other = SigningKey::from_bytes(&[9; 32]).verifying_key().to_bytes();
        assert!(verify_with(&other, &k, ID, 1_000).is_err());
        for junk in ["", "PJ1", "PJ1..", "x.y.z", "PJ1.!!.!!"] {
            assert!(verify_with(&pk(), junk, ID, 1).is_err(), "{junk}");
        }
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(hex_decode(&hex_encode(&[0, 255, 16])).unwrap(), vec![0, 255, 16]);
        assert!(hex_decode("abc").is_none());
        assert!(hex_decode("zz").is_none());
    }
}
