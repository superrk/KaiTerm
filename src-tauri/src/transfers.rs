use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::cmp::min;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::oneshot;
use std::collections::HashMap;
use tauri::Emitter;
use uuid::Uuid;
use std::path::Path;
use tokio::io::AsyncReadExt;
use crate::ssh_session::SessionManager;
use crate::ssh_session::SshHandler;
use crate::sftp_ops::{new_sftp_channel, new_raw_sftp_session};
use crate::error::DshellError;
use russh_sftp::protocol::{OpenFlags, FileAttributes};

/// 文件冲突处理时的用户决策
#[derive(Debug, Clone, Copy)]
pub enum ConflictResolution {
    Overwrite,
    Skip,
    Rename,
}

/// 待前端回复的冲突确认请求：token -> 决策发送端
type ConflictMap = Arc<AsyncMutex<HashMap<String, oneshot::Sender<ConflictResolution>>>>;

pub struct TransferManager {
    pub transfers: Arc<AsyncMutex<HashMap<String, Arc<AtomicBool>>>>,
    /// 会话级"全部应用"覆盖：session_id -> 已选定的决策
    pub conflict_overrides: Arc<AsyncMutex<HashMap<String, ConflictResolution>>>,
    pub pending_conflicts: ConflictMap,
}

impl TransferManager {
    pub fn new() -> Self {
        Self {
            transfers: Arc::new(AsyncMutex::new(HashMap::new())),
            conflict_overrides: Arc::new(AsyncMutex::new(HashMap::new())),
            pending_conflicts: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }
}

/// 若目标已存在，向用户确认冲突处理方式。
/// 返回最终应使用的目标路径；若用户选择跳过，返回 None。
async fn resolve_conflict(
    app: &tauri::AppHandle,
    conflict_overrides: Arc<AsyncMutex<HashMap<String, ConflictResolution>>>,
    pending_conflicts: Arc<AsyncMutex<HashMap<String, oneshot::Sender<ConflictResolution>>>>,
    session_id: &str,
    target: &str,
    exists: bool,
) -> Result<Option<String>, String> {
    if !exists {
        return Ok(Some(target.to_string()));
    }

    // 会话级"全部应用"优先
    {
        let ov = conflict_overrides.lock().await;
        if let Some(res) = ov.get(session_id) {
            return apply_resolution(res, target);
        }
    }

    let token = Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel::<ConflictResolution>();
    {
        let mut pending = pending_conflicts.lock().await;
        pending.insert(token.clone(), tx);
    }

    let _ = app.emit(
        "transfer-conflict",
        serde_json::json!({
            "token": token,
            "session_id": session_id,
            "target": target,
        }),
    );

    // 等待前端回复（5 分钟超时兜底，避免任务永久挂起）
    let resolution = tokio::time::timeout(std::time::Duration::from_secs(300), rx)
        .await
        .map_err(|_| "冲突确认超时（5分钟），已取消传输".to_string())?
        .map_err(|_| "冲突确认被取消".to_string())?;

    // 若选择"全部应用"，记录到会话级覆盖
    {
        let mut ov = conflict_overrides.lock().await;
        if matches!(resolution, ConflictResolution::Overwrite)
            || matches!(resolution, ConflictResolution::Skip)
        {
            ov.insert(session_id.to_string(), resolution);
        }
    }

    apply_resolution(&resolution, target)
}

fn apply_resolution(res: &ConflictResolution, target: &str) -> Result<Option<String>, String> {
    match res {
        ConflictResolution::Overwrite => Ok(Some(target.to_string())),
        ConflictResolution::Skip => Ok(None),
        ConflictResolution::Rename => Ok(Some(unique_name(target))),
    }
}

/// 在文件名（含扩展名）前插入 _N，找到首个不冲突的名字
fn unique_name(target: &str) -> String {
    let path = Path::new(target);
    let parent = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let base = if parent.is_empty() {
        format!("{}{}", stem, ext)
    } else {
        format!("{}/{}{}", parent, stem, ext)
    };
    let mut n = 1;
    loop {
        let cand = if parent.is_empty() {
            format!("{}_{}{}", stem, n, ext)
        } else {
            format!("{}/{}_{}{}", parent, stem, n, ext)
        };
        if !Path::new(&cand).exists() {
            return cand;
        }
        n += 1;
        if n > 9999 {
            return base;
        }
    }
}

#[tauri::command]
pub async fn upload_file(
    session_id: String,
    local_path: String,
    remote_path: String,
    state: tauri::State<'_, SessionManager>,
    transfer_state: tauri::State<'_, TransferManager>,
    app: tauri::AppHandle,
) -> Result<String, DshellError> {
    let tid = Uuid::new_v4().to_string();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut transfers = transfer_state.transfers.lock().await;
        transfers.insert(tid.clone(), cancel_flag.clone());
    }

    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or("会话未找到")?;
    let handle = session.handle.clone();
    drop(sessions);

    let tid_clone = tid.clone();
    let ov_arc = transfer_state.inner().conflict_overrides.clone();
    let pend_arc = transfer_state.inner().pending_conflicts.clone();

    tokio::spawn(async move {
        match transfer_upload(handle, local_path.clone(), remote_path.clone(), tid_clone.clone(), cancel_flag, app.clone(), ov_arc, pend_arc, session_id.clone()).await {
            Ok(()) => {
                let _ = app.emit("transfer-complete", serde_json::json!({
                    "id": tid_clone, "status": "completed"
                }));
            }
            Err(e) => {
                if e == "cancelled" {
                    let _ = app.emit("transfer-cancelled", serde_json::json!({
                        "id": tid_clone, "status": "cancelled"
                    }));
                } else {
                    let _ = app.emit("transfer-error", serde_json::json!({
                        "id": tid_clone, "error": e
                    }));
                }
            }
        }
    });

    Ok(tid)
}

async fn transfer_upload(
    handle: Arc<AsyncMutex<russh::client::Handle<SshHandler>>>,
    local_path: String,
    remote_path: String,
    tid: String,
    cancel_flag: Arc<AtomicBool>,
    app: tauri::AppHandle,
    conflict_overrides: Arc<AsyncMutex<HashMap<String, ConflictResolution>>>,
    pending_conflicts: Arc<AsyncMutex<HashMap<String, oneshot::Sender<ConflictResolution>>>>,
    session_id: String,
) -> Result<(), String> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    // 先取文件大小（用于进度与 >1GB 大文件流式读取，避免整文件读入内存）
    let meta = tokio::fs::metadata(&local_path)
        .await
        .map_err(|e| format!("读取本地文件失败: {}", e))?;
    let total = meta.len();

    if cancel_flag.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let sftp = new_sftp_channel(&handle).await?;

    // 文件冲突处理：目标远程文件已存在时向用户确认
    // （stat 仅 RawSftpSession 提供，故单独开一个原始通道做存在性探测）
    let remote_exists = {
        match new_raw_sftp_session(&handle).await {
            Ok(raw) => {
                let exists = raw.stat(remote_path.to_string()).await.is_ok();
                exists
            }
            Err(_) => false,
        }
    };
    let final_remote = match resolve_conflict(
        &app,
        conflict_overrides,
        pending_conflicts,
        &session_id,
        remote_path.as_str(),
        remote_exists,
    )
    .await?
    {
        Some(p) => p,
        None => {
            // 用户选择跳过
            let _ = app.emit(
                "transfer-complete",
                serde_json::json!({ "id": tid, "status": "skipped" }),
            );
            return Ok(());
        }
    };
    let remote_parent = Path::new(&final_remote).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or("/".to_string());
    let _ = sftp.create_dir(remote_parent.clone()).await;

    if cancel_flag.load(Ordering::Relaxed) {
        // 清理可能已创建的远端空目录
        let _ = sftp.remove_dir(remote_parent).await;
        return Err("cancelled".to_string());
    }

    let mut file = sftp
        .create(final_remote.to_string())
        .await
        .map_err(|e| format!("创建远程文件失败: {}", e))?;

    // 流式读取本地文件，按块上传：内存占用恒定（与文件大小无关），
    // 因此 >1GB 大文件也不会撑爆内存；任意失败/取消都删除半截的远端文件。
    let mut local_file = tokio::fs::File::open(&local_path)
        .await
        .map_err(|e| format!("打开本地文件失败: {}", e))?;
    let mut buffer = vec![0u8; 32768];
    let mut transferred: u64 = 0;
    let start = std::time::Instant::now();
    let fname = Path::new(&final_remote).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut progress_epoch: u64 = 0;

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = sftp.remove_file(final_remote.to_string()).await;
            return Err("cancelled".to_string());
        }
        let n = match local_file.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = sftp.remove_file(final_remote.to_string()).await;
                return Err(format!("读取本地文件失败: {}", e));
            }
        };
        use tokio::io::AsyncWriteExt;
        if let Err(e) = file.write_all(&buffer[..n]).await {
            let _ = sftp.remove_file(final_remote.to_string()).await;
            return Err(format!("写入远程文件失败: {}", e));
        }
        transferred += n as u64;
        let pct = if total > 0 { transferred * 200 / total } else { 0 };
        if pct > progress_epoch {
            progress_epoch = pct;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 { transferred as f64 / elapsed } else { 0.0 };
            let _ = app.emit("transfer-progress", serde_json::json!({
                "id": tid, "filename": fname, "total": total,
                "transferred": transferred, "speed": speed, "status": "uploading"
            }));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn download_file(
    session_id: String,
    remote_path: String,
    local_path: String,
    state: tauri::State<'_, SessionManager>,
    transfer_state: tauri::State<'_, TransferManager>,
    app: tauri::AppHandle,
) -> Result<String, DshellError> {
    let tid = Uuid::new_v4().to_string();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut transfers = transfer_state.transfers.lock().await;
        transfers.insert(tid.clone(), cancel_flag.clone());
    }

    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or("会话未找到")?;
    let handle = session.handle.clone();
    drop(sessions);

    let tid_clone = tid.clone();
    let ov_arc = transfer_state.inner().conflict_overrides.clone();
    let pend_arc = transfer_state.inner().pending_conflicts.clone();

    tokio::spawn(async move {
        match transfer_download(handle, remote_path.clone(), local_path.clone(), tid_clone.clone(), cancel_flag, app.clone(), ov_arc, pend_arc, session_id.clone()).await {
            Ok(()) => {
                let _ = app.emit("transfer-complete", serde_json::json!({
                    "id": tid_clone, "status": "completed"
                }));
            }
            Err(e) => {
                if e == "cancelled" {
                    let _ = app.emit("transfer-cancelled", serde_json::json!({
                        "id": tid_clone, "status": "cancelled"
                    }));
                } else {
                    let _ = app.emit("transfer-error", serde_json::json!({
                        "id": tid_clone, "error": e
                    }));
                }
            }
        }
    });

    Ok(tid)
}

async fn transfer_download(
    handle: Arc<AsyncMutex<russh::client::Handle<SshHandler>>>,
    remote_path: String,
    local_path: String,
    tid: String,
    cancel_flag: Arc<AtomicBool>,
    app: tauri::AppHandle,
    conflict_overrides: Arc<AsyncMutex<HashMap<String, ConflictResolution>>>,
    pending_conflicts: Arc<AsyncMutex<HashMap<String, oneshot::Sender<ConflictResolution>>>>,
    session_id: String,
) -> Result<(), String> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let raw_sftp = new_raw_sftp_session(&handle).await?;

    // Get file size via stat
    let meta = raw_sftp
        .stat(remote_path.to_string())
        .await
        .map_err(|e| format!("获取文件信息失败: {}", e))?;
    let total = meta.attrs.len();

    if cancel_flag.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    // Open file read-only to get the server handle
    let handle_pkt = raw_sftp
        .open(remote_path.to_string(), OpenFlags::READ, FileAttributes::default())
        .await
        .map_err(|e| format!("打开远程文件失败: {}", e))?;
    let file_handle = handle_pkt.handle;
    let close_handle = file_handle.clone();

    let local_parent = Path::new(local_path.as_str()).parent().map(|p| p.to_path_buf()).unwrap_or_else(|| Path::new(".").to_path_buf());
    tokio::fs::create_dir_all(&local_parent)
        .await
        .map_err(|e| format!("创建本地目录失败: {}", e))?;

    // 文件冲突处理：本地目标已存在时向用户确认
    let local_exists = Path::new(local_path.as_str()).exists();
    let final_local = match resolve_conflict(
        &app,
        conflict_overrides,
        pending_conflicts,
        &session_id,
        local_path.as_str(),
        local_exists,
    )
    .await?
    {
        Some(p) => p,
        None => {
            // 用户选择跳过
            let _ = app.emit(
                "transfer-complete",
                serde_json::json!({ "id": tid, "status": "skipped" }),
            );
            return Ok(());
        }
    };

    let mut local_file = tokio::fs::File::create(&final_local)
        .await
        .map_err(|e| format!("创建本地文件失败: {}", e))?;

    // Pre-allocate so we can write chunks at arbitrary offsets
    local_file
        .set_len(total)
        .await
        .map_err(|e| format!("预分配文件失败: {}", e))?;

    let fname = Path::new(remote_path.as_str()).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

    // --- Concurrent segmented download ---
    let num_workers: u64 = 4;
    let read_chunk: u32 = 262_144; // 256KB per read call
    let segment_size = if total == 0 { 0 } else { (total + num_workers - 1) / num_workers };

    let (result_tx, mut result_rx) = tokio::sync::mpsc::channel::<(u64, Vec<u8>)>(16);
    let received = Arc::new(AtomicU64::new(0));

    for i in 0..num_workers {
        let offset = i * segment_size;
        if offset >= total { break; }
        let end = min(offset + segment_size, total);
        let rs = raw_sftp.clone();
        let fh = file_handle.clone();
        let cf = cancel_flag.clone();
        let tx = result_tx.clone();
        let recv = received.clone();

        tokio::spawn(async move {
            let mut pos = offset;
            while pos < end {
                if cf.load(Ordering::Relaxed) { return; }
                let len = min(read_chunk as u64, end - pos) as u32;
                match rs.read(&fh, pos, len).await {
                    Ok(data) => {
                        if data.data.is_empty() { break; }
                        let n = data.data.len() as u64;
                        recv.fetch_add(n, Ordering::Relaxed);
                        if tx.send((pos, data.data)).await.is_err() { return; }
                        pos += n;
                    }
                    Err(_e) => {
                        let _ = tx.send((pos, vec![])).await;
                        return;
                    }
                }
            }
        });
    }
    drop(result_tx);

    // Write chunks at their offset as they arrive — no ordering needed
    use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
    let mut transferred: u64 = 0;
    let start = std::time::Instant::now();
    let mut progress_epoch: u64 = 0;

    // Emit progress whenever percentage advances
    let mut try_emit = |transferred: u64| {
        let recv_total = received.load(Ordering::Relaxed);
        let pct = if total > 0 { transferred * 200 / total } else { 0 };
        if pct > progress_epoch && pct <= 200 {
            progress_epoch = pct;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 { recv_total as f64 / elapsed } else { 0.0 };
            let _ = app.emit("transfer-progress", serde_json::json!({
                "id": tid, "filename": fname, "total": total,
                "transferred": transferred, "received": recv_total,
                "speed": speed, "status": "downloading"
            }));
        }
    };

    let mut tick = tokio::time::interval(std::time::Duration::from_millis(50));

    loop {
        tokio::select! {
            _ = tick.tick() => {
                try_emit(transferred);
            }
            maybe = result_rx.recv() => {
                match maybe {
                    Some((offset, data)) => {
                        if data.is_empty() {
                            let _ = tokio::fs::remove_file(local_path).await;
                            return Err("并发读取失败".to_string());
                        }
                        // Write at the exact offset — no ordering needed
                        local_file.seek(SeekFrom::Start(offset)).await
                            .map_err(|e| format!("seek失败: {}", e))?;
                        local_file.write_all(&data).await
                            .map_err(|e| format!("写入本地文件失败: {}", e))?;
                        transferred += data.len() as u64;
                        try_emit(transferred);
                    }
                    None => {
                        break;
                    }
                }
            }
        }

        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tokio::fs::remove_file(local_path).await;
            let _ = raw_sftp.close(close_handle.as_str()).await;
            return Err("cancelled".to_string());
        }

        if transferred >= total { break; }
    }

    // Final progress
    let recv_total = received.load(Ordering::Relaxed);
    let elapsed = start.elapsed().as_secs_f64();
    let speed = if elapsed > 0.0 { recv_total as f64 / elapsed } else { 0.0 };
    let _ = app.emit("transfer-progress", serde_json::json!({
        "id": tid, "filename": fname, "total": total,
        "transferred": transferred, "received": recv_total,
        "speed": speed, "status": "downloading"
    }));

    let _ = raw_sftp.close(close_handle.as_str()).await;
    Ok(())
}

#[tauri::command]
pub async fn cancel_transfer(
    transfer_id: String,
    transfer_state: tauri::State<'_, TransferManager>,
) -> Result<(), DshellError> {
    let mut transfers = transfer_state.transfers.lock().await;
    if let Some(flag) = transfers.remove(&transfer_id) {
        flag.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err(DshellError::Msg("传输任务未找到".to_string()))
    }
}

/// 前端在文件冲突弹窗中选择处理方式后回调此命令，
/// 唤醒对应传输任务继续（覆盖/跳过/重命名）。
#[tauri::command]
pub async fn resolve_transfer_conflict(
    token: String,
    action: String,
    session_id: String,
    apply_all: bool,
    transfer_state: tauri::State<'_, TransferManager>,
) -> Result<(), DshellError> {
    let resolution = match action.as_str() {
        "overwrite" => ConflictResolution::Overwrite,
        "skip" => ConflictResolution::Skip,
        "rename" => ConflictResolution::Rename,
        other => return Err(DshellError::Msg(format!("δ֪�ĳ�ͻ������ʽ: {}", other))),
    };
    let sender = {
        let mut pending = transfer_state.pending_conflicts.lock().await;
        pending.remove(&token)
    };
    match sender {
        Some(tx) => {
            // 仅当勾选"对所有冲突执行相同操作"时，记录会话级覆盖
            if apply_all
                && (matches!(resolution, ConflictResolution::Overwrite)
                    || matches!(resolution, ConflictResolution::Skip))
            {
                let mut ov = transfer_state.conflict_overrides.lock().await;
                ov.insert(session_id, resolution);
            }
            let _ = tx.send(resolution);
            Ok(())
        }
        None => Err(DshellError::Msg("冲突确认请求已失效或超时".to_string())),
    }
}








