<template>
  <div v-if="transfer.conflict" class="modal-mask">
    <div class="modal-box">
      <h3>文件已存在</h3>
      <p class="target">{{ transfer.conflict.target }}</p>
      <p>请选择处理方式：</p>
      <div class="actions">
        <button @click="resolve('Overwrite')">覆盖</button>
        <button @click="resolve('Skip')">跳过</button>
        <button @click="resolve('Rename')">重命名</button>
      </div>
      <label class="apply-all">
        <input type="checkbox" v-model="applyAll" /> 对所有冲突执行相同操作
      </label>
    </div>
  </div>
</template>

<script setup>
import { ref } from "vue";
import { useTransferStore } from "../stores/transfer";

const transfer = useTransferStore();
const applyAll = ref(false);

async function resolve(action) {
  const c = transfer.conflict;
  await transfer.resolveConflict(c.token, action, c.session_id, applyAll.value);
  transfer.clearConflict();
}
</script>

<style scoped>
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.modal-box {
  background: var(--bg-panel, #1e1e1e);
  color: var(--fg, #ddd);
  padding: 20px;
  border-radius: 8px;
  min-width: 320px;
  border: 1px solid #444;
}
.target {
  word-break: break-all;
  color: #6cf;
  margin: 8px 0;
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}
.actions button {
  flex: 1;
  padding: 8px;
  cursor: pointer;
}
.apply-all {
  display: block;
  margin-top: 12px;
  font-size: 12px;
}
</style>
