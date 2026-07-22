<template>
  <div v-if="conn.hostKeyPending" class="modal-mask">
    <div class="modal-box">
      <div class="dialog-icon" :class="isChanged ? 'warn' : 'info'">
        <i :class="isChanged ? 'fas fa-exclamation-triangle' : 'fas fa-shield-alt'"></i>
      </div>
      <h3>{{ isChanged ? '主机密钥已变更' : '首次连接' }}</h3>
      <p class="host-label">{{ conn.hostKeyPending.host }}:{{ conn.hostKeyPending.port }}</p>

      <div v-if="isChanged" class="warning-box">
        <i class="fas fa-info-circle"></i>
        <span>该主机的密钥指纹与之前记录的不一致，可能是中间人攻击（MITM）。如果您未更换过该服务器，请勿信任此密钥。</span>
      </div>

      <div class="key-info">
        <div class="key-row">
          <span class="key-label">类型</span>
          <span class="key-value mono">{{ conn.hostKeyPending.keyType }}</span>
        </div>
        <div class="key-row">
          <span class="key-label">指纹</span>
          <span class="key-value mono fp">{{ conn.hostKeyPending.fingerprint }}</span>
        </div>
      </div>

      <div class="actions">
        <button class="btn-cancel" @click="cancel">
          <i class="fas fa-times"></i> 取消连接
        </button>
        <button class="btn-trust" @click="trust">
          <i class="fas fa-check"></i> {{ isChanged ? '仍然信任' : '信任此主机' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useConnectionStore } from "../stores/connection";

const conn = useConnectionStore();

const isChanged = computed(() => conn.hostKeyPending?.status === "changed");

async function trust() {
  await conn.trustHostKey();
}

async function cancel() {
  conn.cancelHostKey();
}
</script>

<style scoped>
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.modal-box {
  background: var(--surface);
  color: var(--text-primary);
  padding: 24px;
  border-radius: 12px;
  min-width: 420px;
  max-width: 480px;
  border: 1px solid var(--border);
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
}
.dialog-icon {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 22px;
  margin: 0 auto 16px;
}
.dialog-icon.info {
  background: var(--accent-dim);
  color: var(--accent);
}
.dialog-icon.warn {
  background: rgba(234, 179, 8, 0.15);
  color: #eab308;
}
h3 {
  text-align: center;
  margin: 0 0 4px;
  font-size: 16px;
  font-weight: 600;
}
.host-label {
  text-align: center;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 13px;
  margin: 0 0 16px;
}
.warning-box {
  background: rgba(234, 179, 8, 0.1);
  border: 1px solid rgba(234, 179, 8, 0.3);
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 12px;
  color: #eab308;
  display: flex;
  gap: 8px;
  align-items: flex-start;
  margin-bottom: 16px;
  line-height: 1.5;
}
.warning-box i {
  margin-top: 2px;
  flex-shrink: 0;
}
.key-info {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 12px;
  margin-bottom: 20px;
}
.key-row {
  display: flex;
  align-items: baseline;
  gap: 12px;
  padding: 4px 0;
}
.key-row + .key-row {
  border-top: 1px solid var(--border);
  margin-top: 4px;
  padding-top: 8px;
}
.key-label {
  color: var(--text-muted);
  font-size: 12px;
  flex-shrink: 0;
  width: 36px;
}
.key-value {
  font-size: 12px;
  color: var(--text-secondary);
  word-break: break-all;
}
.key-value.fp {
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.3px;
}
.mono {
  font-family: var(--font-mono);
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.actions button {
  padding: 8px 16px;
  border-radius: 6px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
  transition: background 0.12s, border-color 0.12s;
}
.actions button:hover {
  background: var(--elevated);
}
.btn-trust {
  background: var(--accent-dim) !important;
  border-color: var(--accent) !important;
  color: var(--accent) !important;
}
.btn-trust:hover {
  background: var(--accent) !important;
  color: #fff !important;
}
</style>
