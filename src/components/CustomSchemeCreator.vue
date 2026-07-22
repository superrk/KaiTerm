<template>
  <div class="modal-mask" @click.self="$emit('close')">
    <div class="creator-panel">
      <div class="creator-header">
        <h3><i class="fas fa-palette"></i> 新增配色方案</h3>
        <button class="close-btn" @click="$emit('close')"><i class="fas fa-times"></i></button>
      </div>

      <div class="creator-body">
        <div class="form-row">
          <label>方案名称</label>
          <input v-model="name" placeholder="My Custom Theme" class="input-full" />
        </div>
        <div class="form-row">
          <label>方案类型</label>
          <div class="group-toggle">
            <button :class="{ active: group === 'dark' }" @click="group = 'dark'">暗色</button>
            <button :class="{ active: group === 'light' }" @click="group = 'light'">亮色</button>
          </div>
        </div>

        <div class="color-section">
          <div class="color-section-title">基础色</div>
          <div class="color-grid">
            <div v-for="c in baseColors" :key="c.key" class="color-field">
              <label>{{ c.label }}</label>
              <div class="color-input-row">
                <input type="color" :value="colors[c.key]" @input="colors[c.key] = $event.target.value" />
                <input type="text" :value="colors[c.key]" @input="colors[c.key] = $event.target.value" class="hex-input" />
              </div>
            </div>
          </div>
        </div>

        <div class="color-section">
          <div class="color-section-title">ANSI 色（Normal）</div>
          <div class="color-grid">
            <div v-for="c in ansiColors" :key="c.key" class="color-field">
              <label>{{ c.label }}</label>
              <div class="color-input-row">
                <input type="color" :value="colors[c.key]" @input="colors[c.key] = $event.target.value" />
                <input type="text" :value="colors[c.key]" @input="colors[c.key] = $event.target.value" class="hex-input" />
              </div>
            </div>
          </div>
        </div>

        <div class="color-section">
          <div class="color-section-title">ANSI 色（Bright）</div>
          <div class="color-grid">
            <div v-for="c in brightColors" :key="c.key" class="color-field">
              <label>{{ c.label }}</label>
              <div class="color-input-row">
                <input type="color" :value="colors[c.key]" @input="colors[c.key] = $event.target.value" />
                <input type="text" :value="colors[c.key]" @input="colors[c.key] = $event.target.value" class="hex-input" />
              </div>
            </div>
          </div>
        </div>

        <!-- 预览 -->
        <div class="preview-box" :style="{ background: colors.background, color: colors.foreground }">
          <span :style="{ color: colors.green }">user@host</span>:<span :style="{ color: colors.blue }">~/project</span>$ <span :style="{ color: colors.yellow }">ls -la</span>
          <br />
          <span :style="{ color: colors.cyan }">total 32</span>
          <br />
          <span :style="{ color: colors.magenta }">drwxr-xr-x</span> <span :style="{ color: colors.brightBlack }">4 user user 4096</span> <span :style="{ color: colors.white }">.</span>
          <br />
          <span :style="{ color: colors.red }">-rw-r--r--</span> <span :style="{ color: colors.brightBlack }">1 user user 1024</span> <span :style="{ color: colors.white }">config.js</span>
        </div>
      </div>

      <div class="creator-footer">
        <button class="btn-cancel" @click="$emit('close')">取消</button>
        <button class="btn-save" @click="save" :disabled="!name.trim()">保存</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive } from "vue";
import { useThemeStore } from "../stores/theme";

const emit = defineEmits(["close"]);
const theme = useThemeStore();

const name = ref("");
const group = ref("dark");
const colors = reactive({
  background: "#1A1F2B",
  foreground: "#E2E6ED",
  cursor: "#E8A84C",
  cursorAccent: "#1A1F2B",
  selectionBackground: "rgba(232, 168, 76, 0.25)",
  black: "#1A1F2B",
  red: "#E06C75",
  green: "#98C379",
  yellow: "#E5C07B",
  blue: "#61AFEF",
  magenta: "#C678DD",
  cyan: "#56B6C2",
  white: "#ABB2BF",
  brightBlack: "#5C6370",
  brightRed: "#E06C75",
  brightGreen: "#98C379",
  brightYellow: "#E5C07B",
  brightBlue: "#61AFEF",
  brightMagenta: "#C678DD",
  brightCyan: "#56B6C2",
  brightWhite: "#FFFFFF",
});

const baseColors = [
  { key: "background", label: "背景" },
  { key: "foreground", label: "前景" },
  { key: "cursor", label: "光标" },
  { key: "cursorAccent", label: "光标色" },
  { key: "selectionBackground", label: "选中背景" },
];
const ansiColors = [
  { key: "black", label: "Black" }, { key: "red", label: "Red" },
  { key: "green", label: "Green" }, { key: "yellow", label: "Yellow" },
  { key: "blue", label: "Blue" }, { key: "magenta", label: "Magenta" },
  { key: "cyan", label: "Cyan" }, { key: "white", label: "White" },
];
const brightColors = [
  { key: "brightBlack", label: "Bright Black" }, { key: "brightRed", label: "Bright Red" },
  { key: "brightGreen", label: "Bright Green" }, { key: "brightYellow", label: "Bright Yellow" },
  { key: "brightBlue", label: "Bright Blue" }, { key: "brightMagenta", label: "Bright Magenta" },
  { key: "brightCyan", label: "Bright Cyan" }, { key: "brightWhite", label: "Bright White" },
];

function save() {
  theme.addCustomScheme({
    name: name.value.trim(),
    group: group.value,
    colors: { ...colors },
  });
  emit("close");
}
</script>

<style scoped>
.modal-mask {
  position: fixed; inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex; align-items: center; justify-content: center;
  z-index: 1100;
}
.creator-panel {
  background: var(--surface); color: var(--text-primary);
  border: 1px solid var(--border); border-radius: 12px;
  width: 560px; max-height: 85vh;
  display: flex; flex-direction: column;
  box-shadow: 0 12px 40px rgba(0,0,0,0.5);
}
.creator-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 16px 20px; border-bottom: 1px solid var(--border);
}
.creator-header h3 { font-size: 15px; font-weight: 600; display: flex; align-items: center; gap: 8px; }
.close-btn {
  width: 28px; height: 28px; border-radius: 6px;
  display: flex; align-items: center; justify-content: center;
  color: var(--text-muted);
}
.close-btn:hover { background: var(--elevated); color: var(--text-primary); }
.creator-body {
  flex: 1; overflow-y: auto; padding: 16px 20px;
  scrollbar-width: thin; scrollbar-color: var(--active) transparent;
}
.creator-footer {
  display: flex; justify-content: flex-end; gap: 8px;
  padding: 12px 20px; border-top: 1px solid var(--border);
}
.form-row { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }
.form-row label { font-size: 12px; color: var(--text-secondary); min-width: 80px; flex-shrink: 0; }
.input-full {
  flex: 1; padding: 6px 10px; border-radius: 6px;
  border: 1px solid var(--border); background: var(--panel);
  color: var(--text-primary); font-size: 12px;
}
.group-toggle { display: flex; gap: 4px; }
.group-toggle button {
  padding: 5px 14px; border-radius: 6px; font-size: 12px;
  border: 1px solid var(--border); background: var(--panel);
  color: var(--text-secondary); transition: all 0.12s;
}
.group-toggle button.active { border-color: var(--accent); color: var(--accent); background: var(--accent-dim); }

.color-section { margin-bottom: 16px; }
.color-section-title { font-size: 11px; color: var(--text-muted); font-weight: 600; margin-bottom: 8px; text-transform: uppercase; letter-spacing: 0.5px; }
.color-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px; }
.color-field label { font-size: 10px; color: var(--text-muted); margin-bottom: 2px; display: block; }
.color-input-row { display: flex; gap: 4px; align-items: center; }
.color-input-row input[type="color"] {
  width: 24px; height: 24px; border: 1px solid var(--border); border-radius: 4px;
  padding: 1px; cursor: pointer; background: none;
}
.hex-input {
  width: 72px; padding: 3px 6px; border-radius: 4px;
  border: 1px solid var(--border); background: var(--panel);
  color: var(--text-primary); font-size: 10px; font-family: var(--font-mono);
}
.preview-box {
  padding: 12px; border-radius: 8px; font-family: var(--font-mono);
  font-size: 13px; line-height: 1.6; border: 1px solid var(--border);
}
.btn-cancel {
  padding: 7px 14px; border-radius: 6px; font-size: 12px;
  border: 1px solid var(--border); background: var(--panel);
  color: var(--text-secondary);
}
.btn-cancel:hover { color: var(--text-primary); }
.btn-save {
  padding: 7px 14px; border-radius: 6px; font-size: 12px;
  background: var(--accent); color: #fff; font-weight: 600;
}
.btn-save:disabled { opacity: 0.4; cursor: not-allowed; }
.btn-save:hover:not(:disabled) { background: var(--accent-hover); }
</style>
