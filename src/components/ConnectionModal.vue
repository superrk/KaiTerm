<script setup>
import { ref, computed, onMounted } from "vue";
import { useConnectionStore } from "../stores/connection";

const conn = useConnectionStore();
const emit = defineEmits(["close"]);

const props = defineProps({
  profile: { type: Object, default: null },
});

const editing = computed(() => !!props.profile);

const label = ref("");
const host = ref("");
const port = ref(22);
const username = ref("root");
const authMethod = ref("password");
const authValue = ref("");
const saveConn = ref(true);
const group = ref("");
const syncDir = ref(true);
const groupMode = ref("select"); // "select" | "new"
const newGroupName = ref("");

const connecting = ref(false);

onMounted(() => {
  if (props.profile) {
    // 编辑模式：预填表单
    const p = props.profile;
    label.value = p.name || "";
    host.value = p.host || "";
    port.value = p.port || 22;
    username.value = p.user || "";
    authMethod.value = p.authMethod || "password";
    authValue.value = ""; // 凭据不回显，需用户重新输入
    saveConn.value = true;
    group.value = p.group || "";
    syncDir.value = p.syncDir ?? true;
    // 如果已有分组且在列表中，选中；否则视为自定义
    groupMode.value = conn.existingGroups.includes(group.value) ? "select" : "new";
    newGroupName.value = group.value;
  } else {
    host.value = "";
    port.value = 22;
    username.value = "root";
    authMethod.value = "password";
    authValue.value = "";
    label.value = "";
    group.value = "";
    syncDir.value = true;
    groupMode.value = "select";
    newGroupName.value = "";
  }
});

const effectiveGroup = computed(() => {
  if (groupMode.value === "new") return newGroupName.value || null;
  return group.value || null;
});

async function handleSubmit() {
  if (!host.value || !username.value) return;
  const p = Number(port.value);
  if (!Number.isInteger(p) || p < 1 || p > 65535) {
    conn.pushToast("端口号必须是 1–65535 之间的整数", "error");
    return;
  }
  connecting.value = true;
  try {
    if (editing.value) {
      // 编辑模式：仅保存，不建立连接
      await conn.saveProfile({
        id: props.profile.id,
        name: label.value || host.value,
        host: host.value,
        port: p,
        user: username.value,
        authMethod: authMethod.value,
        password: authMethod.value === "password" ? (authValue.value || "") : "",
        keyPath: authMethod.value === "key" ? (authValue.value || "") : "",
        group: effectiveGroup.value,
        sync_dir: syncDir.value,
      });
      conn.pushToast("连接已更新", "success");
    } else {
      // 新建模式：建立连接 + 可选保存
      let password = null;
      let keyPath = null;
      if (authMethod.value === "password") {
        password = authValue.value || null;
      } else if (authMethod.value === "key") {
        keyPath = authValue.value || null;
      }
      await conn.connect(host.value, p, username.value, password, keyPath, label.value, authMethod.value === "agent", syncDir.value);
      if (saveConn.value) {
        await conn.saveProfile({
          name: label.value || host.value,
          host: host.value,
          port: p,
          user: username.value,
          authMethod: authMethod.value,
          password: authMethod.value === "password" ? (authValue.value || "") : "",
          keyPath: authMethod.value === "key" ? (authValue.value || "") : "",
          group: effectiveGroup.value,
          sync_dir: syncDir.value,
        });
      }
    }
    emit("close");
  } catch (e) {
    conn.pushToast(e, "error");
  } finally {
    connecting.value = false;
  }
}
</script>

<template>
  <div class="modal-overlay" :class="{ active: true }" @click.self="emit('close')">
    <div class="modal">
      <div class="modal-header">
        <h2>{{ editing ? '编辑连接' : '新建连接' }}</h2>
        <button class="modal-close" @click="emit('close')" aria-label="关闭"><i class="fas fa-times"></i></button>
      </div>
      <div class="modal-body">
        <div class="form-row">
          <div class="form-group">
            <label class="form-label" for="connLabel">连接名称</label>
            <input class="form-input" type="text" id="connLabel" v-model="label" placeholder="例如：生产服务器">
          </div>
        </div>
        <div class="form-row">
          <div class="form-group">
            <label class="form-label" for="host">主机地址</label>
            <input class="form-input" type="text" id="host" v-model="host" placeholder="192.168.1.100">
          </div>
          <div class="form-group narrow">
            <label class="form-label" for="port">端口</label>
            <input class="form-input" type="text" id="port" v-model.number="port" placeholder="22">
          </div>
        </div>
        <div class="form-row">
          <div class="form-group">
            <label class="form-label" for="username">用户名</label>
            <input class="form-input" type="text" id="username" v-model="username" placeholder="deploy">
          </div>
          <div class="form-group">
            <label class="form-label" for="group">分组</label>
            <div class="group-row">
              <button
                class="group-mode-btn"
                :class="{ active: groupMode === 'select' }"
                @click="groupMode = 'select'"
                title="选择已有分组"
              ><i class="fas fa-list"></i></button>
              <button
                class="group-mode-btn"
                :class="{ active: groupMode === 'new' }"
                @click="groupMode = 'new'"
                title="输入新分组名"
              ><i class="fas fa-pen"></i></button>
            </div>
            <select
              v-if="groupMode === 'select'"
              class="form-input"
              v-model="group"
            >
              <option value="">（不分组）</option>
              <option v-for="g in conn.existingGroups" :key="g" :value="g">{{ g }}</option>
            </select>
            <input
              v-else
              class="form-input"
              type="text"
              v-model="newGroupName"
              placeholder="输入新分组名称"
            >
          </div>
        </div>
        <div class="form-row">
          <div class="form-group">
            <label class="form-label" for="authField">{{ authMethod === 'password' ? '密码' : authMethod === 'key' ? '密钥文件' : 'SSH Agent' }}</label>
            <input
              class="form-input"
              :type="authMethod === 'password' ? 'password' : 'text'"
              id="authField"
              v-model="authValue"
              :placeholder="authMethod === 'password' ? '输入密码' : authMethod === 'key' ? '~/.ssh/id_rsa' : '使用 SSH Agent'"
              :disabled="authMethod === 'agent'"
            >
          </div>
        </div>

        <div class="auth-label">认证方式</div>
        <div class="auth-toggle">
          <button class="auth-option" :class="{ active: authMethod === 'password' }" @click="authMethod = 'password'">
            <i class="fas fa-key"></i> 密码
          </button>
          <button class="auth-option" :class="{ active: authMethod === 'key' }" @click="authMethod = 'key'">
            <i class="fas fa-file-key"></i> 私钥文件
          </button>
          <button class="auth-option" :class="{ active: authMethod === 'agent' }" @click="authMethod = 'agent'">
            <i class="fas fa-fingerprint"></i> SSH Agent
          </button>
        </div>

        <label class="form-check">
          <input type="checkbox" v-model="syncDir"> 目录跟随（自动同步终端路径到文件管理器）
        </label>
      </div>
      <div class="modal-footer">
        <button class="btn btn-ghost" @click="emit('close')">取消</button>
        <button
          v-if="editing"
          class="btn btn-primary"
          @click="handleSubmit"
          :disabled="connecting"
        >{{ connecting ? "保存中…" : "保存" }}</button>
        <template v-else>
          <label class="form-check" style="margin-right:auto;">
            <input type="checkbox" v-model="saveConn"> 保存此连接
          </label>
          <button class="btn btn-primary" @click="handleSubmit" :disabled="connecting">
            {{ connecting ? "连接中…" : "连接" }}
          </button>
        </template>
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
  width: 460px;
  max-height: 88vh;
  overflow-y: auto;
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
  width: 28px;
  height: 28px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  background: none;
  border: none;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
  font-size: 14px;
  font-family: inherit;
}
.modal-close:hover { background: var(--elevated); color: var(--text-primary); }
.modal-body { padding: 20px 24px; }
.form-row { display: flex; gap: 12px; margin-bottom: 14px; }
.form-group { flex: 1; display: flex; flex-direction: column; gap: 5px; }
.form-group.narrow { flex: 0 0 88px; }
.form-label {
  font-size: 11px;
  font-weight: 500;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.form-input {
  height: 36px;
  padding: 0 10px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 13px;
  outline: none;
  transition: border-color 0.15s;
}
.form-input:focus { border-color: var(--accent); }
.form-input::placeholder { color: var(--text-muted); }
.form-input:disabled { opacity: 0.4; }
.group-row { display: flex; gap: 4px; margin-bottom: 5px; }
.group-mode-btn {
  width: 30px;
  height: 30px;
  border-radius: 5px;
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  font-size: 11px;
  transition: all 0.12s;
  flex-shrink: 0;
}
.group-mode-btn:hover { border-color: var(--text-muted); color: var(--text-primary); }
.group-mode-btn.active {
  border-color: var(--accent);
  background: var(--accent-dim);
  color: var(--accent);
}
.auth-label {
  font-size: 11px;
  font-weight: 500;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 8px;
}
.auth-toggle { display: flex; gap: 8px; margin-bottom: 14px; }
.auth-option {
  flex: 1;
  padding: 8px 12px;
  border-radius: 6px;
  border: 1px solid var(--border);
  background: var(--panel);
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
  text-align: center;
  cursor: pointer;
  transition: all 0.12s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-family: inherit;
}
.auth-option:hover { border-color: var(--text-muted); }
.auth-option.active {
  border-color: var(--accent);
  background: var(--accent-dim);
  color: var(--accent);
}
.form-check {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
  font-size: 12px;
  color: var(--text-secondary);
  cursor: pointer;
}
.form-check input[type="checkbox"] { accent-color: var(--accent); width: 14px; height: 14px; }
.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 4px 24px 20px;
}
.btn {
  padding: 8px 20px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.12s;
  font-family: inherit;
  border: none;
}
.btn-ghost { color: var(--text-secondary); background: transparent; }
.btn-ghost:hover { background: var(--elevated); color: var(--text-primary); }
.btn-primary { background: var(--accent); color: var(--frame); font-weight: 600; }
.btn-primary:hover { background: var(--accent-hover); }
.btn-primary:disabled { opacity: 0.4; cursor: not-allowed; }
</style>
