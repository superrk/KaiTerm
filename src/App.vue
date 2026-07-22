<script setup>
import { computed, ref } from "vue";
import { useConnectionStore } from "./stores/connection";
import { useTerminalStore } from "./stores/terminal";
import { useTransferStore } from "./stores/transfer";
import { useThemeStore } from "./stores/theme";
import TitleBar from "./components/TitleBar.vue";
import SideBar from "./components/SideBar.vue";
import TerminalPanel from "./components/TerminalPanel.vue";
import SftpPanel from "./components/SftpPanel.vue";
import SysInfoPanel from "./components/SysInfoPanel.vue";
import LocalFilePanel from "./components/LocalFilePanel.vue";
import TransferPanel from "./components/TransferPanel.vue";
import StatusBar from "./components/StatusBar.vue";
import ConnectionModal from "./components/ConnectionModal.vue";
import HostKeyDialog from "./components/HostKeyDialog.vue";
import GroupManager from "./components/GroupManager.vue";
import ToastNotification from "./components/ToastNotification.vue";
import ConflictDialog from "./components/ConflictDialog.vue";
import SettingsPanel from "./components/SettingsPanel.vue";

import { onMounted, onUnmounted, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";

const conn = useConnectionStore();
const term = useTerminalStore();
const transfer = useTransferStore();
const themeStore = useThemeStore();
const unlisteners = [];
const selectedShellId = ref("");
const searchQuery = ref("");
const editingProfile = ref(null);

// 主题/字体变化时实时更新所有终端
watch(
  () => [themeStore.terminalSchemeId, themeStore.fontFamily, themeStore.fontSize, themeStore.scrollback],
  () => term.updateAllTerminals(),
  { deep: true }
);
const showGroupManager = ref(false);

onMounted(async () => {
  themeStore.applyAppTheme();
  await conn.loadProfiles();
  await conn.loadGroups();
  await conn.loadShells();
  if (conn.shells.length > 0) {
    selectedShellId.value = conn.shells[0].id;
  }
  unlisteners.push(
    await listen("transfer-progress", (e) => transfer.updateProgress(e.payload))
  );
  unlisteners.push(
    await listen("transfer-complete", (e) => transfer.markComplete(e.payload))
  );
  unlisteners.push(
    await listen("transfer-cancelled", (e) => transfer.markCancelled(e.payload))
  );
  unlisteners.push(
    await listen("transfer-error", (e) => transfer.markError(e.payload))
  );
  unlisteners.push(
    await listen("transfer-conflict", (e) => transfer.setConflict(e.payload))
  );
  unlisteners.push(
    await listen("terminal-shell-crashed", (e) => {
      const d = e.payload;
      if (d.will_retry) {
        conn.pushToast(`终端连接中断，正在自动重连（第 ${d.attempt}/${d.max} 次）…`, "warn");
      } else {
        conn.pushToast("终端连接多次中断，已停止自动重连，请手动重连", "error");
      }
    })
  );
  // shell 通道关闭 / 进程退出：标记会话为"已断开"，状态栏显示重连按钮
  const onClosed = (e) => conn.markDisconnected(e.payload?.session_id || e.payload);
  unlisteners.push(await listen("terminal-closed", onClosed));
  unlisteners.push(await listen("terminal-exit", (e) => conn.markDisconnected(e.payload?.session_id || e.payload)));
  // 新 shell 启动成功（重连 / 崩溃自动恢复）后，清除"已断开"状态
  unlisteners.push(await listen("terminal-started", (e) => conn.markConnected(e.payload)));
  // 主机密钥验证
  unlisteners.push(await listen("host-key-unknown", (e) => { conn.hostKeyPending = e.payload; }));
  unlisteners.push(await listen("host-key-changed", (e) => { conn.hostKeyPending = e.payload; }));
});

onUnmounted(() => unlisteners.forEach((fn) => fn()));

const showLocal = computed(() => conn.showLocalFilePanel && conn.activeSession?.type === "local");
const showSftp = computed(() => conn.showSftpPanel && conn.activeSession?.type === "ssh");
const showSys = computed(() => conn.showSysInfoPanel && conn.activeSession?.type === "ssh");

function tabLabel(s) {
  if (s.type === "local") return s.label || "本地终端";
  return s.label || `${s.user}@${s.host}`;
}

async function closeTab(sessionId) {
  term.destroyTerminal(sessionId);
  await conn.disconnect(sessionId);
}

function handleClear() {
  if (conn.activeSessionId) term.writeToTerminal(conn.activeSessionId, "\x1b[2J\x1b[3J\x1b[H");
}

const groups = computed(() => {
  const map = {};
  for (const p of conn.profiles) {
    const g = p.group || "";
    if (!map[g]) map[g] = [];
    map[g].push(p);
  }
  return map;
});

const filteredGroups = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  const map = {};
  for (const p of conn.profiles) {
    const haystack = `${p.name || ""} ${p.host} ${p.user} ${p.group || ""}`.toLowerCase();
    if (q && !haystack.includes(q)) continue;
    const g = p.group || "";
    if (!map[g]) map[g] = [];
    map[g].push(p);
  }
  return map;
});

async function connectSavedProfile(p) {
  try {
    let password = null;
    let keyPath = null;
    let useAgent = false;
    if (p.authMethod === "password") {
      password = await conn.getCredential(p.id);
    } else if (p.authMethod === "key") {
      keyPath = await conn.getCredential(p.id);
    } else if (p.authMethod === "agent") {
      useAgent = true;
    }
    await conn.connect(p.host, p.port, p.user, password, keyPath, p.name, useAgent, p.syncDir, p.id);
  } catch (e) {
    conn.pushToast("连接失败: " + e, "error");
  }
}

async function openLocalTerminal(shellType) {
  try {
    await conn.connectLocal(shellType);
  } catch (e) {
    conn.pushToast("启动本地终端失败: " + e, "error");
  }
}

async function deleteSavedProfile(p, e) {
  e.stopPropagation();
  if (!await confirm(`删除连接 "${p.name || p.host}"？`)) return;
  try {
    await conn.deleteProfile(p.id);
  } catch (err) {
    conn.pushToast("删除失败: " + err, "error");
  }
}

function openEditModal(p, e) {
  e.stopPropagation();
  editingProfile.value = p;
  conn.showConnectionModal = true;
}

async function handleDuplicate(p, e) {
  e.stopPropagation();
  try {
    await conn.duplicateProfile(p.id);
    conn.pushToast("已复制连接", "success");
  } catch (err) {
    conn.pushToast("复制失败: " + err, "error");
  }
}

function handleCloseModal() {
  editingProfile.value = null;
  conn.showConnectionModal = false;
}
</script>

<template>
  <div class="app">
    <TitleBar />
    <div id="workspace" class="workspace">
      <SideBar :collapsed="conn.sidebarCollapsed" @group-manager="showGroupManager = true" />
      <div class="panel panel--terminal">
        <div class="panel-header">
          <div class="panel-tabs">
            <button
              class="tab tab-add"
              :class="{ active: !conn.activeSessionId }"
              @click="conn.activeSessionId = null"
              title="首页"
            >
              <i class="fas fa-home"></i> 首页
            </button>
            <button
              v-for="s in conn.sessions"
              :key="s.id"
              class="tab"
              :class="{ active: s.id === conn.activeSessionId }"
              @click="conn.setActiveSession(s.id)"
            >
              <i class="fas fa-terminal"></i>
              <span class="tab-label">{{ tabLabel(s) }}</span>
              <span class="tab-close" @click.stop="closeTab(s.id)" title="关闭连接">&times;</span>
            </button>
          </div>
          <div class="panel-actions">
            <button title="清屏" @click="handleClear"><i class="fas fa-eraser"></i></button>
          </div>
        </div>
        <div class="terminal-body" id="terminalContainer">
          <TerminalPanel v-show="!!conn.activeSessionId" />
          <div v-show="!conn.activeSessionId" class="welcome">
            <div class="welcome-box">
              <div class="welcome-logo"><i class="fas fa-terminal"></i></div>
              <h1 class="welcome-title">KaiTerm</h1>
              <p class="welcome-sub">SSH 终端 · 文件传输</p>
              <div style="display:flex;gap:10px;margin-top:12px;flex-wrap:wrap;justify-content:center">
                <div class="local-picker">
                  <select v-model="selectedShellId" class="shell-select">
                    <option v-for="s in conn.shells" :key="s.id" :value="s.id">{{ s.name }}</option>
                  </select>
                  <button class="welcome-btn" @click="openLocalTerminal(selectedShellId)" :disabled="!selectedShellId">
                    <i class="fas fa-terminal"></i> 打开终端
                  </button>
                </div>
                <button class="welcome-btn secondary" @click="conn.showConnectionModal = true">
                  <i class="fas fa-plus"></i> 新建连接
                </button>
              </div>
              <div v-if="conn.profiles.length > 0" class="saved-conns">
                <div class="conn-search-row">
                  <i class="fas fa-search conn-search-icon"></i>
                  <input
                    class="conn-search-input"
                    type="text"
                    v-model="searchQuery"
                    placeholder="搜索连接名称、地址、用户…"
                  >
                  <button v-if="searchQuery" class="conn-search-clear" @click="searchQuery = ''">&times;</button>
                </div>
                <div v-for="(profs, g) in filteredGroups" :key="g" class="conn-group">
                  <div class="conn-group-title">{{ g || '未分组' }}</div>
                  <div class="conn-group-list">
                    <div v-for="p in profs" :key="p.id" class="conn-card" @click="connectSavedProfile(p)">
                      <i class="fas fa-server"></i>
                      <div class="conn-card-info">
                        <span class="conn-card-name">{{ p.name || p.host }}</span>
                        <span class="conn-card-host">{{ p.user }}@{{ p.host }}:{{ p.port }}</span>
                      </div>
                      <button class="conn-card-action" @click="openEditModal(p, $event)" title="编辑连接"><i class="fas fa-pen"></i></button>
                      <button class="conn-card-action" @click="handleDuplicate(p, $event)" title="复制连接"><i class="fas fa-copy"></i></button>
                      <button class="conn-card-action danger" @click="deleteSavedProfile(p, $event)" title="删除连接"><i class="fas fa-trash"></i></button>
                    </div>
                  </div>
                </div>
                <div v-if="Object.keys(filteredGroups).length === 0 && searchQuery" class="conn-empty">无匹配连接</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <Transition name="sftp-slide">
        <SftpPanel v-if="showSftp" @close="conn.showSftpPanel = false" />
      </Transition>
      <Transition name="sftp-slide">
        <SysInfoPanel v-if="showSys" @close="conn.showSysInfoPanel = false" />
      </Transition>
      <Transition name="sftp-slide">
        <LocalFilePanel v-if="showLocal" @close="conn.showLocalFilePanel = false" />
      </Transition>
    </div>
    <TransferPanel v-if="transfer.transfers.length > 0 || transfer.showPanel" />
    <StatusBar />
    <ConnectionModal v-if="conn.showConnectionModal" :profile="editingProfile" @close="handleCloseModal" />
    <GroupManager v-if="showGroupManager" @close="showGroupManager = false" />
    <ConflictDialog />
    <HostKeyDialog />
    <SettingsPanel v-if="themeStore.showSettings" @close="themeStore.showSettings = false" />
    <ToastNotification />
  </div>
</template>

<style>
:root {
  --frame: #141820;
  --panel: #1A1F2B;
  --surface: #222836;
  --elevated: #2A3142;
  --hover: #333C50;
  --active: #3D4759;
  --border: #282F3E;
  --divider: #1E2433;
  --text-primary: #E2E6ED;
  --text-secondary: #8892A4;
  --text-muted: #5A6478;
  --accent: #E8A84C;
  --accent-hover: #F0B860;
  --accent-dim: rgba(232, 168, 76, 0.12);
  --success: #4CAF7D;
  --error: #CF5C4F;
  --info: #5B8FD9;
  --font-ui: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  --font-mono: 'Cascadia Code', 'Fira Code', 'Consolas', 'Courier New', monospace;
  --titlebar-h: 38px;
  --statusbar-h: 26px;
  --panel-header-h: 36px;
}

*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
html, body { height: 100%; min-height: 100vh; overflow: hidden; }
body {
  font-family: var(--font-ui);
  font-size: 13px;
  color: var(--text-primary);
  background: var(--frame);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}
button {
  font-family: inherit;
  font-size: inherit;
  color: inherit;
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
}
input { font-family: inherit; font-size: inherit; }
::selection { background: var(--accent-dim); }

.app {
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  user-select: none;
  position: relative;
}
</style>

<style scoped>
.workspace {
  flex: 1;
  display: flex;
  overflow: hidden;
  position: relative;
}
.panel {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 200px;
}
.panel--terminal {
  background: var(--panel);
  flex: 1;
}
.panel-header {
  height: var(--panel-header-h);
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
  flex-shrink: 0;
}
.panel-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 100%;
}
.tab-label { max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tab-close {
  margin-left: 4px;
  font-size: 14px;
  line-height: 1;
  color: var(--text-muted);
  transition: color 0.12s;
  border-radius: 3px;
  padding: 0 2px;
}
.tab-close:hover { color: var(--error); background: rgba(207, 92, 79, 0.15); }
.tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 14px;
  height: 28px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
  background: none;
  border: none;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
  font-family: inherit;
}
.tab:hover { background: var(--elevated); color: var(--text-primary); }
.tab.active { background: var(--panel); color: var(--text-primary); }
.tab i { font-size: 11px; }
.tab-add { color: var(--text-muted); }
.tab-add:hover { color: var(--accent); background: transparent; }
.panel-actions {
  display: flex;
  align-items: center;
  gap: 1px;
}
.panel-actions button {
  width: 28px;
  height: 28px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--text-muted);
  background: none;
  border: none;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
  font-family: inherit;
}
.panel-actions button:hover {
  background: var(--elevated);
  color: var(--text-primary);
}
.terminal-body {
  flex: 1;
  overflow: hidden;
  position: relative;
}

/* SFTP slide transition */
.sftp-slide-enter-active,
.sftp-slide-leave-active {
  transition: transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}
.sftp-slide-enter-from,
.sftp-slide-leave-to {
  transform: translateX(100%);
}

.welcome {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}
.welcome-box {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}
.welcome-logo {
  font-size: 32px;
  color: var(--accent);
  opacity: 0.6;
  margin-bottom: 8px;
}
.welcome-title {
  font-size: 26px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.3px;
}
.welcome-sub {
  color: var(--text-muted);
  font-size: 14px;
}
.welcome-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 10px 24px;
  background: var(--accent);
  border: none;
  border-radius: 7px;
  color: var(--frame);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
  font-family: inherit;
}
.welcome-btn:hover {
  background: var(--accent-hover);
}
.welcome-btn.secondary {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text-primary);
}
.welcome-btn.secondary:hover {
  background: var(--elevated);
  border-color: var(--hover);
}
.welcome-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.welcome-btn:disabled:hover {
  background: var(--accent);
}
.local-picker {
  display: flex;
  gap: 8px;
  align-items: center;
}
.shell-select {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 7px;
  color: var(--text-primary);
  font-family: inherit;
  font-size: 13px;
  padding: 9px 30px 9px 14px;
  cursor: pointer;
  outline: none;
  appearance: none;
  -webkit-appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6'%3E%3Cpath d='M0 0l5 6 5-6z' fill='%235A6478'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 12px center;
  min-width: 160px;
  transition: border-color 0.12s;
}
.shell-select:hover {
  border-color: var(--hover);
}
.shell-select:focus {
  border-color: var(--accent);
}
.shell-select option {
  background: var(--surface);
  color: var(--text-primary);
}

.saved-conns {
  margin-top: 24px;
  width: 100%;
  max-width: 520px;
  text-align: left;
}
.conn-group {
  margin-bottom: 16px;
}
.conn-group-title {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  margin-bottom: 8px;
  padding-left: 2px;
}
.conn-group-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.conn-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}
.conn-card:hover {
  background: var(--elevated);
  border-color: var(--hover);
}
.conn-card i {
  color: var(--accent);
  font-size: 14px;
  width: 18px;
  text-align: center;
  flex-shrink: 0;
}
.conn-card-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}
.conn-card-name {
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.conn-card-host {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}
.conn-card-action {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: var(--text-muted);
  background: none;
  border: none;
  cursor: pointer;
  transition: color 0.12s, background 0.12s;
  flex-shrink: 0;
  opacity: 0;
}
.conn-card:hover .conn-card-action { opacity: 1; }
.conn-card-action:hover { color: var(--text-primary); background: var(--elevated); }
.conn-card-action.danger:hover { color: var(--error); background: rgba(207, 92, 79, 0.12); }

.conn-search-row {
  position: relative;
  margin-bottom: 14px;
}
.conn-search-icon {
  position: absolute;
  left: 10px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 11px;
  color: var(--text-muted);
  pointer-events: none;
}
.conn-search-input {
  width: 100%;
  height: 32px;
  padding: 0 28px 0 28px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-primary);
  font-size: 12px;
  outline: none;
  transition: border-color 0.15s;
  box-sizing: border-box;
  font-family: inherit;
}
.conn-search-input:focus { border-color: var(--accent); }
.conn-search-input::placeholder { color: var(--text-muted); }
.conn-search-clear {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  width: 20px;
  height: 20px;
  border-radius: 4px;
  border: none;
  background: none;
  color: var(--text-muted);
  font-size: 13px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}
.conn-search-clear:hover { color: var(--text-primary); background: var(--elevated); }
.conn-empty {
  text-align: center;
  color: var(--text-muted);
  font-size: 12px;
  padding: 20px 0;
}
</style>
