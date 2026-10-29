//! Запуск игры. Сейчас через штатный samp.exe из папки GTA: San Andreas
//! (`samp.exe -c -n Ник -h хост -p порт [-z пароль]`). Если позже появится собственный
//! загрузчик (шифрование, свои модели) — меняется только эта функция.

use crate::{query::parse_address, store::validate_nickname};
use std::path::Path;

pub fn launch(gta_path: &str, address: &str, nickname: &str, password: Option<&str>) -> Result<(), String> {
    validate_nickname(nickname)?;
    let (host, port) = parse_address(address)?;
    let dir = Path::new(gta_path.trim());
    if gta_path.trim().is_empty() {
        return Err("Не указана папка GTA San Andreas (⚙ Настройки)".into());
    }
    let exe = dir.join("samp.exe");
    if !exe.is_file() {
        return Err("В папке GTA нет samp.exe — проверьте путь в настройках".into());
    }
    run(&exe, dir, &host, port, nickname, password)
}

#[cfg(windows)]
fn run(exe: &Path, dir: &Path, host: &str, port: u16, nick: &str, pass: Option<&str>) -> Result<(), String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.current_dir(dir)
        .args(["-c", "-n", nick, "-h", host, "-p", &port.to_string()]);
    if let Some(p) = pass.filter(|p| !p.is_empty()) {
        cmd.args(["-z", p]);
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("Не удалось запустить игру: {e}"))
}

#[cfg(not(windows))]
fn run(_: &Path, _: &Path, _: &str, _: u16, _: &str, _: Option<&str>) -> Result<(), String> {
    Err("Запуск игры поддерживается только в Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_before_spawning() {
        assert!(launch("", "a.b:1", "Nick_1", None).unwrap_err().contains("папка"));
        assert!(launch("/definitely/missing", "a.b:1", "Nick_1", None).unwrap_err().contains("samp.exe"));
        assert!(launch("/x", "a.b:1", "no", None).is_err());
        assert!(launch("/x", "bad addr", "Nick_1", None).is_err());
    }
}
