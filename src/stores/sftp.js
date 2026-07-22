import { defineStore } from "pinia";
import { safeInvoke } from "./safeInvoke";
import { ref, computed } from "vue";
import { useConnectionStore } from "./connection";

export const useSftpStore = defineStore("sftp", () => {
  const conn = useConnectionStore();
  const paths = ref({});
  const entriesMap = ref({});
  const loadingMap = ref({});
  const errorMap = ref({});

  function sessionState(sid) {
    if (!paths.value[sid]) paths.value[sid] = "/";
    if (!entriesMap.value[sid]) entriesMap.value[sid] = [];
    if (!loadingMap.value[sid]) loadingMap.value[sid] = false;
    if (!errorMap.value[sid]) errorMap.value[sid] = null;
  }

  const currentPath = computed({
    get: () => paths.value[conn.activeSessionId] || "/",
    set: (v) => { if (conn.activeSessionId) paths.value[conn.activeSessionId] = v; },
  });
  const entries = computed(() => entriesMap.value[conn.activeSessionId] || []);
  const loading = computed(() => loadingMap.value[conn.activeSessionId] || false);
  const error = computed(() => errorMap.value[conn.activeSessionId] || null);

  async function listDir(sessionId, path) {
    sessionState(sessionId);
    loadingMap.value[sessionId] = true;
    errorMap.value[sessionId] = null;
    try {
      const result = await safeInvoke("sftp_list_dir", {
        sessionId,
        path: path || paths.value[sessionId],
      });
      entriesMap.value[sessionId] = result;
      if (path) paths.value[sessionId] = path;
    } catch (e) {
      errorMap.value[sessionId] = e;
      entriesMap.value[sessionId] = [];
    } finally {
      loadingMap.value[sessionId] = false;
    }
  }

  function cd(sessionId, path) {
    const target = path === ".." ? parentPath(paths.value[sessionId]) : path;
    listDir(sessionId, target);
  }

  function parentPath(path) {
    if (path === "/" || path === "") return "/";
    const parts = path.replace(/\/$/, "").split("/");
    parts.pop();
    return parts.join("/") || "/";
  }

  async function mkdir(sessionId, path) {
    await safeInvoke("sftp_mkdir", { sessionId, path });
    await listDir(sessionId, paths.value[sessionId]);
  }

  async function remove(sessionId, path, isDir) {
    await safeInvoke("sftp_remove", { sessionId, path, isDir });
    await listDir(sessionId, paths.value[sessionId]);
  }

  async function rename(sessionId, oldPath, newPath) {
    await safeInvoke("sftp_rename", { sessionId, oldPath, newPath });
    await listDir(sessionId, paths.value[sessionId]);
  }

  function navigateTo(sessionId, path) {
    listDir(sessionId, path);
  }

  // 会话关闭时清理按 sessionId 存储的状态，避免内存泄漏与跨会话串扰
  function clearSession(sessionId) {
    delete paths.value[sessionId];
    delete entriesMap.value[sessionId];
    delete loadingMap.value[sessionId];
    delete errorMap.value[sessionId];
  }

  return {
    currentPath, entries, loading, error,
    listDir, cd, mkdir, remove, rename, navigateTo, parentPath, clearSession,
  };
});

