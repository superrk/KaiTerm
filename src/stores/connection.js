import { defineStore } from "pinia";
import { safeInvoke } from "./safeInvoke";
import { ref, computed, nextTick } from "vue";
import { useSftpStore } from "./sftp";
import { useTransferStore } from "./transfer";
import { useTerminalStore } from "./terminal";

export const useConnectionStore = defineStore("connection", () => {
  const sessions = ref([]);
  const activeSessionId = ref(null);
  const showConnectionModal = ref(false);
  const profiles = ref([]);
  const showSftpPanel = ref(false);
  const showLocalFilePanel = ref(false);
  const showSysInfoPanel = ref(false);
  const showDropdown = ref(false);
  const currentLabel = ref("未连接");
  const sidebarCollapsed = ref(true);

  // 主机密钥验证状态
  const hostKeyPending = ref(null); // { host, port, keyType, fingerprint, status }

  // 面板宽度，从 localStorage 恢复
  const sftpPanelWidth = ref(Number(localStorage.getItem("kaiterm_sftp_width")) || 340);
  const sysinfoPanelWidth = ref(Number(localStorage.getItem("kaiterm_sysinfo_width")) || 440);
  function setSftpPanelWidth(v) { sftpPanelWidth.value = v; localStorage.setItem("kaiterm_sftp_width", v); }
  function setSysinfoPanelWidth(v) { sysinfoPanelWidth.value = v; localStorage.setItem("kaiterm_sysinfo_width", v); }

  // Toast notifications
  const toasts = ref([]);
  let toastSeq = 0;

  function pushToast(message, type = "info", duration = 4000) {
    const id = ++toastSeq;
    toasts.value.push({ id, message, type });
    if (duration > 0) {
      setTimeout(() => {
        toasts.value = toasts.value.filter((t) => t.id !== id);
      }, duration);
    }
  }

  function removeToast(id) {
    toasts.value = toasts.value.filter((t) => t.id !== id);
  }

  const activeSession = computed(() =>
    sessions.value.find((s) => s.id === activeSessionId.value)
  );

  // 标记某个 SSH 会话的终端已断开（shell 通道关闭，但 SSH 连接可能仍在）。
  // 由前端监听后端 terminal-closed / terminal-exit 事件触发，供状态栏显示
  // "连接已断开" 提示与重连按钮。再次重连成功后由 reconnect() 复位为 connected。
  function markDisconnected(sessionId) {
    const s = sessions.value.find((ss) => ss.id === sessionId);
    if (s && s.type !== "local" && s.status !== "disconnected" && s.status !== "reconnecting") {
      s.status = "disconnected";
      pushToast(`${s.label || s.host} 连接已断开`, "warn");
    }
  }

  // 新 shell 启动成功（手动重连或崩溃自动恢复）后复位状态，清除"已断开"提示
  function markConnected(sessionId) {
    const s = sessions.value.find((ss) => ss.id === sessionId);
    if (s && (s.status === "disconnected" || s.status === "reconnecting")) {
      s.status = "connected";
    }
  }

  const MAX_RECONNECT_RETRIES = 3;

  async function reconnect(sessionId) {
    const s = sessions.value.find((ss) => ss.id === sessionId);
    if (!s || s.type === "local") return;

    // 如果已有配置保存的凭据，使用它们；否则使用内存中的
    let password = s.password || null;
    let keyPath = s.keyPath || null;
    let useAgent = s.useAgent || false;
    let syncDir = s.syncDir ?? true;

    // 尝试从已保存的 profile 获取凭据（更安全）
    const profile = profiles.value.find(p => p.host === s.host && p.port === s.port && p.user === s.user);
    if (profile) {
      try {
        const cred = await safeInvoke("get_credential", { id: profile.id });
        if (cred !== null) {
          if (profile.authMethod === "password") password = cred;
          else if (profile.authMethod === "key") keyPath = cred;
          else if (profile.authMethod === "agent") useAgent = true;
        }
      } catch {
        // 回退到内存中的凭据
      }
    }

    for (let attempt = 1; attempt <= MAX_RECONNECT_RETRIES; attempt++) {
      s.status = "reconnecting";
      try {
        await safeInvoke("reconnect_ssh", { sessionId });
        s.status = "connected";
        if (activeSessionId.value === sessionId) {
          await nextTick();
          termStore.fitTerminal(sessionId);
        }
        pushToast(`${s.label || s.host} 重连成功`, "success");
        return;
      } catch (e) {
        if (attempt < MAX_RECONNECT_RETRIES) {
          pushToast(`${s.label || s.host} 重连失败，${attempt}/${MAX_RECONNECT_RETRIES}，正在重试…`, "warn");
          await new Promise(r => setTimeout(r, 1500));
        } else {
          s.status = "disconnected";
          pushToast(`${s.label || s.host} 重连失败（已重试 ${MAX_RECONNECT_RETRIES} 次）: ${e}`, "error");
        }
      }
    }
  }

  const shells = ref([]);

  const connectPromises = new Map();

  async function connect(host, port, user, password, keyPath, label, useAgent, syncDir, profileId) {
    const dedupKey = profileId || `${host}|${port}|${user}`;

    const existing = sessions.value.find((s) => s.profileId && s.profileId === dedupKey);
    if (existing) {
      setActiveSession(existing.id);
      return existing.id;
    }

    if (connectPromises.has(dedupKey)) {
      const id = await connectPromises.get(dedupKey);
      setActiveSession(id);
      return id;
    }

    const promise = _doConnect(host, port, user, password, keyPath, label, useAgent, syncDir, dedupKey);
    connectPromises.set(dedupKey, promise);
    return await promise;
  }

  async function _doConnect(host, port, user, password, keyPath, label, useAgent, syncDir, dedupKey) {
    try {
      const id = await safeInvoke("connect_ssh", {
        host,
        port,
        user,
        password: password || null,
        keyPath: keyPath || null,
        useAgent: useAgent || false,
        syncDir: syncDir ?? true,
      });
      const dup = sessions.value.find((s) => s.profileId && s.profileId === dedupKey);
      if (dup) return dup.id;
      const display = label || `${user}@${host}`;
      const session = { id, type: "ssh", host, port, user, label: display, status: "connected", password: password || null, keyPath: keyPath || null, useAgent: useAgent || false, syncDir, profileId: dedupKey };
      sessions.value.push(session);
      activeSessionId.value = id;
      currentLabel.value = display;
      return id;
    } finally {
      connectPromises.delete(dedupKey);
    }
  }

  async function connectLocal(shellType) {
    const id = await safeInvoke("start_local_shell", { shellType });
    const found = shells.value.find((s) => s.id === shellType);
    const label = found ? found.name : (shellType === "cmd" ? "CMD" : "PowerShell");
    const session = { id, type: "local", shellType, label, status: "connected" };
    sessions.value.push(session);
    activeSessionId.value = id;
    currentLabel.value = label;
    return id;
  }

  async function disconnect(sessionId) {
    const s = sessions.value.find((ss) => ss.id === sessionId);
    const isLocal = s?.type === "local";
    try {
      if (isLocal) {
        await safeInvoke("stop_local_shell", { sessionId });
      } else {
        await safeInvoke("disconnect_ssh", { sessionId });
      }
    } catch (e) {
      console.warn("断开失败:", e);
    }
    // 清理该会话在其它 store 中的残留状态，避免内存泄漏与跨会话串扰
    const sftp = useSftpStore();
    const transfer = useTransferStore();
    const term = useTerminalStore();
    sftp.clearSession?.(sessionId);
    transfer.clearSession?.(sessionId);
    term.destroyTerminal(sessionId);
    sessions.value = sessions.value.filter((ss) => ss.id !== sessionId);
    if (activeSessionId.value === sessionId) {
      activeSessionId.value =
        sessions.value.length > 0 ? sessions.value[0].id : null;
    }
    if (sessions.value.length === 0) {
      currentLabel.value = "首页";
    } else {
      const s2 = sessions.value[0];
      currentLabel.value = s2.type === "local" ? s2.label : `${s2.user}@${s2.host}`;
    }
  }

  function setActiveSession(id) {
    activeSessionId.value = id;
    if (!id) {
      currentLabel.value = "首页";
      return;
    }
    const s = sessions.value.find((ss) => ss.id === id);
    if (s) {
      currentLabel.value = s.type === "local" ? s.label : `${s.user}@${s.host}`;
    }
  }

  function toggleDropdown() {
    showDropdown.value = !showDropdown.value;
  }

  function closeDropdown() {
    showDropdown.value = false;
  }

  function toggleSftpPanel() {
    if (activeSession.value?.type !== "ssh") return;
    showSftpPanel.value = !showSftpPanel.value;
    if (showSftpPanel.value) showSysInfoPanel.value = false;
  }

  function toggleSysInfoPanel() {
    if (activeSession.value?.type !== "ssh") return;
    showSysInfoPanel.value = !showSysInfoPanel.value;
    if (showSysInfoPanel.value) showSftpPanel.value = false;
  }

  function toggleLocalFilePanel() {
    if (activeSession.value?.type !== "local") return;
    showLocalFilePanel.value = !showLocalFilePanel.value;
  }

  // 主机密钥验证
  async function trustHostKey() {
    try {
      await safeInvoke("trust_host_key");
      hostKeyPending.value = null;
    } catch (e) {
      pushToast("信任主机密钥失败: " + e, "error");
    }
  }

  function cancelHostKey() {
    safeInvoke("cancel_host_key").catch(() => {});
    hostKeyPending.value = null;
  }

  async function loadProfiles() {
    try {
      profiles.value = await safeInvoke("load_connections");
    } catch {
      profiles.value = [];
    }
  }

  async function loadShells() {
    try {
      shells.value = await safeInvoke("detect_shells");
    } catch {
      shells.value = [];
    }
  }

  async function saveProfile(profile) {
    const { id, name, host, port, user, authMethod, password, keyPath, group, sync_dir } = profile;
    const credential = authMethod === "password" ? (password || "") : authMethod === "key" ? (keyPath || "") : "";
    const saved = await safeInvoke("save_connection", {
      name,
      host,
      port,
      user,
      authMethod,
      credential,
      group: group || null,
      syncDir: sync_dir ?? true,
      id: id || null,
    });
    // 更新本地 profiles：优先按 id 匹配，其次按 host+port+user
    const idx = id
      ? profiles.value.findIndex((p) => p.id === id)
      : profiles.value.findIndex((p) => p.host === host && p.port === port && p.user === user);
    if (idx >= 0) {
      profiles.value[idx] = { ...profiles.value[idx], ...saved };
    } else {
      profiles.value.push(saved);
    }
  }

  async function deleteProfile(profileId) {
    await safeInvoke("delete_connection", { profileId });
    profiles.value = profiles.value.filter((p) => p.id !== profileId);
  }

  async function getCredential(profileId) {
    return await safeInvoke("get_credential", { profileId });
  }

  /** 复制连接：基于已有 profile 创建一个新条目（新 ID + 新名称） */
  async function duplicateProfile(profileId) {
    const saved = await safeInvoke("duplicate_connection", { profileId });
    profiles.value.push(saved);
  }

  // ---- 分组管理 ----
  const managedGroups = ref([]);

  async function loadGroups() {
    try {
      managedGroups.value = await safeInvoke("load_groups");
    } catch {
      managedGroups.value = [];
    }
  }

  async function persistGroups() {
    await safeInvoke("save_groups", { groups: [...managedGroups.value] });
  }

  async function addGroup(name) {
    if (!name || managedGroups.value.includes(name)) return;
    managedGroups.value.push(name);
    managedGroups.value.sort();
    await persistGroups();
  }

  async function deleteGroup(name) {
    managedGroups.value = managedGroups.value.filter((g) => g !== name);
    await persistGroups();
  }

  /** 所有已使用的分组名（去重），供下拉选择框使用；包含 managedGroups + profiles 中出现的 */
  const existingGroups = computed(() => {
    const set = new Set(managedGroups.value);
    for (const p of profiles.value) {
      if (p.group) set.add(p.group);
    }
    return [...set].sort();
  });

  /** 批量重命名分组：更新后端所有连接的 group 字段 + managedGroups，同步本地 profiles */
  async function renameGroup(oldName, newName) {
    await safeInvoke("rename_group", { oldName, newName });
    for (const p of profiles.value) {
      if (p.group === oldName) {
        p.group = newName;
      }
    }
    const idx = managedGroups.value.indexOf(oldName);
    if (idx >= 0) {
      managedGroups.value[idx] = newName;
      managedGroups.value.sort();
      await persistGroups();
    }
  }

  return {
    sessions,
    activeSessionId,
    showConnectionModal,
    profiles,
    shells,
    showSftpPanel,
    showLocalFilePanel,
    showSysInfoPanel,
    showDropdown,
    currentLabel,
    sidebarCollapsed,
    sftpPanelWidth,
    sysinfoPanelWidth,
    setSftpPanelWidth,
    setSysinfoPanelWidth,
    toasts,
    activeSession,
    connect,
    connectLocal,
    disconnect,
    reconnect,
    markDisconnected,
    markConnected,
    setActiveSession,
    toggleDropdown,
    closeDropdown,
    toggleSftpPanel,
    toggleSysInfoPanel,
    toggleLocalFilePanel,
    hostKeyPending,
    trustHostKey,
    cancelHostKey,
    loadProfiles,
    loadShells,
    saveProfile,
    deleteProfile,
    duplicateProfile,
    getCredential,
    existingGroups,
    managedGroups,
    addGroup,
    deleteGroup,
    renameGroup,
    loadGroups,
    pushToast,
    removeToast,
  };
});

