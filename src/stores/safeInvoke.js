import { invoke } from "@tauri-apps/api/core";

// 把后端返回的错误统一转成可读字符串。
// 后端命令现返回 DshellError（序列化后形如 { Msg: "..." } 或 { Io: "..." } 等），
// 也可能是旧式的纯字符串。统一取出 message 便于前端展示。
function extractErrorMessage(e) {
  if (e == null) return "未知错误";
  if (typeof e === "string") return e;
  if (typeof e === "object") {
    const values = Object.values(e);
    if (values.length > 0) {
      const v = values[0];
      return typeof v === "string" ? v : JSON.stringify(v);
    }
    if (e.message) return e.message;
  }
  return String(e);
}

// 透传 invoke，catch 时把错误归一为 message 再抛出，避免每处 .catch 重复处理。
export async function safeInvoke(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    throw extractErrorMessage(e);
  }
}

export { extractErrorMessage };
