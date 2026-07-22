use serde::Serialize;

/// 统一的错误类型，替代原先散落的 `Result<T, String>`。
///
/// - `Msg` 变体实现了 `From<String>` / `From<&str>`，因此内部已手动 `map_err(format!...)`
///   的 `Result<_, String>` 通过 `?` 自动提升为 `DshellError`，无需改动既有逻辑。
/// - `Io` 变体提供 `From<std::io::Error>` 映射，便于 IO 错误直接 `?` 透传。
/// - 派生 `Serialize` 以便 Tauri 把错误序列化传给前端（前端经 safeInvoke 取 `message`）。
#[derive(Debug, Serialize, thiserror::Error)]
pub enum DshellError {
    #[error("{0}")]
    Msg(String),

    #[error("会话未找到: {0}")]
    SessionNotFound(String),

    #[error("SFTP 未初始化")]
    SftpNotInitialized,

    #[error("IO 错误: {0}")]
    Io(String),
}

impl From<String> for DshellError {
    fn from(s: String) -> Self {
        DshellError::Msg(s)
    }
}

impl From<&str> for DshellError {
    fn from(s: &str) -> Self {
        DshellError::Msg(s.to_string())
    }
}

impl From<std::io::Error> for DshellError {
    fn from(e: std::io::Error) -> Self {
        DshellError::Io(e.to_string())
    }
}

// 注：russh / russh_sftp 的 Error 类型在 0.62 / 2.3 版本中为私有，
// 无法直接 `impl From`。后端已在各调用点用 `map_err(|e| format!(...))`
// 转成 String，经下方 `From<String>` 自动提升为 DshellError，无需此处映射。

