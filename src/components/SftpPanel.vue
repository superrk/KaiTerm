<script setup>
import { ref, watch, computed, onMounted, onUnmounted } from "vue";
const emit = defineEmits(["close"]);
import { useConnectionStore } from "../stores/connection";
import { useSftpStore } from "../stores/sftp";
import { useTransferStore } from "../stores/transfer";
import { listen } from "@tauri-apps/api/event";
import { open, save, confirm } from "@tauri-apps/plugin-dialog";
import { safeInvoke } from "../stores/safeInvoke";

const conn = useConnectionStore();
const sftp = useSftpStore();
const transfer = useTransferStore();

const newFolderName = ref("");
const showNewFolder = ref(false);
const selectedPath = ref(null);
const renamingPath = ref(null);
const renameValue = ref("");
const sortCol = ref("name");
const sortDir = ref("asc");
const panelWidth = computed({
  get: () => conn.sftpPanelWidth,
  set: (v) => conn.setSftpPanelWidth(v),
});

const sortedEntries = computed(() => {
  const arr = [...sftp.entries];
  const col = sortCol.value;
  const dir = sortDir.value === "asc" ? 1 : -1;
  arr.sort((a, b) => {
    if (a.is_dir !== b.is_dir) return b.is_dir ? 1 : -1;
    if (col === "size") return (a.size - b.size) * dir;
    if (col === "modified") return (String(a.modified) > String(b.modified) ? 1 : -1) * dir;
    return a.name.localeCompare(b.name) * dir;
  });
  return arr;
});

function toggleSort(col) {
  if (sortCol.value === col) {
    sortDir.value = sortDir.value === "asc" ? "desc" : "asc";
  } else {
    sortCol.value = col;
    sortDir.value = "asc";
  }
}

const breadcrumbs = computed(() => {
  const parts = sftp.currentPath.split("/").filter(Boolean);
  const crumbs = [{ label: "/", path: "/" }];
  let acc = "";
  for (const p of parts) {
    acc += "/" + p;
    crumbs.push({ label: p, path: acc });
  }
  return crumbs;
});

onMounted(() => {
  if (conn.activeSessionId) sftp.listDir(conn.activeSessionId, "/");
  // 监听后端目录跟随事件：仅当开关打开时后端才会发此事件；
  // 收到后把文件面板切到终端当前目录。
  listen("sftp-cwd-changed", (event) => {
    const { session_id, cwd } = event.payload;
    if (session_id === conn.activeSessionId) {
      // 路径一致时跳过，避免不必要的 sftp_list_dir 请求
      const current = sftp.currentPath.replace(/\/+$/, "") || "/";
      const target = (cwd || "/").replace(/\/+$/, "") || "/";
      if (current !== target) {
        sftp.listDir(session_id, cwd);
      }
    }
  }).then((unlisten) => { unlistenCwd = unlisten; });
});

let unlistenCwd = null;
onUnmounted(() => {
  if (unlistenCwd) unlistenCwd();
});

watch(
  () => conn.activeSessionId,
  (id) => {
    if (id) { sftp.currentPath = "/"; sftp.listDir(id, "/"); }
    else sftp.entries = [];
  }
);

function navigateCrumb(idx) {
  if (conn.activeSessionId) sftp.navigateTo(conn.activeSessionId, breadcrumbs.value[idx].path);
}

function enterDir(entry) {
  if (entry.is_dir && conn.activeSessionId) sftp.cd(conn.activeSessionId, entry.path);
}

function goUp() {
  if (conn.activeSessionId) {
    const parent = sftp.parentPath(sftp.currentPath);
    sftp.navigateTo(conn.activeSessionId, parent);
  }
}

function selectRow(path) {
  selectedPath.value = selectedPath.value === path ? null : path;
}

async function handleMkdir() {
  if (!newFolderName.value) return;
  const path = sftp.currentPath.replace(/\/$/, "") + "/" + newFolderName.value;
  await sftp.mkdir(conn.activeSessionId, path);
  newFolderName.value = "";
  showNewFolder.value = false;
}

function startRename(entry) {
  renamingPath.value = entry.path;
  renameValue.value = entry.name;
}

async function handleRename() {
  if (!renameValue.value || !renamingPath.value) return;
  const dir = sftp.parentPath(renamingPath.value);
  const newPath = (dir === "/" ? "" : dir) + "/" + renameValue.value;
  await sftp.rename(conn.activeSessionId, renamingPath.value, newPath);
  renamingPath.value = null;
  renameValue.value = "";
  selectedPath.value = null;
}

async function removeEntry() {
  if (!selectedPath.value) return;
  const entry = sftp.entries.find((e) => e.path === selectedPath.value);
  if (!entry || !await confirm(`确定删除 ${entry.name} ？`)) return;
  await sftp.remove(conn.activeSessionId, entry.path, entry.is_dir);
  selectedPath.value = null;
}

async function uploadFile() {
  const local = await open({ multiple: false, directory: false });
  if (!local) return;
  const path = typeof local === "string" ? local : local.path;
  const name = path.split("\\").pop().split("/").pop();
  const remote = sftp.currentPath.replace(/\/$/, "") + "/" + name;
  transfer.upload(conn.activeSessionId, path, remote).catch((e) => conn.pushToast(e, "error"));
}

async function downloadSelected() {
  if (!selectedPath.value) return;
  const entry = sftp.entries.find((e) => e.path === selectedPath.value);
  if (!entry || entry.is_dir) return;
  const local = await save({ defaultPath: entry.name });
  if (!local) return;
  const path = typeof local === "string" ? local : local.path;
  transfer.download(conn.activeSessionId, entry.path, path).catch((e) => conn.pushToast(e, "error"));
}

function formatSize(n) {
  if (n === 0) return "—";
  const u = ["B", "KB", "MB", "GB"];
  let i = 0;
  let s = n;
  while (s >= 1024 && i < u.length - 1) { s /= 1024; i++; }
  return s.toFixed(1) + " " + u[i];
}

function formatDate(ts) {
  if (!ts) return "—";
  const d = new Date(Number(ts) * 1000);
  const months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
  return `${months[d.getMonth()]} ${d.getDate()} ${String(d.getHours()).padStart(2,"0")}:${String(d.getMinutes()).padStart(2,"0")}`;
}

// --- Resize ---
let resizeStartX = 0;
let resizeStartW = 340;
const MIN_WIDTH = 200;

function onResizeStart(e) {
  resizeStartX = e.clientX;
  resizeStartW = panelWidth.value;
  document.addEventListener("mousemove", onResizeMove);
  document.addEventListener("mouseup", onResizeEnd);
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
}

function onResizeMove(e) {
  const dx = e.clientX - resizeStartX;
  panelWidth.value = Math.max(MIN_WIDTH, resizeStartW - dx);
}

function onResizeEnd() {
  document.removeEventListener("mousemove", onResizeMove);
  document.removeEventListener("mouseup", onResizeEnd);
  document.body.style.cursor = "";
  document.body.style.userSelect = "";
}

// --- Context menu ---
const ctxMenu = ref({ show: false, x: 0, y: 0, entry: null });

function onContextMenu(e, entry) {
  if (entry.is_dir) return;
  e.preventDefault();
  ctxMenu.value = { show: true, x: e.clientX, y: e.clientY, entry };
}

function closeCtxMenu() {
  ctxMenu.value.show = false;
}

// --- Editor ---
const editorFile = ref(null);
const editorContent = ref("");
const editorSaving = ref(false);

async function openEditor(entry) {
  ctxMenu.value.show = false;
  editorFile.value = entry;
  editorSaving.value = false;
  editorContent.value = "";
  try {
    editorContent.value = await safeInvoke("sftp_read_file", {
      sessionId: conn.activeSessionId,
      path: entry.path,
    });
  } catch (e) {
    conn.pushToast(e.includes("不是合法UTF-8") ? "仅支持读取UTF-8文件" : "读取文件失败: " + e, "error");
    editorFile.value = null;
  }
}

async function saveEditor() {
  if (!editorFile.value) return;
  editorSaving.value = true;
  try {
    await safeInvoke("sftp_write_file", {
      sessionId: conn.activeSessionId,
      path: editorFile.value.path,
      content: editorContent.value,
    });
    conn.pushToast("文件已保存", "success");
    editorFile.value = null;
  } catch (e) {
    conn.pushToast("保存文件失败: " + e, "error");
  } finally {
    editorSaving.value = false;
  }
}

function closeEditor() {
  editorFile.value = null;
}

// close context menu on click outside
function onDocClick() {
  if (ctxMenu.value.show) closeCtxMenu();
}
onMounted(() => document.addEventListener("click", onDocClick));
onUnmounted(() => document.removeEventListener("click", onDocClick));
</script>

<template>
  <div class="panel panel--sftp" :style="{ width: panelWidth + 'px' }">
    <div class="resize-handle" @mousedown.prevent="onResizeStart"></div>

    <div class="panel-header">
      <div class="panel-tabs">
        <button class="tab active"><i class="fas fa-folder-open"></i> 文件</button>
      </div>
      <div class="panel-actions">
        <button title="上传" aria-label="上传文件" @click="uploadFile"><i class="fas fa-cloud-upload-alt"></i></button>
        <button title="下载" aria-label="下载选中文件" @click="downloadSelected" :disabled="!selectedPath"><i class="fas fa-cloud-download-alt"></i></button>
        <button title="新建文件夹" aria-label="新建文件夹" @click="showNewFolder = !showNewFolder"><i class="fas fa-folder-plus"></i></button>
        <button title="重命名" aria-label="重命名" @click="startRename(sftp.entries.find(e => e.path === selectedPath))" :disabled="!selectedPath"><i class="fas fa-pen"></i></button>
        <button title="删除" aria-label="删除选中文件" @click="removeEntry" :disabled="!selectedPath"><i class="fas fa-trash-alt"></i></button>
        <button title="刷新" aria-label="刷新文件列表" @click="sftp.listDir(conn.activeSessionId, sftp.currentPath)"><i class="fas fa-sync-alt"></i></button>
        <span class="action-sep"></span>
        <button title="关闭面板" aria-label="关闭文件面板" @click="$emit('close')" class="btn-close-panel"><i class="fas fa-times"></i></button>
      </div>
    </div>

    <nav class="sftp-path" aria-label="当前路径">
      <template v-for="(cr, i) in breadcrumbs" :key="i">
        <span
          class="path-segment"
          :class="{ active: i === breadcrumbs.length - 1 }"
          @click="navigateCrumb(i)"
        >{{ cr.label }}</span>
        <span v-if="i < breadcrumbs.length - 1" class="path-sep">
          <i class="fas fa-chevron-right"></i>
        </span>
      </template>
    </nav>

    <div v-if="showNewFolder" class="new-folder-bar">
      <input v-model="newFolderName" placeholder="文件夹名称" @keyup.enter="handleMkdir" />
      <button @click="handleMkdir" class="sftp-btn-inline primary">创建</button>
      <button @click="showNewFolder = false" class="sftp-btn-inline">取消</button>
    </div>

    <div v-if="renamingPath" class="new-folder-bar">
      <input v-model="renameValue" placeholder="新名称" @keyup.enter="handleRename" />
      <button @click="handleRename" class="sftp-btn-inline primary">确定</button>
      <button @click="renamingPath = null" class="sftp-btn-inline">取消</button>
    </div>

    <div class="sftp-content">
      <table class="sftp-table" role="grid">
        <thead>
          <tr>
            <th @click="toggleSort('name')">名称 <i class="fas fa-sort" :class="{ active: sortCol === 'name' }"></i></th>
            <th @click="toggleSort('size')">大小 <i class="fas fa-sort" :class="{ active: sortCol === 'size' }"></i></th>
            <th @click="toggleSort('modified')">修改时间 <i class="fas fa-sort" :class="{ active: sortCol === 'modified' }"></i></th>
            <th>权限</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="sftp.currentPath !== '/'" class="file-row" data-type="parent" @click="goUp">
            <td>
              <span class="file-icon dir"><i class="fas fa-level-up-alt"></i></span>
              <span class="file-name dir" style="color:var(--text-secondary)">..</span>
            </td>
            <td class="file-meta">—</td>
            <td class="file-meta">—</td>
            <td>—</td>
          </tr>
          <tr v-if="sftp.loading"><td colspan="4" class="file-meta" style="text-align:center;padding:24px">加载中…</td></tr>
          <tr v-else-if="sftp.error"><td colspan="4" class="file-meta" style="text-align:center;padding:24px;color:var(--error)">{{ sftp.error }}</td></tr>
          <tr
            v-for="entry in sortedEntries"
            :key="entry.path"
            class="file-row"
            :class="{ selected: selectedPath === entry.path }"
            :data-type="entry.is_dir ? 'dir' : 'file'"
            @click="selectRow(entry.path)"
            @dblclick="enterDir(entry)"
            @contextmenu.prevent="onContextMenu($event, entry)"
          >
            <td>
              <span v-if="entry.is_dir" class="file-icon dir"><i class="fas fa-folder"></i></span>
              <span v-else class="file-icon file"><i class="fas fa-file-code"></i></span>
              <span class="file-name" :class="{ dir: entry.is_dir }">{{ entry.name }}</span>
            </td>
            <td class="file-meta">{{ entry.is_dir ? "—" : formatSize(entry.size) }}</td>
            <td class="file-meta">{{ formatDate(entry.modified) }}</td>
            <td><span class="file-meta">{{ entry.permissions }}</span></td>
          </tr>
          <tr v-if="!sftp.loading && !sftp.error && sftp.entries.length === 0">
            <td colspan="4" class="file-meta" style="text-align:center;padding:24px;color:var(--text-muted)">空目录</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Context menu -->
    <Teleport to="body">
      <div v-if="ctxMenu.show" class="ctx-menu" :style="{ left: ctxMenu.x + 'px', top: ctxMenu.y + 'px' }">
        <div class="ctx-item" @click="openEditor(ctxMenu.entry)"><i class="fas fa-code"></i> 查看 / 编辑</div>
      </div>
    </Teleport>

    <!-- Editor modal -->
    <Teleport to="body">
      <div v-if="editorFile" class="editor-overlay" @click.self="closeEditor">
        <div class="editor-modal">
          <div class="editor-header">
            <span class="editor-title"><i class="fas fa-file-code"></i> {{ editorFile.name }}</span>
            <div class="editor-actions">
              <button class="editor-btn primary" :disabled="editorSaving" @click="saveEditor">
                <i class="fas fa-save"></i> {{ editorSaving ? "保存中…" : "保存" }}
              </button>
              <button class="editor-btn" @click="closeEditor">取消</button>
            </div>
          </div>
          <textarea
            class="editor-textarea"
            v-model="editorContent"
            spellcheck="false"
          ></textarea>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.panel--sftp {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 340px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--surface);
  border-left: 1px solid var(--border);
  z-index: 20;
  box-shadow: -4px 0 20px rgba(0,0,0,0.25);
}
.resize-handle {
  position: absolute;
  left: -3px;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: col-resize;
  z-index: 5;
}
.resize-handle:hover,
.resize-handle:active {
  background: var(--accent);
  opacity: 0.4;
}
.panel-header {
  height: var(--panel-header-h);
  background: var(--surface);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 4px 0 8px;
  flex-shrink: 0;
}
.panel-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 100%;
}
.panel-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 12px;
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
.panel-actions button:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}
.action-sep {
  width: 1px;
  height: 18px;
  background: var(--border);
  margin: 0 3px;
}
.btn-close-panel:hover {
  color: var(--error) !important;
}
.sftp-path {
  padding: 5px 16px;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-muted);
  background: var(--panel);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}
.path-segment {
  cursor: pointer;
  padding: 2px 5px;
  border-radius: 3px;
  transition: color 0.12s, background 0.12s;
}
.path-segment:hover { color: var(--accent); background: var(--accent-dim); }
.path-segment:last-child { color: var(--text-primary); }
.path-sep { color: var(--text-muted); font-size: 9px; margin: 0 1px; display: inline; }
.new-folder-bar {
  display: flex;
  gap: 4px;
  padding: 6px 16px;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
}
.new-folder-bar input {
  flex: 1;
  padding: 4px 8px;
  background: var(--frame);
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--text-primary);
  font-size: 12px;
  font-family: var(--font-mono);
  outline: none;
}
.new-folder-bar input:focus { border-color: var(--accent); }
.sftp-btn-inline {
  padding: 4px 10px;
  background: var(--elevated);
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--text-secondary);
  cursor: pointer;
  font-size: 11px;
  font-family: inherit;
  transition: background 0.12s, color 0.12s;
}
.sftp-btn-inline:hover { background: var(--hover); color: var(--text-primary); }
.sftp-btn-inline.primary { color: var(--accent); border-color: var(--accent); }
.sftp-btn-inline.primary:hover { background: var(--accent-dim); }
.sftp-content {
  flex: 1;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--active) transparent;
}
.sftp-content::-webkit-scrollbar { width: 6px; }
.sftp-content::-webkit-scrollbar-track { background: transparent; }
.sftp-content::-webkit-scrollbar-thumb { background: var(--active); border-radius: 3px; }
.sftp-content::-webkit-scrollbar-thumb:hover { background: var(--hover); }
.sftp-table { width: 100%; font-size: 12px; border-collapse: collapse; }
.sftp-table thead { position: sticky; top: 0; z-index: 2; }
.sftp-table th {
  padding: 7px 16px;
  text-align: left;
  font-weight: 500;
  color: var(--text-muted);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.6px;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  cursor: pointer;
  transition: color 0.12s;
  white-space: nowrap;
}
.sftp-table th:hover { color: var(--text-secondary); }
.sftp-table th .fa-sort { font-size: 9px; margin-left: 3px; opacity: 0.4; }
.sftp-table th .fa-sort.active { opacity: 1; color: var(--accent); }
.sftp-table td { padding: 0 16px; height: 34px; vertical-align: middle; border-bottom: 1px solid var(--divider); }
.sftp-table td:nth-child(2) { text-align: right; }
.sftp-table th:nth-child(2) { text-align: right; }
.sftp-table td:nth-child(4) { font-family: var(--font-mono); font-size: 11px; color: var(--text-muted); }
.file-row { cursor: pointer; transition: background 0.08s; }
.file-row:hover td { background: var(--elevated); }
.file-row.selected td { background: var(--accent-dim); }
.file-row.selected td:first-child { box-shadow: inset 2px 0 0 var(--accent); }

.file-icon { margin-right: 8px; font-size: 13px; width: 16px; display: inline-block; text-align: center; }
.file-icon.dir { color: var(--accent); }
.file-icon.file { color: var(--text-muted); }
.file-name { font-family: var(--font-mono); font-size: 12px; }
.file-name.dir { color: var(--text-primary); font-weight: 500; }
.file-meta { font-family: var(--font-mono); font-size: 11px; color: var(--text-muted); white-space: nowrap; }

/* Context menu */
.ctx-menu {
  position: fixed;
  z-index: 9999;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 4px 0;
  min-width: 140px;
  box-shadow: 0 8px 32px rgba(0,0,0,0.4);
}
.ctx-item {
  padding: 7px 14px;
  font-size: 12px;
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  transition: background 0.08s;
}
.ctx-item:hover { background: var(--elevated); color: var(--text-primary); }
.ctx-item i { width: 16px; text-align: center; font-size: 11px; }

/* Editor modal */
.editor-overlay {
  position: fixed;
  inset: 0;
  z-index: 9998;
  background: rgba(0,0,0,0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 32px;
}
.editor-modal {
  width: 100%;
  max-width: 900px;
  height: 80vh;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 12px 48px rgba(0,0,0,0.5);
}
.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
.editor-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
  display: flex;
  align-items: center;
  gap: 8px;
}
.editor-title i { color: var(--accent); }
.editor-actions { display: flex; gap: 6px; }
.editor-btn {
  padding: 6px 14px;
  border-radius: 5px;
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  border: 1px solid var(--border);
  background: var(--elevated);
  color: var(--text-secondary);
  transition: background 0.12s, color 0.12s;
  display: flex;
  align-items: center;
  gap: 5px;
}
.editor-btn:hover { background: var(--hover); color: var(--text-primary); }
.editor-btn.primary {
  color: var(--accent);
  border-color: var(--accent);
}
.editor-btn.primary:hover { background: var(--accent-dim); }
.editor-btn.primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.editor-textarea {
  flex: 1;
  width: 100%;
  padding: 16px;
  background: var(--frame);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.6;
  border: none;
  outline: none;
  resize: none;
  tab-size: 2;
}
.editor-textarea::selection { background: var(--accent-dim); }
</style>
