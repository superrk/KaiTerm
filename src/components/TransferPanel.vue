<script setup>
import { useTransferStore } from "../stores/transfer";

const transfer = useTransferStore();

function speedStr(bps) {
  if (!bps) return "—";
  const u = ["B/s", "KB/s", "MB/s"];
  let i = 0;
  let s = bps;
  while (s >= 1024 && i < u.length - 1) { s /= 1024; i++; }
  return s.toFixed(1) + " " + u[i];
}

function sizeStr(n) {
  if (!n) return "—";
  const u = ["B", "KB", "MB", "GB"];
  let i = 0;
  let s = n;
  while (s >= 1024 && i < u.length - 1) { s /= 1024; i++; }
  return s.toFixed(1) + " " + u[i];
}
</script>

<template>
  <div class="transfer-panel">
    <div class="tp-header">
      <span class="tp-title">传输队列</span>
      <span v-if="transfer.transfers.length" class="tp-badge">{{ transfer.transfers.length }}</span>
      <div class="tp-actions">
        <button class="tp-btn" @click="transfer.clearCompleted()">清除已完成</button>
        <button class="tp-btn" @click="transfer.showPanel = false">
          <i class="fas fa-times"></i>
        </button>
      </div>
    </div>
    <div v-if="transfer.transfers.length === 0" class="tp-empty">无传输任务</div>
    <div v-else class="tp-list">
      <div
        v-for="t in transfer.transfers"
        :key="t.id"
        class="tp-item"
        :class="t.status"
      >
        <div class="tp-row">
          <span class="tp-name">{{ t.filename }}</span>
          <span class="tp-status" :class="t.status">
            {{ t.status === 'uploading' ? '上传中' : t.status === 'downloading' ? '下载中' : t.status === 'completed' ? '已完成' : t.status === 'cancelled' ? '已取消' : t.status }}
          </span>
        </div>
        <div class="tp-bar-wrap">
          <div class="tp-bar" :style="{ width: t.total > 0 ? (t.transferred / t.total * 100) + '%' : '0%' }"></div>
        </div>
        <div class="tp-meta">
          <span>{{ sizeStr(t.transferred) }} / {{ sizeStr(t.total) }}</span>
          <span>{{ speedStr(t.speed) }}</span>
          <span v-if="t.error" class="tp-err">{{ t.error }}</span>
          <button
            v-if="t.status === 'uploading' || t.status === 'downloading'"
            class="tp-cancel"
            @click="transfer.cancelTransfer(t.id)"
          >取消</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.transfer-panel {
  background: var(--surface);
  border-top: 1px solid var(--border);
  max-height: 200px;
  overflow-y: auto;
  flex-shrink: 0;
  scrollbar-width: thin;
  scrollbar-color: var(--active) transparent;
}
.transfer-panel::-webkit-scrollbar { width: 5px; }
.transfer-panel::-webkit-scrollbar-thumb { background: var(--active); border-radius: 3px; }
.tp-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-bottom: 1px solid var(--border);
  position: sticky;
  top: 0;
  background: var(--surface);
}
.tp-title {
  font-size: 12px;
  color: var(--text-secondary);
  font-weight: 500;
}
.tp-badge {
  font-size: 10px;
  background: var(--elevated);
  color: var(--text-secondary);
  padding: 1px 6px;
  border-radius: 8px;
}
.tp-actions {
  margin-left: auto;
  display: flex;
  gap: 4px;
}
.tp-btn {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 3px 8px;
  border-radius: 4px;
  font-size: 11px;
  transition: color 0.15s, background 0.15s;
  font-family: inherit;
}
.tp-btn:hover {
  color: var(--text-secondary);
  background: var(--elevated);
}
.tp-empty {
  padding: 24px;
  text-align: center;
  color: var(--text-muted);
  font-size: 13px;
}
.tp-list { padding: 4px 0; }
.tp-item {
  padding: 8px 14px;
  border-bottom: 1px solid var(--divider);
}
.tp-item.completed { opacity: 0.5; }
.tp-item.cancelled { opacity: 0.35; }
.tp-item.error { border-left: 3px solid var(--error); }
.tp-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 5px;
}
.tp-name {
  color: var(--text-primary);
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}
.tp-status {
  font-size: 10px;
  padding: 1px 7px;
  border-radius: 4px;
  background: var(--elevated);
  color: var(--text-muted);
  flex-shrink: 0;
}
.tp-status.uploading, .tp-status.downloading { color: var(--accent); background: var(--accent-dim); }
.tp-status.completed { color: var(--success); background: rgba(76, 175, 125, 0.1); }
.tp-status.cancelled { color: var(--text-muted); }
.tp-status.error { color: var(--error); background: rgba(207, 92, 79, 0.1); }
.tp-bar-wrap {
  height: 3px;
  background: var(--elevated);
  border-radius: 2px;
  margin-bottom: 5px;
  overflow: hidden;
}
.tp-bar {
  height: 100%;
  background: linear-gradient(90deg, var(--accent), var(--accent-hover));
  border-radius: 2px;
  transition: width 0.3s ease;
}
.tp-item.completed .tp-bar { background: linear-gradient(90deg, var(--success), #6BCF9A); }
.tp-item.error .tp-bar { background: linear-gradient(90deg, var(--error), #E07A6E); }
.tp-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 11px;
  color: var(--text-muted);
}
.tp-err { color: var(--error); }
.tp-cancel {
  margin-left: auto;
  background: none;
  border: 1px solid var(--text-muted);
  color: var(--text-muted);
  padding: 1px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 10px;
  transition: color 0.15s, border-color 0.15s;
  font-family: inherit;
}
.tp-cancel:hover {
  color: var(--error);
  border-color: var(--error);
}
</style>
