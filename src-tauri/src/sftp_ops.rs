use crate::models::SftpFileInfo;
use crate::error::DshellError;
use crate::ssh_session::{SessionManager, SshHandler};
use std::sync::Arc;
use tokio::sync::Mutex;
use russh_sftp::client::RawSftpSession;
use russh_sftp::protocol::{OpenFlags, FileAttributes};

/// 为传输任务创建独立的 SFTP 通道（长操作不阻塞快速操作）
pub async fn new_sftp_channel(
    handle: &Arc<Mutex<russh::client::Handle<SshHandler>>>,
) -> Result<russh_sftp::client::SftpSession, String> {
    let h = handle.lock().await;
    let channel = h
        .channel_open_session()
        .await
        .map_err(|e| format!("打开SFTP通道失败: {}", e))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| format!("SFTP子系统启动失败: {}", e))?;
    let stream = channel.into_stream();
    russh_sftp::client::SftpSession::new(stream)
        .await
        .map_err(|e| format!("SFTP会话初始化失败: {}", e))
}

/// 创建独立的 RawSftpSession（用于并发读取 / 文件编辑等）
pub async fn new_raw_sftp_session(
    handle: &Arc<Mutex<russh::client::Handle<SshHandler>>>,
) -> Result<Arc<RawSftpSession>, String> {
    let h = handle.lock().await;
    let channel = h
        .channel_open_session()
        .await
        .map_err(|e| format!("打开SFTP通道失败: {}", e))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| format!("SFTP子系统启动失败: {}", e))?;
    let stream = channel.into_stream();
    let raw = RawSftpSession::new(stream);
    raw.init().await.map_err(|e| format!("SFTP初始化失败: {}", e))?;
    Ok(Arc::new(raw))
}

#[tauri::command]
pub async fn sftp_read_file(
    session_id: String,
    path: String,
    state: tauri::State<'_, SessionManager>,
) -> Result<String, DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or("会话未找到")?;
    let raw_sftp = new_raw_sftp_session(&session.handle).await?;

    // Get file size first so we know exactly how much to read
    let attrs = raw_sftp
        .stat(&path)
        .await
        .map_err(|e| format!("获取文件属性失败: {}", e))?;
    let file_size = attrs.attrs.len();

    let handle_pkt = raw_sftp
        .open(&path, OpenFlags::READ, FileAttributes::default())
        .await
        .map_err(|e| format!("打开文件失败: {}", e))?;
    let fh = &handle_pkt.handle;

    let mut content = Vec::with_capacity(file_size as usize);
    let mut offset: u64 = 0;
    let mut read_result: Result<(), DshellError> = Ok(());
    while offset < file_size {
        let len = std::cmp::min(65536u64, file_size - offset) as u32;
        match raw_sftp.read(fh, offset, len).await {
            Ok(data) => {
                if data.data.is_empty() {
                    break;
                }
                content.extend_from_slice(&data.data);
                offset += data.data.len() as u64;
            }
            Err(e) => {
                read_result = Err(DshellError::Msg(format!("读取文件失败: {}", e)));
                break;
            }
        }
    }

    // 无论读取成功或失败，都必须关闭远端文件句柄，避免 SFTP 资源泄漏
    if let Err(e) = raw_sftp.close(fh.as_str()).await {
        if read_result.is_ok() {
            read_result = Err(DshellError::Msg(format!("关闭文件失败: {}", e)));
        }
    }

    read_result?;
    String::from_utf8(content).map_err(|e| DshellError::Msg(format!("不是合法文件内容UTF-8: {}", e)))
}

#[tauri::command]
pub async fn sftp_write_file(
    session_id: String,
    path: String,
    content: String,
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or("会话未找到")?;
    let raw_sftp = new_raw_sftp_session(&session.handle).await?;

    let flags = OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE;
    let handle_pkt = raw_sftp
        .open(&path, flags, FileAttributes::default())
        .await
        .map_err(|e| format!("打开文件失败: {}", e))?;
    let fh = &handle_pkt.handle;

    let data = content.as_bytes();
    let mut offset: u64 = 0;
    while offset < data.len() as u64 {
        let end = std::cmp::min(data.len(), offset as usize + 65536);
        raw_sftp
            .write(fh.as_str(), offset, data[offset as usize..end].to_vec())
            .await
            .map_err(|e| format!("写入文件失败: {}", e))?;
        offset = end as u64;
    }

    raw_sftp
        .close(fh.as_str())
        .await
        .map_err(|e| format!("关闭文件失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn sftp_list_dir(
    session_id: String,
    path: String,
    state: tauri::State<'_, SessionManager>,
) -> Result<Vec<SftpFileInfo>, DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or("会话未找到")?;

    let sftp_lock = session.sftp.lock().await;
    let sftp = sftp_lock.as_ref().ok_or("SFTP未初始化，请确认连接支持SFTP")?;
    let entries = sftp
        .read_dir(path.clone())
        .await
        .map_err(|e| format!("读取目录失败: {}", e))?;

    let mut files: Vec<SftpFileInfo> = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." {
            continue;
        }

        let file_path = if path.ends_with('/') {
            format!("{}{}", path, name)
        } else {
            format!("{}/{}", path, name)
        };

        let meta = entry.metadata();
        let is_dir = meta.is_dir();
        let size = meta.len() as i64;
        let perms = format!("{}", meta.permissions());
        let modified = meta.mtime
            .map(|t| format!("{}", t))
            .unwrap_or_default();

        files.push(SftpFileInfo {
            name,
            path: file_path,
            size,
            modified,
            is_dir,
            permissions: perms,
        });
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
pub async fn sftp_mkdir(
    session_id: String,
    path: String,
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or("会话未找到")?;

    let sftp_lock = session.sftp.lock().await;
    let sftp = sftp_lock.as_ref().ok_or(DshellError::SftpNotInitialized)?;
    sftp.create_dir(path).await.map_err(|e| DshellError::Msg(format!("创建目录失败: {}", e)))
}

#[tauri::command]
pub async fn sftp_rename(
    session_id: String,
    old_path: String,
    new_path: String,
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or("会话未找到")?;

    let sftp_lock = session.sftp.lock().await;
    let sftp = sftp_lock.as_ref().ok_or(DshellError::SftpNotInitialized)?;
    sftp.rename(old_path, new_path).await.map_err(|e| DshellError::Msg(format!("重命名失败: {}", e)))
}
#[tauri::command]
pub async fn sftp_remove(
    session_id: String,
    path: String,
    is_dir: bool,
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or("会话未找到")?;

    let sftp_lock = session.sftp.lock().await;
    let sftp = sftp_lock.as_ref().ok_or(DshellError::SftpNotInitialized)?;
    if is_dir {
        sftp.remove_dir(path).await.map_err(|e| DshellError::Msg(format!("删除目录失败: {}", e)))
    } else {
        sftp.remove_file(path).await.map_err(|e| DshellError::Msg(format!("删除文件失败: {}", e)))
    }
}


