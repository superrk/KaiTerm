use std::collections::HashMap;
use crate::error::DshellError;
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Emitter;
use uuid::Uuid;
use encoding_rs::Encoding;
use portable_pty::{CommandBuilder, native_pty_system, PtySize};
use crate::models::{LocalFileInfo, ShellInfo};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

struct LocalShell {
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    writer: Mutex<Box<dyn Write + Send>>,
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
    pid: Option<u32>,
}

pub struct LocalManager {
    shells: Arc<Mutex<HashMap<String, LocalShell>>>,
}

impl LocalManager {
    pub fn new() -> Self {
        Self {
            shells: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn system_decoder() -> &'static Encoding {
    #[cfg(windows)]
    {
        extern "system" {
            fn GetOEMCP() -> u32;
        }
        let cp = unsafe { GetOEMCP() };
        return match cp {
            936 => encoding_rs::GBK,
            932 => encoding_rs::SHIFT_JIS,
            949 => encoding_rs::EUC_KR,
            950 => encoding_rs::BIG5,
            1250 => encoding_rs::WINDOWS_1250,
            1251 => encoding_rs::WINDOWS_1251,
            1252 => encoding_rs::WINDOWS_1252,
            1253 => encoding_rs::WINDOWS_1253,
            1254 => encoding_rs::WINDOWS_1254,
            1255 => encoding_rs::WINDOWS_1255,
            1256 => encoding_rs::WINDOWS_1256,
            1257 => encoding_rs::WINDOWS_1257,
            1258 => encoding_rs::WINDOWS_1258,
            _ => encoding_rs::UTF_8,
        };
    }
    #[cfg(not(windows))]
    encoding_rs::UTF_8
}

fn decode_output(data: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(data) {
        return s.to_string();
    }
    let (decoded, _) = system_decoder().decode_without_bom_handling(data);
    decoded.into_owned()
}

#[cfg(windows)]
fn resolve_shell(shell_type: &str) -> (String, Vec<String>) {
    match shell_type {
        "cmd" => {
            let path = std::env::var("COMSPEC").unwrap_or_else(|_| "C:\\Windows\\System32\\cmd.exe".to_string());
            (path, vec![])
        }
        "pwsh" | "powershell" => {
            let path = if shell_type == "pwsh" {
                let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
                let mut p = "pwsh.exe".to_string();
                for c in &[
                    "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
                    "C:\\Program Files\\PowerShell\\7-preview\\pwsh.exe",
                ] {
                    if std::path::Path::new(c).exists() { p = c.to_string(); break; }
                }
                if !std::path::Path::new(&p).exists() {
                    let app = format!("{}\\Microsoft\\WindowsApps\\pwsh.exe", local);
                    if std::path::Path::new(&app).exists() { p = app; }
                }
                p
            } else {
                "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe".to_string()
            };
            (path, vec!["-NoLogo".into()])
        }
        s if s.starts_with("wsl:") => {
            let distro = s.trim_start_matches("wsl:");
            ("C:\\Windows\\System32\\wsl.exe".into(), vec!["-d".into(), distro.into()])
        }
        "gitbash" => {
            ("C:\\Program Files\\Git\\bin\\bash.exe".into(), vec!["--login".into(), "-i".into()])
        }
        "msys2" => {
            ("C:\\msys64\\usr\\bin\\bash.exe".into(), vec!["--login".into(), "-i".into()])
        }
        _ => {
            ("C:\\Windows\\System32\\wsl.exe".into(), vec![])
        }
    }
}

#[cfg(not(windows))]
fn resolve_shell(shell_type: &str) -> (String, Vec<String>) {
    match shell_type {
        "bash" => ("/bin/bash".into(), vec!["--login".into()]),
        "zsh" => ("/bin/zsh".into(), vec!["--login".into()]),
        "fish" => ("/usr/bin/fish".into(), vec!["--login".into()]),
        _ => {
            let fallback = if std::path::Path::new("/bin/bash").exists() {
                "/bin/bash"
            } else {
                "/bin/sh"
            };
            (fallback.into(), vec![])
        }
    }
}

#[cfg(windows)]
fn detect_shells_inner() -> Vec<ShellInfo> {
    let mut shells: Vec<ShellInfo> = Vec::new();

    let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "C:\\Windows\\System32\\cmd.exe".to_string());
    if std::path::Path::new(&comspec).exists() {
        shells.push(ShellInfo {
            id: "cmd".into(),
            name: "CMD".into(),
            path: comspec,
            args: vec![],
        });
    }

    let ps5_path = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
    if std::path::Path::new(ps5_path).exists() {
        shells.push(ShellInfo {
            id: "powershell".into(),
            name: "PowerShell 5".into(),
            path: ps5_path.into(),
            args: vec!["-NoLogo".into()],
        });
    }

    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    for c in &[
        "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
        "C:\\Program Files\\PowerShell\\7-preview\\pwsh.exe",
    ] {
        if std::path::Path::new(c).exists() {
            shells.push(ShellInfo {
                id: "pwsh".into(),
                name: if c.contains("7-preview") { "PowerShell 7 (Preview)".into() } else { "PowerShell 7".into() },
                path: c.to_string(),
                args: vec!["-NoLogo".into()],
            });
            break;
        }
    }
    if !shells.iter().any(|s| s.id == "pwsh") {
        let app = format!("{}\\Microsoft\\WindowsApps\\pwsh.exe", local);
        if std::path::Path::new(&app).exists() {
            shells.push(ShellInfo {
                id: "pwsh".into(),
                name: "PowerShell 7".into(),
                path: app,
                args: vec!["-NoLogo".into()],
            });
        }
    }

    for p in &[
        "C:\\Program Files\\Git\\bin\\bash.exe",
        "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
    ] {
        if std::path::Path::new(p).exists() {
            shells.push(ShellInfo {
                id: "gitbash".into(),
                name: "Git Bash".into(),
                path: p.to_string(),
                args: vec!["--login".into(), "-i".into()],
            });
            break;
        }
    }

    let msys2_path = "C:\\msys64\\usr\\bin\\bash.exe";
    if std::path::Path::new(msys2_path).exists() {
        shells.push(ShellInfo {
            id: "msys2".into(),
            name: "MSYS2".into(),
            path: msys2_path.into(),
            args: vec!["--login".into(), "-i".into()],
        });
    }

    if let Ok(output) = std::process::Command::new("wsl.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(&["--list", "--quiet"])
        .output()
    {
        if output.status.success() {
            let decoded = String::from_utf8_lossy(&output.stdout);
            for line in decoded.lines() {
                let distro = line.trim();
                if distro.is_empty() || distro.eq_ignore_ascii_case("docker-desktop") || distro.eq_ignore_ascii_case("docker-desktop-data") {
                    continue;
                }
                shells.push(ShellInfo {
                    id: format!("wsl:{}", distro),
                    name: format!("WSL: {}", distro),
                    path: "C:\\Windows\\System32\\wsl.exe".into(),
                    args: vec!["-d".into(), distro.into()],
                });
            }
        }
    }

    shells
}

#[cfg(not(windows))]
fn detect_shells_inner() -> Vec<ShellInfo> {
    let mut shells: Vec<ShellInfo> = Vec::new();
    let candidates = [
        ("bash", "/bin/bash"),
        ("zsh", "/bin/zsh"),
        ("fish", "/usr/bin/fish"),
        ("sh", "/bin/sh"),
    ];
    for (id, path) in &candidates {
        if std::path::Path::new(path).exists() {
            shells.push(ShellInfo {
                id: id.to_string(),
                name: id.to_string(),
                path: path.to_string(),
                args: vec![],
            });
        }
    }
    shells
}

#[tauri::command]
pub async fn detect_shells() -> Result<Vec<ShellInfo>, DshellError> {
    Ok(detect_shells_inner())
}

#[tauri::command]
pub async fn start_local_shell(
    shell_type: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, LocalManager>,
) -> Result<String, DshellError> {
    let id = Uuid::new_v4().to_string();
    let (shell_path, shell_args) = resolve_shell(&shell_type);

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 40,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    }).map_err(|e| format!("创建PTY失败: {}", e))?;

    let mut cmd = CommandBuilder::new(&shell_path);
    for arg in &shell_args {
        cmd.arg(arg);
    }
    let child = pair.slave.spawn_command(cmd)
        .map_err(|e| format!("启动 {} 失败: {}", shell_type, e))?;

    let pid = child.process_id();

    let reader = pair.master.try_clone_reader()
        .map_err(|e| format!("获取PTY读取器失败: {}", e))?;
    let writer = pair.master.take_writer()
        .map_err(|e| format!("获取PTY写入器失败: {}", e))?;

    let child = Arc::new(Mutex::new(child));

    let sid = id.clone();
    let app_out = app.clone();
    let _ = app_out.emit("terminal-started", sid.clone());
    tokio::task::spawn_blocking(move || {
        let mut buf = vec![0u8; 65536];
        let mut reader = reader;
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = decode_output(&buf[..n]);
                    let _ = app_out.emit("terminal-output", serde_json::json!({
                        "session_id": sid,
                        "data": text
                    }));
                }
            }
        }
    });

    let child_waiter = child.clone();
    let sid_watch = id.clone();
    let app_watch = app.clone();
    tokio::task::spawn_blocking(move || {
        let mut c = child_waiter.blocking_lock();
        let _ = c.wait();
        let _ = app_watch.emit("terminal-closed", &sid_watch);
    });

    let shell = LocalShell {
        child,
        writer: Mutex::new(writer),
        master: Some(pair.master),
        pid,
    };

    state.shells.lock().await.insert(id.clone(), shell);

    Ok(id)
}

#[tauri::command]
pub async fn write_local_stdin(
    session_id: String,
    data: String,
    state: tauri::State<'_, LocalManager>,
) -> Result<(), DshellError> {
    let shells = state.shells.lock().await;
    let shell = shells.get(&session_id).ok_or("本地Shell未找到")?;
    let mut writer = shell.writer.lock().await;
    writer.write_all(data.as_bytes()).map_err(|e| format!("写入失败: {}", e))?;
    writer.flush().map_err(|e| format!("刷新失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn stop_local_shell(
    session_id: String,
    state: tauri::State<'_, LocalManager>,
) -> Result<(), DshellError> {
    let shell = {
        let mut shells = state.shells.lock().await;
        shells.remove(&session_id)
    };
    if let Some(shell) = shell {
        drop(shell.master);
        drop(shell.writer);

        #[cfg(windows)]
        if let Some(pid) = shell.pid {
            let _ = std::process::Command::new("taskkill")
                .creation_flags(CREATE_NO_WINDOW)
                .args(&["/PID", &pid.to_string(), "/F"])
                .spawn()
                .and_then(|mut c| c.wait());
        }

        #[cfg(unix)]
        if let Some(pid) = shell.pid {
            let _ = std::process::Command::new("kill")
                .args(&["-9", &pid.to_string()])
                .spawn()
                .and_then(|mut c| c.wait());
        }

        drop(shell.child);
    }
    Ok(())
}

#[tauri::command]
pub async fn resize_local_terminal(
    session_id: String,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, LocalManager>,
) -> Result<(), DshellError> {
    let shells = state.shells.lock().await;
    let shell = shells.get(&session_id).ok_or("本地Shell未找到")?;
    if let Some(ref master) = shell.master {
        master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| format!("调整大小失败: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_local_files(path: String) -> Result<Vec<LocalFileInfo>, DshellError> {
    let dir = std::path::Path::new(&path);
    if !dir.exists() {
        return Err(DshellError::Msg("路径不存在".to_string()));
    }

    let mut files = Vec::new();
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| format!("读取目录失败: {}", e))?;

    loop {
        let entry = entries.next_entry().await.map_err(|e| format!("读取目录项失败: {}", e))?;
        match entry {
            Some(entry) => {
                let metadata = entry.metadata().await.map_err(|e| format!("读取元数据失败: {}", e))?;
                files.push(LocalFileInfo {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: entry.path().to_string_lossy().to_string(),
                    size: if metadata.is_dir() { 0 } else { metadata.len() as i64 },
                    modified: metadata
                        .modified()
                        .ok()
                        .map(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs().to_string())
                                .unwrap_or_default()
                        })
                        .unwrap_or_default(),
                    is_dir: metadata.is_dir(),
                });
            }
            None => break,
        }
    }

    files.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.cmp(&b.name)
        }
    });

    Ok(files)
}

#[tauri::command]
pub async fn open_in_explorer(path: String) -> Result<(), DshellError> {
    #[cfg(windows)]
    let status = std::process::Command::new("explorer")
        .arg(path.replace("/", "\\"))
        .spawn()
        .map_err(|e| format!("打开资源管理器失败: {}", e));

    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("打开 Finder 失败: {}", e));

    #[cfg(target_os = "linux")]
    let status = std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("打开文件管理器失败: {}", e));

    let _ = status?;
    Ok(())
}
