<script setup>
import { useConnectionStore } from "../stores/connection";
const conn = useConnectionStore();
</script>

<template>
  <Teleport to="body">
    <div class="toast-container">
      <div
        v-for="t in conn.toasts"
        :key="t.id"
        class="toast"
        :class="'toast--' + t.type"
        @click="conn.removeToast(t.id)"
      >
        <i v-if="t.type === 'error'" class="fas fa-circle-exclamation"></i>
        <i v-else-if="t.type === 'success'" class="fas fa-circle-check"></i>
        <i v-else class="fas fa-circle-info"></i>
        <span class="toast-msg">{{ t.message }}</span>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-container {
  position: fixed;
  bottom: 48px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 99999;
  display: flex;
  flex-direction: column;
  gap: 6px;
  pointer-events: none;
}
.toast {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 18px;
  border-radius: 8px;
  font-size: 13px;
  font-family: var(--font-ui);
  color: var(--text-primary);
  background: var(--panel);
  border: 1px solid var(--border);
  box-shadow: 0 6px 24px rgba(0,0,0,0.45);
  cursor: pointer;
  white-space: nowrap;
  backdrop-filter: blur(8px);
  transition: opacity 0.15s;
}
.toast i { font-size: 14px; }
.toast--info i { color: var(--info); }
.toast--success i { color: var(--success); }
.toast--error i { color: var(--error); }
.toast--info { border-left: 3px solid var(--info); }
.toast--success { border-left: 3px solid var(--success); }
.toast--error { border-left: 3px solid var(--error); }
</style>
