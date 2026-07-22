<script setup>
import { ref, onMounted, watch, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useConnectionStore } from "../stores/connection";

const emit = defineEmits(["close"]);
const conn = useConnectionStore();

const currentPath = ref("");
const entries = ref([]);
const loading = ref(false);
const selectedPath = ref(null);
const drives = ref([]);
const showGoTo = ref(false);
const goToPath = ref("");

const breadcrumbs = computed(() => {
  if (!currentPath.value) return [];
  const parts = currentPath.value.split(/[\\/]/).filter(Boolean);
  const crumbs = [];
  for (let i = 0; i < parts.length; i++) {
    crumbs.push({
      name: parts[i],
      path: parts.slice(0, i + 1).join("\\"),
    });
  }
  return crumbs;
});

async function loadDrives() {
  for (let c = 65; c <= 90; c++) {
    const letter = String.fromCharCode(c);
    try {
      const entries = await invoke("list_local_files", { path: `${letter}:\\` });
      drives.value.push({ letter, label: `${letter}:`, entries });
    } catch {}
  }
}

async function listDir(path) {
  loading.value = true;
  entries.value = [];
  try {
    const result = await invoke("list_local_files", { path });
    entries.value = result;
    currentPath.value = path;
  } catch (e) {
    entries.value = [];
  } finally {
    loading.value = false;
  }
}

function enterDir(entry) {
  if (entry.is_dir) {
    listDir(entry.path);
  }
}

function goUp() {
  if (!currentPath.value) return;
  const parts = currentPath.value.split(/[\\/]/).filter(Boolean);
  if (parts.length <= 1) {
    currentPath.value = "";
    entries.value = [];
    return;
  }
  parts.pop();
  const parent = parts.join("\\");
  listDir(parent);
}

function selectRow(path) {
  selectedPath.value = selectedPath.value === path ? null : path;
}

function openInExplorer() {
  const target = selectedPath.value || currentPath.value;
  if (target) {
    invoke("open_in_explorer", { path: target });
  }
}

function navigateCrumb(idx) {
  if (idx < 0) {
    currentPath.value = "";
    entries.value = [];
    return;
  }
  listDir(breadcrumbs.value[idx].path);
}

function handleGoTo() {
  if (goToPath.value) {
    listDir(goToPath.value);
    showGoTo.value = false;
    goToPath.value = "";
  }
}

function formatSize(n) {
  if (n === 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let size = n;
  while (size >= 1024 && i < units.length - 1) { size /= 1024; i++; }
  return `${size.toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
}

function formatDate(ts) {
  if (!ts) return "";
  const d = new Date(Number(ts) * 1000);
  const months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
  return `${months[d.getMonth()]} ${String(d.getDate()).padStart(2,"0")} ${String(d.getHours()).padStart(2,"0")}:${String(d.getMinutes()).padStart(2,"0")}`;
}

onMounted(() => {
  loadDrives();
});
</script>

<template>
  <div class="panel panel--local">
    <div class="panel-header">
      <span class="panel-title"><i class="fas fa-folder"></i> 本地文件</span>
      <div class="panel-actions">
        <button title="转到路径" @click="showGoTo = !showGoTo"><i class="fas fa-location-arrow"></i></button>
        <button title="在资源管理器中打开" @click="openInExplorer" :disabled="!selectedPath && !currentPath"><i class="fas fa-external-link-alt"></i></button>
        <button title="向上" @click="goUp" :disabled="!currentPath"><i class="fas fa-level-up-alt"></i></button>
        <button title="关闭" @click="emit('close')"><i class="fas fa-times"></i></button>
      </div>
    </div>

    <div v-if="showGoTo" class="goto-bar">
      <input
        v-model="goToPath"
        placeholder="输入路径，如 C:\Users"
        @keydown.enter="handleGoTo"
        @keydown.escape="showGoTo = false"
        autofocus
      />
      <button @click="handleGoTo">转到</button>
    </div>

    <nav class="local-path" v-if="currentPath">
      <a @click="navigateCrumb(-1)"><i class="fas fa-home"></i></a>
      <span class="sep" v-for="(crumb, i) in breadcrumbs" :key="i">
        <span class="arrow">&gt;</span>
        <a @click="navigateCrumb(i)">{{ crumb.name }}</a>
      </span>
    </nav>

    <div v-if="!currentPath" class="drive-grid">
      <div
        v-for="d in drives"
        :key="d.letter"
        class="drive-item"
        @click="listDir(d.label + '\\')"
      >
        <i class="fas fa-hdd"></i>
        <span>{{ d.label }}</span>
      </div>
    </div>

    <div class="local-table-wrap">
      <table class="local-table" v-if="currentPath">
        <thead>
          <tr>
            <th>名称</th>
            <th class="col-size">大小</th>
            <th class="col-date">修改时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading">
            <td colspan="3" class="loading-row">加载中...</td>
          </tr>
          <tr
            v-for="entry in entries"
            :key="entry.path"
            class="entry-row"
            :class="{ selected: selectedPath === entry.path }"
            @click="selectRow(entry.path)"
            @dblclick="enterDir(entry)"
          >
            <td class="col-name">
              <i class="fas" :class="entry.is_dir ? 'fa-folder' : 'fa-file'"></i>
              <span class="name-text">{{ entry.name }}</span>
            </td>
            <td class="col-size">{{ entry.is_dir ? "" : formatSize(entry.size) }}</td>
            <td class="col-date">{{ formatDate(entry.modified) }}</td>
          </tr>
          <tr v-if="!loading && entries.length === 0 && currentPath">
            <td colspan="3" class="empty-row">空目录</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.panel--local {
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: 340px;
  background: var(--surface);
  border-left: 1px solid var(--border);
  box-shadow: -4px 0 20px rgba(0,0,0,0.25);
  z-index: 20;
  display: flex;
  flex-direction: column;
  overflow: hidden;
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
.panel-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
  display: flex;
  align-items: center;
  gap: 6px;
}
.panel-title i { color: var(--accent); font-size: 13px; }
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
.panel-actions button:hover { background: var(--elevated); color: var(--text-primary); }
.panel-actions button:disabled { opacity: 0.3; cursor: not-allowed; }

.goto-bar {
  display: flex;
  gap: 6px;
  padding: 8px;
  border-bottom: 1px solid var(--border);
}
.goto-bar input {
  flex: 1;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 5px;
  color: var(--text-primary);
  padding: 5px 10px;
  font-size: 12px;
  font-family: var(--font-mono);
  outline: none;
}
.goto-bar input:focus { border-color: var(--accent); }
.goto-bar button {
  padding: 5px 12px;
  background: var(--accent);
  border: none;
  border-radius: 5px;
  color: var(--frame);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  font-family: inherit;
}

.local-path {
  padding: 6px 10px;
  font-size: 12px;
  color: var(--text-secondary);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  gap: 2px;
  flex-wrap: wrap;
  flex-shrink: 0;
}
.local-path a {
  color: var(--accent);
  cursor: pointer;
  text-decoration: none;
}
.local-path a:hover { text-decoration: underline; }
.local-path .sep { display: inline-flex; align-items: center; gap: 2px; }
.local-path .arrow { color: var(--text-muted); margin: 0 2px; }

.drive-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 12px;
  flex-shrink: 0;
}
.drive-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 7px;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}
.drive-item:hover { background: var(--elevated); border-color: var(--hover); }
.drive-item i { color: var(--accent); font-size: 16px; }

.local-table-wrap {
  flex: 1;
  overflow: auto;
}
.local-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}
.local-table thead {
  position: sticky;
  top: 0;
  z-index: 1;
}
.local-table th {
  background: var(--surface);
  color: var(--text-muted);
  font-weight: 600;
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 6px 10px;
  text-align: left;
  border-bottom: 1px solid var(--border);
}
.col-name { width: auto; }
.col-size { width: 80px; text-align: right; }
.col-date { width: 110px; }
.entry-row { cursor: pointer; transition: background 0.1s; }
.entry-row:hover { background: var(--elevated); }
.entry-row.selected { background: var(--accent-dim); border-left: 2px solid var(--accent); }
.entry-row td { padding: 5px 10px; border-bottom: 1px solid var(--border); white-space: nowrap; }
.entry-row .col-name { display: flex; align-items: center; gap: 8px; }
.entry-row .col-name i { width: 14px; text-align: center; color: var(--accent); font-size: 13px; }
.entry-row .col-size { font-family: var(--font-mono); font-size: 11px; color: var(--text-muted); }
.entry-row .col-date { font-family: var(--font-mono); font-size: 11px; color: var(--text-muted); }
.name-text { overflow: hidden; text-overflow: ellipsis; }
.loading-row, .empty-row { text-align: center; color: var(--text-muted); padding: 20px; }
</style>
