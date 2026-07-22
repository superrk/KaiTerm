use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use russh::{client, ChannelMsg};
use russh::keys::PrivateKeyWithHashAlg;
use russh_sftp::client::SftpSession;
use tauri::Emitter;
use crate::error::DshellError;
use crate::models::HostKeyInfo;

/// 主机密钥验证的共享状态，用于 check_server_key 与前端之间的异步通信。
/// 每次连接握手时由 create_session 创建，信任/取消命令消费。
#[derive(Clone)]
pub struct HostKeyState {
    pub info: Arc<Mutex<Option<HostKeyInfo>>>,
    pub decision: Arc<Mutex<Option<bool>>>,
    pub notify: Arc<tokio::sync::Notify>,
}

impl HostKeyState {
    pub fn new() -> Self {
        Self {
            info: Arc::new(Mutex::new(None)),
            decision: Arc::new(Mutex::new(None)),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// 重置状态，为下一次连接握手做准备
    pub async fn reset(&self) {
        *self.info.lock().await = None;
        *self.decision.lock().await = None;
    }
}

pub struct SshHandler {
    pub host: String,
    pub port: u16,
    pub app_handle: tauri::AppHandle,
    pub host_key_state: HostKeyState,
}

impl client::Handler for SshHandler {
    type Error = anyhow::Error;

    fn check_server_key(&mut self, key: &ssh_key::PublicKey) -> impl std::future::Future<Output = Result<bool, Self::Error>> + Send {
        let host = self.host.clone();
        let port = self.port;
        let app = self.app_handle.clone();
        let state = self.host_key_state.clone();
        let fp = key.fingerprint(ssh_key::HashAlg::Sha256).to_string();
        let key_type = format!("{:?}", key.algorithm());

        async move {
            use russh::keys::known_hosts;

            let check_result = known_hosts::check_known_hosts(&host, port, key);

            match check_result {
                Ok(true) => {
                    log::info!("[dshell] 主机密钥已验证: {}:{} fp={}", host, port, fp);
                    Ok(true)
                }
                Ok(false) => {
                    // 首次连接 — known_hosts 中无此主机
                    log::info!("[dshell] 首次连接主机 {}:{}，等待用户确认", host, port);
                    let info = HostKeyInfo {
                        host: host.clone(),
                        port,
                        key_type: key_type.clone(),
                        fingerprint: fp.clone(),
                        status: "unknown".to_string(),
                    };
                    *state.info.lock().await = Some(info.clone());
                    *state.decision.lock().await = None;
                    let _ = app.emit("host-key-unknown", info);
                    state.notify.notified().await;
                    let decision = *state.decision.lock().await;
                    if decision.unwrap_or(false) {
                        if let Err(e) = known_hosts::learn_known_hosts(&host, port, key) {
                            log::warn!("[dshell] 写入 known_hosts 失败: {}", e);
                        } else {
                            log::info!("[dshell] 已将 {}:{} 写入 known_hosts", host, port);
                        }
                        Ok(true)
                    } else {
                        log::info!("[dshell] 用户取消了对 {}:{} 的信任", host, port);
                        Ok(false)
                    }
                }
                Err(russh::keys::Error::KeyChanged { line }) => {
                    // 密钥已变更 — 可能是 MITM 攻击
                    log::warn!("[dshell] 主机密钥已变更! {}:{} (line {})", host, port, line);
                    let info = HostKeyInfo {
                        host: host.clone(),
                        port,
                        key_type: key_type.clone(),
                        fingerprint: fp.clone(),
                        status: "changed".to_string(),
                    };
                    *state.info.lock().await = Some(info.clone());
                    *state.decision.lock().await = None;
                    let _ = app.emit("host-key-changed", info);
                    state.notify.notified().await;
                    let decision = *state.decision.lock().await;
                    if decision.unwrap_or(false) {
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                }
                Err(e) => {
                    log::warn!("[dshell] known_hosts 校验出错 ({}:{}): {}，按首次连接处理", host, port, e);
                    let info = HostKeyInfo {
                        host: host.clone(),
                        port,
                        key_type: key_type.clone(),
                        fingerprint: fp.clone(),
                        status: "unknown".to_string(),
                    };
                    *state.info.lock().await = Some(info.clone());
                    *state.decision.lock().await = None;
                    let _ = app.emit("host-key-unknown", info);
                    state.notify.notified().await;
                    let decision = *state.decision.lock().await;
                    if decision.unwrap_or(false) {
                        let _ = known_hosts::learn_known_hosts(&host, port, key);
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                }
            }
        }
    }
}


pub struct SshSession {
    pub id: String,
    pub handle: Arc<Mutex<client::Handle<SshHandler>>>,
    pub sftp: Arc<Mutex<Option<SftpSession>>>,
    pub cwd: Arc<Mutex<String>>,
    pub shell_active: Arc<Mutex<bool>>,
    pub sync_dir: Arc<Mutex<bool>>,
    pub stdin_tx: broadcast::Sender<String>,
    pub resize_tx: broadcast::Sender<(u32, u32)>,
    pub closed: Arc<AtomicBool>,
    // 重连所需的连接参数
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
    pub use_agent: bool,
}

impl SshSession {
    pub fn new(
        id: String,
        handle: client::Handle<SshHandler>,
        sync_dir: bool,
        host: String,
        port: u16,
        user: String,
        password: Option<String>,
        key_path: Option<String>,
        use_agent: bool,
    ) -> Self {
        let (stdin_tx, _) = broadcast::channel(1024);
        let (resize_tx, _) = broadcast::channel(32);
        let cwd = "/".to_string();
        Self {
            id,
            handle: Arc::new(Mutex::new(handle)),
            sftp: Arc::new(Mutex::new(None)),
            cwd: Arc::new(Mutex::new(cwd)),
            shell_active: Arc::new(Mutex::new(false)),
            sync_dir: Arc::new(Mutex::new(sync_dir)),
            stdin_tx,
            resize_tx,
            closed: Arc::new(AtomicBool::new(false)),
            host,
            port,
            user,
            password,
            key_path,
            use_agent,
        }
    }

    pub async fn init_sftp(&self) -> Result<(), String> {
        let handle = self.handle.lock().await;
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| format!("打开SFTP通道失败: {}", e))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| format!("SFTP子系统启动失败: {}", e))?;
        let stream = channel.into_stream();
        let session = SftpSession::new(stream)
            .await
            .map_err(|e| format!("SFTP会话初始化失败: {}", e))?;
        let mut s = self.sftp.lock().await;
        *s = Some(session);
        Ok(())
    }
}

#[derive(Clone)]
pub struct SessionManager {
    pub sessions: Arc<Mutex<Vec<SshSession>>>,
    pub host_key_state: HostKeyState,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
            host_key_state: HostKeyState::new(),
        }
    }

    pub async fn create_session(
        &self,
        host: String,
        port: u16,
        user: String,
        password: Option<String>,
        key_path: Option<String>,
        use_agent: bool,
        sync_dir: bool,
        app_handle: tauri::AppHandle,
    ) -> Result<(String, String), String> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut config = client::Config::default();
        config.keepalive_interval = Some(std::time::Duration::from_secs(30));
        config.keepalive_max = 3;
        let config = Arc::new(config);

        // 每次连接前重置 host_key_state，供 check_server_key 与前端通信
        self.host_key_state.reset().await;

        let handler = SshHandler {
            host: host.clone(),
            port,
            app_handle,
            host_key_state: self.host_key_state.clone(),
        };
        let addr = format!("{}:{}", host, port);

        let connect_fut = client::connect(config, addr.as_str(), handler);
        let mut handle = match tokio::time::timeout(std::time::Duration::from_secs(15), connect_fut).await {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                let msg = e.to_string();
                // 区分常见不可达场景，给出更友好提示
                if msg.to_lowercase().contains("timed out") || msg.to_lowercase().contains("timeout") {
                    return Err("连接超时，请检查主机地址和端口".to_string());
                } else if msg.to_lowercase().contains("refused") {
                    return Err("连接被拒绝，请确认远端 SSH 服务已启动且端口正确".to_string());
                } else if msg.to_lowercase().contains("no route") || msg.to_lowercase().contains("host unreachable") {
                    return Err("无法连接到主机，请检查网络连接".to_string());
                }
                return Err(format!("连接失败: {}", msg));
            }
            Err(_) => return Err("连接超时（15s），请检查主机地址和端口".to_string()),
        };

        if let Some(ref pass) = password {
            let auth = handle
                .authenticate_password(&user, pass)
                .await
                .map_err(|e| format!("认证失败: {}", e))?;
            if !auth.success() {
                return Err("密码认证失败".to_string());
            }
        } else if let Some(ref key_path) = key_path {
            let key_data = std::fs::read_to_string(key_path)
                .map_err(|e| format!("读取密钥文件失败: {}", e))?;
            let private_key = ssh_key::PrivateKey::from_openssh(key_data.as_str())
                .map_err(|e| format!("密钥解析失败: {}", e))?;
            let key = PrivateKeyWithHashAlg::new(Arc::new(private_key), None);

            let auth = handle
                .authenticate_publickey(&user, key)
                .await
                .map_err(|e| format!("密钥认证失败: {}", e))?;
            if !auth.success() {
                return Err("密钥认证失败".to_string());
            }
        } else if use_agent {
            use russh::keys::agent::client::AgentClient;

            let mut agent = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
                .await
                .map_err(|e| format!("连接 SSH Agent 失败: {}", e))?;
            let identities = agent
                .request_identities()
                .await
                .map_err(|e| format!("获取 SSH Agent 身份失败: {}", e))?;
            if identities.is_empty() {
                return Err("SSH Agent 中没有可用的身份".to_string());
            }

            let mut auth_success = false;
            for identity in &identities {
                let pk = identity.public_key().into_owned();
                let result = handle
                    .authenticate_publickey_with(&user, pk, None, &mut agent)
                    .await
                    .map_err(|e| format!("SSH Agent 认证失败: {}", e))?;
                if result.success() {
                    auth_success = true;
                    break;
                }
            }
            if !auth_success {
                return Err("SSH Agent 所有身份认证均失败".to_string());
            }
        } else {
            return Err("请提供密码或密钥".to_string());
        }

        let session = SshSession::new(
            id.clone(),
            handle,
            sync_dir,
            host.clone(),
            port,
            user.clone(),
            password,
            key_path,
            use_agent,
        );
        // SFTP 初始化失败（如远端无 sftp 子系统）不应阻断 shell 登录，
        // 但也不能静默吞掉——记录日志，便于用户遇到文件功能失效时排查。
        if let Err(e) = session.init_sftp().await {
            eprintln!("[dshell] SFTP 初始化失败（文件功能将不可用）: {}", e);
        }
        self.sessions.lock().await.push(session);

        Ok((id.clone(), id))
    }

    pub async fn disconnect(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        if let Some(pos) = sessions.iter().position(|s| s.id == session_id) {
            let session = sessions.remove(pos);

            // 标记会话已关闭，避免 shell 任务崩溃后误触发自动重启
            session.closed.store(true, Ordering::SeqCst);
            {
                let mut active = session.shell_active.lock().await;
                *active = false;
            }

            // Clean up SFTP
            {
                let mut sf = session.sftp.lock().await;
                *sf = None;
            }

            // Send SSH DISCONNECT to server
            let handle = session.handle.lock().await;
            let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "").await;
            Ok(())
        } else {
            Err("会话未找到".to_string())
        }
    }

    pub async fn start_shell(
        &self,
        session_id: &str,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        let sessions = self.sessions.lock().await;
        let session = sessions.iter().find(|s| s.id == session_id).ok_or("会话未找到")?;
        log::info!("[dshell] 启动 shell 会话: {}, sync_dir: {}", session.id, session.sync_dir.lock().await);
        let mut active = session.shell_active.lock().await;
        if *active {
            return Err("Shell 已在运行中".to_string());
        }
        *active = true;
        drop(active);

        let handle_arc = session.handle.clone();
        let sid = session_id.to_string();
        let shell_active = session.shell_active.clone();
        let cwd = session.cwd.clone();
        let sync_dir = session.sync_dir.clone();
        let stdin_tx = session.stdin_tx.clone();
        let resize_tx = session.resize_tx.clone();
        let closed = session.closed.clone();

        drop(sessions);

        tokio::spawn(run_shell(
            handle_arc,
            shell_active,
            cwd,
            sync_dir,
            stdin_tx,
            resize_tx,
            closed,
            app_handle,
            sid,
            0,
        ));
        Ok(())
    }

    /// 完整重连：断开旧 TCP 连接，用保存的连接参数重新建立 SSH + SFTP + Shell。
    /// 适用于底层 TCP 连接已断开的场景。
    pub async fn full_reconnect(
        &self,
        session_id: &str,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        // 1. 取出旧会话的连接参数
        let conn_params = {
            let sessions = self.sessions.lock().await;
            let s = sessions.iter().find(|s| s.id == session_id)
                .ok_or("会话未找到")?;
            s.closed.store(true, Ordering::SeqCst);
            {
                let mut active = s.shell_active.lock().await;
                *active = false;
            }
            {
                let mut sf = s.sftp.lock().await;
                *sf = None;
            }
            // 发送 SSH DISCONNECT 关闭旧连接
            {
                let handle = s.handle.lock().await;
                let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "").await;
            }
            let host = s.host.clone();
            let port = s.port;
            let user = s.user.clone();
            let password = s.password.clone();
            let key_path = s.key_path.clone();
            let use_agent = s.use_agent;
            (host, port, user, password, key_path, use_agent)
        };
        let (host, port, user, password, key_path, use_agent) = conn_params;

        log::info!("[dshell] 完整重连 {}:{} (session {})", host, port, session_id);

        // 2. 建立全新的 SSH 连接
        let mut config = client::Config::default();
        config.keepalive_interval = Some(std::time::Duration::from_secs(30));
        config.keepalive_max = 3;
        let config = Arc::new(config);

        self.host_key_state.reset().await;
        let handler = SshHandler {
            host: host.clone(),
            port,
            app_handle: app_handle.clone(),
            host_key_state: self.host_key_state.clone(),
        };
        let addr = format!("{}:{}", host, port);

        let connect_fut = client::connect(config, addr.as_str(), handler);
        let mut handle = match tokio::time::timeout(std::time::Duration::from_secs(15), connect_fut).await {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                let msg = e.to_string();
                if msg.to_lowercase().contains("timed out") || msg.to_lowercase().contains("timeout") {
                    return Err("重连超时，请检查主机地址和端口".to_string());
                } else if msg.to_lowercase().contains("refused") {
                    return Err("重连被拒绝，请确认远端 SSH 服务已启动且端口正确".to_string());
                } else if msg.to_lowercase().contains("no route") || msg.to_lowercase().contains("host unreachable") {
                    return Err("无法连接到主机，请检查网络连接".to_string());
                }
                return Err(format!("重连失败: {}", msg));
            }
            Err(_) => return Err("重连超时（15s）".to_string()),
        };

        // 3. 认证
        if let Some(ref pass) = password {
            let auth = handle.authenticate_password(&user, pass).await
                .map_err(|e| format!("重连认证失败: {}", e))?;
            if !auth.success() {
                return Err("密码认证失败".to_string());
            }
        } else if let Some(ref kp) = key_path {
            let key_data = std::fs::read_to_string(kp)
                .map_err(|e| format!("读取密钥文件失败: {}", e))?;
            let private_key = ssh_key::PrivateKey::from_openssh(key_data.as_str())
                .map_err(|e| format!("密钥解析失败: {}", e))?;
            let key = PrivateKeyWithHashAlg::new(Arc::new(private_key), None);
            let auth = handle.authenticate_publickey(&user, key).await
                .map_err(|e| format!("密钥认证失败: {}", e))?;
            if !auth.success() {
                return Err("密钥认证失败".to_string());
            }
        } else if use_agent {
            use russh::keys::agent::client::AgentClient;
            let mut agent = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
                .await
                .map_err(|e| format!("连接 SSH Agent 失败: {}", e))?;
            let identities = agent.request_identities().await
                .map_err(|e| format!("获取 SSH Agent 身份失败: {}", e))?;
            if identities.is_empty() {
                return Err("SSH Agent 中没有可用的身份".to_string());
            }
            let mut auth_success = false;
            for identity in &identities {
                let pk = identity.public_key().into_owned();
                let result = handle.authenticate_publickey_with(&user, pk, None, &mut agent).await
                    .map_err(|e| format!("SSH Agent 认证失败: {}", e))?;
                if result.success() {
                    auth_success = true;
                    break;
                }
            }
            if !auth_success {
                return Err("SSH Agent 所有身份认证均失败".to_string());
            }
        } else {
            return Err("请提供密码或密钥".to_string());
        }

        // 4. 替换旧会话的 handle，重置状态
        {
            let mut sessions = self.sessions.lock().await;
            if let Some(s) = sessions.iter_mut().find(|s| s.id == session_id) {
                *s.handle.lock().await = handle;
                s.closed.store(false, Ordering::SeqCst);
                *s.cwd.lock().await = "/".to_string();
            } else {
                return Err("会话已不存在".to_string());
            }
        }

        // 5. 重新初始化 SFTP
        {
            let sessions = self.sessions.lock().await;
            if let Some(s) = sessions.iter().find(|s| s.id == session_id) {
                if let Err(e) = s.init_sftp().await {
                    log::warn!("[dshell] 重连后 SFTP 初始化失败: {}", e);
                }
            }
        }

        // 6. 启动新 shell
        self.start_shell(session_id, app_handle).await?;

        log::info!("[dshell] 重连成功: {} session {}", host, session_id);
        Ok(())
    }
}

/// Shell 任务主体：打开 PTY + shell，注入目录跟随钩子，转发输出到前端。
/// 当 shell 因远端崩溃 / 通道异常结束时，若会话未被主动断开（closed=false），
/// 则自动重新拉起一个新 shell（最多 MAX_SHELL_RETRIES 次），实现崩溃自动重启。
fn run_shell(
    handle: Arc<Mutex<client::Handle<SshHandler>>>,
    shell_active: Arc<Mutex<bool>>,
    cwd: Arc<Mutex<String>>,
    sync_dir: Arc<Mutex<bool>>,
    stdin_tx: broadcast::Sender<String>,
    resize_tx: broadcast::Sender<(u32, u32)>,
    closed: Arc<AtomicBool>,
    app_handle: tauri::AppHandle,
    sid: String,
    attempt: u32,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
    const MAX_SHELL_RETRIES: u32 = 5;

    let channel = {
        let h = handle.lock().await;
        match h.channel_open_session().await {
            Ok(ch) => ch,
            Err(e) => {
                let _ = app_handle.emit("terminal-error", format!("打开通道失败: {}", e));
                return;
            }
        }
    };

    if let Err(e) = channel
        .request_pty(true, "xterm-256color", 120, 40, 0, 0, &[])
        .await
    {
        let _ = app_handle.emit("terminal-error", format!("PTY请求失败: {}", e));
        return;
    }

    if let Err(e) = channel.request_shell(true).await {
        let _ = app_handle.emit("terminal-error", format!("Shell启动失败: {}", e));
        return;
    }

    let sync_enabled = *sync_dir.lock().await;
    if sync_enabled {
        // ── 目录跟随：注入 Shell 集成钩子 ─────────────────────────────
        // 每条命令单独一行，前缀空格使其不写入历史记录
        // 第一行设置 HISTCONTROL（会出现在历史里，但仅此一条）
        let hook = "\
HISTCONTROL=ignoreboth\n\
setopt HIST_IGNORE_SPACE\n\
 [ -n \"$BASH_VERSION\" ] && eval ' __dshell_cwd() { printf \"\\033]7;file://%s%s\\007\" \"$HOSTNAME\" \"$PWD\"; }; case \"$PROMPT_COMMAND\" in *__dshell_cwd*) ;; *) PROMPT_COMMAND=\"__dshell_cwd${PROMPT_COMMAND:+; $PROMPT_COMMAND}\" ;; esac'\n\
 [ -n \"$ZSH_VERSION\" ] && eval ' __dshell_cwd() { printf \"\\033]7;file://%s%s\\007\" \"$HOSTNAME\" \"$PWD\"; }; autoload -Uz add-zsh-hook 2>/dev/null && add-zsh-hook precmd __dshell_cwd'\n\
 [ -n \"$FISH_VERSION\" ] && eval ' function __dshell_cwd --on-event fish_prompt; printf \"\\033]7;file://%s%s\\007\" \"$hostname\" \"$PWD\"; end'\n";
        let init = format!("{}\nclear\n", hook);
        let _ = channel.data(init.as_bytes()).await;
        eprintln!("[dshell] 目录跟随：已向远端注入 OSC 7 shell 集成钩子 (bash/zsh/fish)，并清屏");
    }

    let _ = app_handle.emit("terminal-started", sid.clone());

    let (mut read_ch, write_ch) = channel.split();
    // 每次启动都重新订阅广播通道，确保重启后能继续接收用户输入与 resize
    let mut rx = stdin_tx.subscribe();
    let mut resize_rx = resize_tx.subscribe();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                recv = rx.recv() => {
                    match recv {
                        Ok(input) => { let _ = write_ch.data(input.as_bytes()).await; }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                resize = resize_rx.recv() => {
                    match resize {
                        Ok((cols, rows)) => { let _ = write_ch.window_change(cols, rows, 0, 0).await; }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    });

    let mut osb_buf: String = String::new();

    loop {
        tokio::select! {
            msg = read_ch.wait() => {
                match msg {
                    Some(ChannelMsg::Data { ref data }) | Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                        let text = String::from_utf8_lossy(data).to_string();
                        let _ = app_handle.emit("terminal-output", serde_json::json!({
                            "session_id": sid,
                            "data": text
                        }));
                        if sync_enabled {
                            osb_buf.push_str(&text);
                            if let Some(cwd_path) = extract_osc7_cwd(&osb_buf) {
                                osb_buf.clear();
                                *cwd.lock().await = cwd_path.clone();
                                let _ = app_handle.emit("sftp-cwd-changed", serde_json::json!({
                                    "session_id": sid,
                                    "cwd": cwd_path
                                }));
                            } else if osb_buf.len() > 1024 {
                                let drop = osb_buf.len() - 1024;
                                let start = osb_buf
                                    .char_indices()
                                    .find(|(i, _)| *i >= drop)
                                    .map(|(i, _)| i)
                                    .unwrap_or(drop);
                                osb_buf.replace_range(..start, "");
                            }
                        }
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                        let _ = app_handle.emit("terminal-closed", sid.clone());
                        break;
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        let _ = app_handle.emit("terminal-exit", serde_json::json!({
                            "session_id": sid,
                            "code": exit_status
                        }));
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // 会话已被主动断开（点关闭 / 断开连接），不自动重启
    if closed.load(Ordering::SeqCst) {
        *shell_active.lock().await = false;
        return;
    }

    // shell 异常结束 → 自动重启
    if attempt < MAX_SHELL_RETRIES {
        let _ = app_handle.emit("terminal-shell-crashed", serde_json::json!({
            "session_id": sid,
            "attempt": attempt + 1,
            "max": MAX_SHELL_RETRIES,
            "will_retry": true
        }));
        eprintln!("[dshell] Shell 崩溃（第 {} 次），自动重启中…", attempt + 1);
        tokio::spawn(run_shell(
            handle,
            shell_active,
            cwd,
            sync_dir,
            stdin_tx,
            resize_tx,
            closed,
            app_handle,
            sid,
            attempt + 1,
        ));
    } else {
        *shell_active.lock().await = false;
        let _ = app_handle.emit("terminal-shell-crashed", serde_json::json!({
            "session_id": sid,
            "attempt": attempt,
            "max": MAX_SHELL_RETRIES,
            "will_retry": false
        }));
        eprintln!("[dshell] Shell 崩溃超过最大重试次数（{}），停止自动重启", MAX_SHELL_RETRIES);
    }
    })
}

#[tauri::command]
pub async fn connect_ssh(
    host: String,
    port: u16,
    user: String,
    password: Option<String>,
    key_path: Option<String>,
    use_agent: Option<bool>,
    sync_dir: Option<bool>,
    state: tauri::State<'_, SessionManager>,
    app: tauri::AppHandle,
) -> Result<String, DshellError> {
    let use_agent = use_agent.unwrap_or(false);
    let sync_dir = sync_dir.unwrap_or(true);
    let (id, _) = state.create_session(host, port, user, password, key_path, use_agent, sync_dir, app.clone()).await?;
    state.start_shell(&id, app).await?;
    Ok(id)
}

#[tauri::command]
pub async fn disconnect_ssh(
    session_id: String,
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    state.disconnect(&session_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn reconnect_ssh(
    session_id: String,
    state: tauri::State<'_, SessionManager>,
    app: tauri::AppHandle,
) -> Result<(), DshellError> {
    state.full_reconnect(&session_id, app).await?;
    Ok(())
}

#[tauri::command]
pub async fn write_stdin(
    session_id: String,
    data: String,
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions.iter().find(|s| s.id == session_id).ok_or("会话未找到")?;
    let _ = session.stdin_tx.send(data);
    Ok(())
}

#[tauri::command]
pub async fn resize_terminal(
    session_id: String,
    cols: u32,
    rows: u32,
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions.iter().find(|s| s.id == session_id).ok_or("会话未找到")?;
    let _ = session.resize_tx.send((cols, rows));
    Ok(())
}

/// 解析 OSC 7 工作目录上报，格式：ESC ] 7 ; file://<host><path> BEL
/// 该目录由 shell 集成钩子每次命令后主动上报，是权威值（无需猜提示符）。
/// 返回解码后的路径；找不到完整序列时返回 None（调用方继续缓冲）。
fn extract_osc7_cwd(buf: &str) -> Option<String> {
    // 直接搜索 \x1b]7; 序列，避免命中颜色等其他 \x1b 转义序列
    let marker = "\u{1b}]7;";
    let start = buf.find(marker)?;
    let body_start = start + marker.len();
    // 序列以 BEL(0x07) 或 ST(ESC \) 结尾
    let end = buf[body_start..]
        .find('\u{07}')
        .map(|i| body_start + i)
        .or_else(|| {
            buf[body_start..]
                .find("\u{1b}\\")
                .map(|i| body_start + i + 2)
        })?;
    let body = &buf[body_start..end];
    Some(parse_osc7_body(body))
}

fn parse_osc7_body(body: &str) -> String {
    // body 形如：file://<host>/abs/path 或 file:///abs/path
    // 先去掉 "file://" scheme
    let without_scheme = body.strip_prefix("file://").unwrap_or(body);
    // 路径从（可选的 host 之后的）第一个 '/' 开始
    if let Some(slash) = without_scheme.find('/') {
        let path = &without_scheme[slash..];
        // 对空格等做百分号解码
        percent_decode(path)
    } else {
        body.to_string()
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = format!("{}{}", bytes[i + 1] as char, bytes[i + 2] as char);
            if let Ok(v) = u8::from_str_radix(&h, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

// ═══════════════════════════════════════════════════════
// 主机密钥管理
// ═══════════════════════════════════════════════════════

#[tauri::command]
pub async fn trust_host_key(
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    *state.host_key_state.decision.lock().await = Some(true);
    state.host_key_state.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn cancel_host_key(
    state: tauri::State<'_, SessionManager>,
) -> Result<(), DshellError> {
    *state.host_key_state.decision.lock().await = Some(false);
    state.host_key_state.notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn get_known_hosts() -> Result<Vec<crate::models::KnownHostEntry>, DshellError> {
    let path = known_hosts_path()?;
    let entries = known_hosts_list(&path)?;
    Ok(entries)
}

#[tauri::command]
pub async fn remove_known_host(
    host: String,
    port: u16,
) -> Result<(), DshellError> {
    let path = known_hosts_path()?;
    remove_host_from_known_hosts(&path, &host, port)?;
    Ok(())
}

fn known_hosts_path() -> Result<std::path::PathBuf, DshellError> {
    if let Some(home_dir) = home::home_dir() {
        Ok(home_dir.join(".ssh").join("known_hosts"))
    } else {
        Err(DshellError::Msg("无法获取用户主目录".to_string()))
    }
}

fn known_hosts_list(path: &std::path::Path) -> Result<Vec<crate::models::KnownHostEntry>, DshellError> {
    use std::io::{BufRead, BufReader};
    use std::fs::File;

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(vec![]),
    };
    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| DshellError::Msg(e.to_string()))?;
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hosts = parts.next().unwrap_or("");
        let key_type = parts.next().unwrap_or("");
        let key_data = parts.next().unwrap_or("");

        if hosts.is_empty() || key_data.is_empty() {
            continue;
        }

        // 解析 host:port（非标准端口格式 [host]:port）
        let (h, p) = if hosts.starts_with('[') {
            if let Some(end) = hosts.find(']') {
                let h = &hosts[1..end];
                let p = hosts[end..].trim_start_matches(']').trim_start_matches(':')
                    .parse::<u16>().unwrap_or(22);
                (h.to_string(), p)
            } else {
                (hosts.to_string(), 22)
            }
        } else {
            (hosts.to_string(), 22)
        };

        // 计算 fingerprint
        if let Ok(pubkey) = parse_public_key_from_known_hosts(key_type, key_data) {
            let fp = pubkey.fingerprint(ssh_key::HashAlg::Sha256).to_string();
            entries.push(crate::models::KnownHostEntry {
                host: h,
                port: p,
                key_type: key_type.to_string(),
                fingerprint: fp,
            });
        }
    }
    Ok(entries)
}

fn parse_public_key_from_known_hosts(key_type: &str, key_data: &str) -> Result<ssh_key::PublicKey, DshellError> {
    // known_hosts 中的 key_data 是 base64 编码的公钥
    // 格式: <key_type> <base64_data>
    let full = format!("{} {}", key_type, key_data);
    ssh_key::PublicKey::from_openssh(&full)
        .map_err(|e| DshellError::Msg(format!("解析公钥失败: {}", e)))
}

fn remove_host_from_known_hosts(
    path: &std::path::Path,
    host: &str,
    port: u16,
) -> Result<(), DshellError> {
    use std::io::{BufRead, BufReader, Write};
    use std::fs::File;

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(()), // 文件不存在，无需删除
    };
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines()
        .filter_map(|l| l.ok())
        .collect();

    let host_port = if port == 22 {
        host.to_string()
    } else {
        format!("[{}]:{}", host, port)
    };

    let filtered: Vec<&str> = lines.iter()
        .filter(|line| {
            if line.starts_with('#') || line.trim().is_empty() {
                return true; // 保留注释和空行
            }
            let first_part = line.split_whitespace().next().unwrap_or("");
            // 匹配精确主机名或 hashed hostname（|1|... 不做反向解析，跳过）
            first_part != host_port && !first_part.starts_with("|1|")
        })
        .map(|s| s.as_str())
        .collect();

    let mut file = File::create(path)
        .map_err(|e| DshellError::Msg(format!("写入 known_hosts 失败: {}", e)))?;
    for line in &filtered {
        writeln!(file, "{}", line)
            .map_err(|e| DshellError::Msg(format!("写入 known_hosts 失败: {}", e)))?;
    }
    Ok(())
}

