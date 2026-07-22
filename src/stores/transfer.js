import { defineStore } from "pinia";
import { safeInvoke } from "./safeInvoke";
import { ref } from "vue";

export const useTransferStore = defineStore("transfer", () => {
  const transfers = ref([]);
  const showPanel = ref(false);
  const conflict = ref(null);

  async function resolveConflict(token, action, sessionId, applyAll) {
    await safeInvoke("resolve_transfer_conflict", {
      token,
      action,
      sessionId,
      applyAll,
    });
  }

  function setConflict(data) {
    conflict.value = data;
  }

  function clearConflict() {
    conflict.value = null;
  }

  async function upload(sessionId, localPath, remotePath) {
    showPanel.value = true;
    const id = await safeInvoke("upload_file", {
      sessionId,
      localPath,
      remotePath,
    });
    transfers.value.push({
      id,
      sessionId,
      filename: localPath.split("\\").pop().split("/").pop(),
      total: 0,
      transferred: 0,
      speed: 0,
      status: "uploading",
    });
    return id;
  }

  async function download(sessionId, remotePath, localPath) {
    showPanel.value = true;
    const id = await safeInvoke("download_file", {
      sessionId,
      remotePath,
      localPath,
    });
    transfers.value.push({
      id,
      sessionId,
      filename: remotePath.split("/").pop(),
      total: 0,
      transferred: 0,
      speed: 0,
      status: "downloading",
    });
    return id;
  }

  async function cancelTransfer(transferId) {
    await safeInvoke("cancel_transfer", { transferId });
    const t = transfers.value.find((t) => t.id === transferId);
    if (t) t.status = "cancelling";
  }

  function updateProgress(data) {
    const t = transfers.value.find((t) => t.id === data.id);
    if (t) {
      t.total = data.total;
      t.transferred = data.transferred;
      t.speed = data.speed;
      t.status = data.status;
    }
  }

  function markComplete(data) {
    const t = transfers.value.find((t) => t.id === data.id);
    if (t) t.status = "completed";
  }

  function markCancelled(data) {
    const t = transfers.value.find((t) => t.id === data.id);
    if (t) t.status = "cancelled";
  }

  function markError(data) {
    const t = transfers.value.find((t) => t.id === data.id);
    if (t) {
      t.status = "error";
      t.error = data.error;
    }
  }

  function clearCompleted() {
    transfers.value = transfers.value.filter(
      (t) =>
        t.status === "uploading" ||
        t.status === "downloading" ||
        t.status === "cancelling"
    );
  }

  // 会话关闭时移除该会话相关的传输任务，避免内存泄漏
  function clearSession(sessionId) {
    transfers.value = transfers.value.filter((t) => t.sessionId !== sessionId);
  }

  return {
    transfers,
    showPanel,
    conflict,
    upload,
    download,
    cancelTransfer,
    updateProgress,
    markComplete,
    markCancelled,
    markError,
    clearCompleted,
    clearSession,
    resolveConflict,
    setConflict,
    clearConflict,
  };
});

