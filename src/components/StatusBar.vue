<script setup>
import { useConnectionStore } from "../stores/connection";

const conn = useConnectionStore();
</script>

<template>
  <footer class="statusbar">
    <div class="statusbar-left">
      <span v-if="conn.activeSession" class="status-connected">
        <i class="fas fa-circle"></i> 已连接
      </span>
      <span v-else class="status-connected" style="color:var(--text-muted)">
        <i class="fas fa-circle"></i> 未连接
      </span>
      <span class="status-sep">|</span>
      <span class="status-stat">{{ conn.activeSession?.type === 'local' ? '本地终端' : 'SSH-2.0 / SFTP' }}</span>
    </div>
    <div class="statusbar-right">
      <template v-if="conn.activeSession">
        <template v-if="conn.activeSession.type === 'local'">
          <span class="status-stat">{{ conn.activeSession.label || conn.activeSession.shellType }}</span>
        </template>
        <template v-else>
          <span class="status-stat">{{ conn.activeSession.host }}</span>
          <span class="status-sep">|</span>
          <span class="status-stat">{{ conn.activeSession.user }}@{{ conn.activeSession.host }}</span>
          <template v-if="conn.activeSession.status === 'disconnected'">
            <span class="status-sep">|</span>
            <span class="status-disconnected"><i class="fas fa-exclamation-triangle"></i> 连接已断开</span>
            <button class="reconnect-btn" @click="conn.reconnect(conn.activeSession.id)" title="重新连接">
              <i class="fas fa-redo"></i> 重连
            </button>
          </template>
          <template v-else-if="conn.activeSession.status === 'reconnecting'">
            <span class="status-sep">|</span>
            <span class="status-reconnecting"><i class="fas fa-spinner spin"></i> 重连中…</span>
          </template>
        </template>
        <span class="status-sep">|</span>
        <button
          class="sftp-toggle"
          :class="{ active: conn.showSftpPanel }"
          @click="conn.showSftpPanel = !conn.showSftpPanel"
          title="文件面板"
        >
          <i class="fas fa-folder-open"></i>
          <span>文件</span>
        </button>
      </template>
    </div>
  </footer>
</template>

<style scoped>
.statusbar {
  height: var(--statusbar-h);
  background: var(--surface);
  border-top: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 14px;
  font-size: 11px;
  color: var(--text-muted);
  flex-shrink: 0;
}
.statusbar-left, .statusbar-right {
  display: flex;
  align-items: center;
  gap: 14px;
}
.status-connected {
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--success);
  font-weight: 500;
}
.status-connected i { font-size: 6px; }
.status-sep { color: var(--border); }
.status-disconnected {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--error);
  font-weight: 500;
}
.status-reconnecting {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--accent);
  font-weight: 500;
}
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.reconnect-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--error);
  color: #fff;
  border: none;
  border-radius: 3px;
  cursor: pointer;
  font-family: var(--font-mono);
  font-size: 10px;
  padding: 2px 7px;
  margin-left: 6px;
  transition: background 0.12s;
}
.reconnect-btn:hover { background: #e06a5c; }
.reconnect-btn i { font-size: 10px; }
.status-stat {
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: -0.2px;
}
.sftp-toggle {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-family: var(--font-mono);
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 3px;
  transition: color 0.12s, background 0.12s;
}
.sftp-toggle:hover { color: var(--text-primary); background: var(--elevated); }
.sftp-toggle.active { color: var(--accent); }
.sftp-toggle i { font-size: 10px; }
</style>
