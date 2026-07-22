<script setup>
import { ref, computed, nextTick } from "vue";
import { useConnectionStore } from "../stores/connection";

const conn = useConnectionStore();
const emit = defineEmits(["close"]);

const editingGroup = ref(null);
const editValue = ref("");
const newGroupName = ref("");
const addingMode = ref(false);
const addInputRef = ref(null);

function groupCount(g) {
  return conn.profiles.filter((p) => (p.group || "") === g).length;
}

function startRename(g) {
  editingGroup.value = g;
  editValue.value = g;
}

async function confirmRename() {
  const oldName = editingGroup.value;
  const newName = editValue.value.trim();
  if (!newName) {
    conn.pushToast("分组名不能为空", "error");
    return;
  }
  if (newName === oldName) {
    editingGroup.value = null;
    return;
  }
  if (conn.existingGroups.includes(newName)) {
    conn.pushToast("分组名已存在", "error");
    return;
  }
  try {
    await conn.renameGroup(oldName, newName);
    conn.pushToast("分组已重命名", "success");
    editingGroup.value = null;
  } catch (e) {
    conn.pushToast("重命名失败: " + e, "error");
  }
}

async function handleDelete(g) {
  const count = groupCount(g);
  if (count > 0) {
    conn.pushToast(`无法删除：分组「${g}」下还有 ${count} 个连接`, "error");
    return;
  }
  try {
    await conn.deleteGroup(g);
    conn.pushToast("分组已删除", "success");
  } catch (e) {
    conn.pushToast("删除失败: " + e, "error");
  }
}

function startAdd() {
  addingMode.value = true;
  newGroupName.value = "";
  nextTick(() => addInputRef.value?.focus());
}

async function confirmAdd() {
  const name = newGroupName.value.trim();
  if (!name) {
    conn.pushToast("分组名不能为空", "error");
    return;
  }
  if (conn.existingGroups.includes(name)) {
    conn.pushToast("分组名已存在", "error");
    return;
  }
  try {
    await conn.addGroup(name);
    conn.pushToast("分组已创建", "success");
    newGroupName.value = "";
    addingMode.value = false;
  } catch (e) {
    conn.pushToast("创建失败: " + e, "error");
  }
}

const sortedGroups = computed(() => {
  return conn.existingGroups.map((g) => ({
    name: g,
    count: groupCount(g),
    isManaged: conn.managedGroups.includes(g),
  }));
});
</script>

<template>
  <div class="modal-overlay" :class="{ active: true }" @click.self="emit('close')">
    <div class="modal">
      <div class="modal-header">
        <h2>分组管理</h2>
        <button class="modal-close" @click="emit('close')" aria-label="关闭"><i class="fas fa-times"></i></button>
      </div>
      <div class="modal-body">
        <div class="gm-add-row">
          <template v-if="addingMode">
            <input
              ref="addInputRef"
              class="gm-edit-input"
              v-model="newGroupName"
              @keyup.enter="confirmAdd"
              @keyup.escape="addingMode = false"
              placeholder="输入新分组名称"
            >
            <button class="gm-btn gm-btn-confirm" @click="confirmAdd" title="确认创建"><i class="fas fa-check"></i></button>
            <button class="gm-btn gm-btn-cancel" @click="addingMode = false" title="取消"><i class="fas fa-times"></i></button>
          </template>
          <button v-else class="gm-add-btn" @click="startAdd">
            <i class="fas fa-plus"></i> 新建分组
          </button>
        </div>
        <div v-if="sortedGroups.length === 0" class="gm-empty">暂无分组</div>
        <div v-else class="gm-list">
          <div v-for="g in sortedGroups" :key="g.name" class="gm-row">
            <template v-if="editingGroup === g.name">
              <input
                class="gm-edit-input"
                v-model="editValue"
                @keyup.enter="confirmRename"
                @keyup.escape="editingGroup = null"
                autofocus
              >
              <button class="gm-btn gm-btn-confirm" @click="confirmRename" title="确认"><i class="fas fa-check"></i></button>
              <button class="gm-btn gm-btn-cancel" @click="editingGroup = null" title="取消"><i class="fas fa-times"></i></button>
            </template>
            <template v-else>
              <span class="gm-name">{{ g.name }}</span>
              <span class="gm-count">{{ g.count }} 个连接</span>
              <div class="gm-actions">
                <button class="gm-btn" @click="startRename(g.name)" title="重命名"><i class="fas fa-pen"></i></button>
                <button class="gm-btn gm-btn-danger" @click="handleDelete(g.name)" title="删除分组" :disabled="g.count > 0"><i class="fas fa-trash"></i></button>
              </div>
            </template>
          </div>
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn btn-primary" @click="emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.modal {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  width: 420px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
}
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px 0;
}
.modal-header h2 { font-size: 16px; font-weight: 600; color: var(--text-primary); }
.modal-close {
  width: 28px; height: 28px; border-radius: 6px;
  display: flex; align-items: center; justify-content: center;
  color: var(--text-muted); background: none; border: none;
  cursor: pointer; transition: background 0.12s, color 0.12s; font-size: 14px;
  font-family: inherit;
}
.modal-close:hover { background: var(--elevated); color: var(--text-primary); }
.modal-body { padding: 16px 24px; overflow-y: auto; flex: 1; }
.gm-add-row { margin-bottom: 12px; }
.gm-add-btn {
  width: 100%;
  padding: 8px 12px;
  border-radius: 6px;
  border: 1px dashed var(--border);
  background: none;
  color: var(--text-muted);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.12s;
  font-family: inherit;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}
.gm-add-btn:hover { border-color: var(--accent); color: var(--accent); }
.gm-empty { text-align: center; color: var(--text-muted); font-size: 13px; padding: 24px 0; }
.gm-list { display: flex; flex-direction: column; gap: 4px; }
.gm-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 6px;
  transition: border-color 0.12s;
}
.gm-row:hover { border-color: var(--hover); }
.gm-name { flex: 1; font-size: 13px; font-weight: 500; color: var(--text-primary); }
.gm-count { font-size: 11px; color: var(--text-muted); flex-shrink: 0; }
.gm-actions { display: flex; gap: 4px; flex-shrink: 0; }
.gm-btn {
  width: 26px; height: 26px; border-radius: 5px;
  display: flex; align-items: center; justify-content: center;
  background: none; border: 1px solid var(--border);
  color: var(--text-muted); cursor: pointer; font-size: 11px;
  transition: all 0.12s; font-family: inherit;
}
.gm-btn:hover { color: var(--text-primary); border-color: var(--text-muted); }
.gm-btn:disabled { opacity: 0.3; cursor: not-allowed; }
.gm-btn-danger:hover:not(:disabled) { color: var(--error); border-color: var(--error); background: rgba(207, 92, 79, 0.1); }
.gm-btn-confirm { color: var(--accent); border-color: var(--accent); }
.gm-btn-confirm:hover { background: var(--accent-dim); }
.gm-btn-cancel:hover { color: var(--text-primary); }
.gm-edit-input {
  flex: 1;
  height: 28px;
  padding: 0 8px;
  background: var(--panel);
  border: 1px solid var(--accent);
  border-radius: 5px;
  color: var(--text-primary);
  font-size: 13px;
  outline: none;
  font-family: inherit;
}
.modal-footer {
  display: flex;
  justify-content: flex-end;
  padding: 12px 24px 20px;
}
.btn {
  padding: 8px 20px; border-radius: 6px; font-size: 13px; font-weight: 500;
  cursor: pointer; transition: all 0.12s; font-family: inherit; border: none;
}
.btn-primary { background: var(--accent); color: var(--frame); font-weight: 600; }
.btn-primary:hover { background: var(--accent-hover); }
</style>
