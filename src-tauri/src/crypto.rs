use std::io::{Read, Write};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use crate::error::DshellError;
use crate::models::ConnectionProfile;

#[derive(Clone, Serialize, Deserialize)]
struct StoredConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth_method: String,
    pub group: Option<String>,
    pub encrypted_credential: Option<String>,
    pub sync_dir: bool,
}

// ── cross-platform encryption ──────────────────────────────────────────────

#[cfg(windows)]
mod platform_crypto {
    use std::ptr;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct DATA_BLOB {
        cbData: u32,
        pbData: *mut u8,
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x01;

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            pDataIn: *const DATA_BLOB,
            szDataDescr: *const u16,
            pOptionalEntropy: *const DATA_BLOB,
            pvReserved: *mut std::ffi::c_void,
            pPromptStruct: *const std::ffi::c_void,
            dwFlags: u32,
            pDataOut: *mut DATA_BLOB,
        ) -> i32;

        fn CryptUnprotectData(
            pDataIn: *const DATA_BLOB,
            ppszDataDescr: *mut *mut u16,
            pOptionalEntropy: *const DATA_BLOB,
            pvReserved: *mut std::ffi::c_void,
            pPromptStruct: *const std::ffi::c_void,
            dwFlags: u32,
            pDataOut: *mut DATA_BLOB,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    pub fn encrypt(plaintext: &str) -> Result<String, String> {
        let plain_bytes = plaintext.as_bytes();
        let in_blob = DATA_BLOB {
            cbData: plain_bytes.len() as u32,
            pbData: plain_bytes.as_ptr() as *mut u8,
        };
        let mut out_blob = DATA_BLOB { cbData: 0, pbData: ptr::null_mut() };

        let ret = unsafe {
            CryptProtectData(
                &in_blob as *const DATA_BLOB,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob as *mut DATA_BLOB,
            )
        };

        if ret == 0 {
            return Err("DPAPI 加密失败".to_string());
        }

        let encrypted = unsafe {
            let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
            let result = slice.to_vec();
            LocalFree(out_blob.pbData as *mut std::ffi::c_void);
            result
        };

        Ok(super::base64_encode(&encrypted))
    }

    pub fn decrypt(encoded: &str) -> Result<String, String> {
        let encrypted = super::base64_decode(encoded)?;
        let in_blob = DATA_BLOB {
            cbData: encrypted.len() as u32,
            pbData: encrypted.as_ptr() as *mut u8,
        };
        let mut out_blob = DATA_BLOB { cbData: 0, pbData: ptr::null_mut() };

        let ret = unsafe {
            CryptUnprotectData(
                &in_blob as *const DATA_BLOB,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob as *mut DATA_BLOB,
            )
        };

        if ret == 0 {
            return Err("DPAPI 解密失败".to_string());
        }

        let decrypted = unsafe {
            let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
            let result = String::from_utf8(slice.to_vec())
                .map_err(|e| format!("解密结果不是有效 UTF-8: {}", e));
            LocalFree(out_blob.pbData as *mut std::ffi::c_void);
            result
        };

        decrypted
    }
}

#[cfg(not(windows))]
mod platform_crypto {
    pub fn encrypt(plaintext: &str) -> Result<String, String> {
        let bytes: Vec<u8> = plaintext.bytes().map(|b| b ^ 0x5A).collect();
        Ok(super::base64_encode(&bytes))
    }

    pub fn decrypt(encoded: &str) -> Result<String, String> {
        let decoded = super::base64_decode(encoded)?;
        let bytes: Vec<u8> = decoded.iter().map(|b| b ^ 0x5A).collect();
        String::from_utf8(bytes).map_err(|e| format!("解密失败: {}", e))
    }
}

fn deobfuscate_xor(data: &str) -> Result<String, String> {
    let decoded = base64_decode(data)?;
    let bytes: Vec<u8> = decoded.iter().map(|b| b ^ 0x5A).collect();
    String::from_utf8(bytes).map_err(|e| format!("解密失败: {}", e))
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("Base64 解码失败: {}", e))
}

// ── config file helpers ────────────────────────────────────────────────────

fn config_path() -> Result<PathBuf, String> {
    let dir = directories::ProjectDirs::from("com", "kai", "kaiterm")
        .ok_or("无法获取配置目录")?;
    let data_dir = dir.data_dir();
    std::fs::create_dir_all(data_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
    Ok(data_dir.join("connections.json"))
}

fn read_connections(path: &PathBuf) -> Result<Vec<StoredConnection>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file = std::fs::File::open(path).map_err(|e| format!("打开配置文件失败: {}", e))?;
    let mut content = String::new();
    file.read_to_string(&mut content).map_err(|e| format!("读取配置文件失败: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("解析配置文件失败: {}", e))
}

fn write_connections(path: &PathBuf, connections: &[StoredConnection]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(connections)
        .map_err(|e| format!("序列化失败: {}", e))?;
    let mut file = std::fs::File::create(path).map_err(|e| format!("写入配置文件失败: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入配置文件失败: {}", e))
}

// ── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn save_connection(
    name: String,
    host: String,
    port: u16,
    user: String,
    auth_method: String,
    credential: String,
    group: Option<String>,
    sync_dir: bool,
    id: Option<String>,
) -> Result<ConnectionProfile, DshellError> {
    let path = config_path()?;
    let mut connections = read_connections(&path)?;

    let encrypted = if !credential.is_empty() {
        Some(platform_crypto::encrypt(&credential)?)
    } else {
        None
    };

    let conn_id = match id {
        Some(ref eid) if connections.iter().any(|c| c.id == *eid) => eid.clone(),
        _ => uuid::Uuid::new_v4().to_string(),
    };

    let entry = StoredConnection {
        id: conn_id.clone(),
        name: name.clone(),
        host: host.clone(),
        port,
        user: user.clone(),
        auth_method: auth_method.clone(),
        group: group.clone(),
        encrypted_credential: encrypted.or_else(|| {
            connections.iter().find(|c| c.id == conn_id)
                .and_then(|c| c.encrypted_credential.clone())
        }),
        sync_dir,
    };

    if let Some(pos) = connections.iter().position(|c| c.id == conn_id) {
        connections[pos] = entry;
    } else {
        let key = format!("{}|{}|{}", host, port, user);
        if let Some(pos) = connections.iter().position(|c| format!("{}|{}|{}", c.host, c.port, c.user) == key) {
            connections[pos] = entry;
        } else {
            connections.push(entry);
        }
    }

    write_connections(&path, &connections)?;

    Ok(ConnectionProfile {
        id: conn_id,
        name,
        host,
        port,
        user,
        auth_method,
        group,
        encrypted_credential: None,
        sync_dir,
    })
}

#[tauri::command]
pub async fn duplicate_connection(
    profile_id: String,
) -> Result<ConnectionProfile, DshellError> {
    let path = config_path()?;
    let connections = read_connections(&path)?;

    let src = connections
        .iter()
        .find(|c| c.id == profile_id)
        .ok_or_else(|| DshellError::Msg("连接不存在".into()))?
        .clone();

    let new_id = uuid::Uuid::new_v4().to_string();
    let new_entry = StoredConnection {
        id: new_id.clone(),
        name: format!("{} (副本)", &src.name),
        host: src.host.clone(),
        port: src.port,
        user: src.user.clone(),
        auth_method: src.auth_method.clone(),
        group: src.group.clone(),
        encrypted_credential: src.encrypted_credential,
        sync_dir: src.sync_dir,
    };

    let mut all = connections;
    all.push(new_entry);
    write_connections(&path, &all)?;

    Ok(ConnectionProfile {
        id: new_id,
        name: format!("{} (副本)", &src.name),
        host: src.host,
        port: src.port,
        user: src.user,
        auth_method: src.auth_method,
        group: src.group,
        encrypted_credential: None,
        sync_dir: src.sync_dir,
    })
}

#[tauri::command]
pub async fn load_connections() -> Result<Vec<ConnectionProfile>, DshellError> {
    let stored = read_connections(&config_path()?)?;

    let mut profiles: Vec<ConnectionProfile> = stored
        .into_iter()
        .map(|s| ConnectionProfile {
            id: s.id,
            name: s.name,
            host: s.host,
            port: s.port,
            user: s.user,
            auth_method: s.auth_method,
            group: s.group,
            encrypted_credential: None,
            sync_dir: s.sync_dir,
        })
        .collect();

    profiles.sort_by(|a, b| {
        match (&a.group, &b.group) {
            (Some(ga), Some(gb)) => ga.cmp(gb).then(a.name.cmp(&b.name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });

    Ok(profiles)
}

#[tauri::command]
pub async fn get_credential(profile_id: String) -> Result<String, DshellError> {
    let stored = read_connections(&config_path()?)?;
    let conn = stored
        .iter()
        .find(|s| s.id == profile_id)
        .ok_or(DshellError::SessionNotFound(profile_id.clone()))?;

    match &conn.encrypted_credential {
        Some(enc) => Ok(platform_crypto::decrypt(enc).or_else(|_| deobfuscate_xor(enc))?),
        None => Ok(String::new()),
    }
}

#[tauri::command]
pub async fn delete_connection(profile_id: String) -> Result<(), DshellError> {
    let path = config_path()?;
    let mut stored = read_connections(&path)?;
    stored.retain(|s| s.id != profile_id);
    write_connections(&path, &stored)?;
    Ok(())
}

#[tauri::command]
pub async fn rename_group(old_name: String, new_name: String) -> Result<(), DshellError> {
    let path = config_path()?;
    let mut stored = read_connections(&path)?;
    for conn in &mut stored {
        if conn.group.as_deref() == Some(old_name.as_str()) {
            conn.group = Some(new_name.clone());
        }
    }
    write_connections(&path, &stored)?;
    Ok(())
}

fn groups_path() -> Result<PathBuf, String> {
    let dir = directories::ProjectDirs::from("com", "kai", "kaiterm")
        .ok_or("无法获取配置目录")?;
    let data_dir = dir.data_dir();
    std::fs::create_dir_all(data_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
    Ok(data_dir.join("groups.json"))
}

fn read_groups(path: &PathBuf) -> Result<Vec<String>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file = std::fs::File::open(path).map_err(|e| format!("打开分组文件失败: {}", e))?;
    let mut content = String::new();
    file.read_to_string(&mut content).map_err(|e| format!("读取分组文件失败: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("解析分组文件失败: {}", e))
}

fn write_groups(path: &PathBuf, groups: &[String]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(groups)
        .map_err(|e| format!("序列化分组失败: {}", e))?;
    let mut file = std::fs::File::create(path).map_err(|e| format!("写入分组文件失败: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入分组文件失败: {}", e))
}

#[tauri::command]
pub async fn load_groups() -> Result<Vec<String>, DshellError> {
    let path = groups_path().map_err(|e| DshellError::SessionNotFound(e))?;
    Ok(read_groups(&path).map_err(|e| DshellError::SessionNotFound(e))?)
}

#[tauri::command]
pub async fn save_groups(groups: Vec<String>) -> Result<(), DshellError> {
    let path = groups_path().map_err(|e| DshellError::SessionNotFound(e))?;
    write_groups(&path, &groups).map_err(|e| DshellError::SessionNotFound(e))?;
    Ok(())
}
