<script setup>
defineProps({ collapsed: Boolean });
const emit = defineEmits(["group-manager"]);
</script>

<template>
  <div class="sidebar" :class="{ collapsed }">
    <div class="sidebar-menu">
      <button class="sidebar-item" @click="emit('group-manager')" title="分组管理">
        <i class="fas fa-folder-tree"></i>
        <span class="sidebar-label">分组管理</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.sidebar {
  width: 180px;
  background: var(--frame);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  transition: width 0.2s ease;
  overflow: hidden;
}
.sidebar.collapsed { width: 0; border-right: none; }
.sidebar-menu {
  display: flex;
  flex-direction: column;
  padding: 6px;
  gap: 2px;
  min-width: 180px;
}
.sidebar-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
  border: none;
  background: none;
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
  white-space: nowrap;
  overflow: hidden;
  font-family: inherit;
}
.sidebar-item:hover {
  background: var(--elevated);
  color: var(--text-primary);
}
.sidebar-item i {
  width: 18px;
  text-align: center;
  flex-shrink: 0;
  font-size: 14px;
}
.sidebar-label {
  opacity: 1;
  transition: opacity 0.15s;
}
.sidebar.collapsed .sidebar-label {
  opacity: 0;
  pointer-events: none;
}
</style>
