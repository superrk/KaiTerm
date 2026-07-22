<script setup>
import { useConnectionStore } from "../stores/connection";
import { useThemeStore } from "../stores/theme";
import { getCurrentWindow } from "@tauri-apps/api/window";

const conn = useConnectionStore();
const themeStore = useThemeStore();
const appWindow = getCurrentWindow();

function switchToSession(s) {
  conn.setActiveSession(s.id);
  conn.closeDropdown();
}

function openNewConnection() {
  conn.closeDropdown();
  conn.showConnectionModal = true;
}

async function handleLocalClick(shellId) {
  conn.closeDropdown();
  try {
    await conn.connectLocal(shellId);
  } catch (e) {
    conn.pushToast("启动本地终端失败: " + e, "error");
  }
}

async function connectSavedProfile(p) {
  conn.closeDropdown();
  try {
    let password = null;
    let keyPath = null;
    let useAgent = false;
    if (p.authMethod === "password") {
      password = p.id ? await conn.getCredential(p.id) : (p.password || null);
    } else if (p.authMethod === "key") {
      keyPath = p.id ? await conn.getCredential(p.id) : (p.keyPath || null);
    } else if (p.authMethod === "agent") {
      useAgent = true;
    }
    await conn.connect(p.host, p.port, p.user, password, keyPath, p.name, useAgent, p.sync_dir, p.id);
  } catch (e) {
    conn.pushToast("连接失败: " + e, "error");
  }
}
</script>

<template>
  <header class="titlebar">
    <div class="titlebar-left">
      <button
        class="sidebar-toggle-btn"
        :class="{ active: !conn.sidebarCollapsed }"
        @click="conn.sidebarCollapsed = !conn.sidebarCollapsed"
        :title="conn.sidebarCollapsed ? '展开侧边栏' : '收起侧边栏'"
      ><i class="fas fa-bars"></i></button>
      <span class="app-logo"><i class="fas fa-terminal"></i></span>
      <span class="app-name">KaiTerm</span>
    </div>

    <div class="titlebar-center">
      <button class="connection-btn" @click.stop="conn.toggleDropdown()" aria-label="当前连接">
        <span class="connection-dot" :class="{ online: conn.sessions.length > 0 }"></span>
        <span>{{ conn.currentLabel }}</span>
        <i class="fas fa-chevron-down"></i>
      </button>

      <Teleport to="body">
        <div v-if="conn.showDropdown" class="conn-dropdown-overlay" @click="conn.closeDropdown()"></div>
      </Teleport>
      <div class="conn-dropdown" :class="{ active: conn.showDropdown }">
          <div class="conn-dropdown-item submenu-trigger">
            <i class="fas fa-terminal"></i>
            <span>本地终端</span>
            <i class="fas fa-chevron-right submenu-arrow"></i>
            <div class="submenu">
              <div
                v-for="s in conn.shells"
                :key="s.id"
                class="submenu-item"
                @click="handleLocalClick(s.id)"
              >
                <i class="fas" :class="s.id === 'cmd' ? 'fa-terminal' : 'fa-laptop-code'"></i>
                <span class="conn-name">{{ s.name }}</span>
              </div>
            </div>
          </div>

          <div class="conn-dropdown-sep"></div>

          <div v-if="conn.sessions.length > 0" class="conn-dropdown-label">当前会话</div>

          <div
            v-for="s in conn.sessions"
            :key="s.id"
            class="conn-dropdown-item"
            :class="{ current: s.id === conn.activeSessionId }"
            @click="switchToSession(s)"
          >
            <span class="saved-dot online"></span>
            <div class="conn-info">
              <span class="conn-name">{{ s.type === 'local' ? s.label : s.user + '@' + s.host }}</span>
              <span class="conn-host">{{ s.type === 'local' ? (s.shellType === 'cmd' ? 'CMD' : 'PowerShell') : s.user + '@' + s.host + ':' + s.port }}</span>
            </div>
          </div>

          <div v-if="conn.profiles.length > 0" class="conn-dropdown-label">已保存的连接</div>
          <div
            v-for="p in conn.profiles"
            :key="`${p.host}|${p.port}|${p.user}`"
            class="conn-dropdown-item"
            @click="connectSavedProfile(p)"
          >
            <span class="saved-dot"></span>
            <div class="conn-info">
              <span class="conn-name">{{ p.name || p.host }}</span>
              <span class="conn-host">{{ p.user }}@{{ p.host }}:{{ p.port }}</span>
            </div>
          </div>
          <div v-if="conn.profiles.length > 0" class="conn-dropdown-sep"></div>

          <div class="conn-dropdown-item new-item" @click="openNewConnection">
            <i class="fas fa-plus"></i>
            <span>新建连接</span>
          </div>
        </div>
    </div>

    <div class="titlebar-right">
      <button v-if="conn.activeSession?.type === 'local'" title="本地文件" aria-label="本地文件" @click="conn.toggleLocalFilePanel()" :class="{ active: conn.showLocalFilePanel }">
        <i class="fas fa-folder"></i>
      </button>
      <button v-if="conn.activeSession?.type === 'ssh'" title="文件管理器" aria-label="文件管理器" @click="conn.toggleSftpPanel()" :class="{ active: conn.showSftpPanel }">
        <i class="fas fa-folder-open"></i>
      </button>
      <button v-if="conn.activeSession?.type === 'ssh'" title="系统信息" aria-label="系统信息" @click="conn.toggleSysInfoPanel()" :class="{ active: conn.showSysInfoPanel }">
        <i class="fas fa-chart-bar"></i>
      </button>
      <button class="settings-btn" title="设置" aria-label="设置" @click="themeStore.showSettings = true">
        <i class="fas fa-gear"></i>
      </button>
      <div class="win-controls">
        <button class="win-btn" @click="appWindow.minimize()" title="最小化" aria-label="最小化">
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor"/></svg>
        </button>
        <button class="win-btn" @click="appWindow.toggleMaximize()" title="最大化" aria-label="最大化">
          <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1"/></svg>
        </button>
        <button class="win-btn win-btn-close" @click="appWindow.close()" title="关闭" aria-label="关闭">
          <svg width="10" height="10" viewBox="0 0 10 10"><path d="M1 1L9 9M9 1L1 9" stroke="currentColor" stroke-width="1.2"/></svg>
        </button>
      </div>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  height: var(--titlebar-h);
  background: var(--frame);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  padding: 0 12px;
  gap: 16px;
  flex-shrink: 0;
  -webkit-app-region: drag;
  user-select: none;
  position: relative;
  z-index: 60;
}
.titlebar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.app-logo { color: var(--accent); font-size: 14px; }
.sidebar-toggle-btn {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: 1px solid var(--border);
  color: var(--text-muted);
  font-size: 13px;
  cursor: pointer;
  transition: color 0.15s, border-color 0.15s;
  flex-shrink: 0;
  -webkit-app-region: no-drag;
}
.sidebar-toggle-btn:hover { color: var(--text-primary); border-color: var(--text-muted); }
.sidebar-toggle-btn.active { color: var(--accent); border-color: var(--accent); }
.app-name {
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 13px;
  letter-spacing: -0.3px;
  color: var(--text-secondary);
}
.titlebar-center {
  flex: 1;
  display: flex;
  justify-content: center;
  position: relative;
}
.connection-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 14px;
  border-radius: 6px;
  background: var(--surface);
  border: 1px solid var(--border);
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-primary);
  cursor: pointer;
  -webkit-app-region: no-drag;
  transition: background 0.15s, border-color 0.15s;
  height: 30px;
}
.connection-btn:hover {
  background: var(--elevated);
  border-color: var(--hover);
}
.connection-btn:hover .fa-chevron-down { color: var(--text-secondary); }
.connection-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-muted);
  flex-shrink: 0;
}
.connection-dot.online {
  background: var(--success);
  box-shadow: 0 0 6px rgba(76, 175, 125, 0.4);
}
.connection-btn .fa-chevron-down {
  font-size: 9px;
  color: var(--text-muted);
  margin-left: 4px;
  transition: color 0.15s;
}
.titlebar-right {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}
.titlebar-right button {
  width: 30px;
  height: 30px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
  font-size: 13px;
  background: none;
  border: none;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
  font-family: inherit;
  -webkit-app-region: no-drag;
}
.titlebar-right button:hover {
  background: var(--elevated);
  color: var(--text-primary);
}
.titlebar-right button.active {
  color: var(--accent);
  background: var(--accent-dim);
}
.settings-btn {
  margin-left: 4px;
}
.win-controls {
  display: flex;
  align-items: center;
  margin-left: 6px;
  padding-left: 8px;
  border-left: 1px solid var(--border);
  gap: 0;
  -webkit-app-region: no-drag;
}
.win-btn {
  width: 36px;
  height: var(--titlebar-h);
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
  -webkit-app-region: no-drag;
}
.win-btn:hover { background: var(--elevated); color: var(--text-primary); }
.win-btn-close:hover { background: var(--error); color: #fff; }

.conn-dropdown-overlay {
  position: fixed;
  inset: 0;
  z-index: 49;
}
.conn-dropdown {
  position: absolute;
  top: calc(100% + 6px);
  left: 50%;
  width: 320px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
  padding: 8px;
  z-index: 50;
  opacity: 0;
  pointer-events: none;
  transform: translateX(-50%) translateY(-4px);
  transition: opacity 0.12s ease, transform 0.12s ease;
  -webkit-app-region: no-drag;
}
.conn-dropdown.active {
  opacity: 1;
  pointer-events: auto;
  transform: translateX(-50%) translateY(0);
}
.conn-dropdown-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  padding: 4px 10px 8px;
}
.conn-dropdown-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s;
}
.conn-dropdown-item:hover { background: var(--elevated); }
.conn-dropdown-item.current { background: var(--accent-dim); }
.conn-dropdown-item .conn-name {
  font-size: 13px;
  font-weight: 500;
  flex: 1;
}
.conn-dropdown-item .conn-host {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}
.conn-dropdown-item .conn-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  flex: 1;
}
.conn-dropdown-sep {
  height: 1px;
  background: var(--border);
  margin: 4px 0;
}
.conn-dropdown-item.new-item {
  color: var(--accent);
  font-weight: 500;
  font-size: 12px;
}
.conn-dropdown-item.new-item i { width: 16px; text-align: center; font-size: 11px; }
.saved-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-muted);
  flex-shrink: 0;
}
.saved-dot.online { background: var(--success); box-shadow: 0 0 4px rgba(76, 175, 125, 0.3); }
.submenu-trigger {
  position: relative;
  color: var(--success);
  font-weight: 600;
}
.submenu-trigger i:first-child { color: var(--success); }
.submenu-trigger .submenu-arrow {
  margin-left: auto;
  font-size: 10px;
  color: var(--success);
}
.submenu {
  display: none;
  position: absolute;
  left: 100%;
  top: -4px;
  width: 280px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.4);
  padding: 6px;
  z-index: 51;
}
.submenu-trigger:hover .submenu {
  display: block;
}
.submenu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s;
}
.submenu-item:hover { background: var(--elevated); }
.submenu-item i { color: var(--accent); width: 16px; text-align: center; }
.submenu-item .conn-name { font-size: 13px; font-weight: 500; color: var(--accent); }
.submenu-item .conn-host {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}
.submenu-item .conn-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  flex: 1;
}
</style>
