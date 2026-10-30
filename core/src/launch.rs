//! Собственный запуск игры — без установленного SA-MP-лаунчера (samp.exe).
//!
//! Так же, как это делает samp.exe: запускаем `gta_sa.exe` приостановленным с аргументами
//! `-c -n Ник -h хост -p порт [-z пароль]`, загружаем в процесс клиентскую DLL
//! (`plugjam.dll`, а если её нет — `samp.dll` из папки игры) и возобновляем игру.
//! Папка игры находится автоматически: приложение установлено в неё или лежит рядом,
//! либо путь взят из реестра / типовых мест.

use crate::{query::parse_address, store::validate_nickname};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const GAME_EXE: &str = "gta_sa.exe";
/// Порядок поиска клиентской DLL.
pub const CLIENT_DLLS: [&str; 2] = ["plugjam.dll", "samp.dll"];
/// Размер gta_sa.exe версии 1.0 US — той, под которую сделан SA-MP 0.3.7.
const EXE_V100_SIZE: u64 = 14_383_616;

#[derive(Serialize, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameCheck {
    pub exe: bool,
    /// Имя найденной клиентской DLL.
    pub client_dll: Option<String>,
    /// None — версию определить не удалось.
    pub exe_v100: Option<bool>,
    /// false — разрядность лаунчера не совпадает с игрой (gta_sa.exe 32-битный).
    pub bitness_ok: Option<bool>,
}

pub fn check_game(path: &str) -> GameCheck {
    let dir = Path::new(path.trim());
    let exe = dir.join(GAME_EXE);
    GameCheck {
        exe: exe.is_file(),
        client_dll: CLIENT_DLLS.iter().find(|d| dir.join(d).is_file()).map(|d| d.to_string()),
        exe_v100: std::fs::metadata(&exe).ok().map(|m| m.len() == EXE_V100_SIZE),
        bitness_ok: pe_machine(&exe).map(|m| bitness_error(m, cfg!(target_pointer_width = "64")).is_none()),
    }
}

// ───────────────────────── разрядность ─────────────────────────
// gta_sa.exe — 32-битный. Чтобы загрузить библиотеку в его процесс, лаунчер должен быть тоже
// 32-битным: адрес LoadLibraryW берётся из kernel32 самого лаунчера, а у 64-битного он другой.

const MACHINE_X86: u16 = 0x014c;

/// Тип процессора из PE-заголовка (0x14c — x86, 0x8664 — x64).
pub fn pe_machine(path: &Path) -> Option<u16> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut dos = [0u8; 64];
    f.read_exact(&mut dos).ok()?;
    if &dos[..2] != b"MZ" {
        return None;
    }
    let off = u32::from_le_bytes([dos[0x3c], dos[0x3d], dos[0x3e], dos[0x3f]]) as u64;
    f.seek(SeekFrom::Start(off)).ok()?;
    let mut h = [0u8; 6];
    f.read_exact(&mut h).ok()?;
    (&h[..4] == b"PE\0\0").then(|| u16::from_le_bytes([h[4], h[5]]))
}

/// Сообщение об ошибке, если лаунчер нельзя использовать с такой игрой.
pub fn bitness_error(game_machine: u16, launcher_is_64bit: bool) -> Option<String> {
    (game_machine == MACHINE_X86 && launcher_is_64bit).then(|| {
        "Эта сборка plugjam.mp 64-битная, а gta_sa.exe 32-битный — библиотеку в игру так не загрузить. \
         Установите 32-битную сборку (x86)"
            .to_string()
    })
}

// ───────────────────────── поиск папки игры ─────────────────────────

fn is_game_dir(p: &Path) -> bool {
    p.join(GAME_EXE).is_file()
}

/// Первая папка из списка, где лежит gta_sa.exe.
pub fn detect_in(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| is_game_dir(p)).cloned()
}

/// Значение `gta_sa_exe` из вывода `reg query` → папка игры.
pub fn parse_reg_output(out: &str) -> Option<PathBuf> {
    let line = out.lines().find(|l| l.contains("gta_sa_exe"))?;
    let (_, value) = line.split_once("REG_SZ")?;
    // разбираем вручную: путь приходит в формате Windows при любой ОС (удобно для тестов)
    let (dir, _) = value.trim().rsplit_once(['\\', '/'])?;
    Some(PathBuf::from(dir))
}

#[cfg(windows)]
fn registry_candidate() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\SAMP", "/v", "gta_sa_exe"])
        .creation_flags(0x0800_0000) // без окна консоли
        .output()
        .ok()?;
    parse_reg_output(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(windows))]
fn registry_candidate() -> Option<PathBuf> {
    None
}

fn default_candidates() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = vec![];
    // 1. Приложение установлено в папку с игрой (или в её подпапку).
    if let Ok(exe) = std::env::current_exe() {
        for d in exe.ancestors().skip(1).take(2) {
            v.push(d.to_path_buf());
        }
    }
    // 2. Папка, которую запомнил обычный SA-MP.
    v.extend(registry_candidate());
    // 3. Типовые места установки.
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(pf) = std::env::var(var) {
            let pf = PathBuf::from(pf);
            v.push(pf.join(r"Rockstar Games\GTA San Andreas"));
            v.push(pf.join(r"Steam\steamapps\common\Grand Theft Auto San Andreas"));
        }
    }
    for drive in ["C", "D", "E"] {
        v.push(PathBuf::from(format!(r"{drive}:\Games\GTA San Andreas")));
        v.push(PathBuf::from(format!(r"{drive}:\GTA San Andreas")));
        v.push(PathBuf::from(format!(r"{drive}:\Games\GTA San Andreas SAMP")));
    }
    v
}

/// Ищет папку игры автоматически.
pub fn detect_gta() -> Option<String> {
    detect_in(&default_candidates()).map(|p| p.to_string_lossy().into_owned())
}

// ───────────────────────── запуск ─────────────────────────

/// Командная строка gta_sa.exe. Пробелы в пароле экранируются кавычками, кавычки запрещены.
pub fn build_cmdline(exe: &Path, nick: &str, host: &str, port: u16, password: Option<&str>) -> Result<String, String> {
    let mut s = format!("\"{}\" -c -n {} -h {} -p {}", exe.display(), nick, host, port);
    if let Some(p) = password.filter(|p| !p.is_empty()) {
        if p.chars().any(|c| c == '"' || c.is_control()) {
            return Err("В пароле недопустимы кавычки и управляющие символы".into());
        }
        if p.contains(' ') {
            s.push_str(&format!(" -z \"{p}\""));
        } else {
            s.push_str(&format!(" -z {p}"));
        }
    }
    Ok(s)
}

pub fn launch(gta_path: &str, address: &str, nickname: &str, password: Option<&str>) -> Result<(), String> {
    validate_nickname(nickname)?;
    let (host, port) = parse_address(address)?;
    if gta_path.trim().is_empty() {
        return Err("Не указана папка GTA San Andreas (⚙ Настройки → Обзор)".into());
    }
    let dir = Path::new(gta_path.trim());
    let exe = dir.join(GAME_EXE);
    if !exe.is_file() {
        return Err("В папке нет gta_sa.exe — проверьте путь в настройках".into());
    }
    let dll = CLIENT_DLLS
        .iter()
        .map(|d| dir.join(d))
        .find(|p| p.is_file())
        .ok_or("В папке игры нет клиентской библиотеки (plugjam.dll или samp.dll)")?;
    if let Some(m) = pe_machine(&exe) {
        if let Some(e) = bitness_error(m, cfg!(target_pointer_width = "64")) {
            return Err(e);
        }
    }
    let cmdline = build_cmdline(&exe, nickname, &host, port, password)?;
    run(&exe, dir, &dll, &cmdline)
}

#[cfg(not(windows))]
fn run(_: &Path, _: &Path, _: &Path, _: &str) -> Result<(), String> {
    Err("Запуск игры поддерживается только в Windows".into())
}

#[cfg(windows)]
fn run(exe: &Path, dir: &Path, dll: &Path, cmdline: &str) -> Result<(), String> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr::null, ptr::null_mut};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, FALSE},
        System::{
            Diagnostics::Debug::WriteProcessMemory,
            LibraryLoader::{GetModuleHandleW, GetProcAddress},
            Memory::{VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE},
            Threading::{
                CreateProcessW, CreateRemoteThread, GetExitCodeThread, ResumeThread, TerminateProcess,
                WaitForSingleObject, CREATE_SUSPENDED, PROCESS_INFORMATION, STARTUPINFOW,
            },
        },
    };

    let wide = |s: &std::ffi::OsStr| -> Vec<u16> { s.encode_wide().chain(Some(0)).collect() };
    let exe_w = wide(exe.as_os_str());
    let dir_w = wide(dir.as_os_str());
    let dll_w = wide(dll.as_os_str());
    let mut cmd_w = wide(std::ffi::OsStr::new(cmdline)); // CreateProcessW может менять буфер

    unsafe {
        let si: STARTUPINFOW = {
            let mut s: STARTUPINFOW = std::mem::zeroed();
            s.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            s
        };
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        if CreateProcessW(
            exe_w.as_ptr(),
            cmd_w.as_mut_ptr(),
            null(),
            null(),
            FALSE,
            CREATE_SUSPENDED,
            null(),
            dir_w.as_ptr(),
            &si,
            &mut pi,
        ) == 0
        {
            return Err(format!("Не удалось запустить gta_sa.exe (код {})", GetLastError()));
        }

        let fail = |msg: String, pi: &PROCESS_INFORMATION| -> Result<(), String> {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
            Err(msg)
        };

        // Путь к DLL записываем в память игры и вызываем в ней LoadLibraryW.
        let bytes = dll_w.len() * 2;
        let remote = VirtualAllocEx(pi.hProcess, null(), bytes, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if remote.is_null() {
            return fail(format!("Нет памяти в процессе игры (код {})", GetLastError()), &pi);
        }
        let mut written = 0usize;
        if WriteProcessMemory(pi.hProcess, remote, dll_w.as_ptr() as *const c_void, bytes, &mut written) == 0 {
            VirtualFreeEx(pi.hProcess, remote, 0, MEM_RELEASE);
            return fail(format!("Не удалось передать путь к библиотеке (код {})", GetLastError()), &pi);
        }
        let k32: Vec<u16> = "kernel32.dll\0".encode_utf16().collect();
        let load = GetProcAddress(GetModuleHandleW(k32.as_ptr()), b"LoadLibraryW\0".as_ptr());
        let Some(load) = load else {
            VirtualFreeEx(pi.hProcess, remote, 0, MEM_RELEASE);
            return fail("LoadLibraryW не найден".into(), &pi);
        };
        let start: unsafe extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(load);
        let th = CreateRemoteThread(pi.hProcess, null(), 0, Some(start), remote, 0, null_mut());
        if th.is_null() {
            VirtualFreeEx(pi.hProcess, remote, 0, MEM_RELEASE);
            return fail(format!("Не удалось загрузить клиентскую библиотеку (код {})", GetLastError()), &pi);
        }
        let waited = WaitForSingleObject(th, 10_000);
        let mut code = 0u32;
        GetExitCodeThread(th, &mut code); // для LoadLibraryW код — младшие 32 бита HMODULE; 0 = ошибка
        CloseHandle(th);
        if waited != 0 {
            // поток завис — память под путь не освобождаем, чтобы не оборвать его на полпути
            return fail("Клиентская библиотека не ответила за 10 секунд".into(), &pi);
        }
        VirtualFreeEx(pi.hProcess, remote, 0, MEM_RELEASE);
        if code == 0 {
            return fail(
                format!("Клиентская библиотека {} не загрузилась (LoadLibrary вернул 0): проверьте, что это SA-MP 0.3.7 для gta_sa.exe 1.0 US и рядом нет повреждённых файлов", dll.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                &pi,
            );
        }

        ResumeThread(pi.hThread);
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("plugjam-launch-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn validates_before_spawning() {
        assert!(launch("", "a.b:1", "Nick_1", None).unwrap_err().contains("папка"));
        assert!(launch("/definitely/missing", "a.b:1", "Nick_1", None).unwrap_err().contains("gta_sa.exe"));
        assert!(launch("/x", "a.b:1", "no", None).is_err());
        assert!(launch("/x", "bad addr", "Nick_1", None).is_err());
    }

    #[test]
    fn needs_client_dll() {
        let d = tmpdir("nodll");
        fs::write(d.join(GAME_EXE), b"x").unwrap();
        let e = launch(d.to_str().unwrap(), "a.b:1", "Nick_1", None).unwrap_err();
        assert!(e.contains("библиотеки"), "{e}");
    }

    #[test]
    fn game_check_and_detect() {
        let d = tmpdir("check");
        assert!(!check_game(d.to_str().unwrap()).exe);
        assert!(detect_in(&[d.clone()]).is_none());
        fs::write(d.join(GAME_EXE), b"x").unwrap();
        fs::write(d.join("samp.dll"), b"x").unwrap();
        let c = check_game(d.to_str().unwrap());
        assert!(c.exe);
        assert_eq!(c.client_dll.as_deref(), Some("samp.dll"));
        assert_eq!(c.exe_v100, Some(false));
        fs::write(d.join("plugjam.dll"), b"x").unwrap();
        assert_eq!(check_game(d.to_str().unwrap()).client_dll.as_deref(), Some("plugjam.dll")); // свой клиент в приоритете
        let other = tmpdir("empty");
        assert_eq!(detect_in(&[other, d.clone()]), Some(d));
    }

    #[test]
    fn registry_output_parsing() {
        let out = "\r\nHKEY_CURRENT_USER\\Software\\SAMP\r\n    gta_sa_exe    REG_SZ    C:\\Games\\GTA San Andreas\\gta_sa.exe\r\n\r\n";
        assert_eq!(parse_reg_output(out), Some(PathBuf::from(r"C:\Games\GTA San Andreas")));
        assert_eq!(parse_reg_output("ничего"), None);
    }

    #[test]
    fn pe_header_and_bitness() {
        let d = tmpdir("pe");
        let mut pe = vec![0u8; 0x100];
        pe[0] = b'M';
        pe[1] = b'Z';
        pe[0x3c] = 0x80; // e_lfanew
        pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        pe[0x84..0x86].copy_from_slice(&0x014cu16.to_le_bytes());
        fs::write(d.join("x86.exe"), &pe).unwrap();
        pe[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
        fs::write(d.join("x64.exe"), &pe).unwrap();
        fs::write(d.join("junk.exe"), b"not an exe").unwrap();
        assert_eq!(pe_machine(&d.join("x86.exe")), Some(0x014c));
        assert_eq!(pe_machine(&d.join("x64.exe")), Some(0x8664));
        assert_eq!(pe_machine(&d.join("junk.exe")), None);
        assert_eq!(pe_machine(&d.join("missing.exe")), None);
        // 32-битная игра + 64-битный лаунчер — ошибка; остальные сочетания — нормально
        assert!(bitness_error(0x014c, true).unwrap().contains("32-битную"));
        assert!(bitness_error(0x014c, false).is_none());
        assert!(bitness_error(0x8664, true).is_none());
    }

    #[test]
    fn cmdline() {
        let exe = Path::new("gta_sa.exe");
        assert_eq!(
            build_cmdline(exe, "Tina_Carter", "1.2.3.4", 7777, None).unwrap(),
            "\"gta_sa.exe\" -c -n Tina_Carter -h 1.2.3.4 -p 7777"
        );
        assert!(build_cmdline(exe, "N_1", "h", 1, Some("pa ss")).unwrap().ends_with("-z \"pa ss\""));
        assert!(build_cmdline(exe, "N_1", "h", 1, Some("pass")).unwrap().ends_with("-z pass"));
        assert!(!build_cmdline(exe, "N_1", "h", 1, Some("")).unwrap().contains("-z"));
        assert!(build_cmdline(exe, "N_1", "h", 1, Some("a\"b")).is_err());
    }
}
