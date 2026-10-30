//! Обложки проектов (ЭКСПЕРИМЕНТ). Источник — сообщество VK проекта.
//!
//! 1. Если при сборке задан сервисный ключ VK (`PLUGJAM_VK_TOKEN`), берём настоящую обложку через
//!    API `groups.getById?fields=cover` (самая широкая картинка).
//! 2. Иначе — `og:image` публичной страницы сообщества (обычно аватар: VK отдаёт обложку только через API).
//! Картинка скачивается, кладётся в кэш на сутки и отдаётся интерфейсу как data-URL.
//! Скачиваются только https-адреса с доменов VK, размер ограничен.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

/// Какому серверу какое сообщество соответствует. Пока только Gambit.
const SOURCES: &[(&str, &str)] = &[("gambit", "gambitrp")];
const MAX_BYTES: usize = 3 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(24 * 3600);
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124 Safari/537.36";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cover {
    pub slug: String,
    /// "vk-cover" — настоящая обложка, "vk-page" — картинка страницы (обычно аватар).
    pub source: String,
    pub data_url: String,
}

pub fn source_for(hostname: &str) -> Option<&'static str> {
    let h = hostname.to_lowercase();
    SOURCES.iter().find(|(needle, _)| h.contains(needle)).map(|(_, slug)| *slug)
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.')
}

pub fn allowed_image_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or("");
    if host.contains(':') || host.is_empty() {
        return false;
    }
    ["userapi.com", "vk.com", "vk.ru", "vk.me", "vkuserphoto.ru", "vk-cdn.net", "mycdn.me"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Ответ `groups.getById` (оба формата: старый — массив, новый — `groups`) → адрес самой широкой обложки.
pub fn parse_vk_cover(json: &str) -> Result<Option<String>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("Ответ VK: {e}"))?;
    if let Some(err) = v.get("error") {
        let msg = err.get("error_msg").and_then(|m| m.as_str()).unwrap_or("ошибка API");
        return Err(format!("VK: {msg}"));
    }
    let resp = v.get("response").ok_or("Ответ VK без response")?;
    let group = resp.get("groups").and_then(|g| g.get(0)).or_else(|| resp.get(0));
    let Some(cover) = group.and_then(|g| g.get("cover")) else { return Ok(None) };
    if cover.get("enabled").and_then(|e| e.as_i64()) == Some(0) {
        return Ok(None);
    }
    let best = cover
        .get("images")
        .and_then(|i| i.as_array())
        .into_iter()
        .flatten()
        .filter_map(|im| Some((im.get("width")?.as_u64()?, im.get("url")?.as_str()?)))
        .max_by_key(|(w, _)| *w);
    Ok(best.map(|(_, u)| u.to_string()))
}

/// Значение `og:image` из HTML страницы.
pub fn parse_og_image(html: &str) -> Option<String> {
    let at = html.find("og:image")?;
    let start = html[..at].rfind('<')?;
    let end = at + html[at..].find('>')?;
    let tag = &html[start..end];
    let c = tag.find("content=")? + "content=".len();
    let q = tag[c..].chars().next().filter(|q| *q == '"' || *q == '\'')?;
    let val = &tag[c + 1..];
    let url = &val[..val.find(q)?];
    Some(url.replace("&amp;", "&"))
}

fn mime_of(bytes: &[u8], header: Option<&str>) -> Option<&'static str> {
    match header.map(|h| h.split(';').next().unwrap_or("").trim()) {
        Some("image/jpeg") => return Some("image/jpeg"),
        Some("image/png") => return Some("image/png"),
        Some("image/webp") => return Some("image/webp"),
        _ => {}
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn read_cache(path: &Path) -> Option<Cover> {
    let age = std::fs::metadata(path).ok()?.modified().ok()?.elapsed().ok()?;
    if age > TTL {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

async fn get_bytes(url: &str, token: Option<&str>) -> Result<(Vec<u8>, Option<String>), String> {
    let mut req = crate::browser::client().get(url).header("User-Agent", BROWSER_UA);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let resp = req.send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).map(str::to_string);
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Файл слишком большой".into());
    }
    Ok((bytes.to_vec(), ct))
}

/// Обложка сообщества `slug`; результат кэшируется в `cache_dir` на сутки.
pub async fn fetch(slug: &str, token: Option<&str>, cache_dir: &Path) -> Result<Cover, String> {
    if !valid_slug(slug) {
        return Err("Некорректное имя сообщества".into());
    }
    let cache = cache_dir.join(format!("{slug}.json"));
    if let Some(c) = read_cache(&cache) {
        return Ok(c);
    }
    let (image_url, source) = match token.filter(|t| !t.is_empty()) {
        Some(t) => {
            let api = format!("https://api.vk.com/method/groups.getById?group_id={slug}&fields=cover&v=5.199");
            let (body, _) = get_bytes(&api, Some(t)).await?;
            match parse_vk_cover(&String::from_utf8_lossy(&body))? {
                Some(u) => (u, "vk-cover"),
                None => return Err("У сообщества нет обложки".into()),
            }
        }
        None => {
            let (body, _) = get_bytes(&format!("https://vk.com/{slug}"), None).await?;
            let u = parse_og_image(&String::from_utf8_lossy(&body)).ok_or("На странице нет картинки")?;
            (u, "vk-page")
        }
    };
    if !allowed_image_url(&image_url) {
        return Err("Адрес картинки не из VK".into());
    }
    let (bytes, ct) = get_bytes(&image_url, None).await?;
    let mime = mime_of(&bytes, ct.as_deref()).ok_or("Неизвестный формат картинки")?;
    let cover = Cover { slug: slug.into(), source: source.into(), data_url: format!("data:{mime};base64,{}", B64.encode(&bytes)) };
    let _ = std::fs::create_dir_all(cache_dir);
    let _ = std::fs::write(&cache, serde_json::to_string(&cover).unwrap_or_default());
    Ok(cover)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_gambit_only() {
        assert_eq!(source_for("Gambit Role Play | San Andreas"), Some("gambitrp"));
        assert_eq!(source_for("GAMBIT v4.7"), Some("gambitrp"));
        assert_eq!(source_for("Absolute Role Play"), None);
    }

    #[test]
    fn vk_cover_both_response_shapes() {
        let new = r#"{"response":{"groups":[{"id":1,"cover":{"enabled":1,"images":[
            {"url":"https://sun9-1.userapi.com/a.jpg","width":795,"height":265},
            {"url":"https://sun9-1.userapi.com/b.jpg","width":1590,"height":530}]}}]}}"#;
        assert_eq!(parse_vk_cover(new).unwrap().as_deref(), Some("https://sun9-1.userapi.com/b.jpg"));
        let old = r#"{"response":[{"id":1,"cover":{"enabled":1,"images":[{"url":"https://x.vk.com/c.jpg","width":100,"height":10}]}}]}"#;
        assert_eq!(parse_vk_cover(old).unwrap().as_deref(), Some("https://x.vk.com/c.jpg"));
        assert_eq!(parse_vk_cover(r#"{"response":{"groups":[{"id":1}]}}"#).unwrap(), None);
        assert_eq!(parse_vk_cover(r#"{"response":{"groups":[{"cover":{"enabled":0,"images":[]}}]}}"#).unwrap(), None);
        let e = parse_vk_cover(r#"{"error":{"error_code":5,"error_msg":"User authorization failed"}}"#).unwrap_err();
        assert!(e.contains("authorization"));
        assert!(parse_vk_cover("not json").is_err());
    }

    #[test]
    fn og_image_parsing() {
        let h = r#"<head><meta charset="utf-8"><meta property="og:image" content="https://sun1.userapi.com/x.jpg?a=1&amp;b=2" /></head>"#;
        assert_eq!(parse_og_image(h).as_deref(), Some("https://sun1.userapi.com/x.jpg?a=1&b=2"));
        let rev = r#"<meta content='https://vk.com/i.png' property='og:image'>"#;
        assert_eq!(parse_og_image(rev).as_deref(), Some("https://vk.com/i.png"));
        assert_eq!(parse_og_image("<html></html>"), None);
    }

    #[test]
    fn only_vk_https_images() {
        assert!(allowed_image_url("https://sun9-12.userapi.com/impg/x.jpg"));
        assert!(allowed_image_url("https://vk.com/a.jpg"));
        for bad in ["http://vk.com/a.jpg", "https://evil.com/a.jpg", "https://vk.com.evil.com/a.jpg", "https://evilvk.com/a", "https://user@evil.com/a", "https://vk.com:8080/a", "ftp://vk.com/a", "https:///a"] {
            assert!(!allowed_image_url(bad), "{bad}");
        }
    }

    #[test]
    fn mime_and_slug() {
        assert_eq!(mime_of(&[0xFF, 0xD8, 0xFF, 0xE0], None), Some("image/jpeg"));
        assert_eq!(mime_of(b"\x89PNG\r\n", Some("application/octet-stream")), Some("image/png"));
        assert_eq!(mime_of(b"<html>", Some("text/html")), None);
        assert!(valid_slug("gambitrp") && valid_slug("club_1.x"));
        assert!(!valid_slug("a/b") && !valid_slug("") && !valid_slug("a b"));
    }
}
