<template>
  <div class="modal-mask" @click.self="$emit('close')">
    <div class="settings-panel">
      <div class="settings-header">
        <h3><i class="fas fa-cog"></i> 设置</h3>
        <button class="close-btn" @click="$emit('close')"><i class="fas fa-times"></i></button>
      </div>

      <div class="settings-body">
        <!-- ═══ App 主题 ═══ -->
        <section class="settings-section">
          <h4>外观主题</h4>
          <div class="theme-grid">
            <button
              v-for="(t, id) in appThemes"
              :key="id"
              class="theme-card"
              :class="{ active: theme.appThemeId === id }"
              @click="theme.setAppTheme(id)"
            >
              <i :class="t.icon"></i>
              <span>{{ t.name }}</span>
            </button>
          </div>
        </section>

        <!-- ═══ 终端配色 ═══ -->
        <section class="settings-section">
          <h4>终端配色方案</h4>
          <div class="scheme-group">
            <div class="scheme-group-label">暗色</div>
            <div class="scheme-grid">
              <button
                v-for="s in darkSchemes"
                :key="s.id"
                class="scheme-card"
                :class="{ active: theme.terminalSchemeId === s.id }"
                @click="theme.setTerminalScheme(s.id)"
              >
                <div class="scheme-preview">
                  <span v-for="(color, name) in previewColors(s.colors)" :key="name" class="color-dot" :style="{ background: color }"></span>
                </div>
                <span class="scheme-name">{{ s.name }}</span>
              </button>
            </div>
          </div>
          <div class="scheme-group">
            <div class="scheme-group-label">亮色</div>
            <div class="scheme-grid">
              <button
                v-for="s in lightSchemes"
                :key="s.id"
                class="scheme-card"
                :class="{ active: theme.terminalSchemeId === s.id }"
                @click="theme.setTerminalScheme(s.id)"
              >
                <div class="scheme-preview">
                  <span v-for="(color, name) in previewColors(s.colors)" :key="name" class="color-dot" :style="{ background: color }"></span>
                </div>
                <span class="scheme-name">{{ s.name }}</span>
              </button>
            </div>
          </div>

          <!-- 自定义方案 -->
          <div v-if="customSchemes.length > 0" class="scheme-group">
            <div class="scheme-group-label">自定义</div>
            <div class="scheme-grid">
              <div
                v-for="(s, i) in customSchemes"
                :key="`custom-${i}`"
                class="scheme-card custom"
                :class="{ active: theme.terminalSchemeId === `custom-${i}` }"
              >
                <div class="scheme-card-body" @click="theme.setTerminalScheme(`custom-${i}`)">
                  <div class="scheme-preview">
                    <span v-for="(color, name) in previewColors(s.colors)" :key="name" class="color-dot" :style="{ background: color }"></span>
                  </div>
                  <span class="scheme-name">{{ s.name }}</span>
                </div>
                <button class="scheme-delete" @click.stop="theme.removeCustomScheme(i)" title="删除">
                  <i class="fas fa-times"></i>
                </button>
              </div>
            </div>
          </div>

          <button class="add-scheme-btn" @click="showCustomCreator = true">
            <i class="fas fa-plus"></i> 新增配色方案
          </button>
        </section>

        <!-- ═══ 终端字体 ═══ -->
        <section class="settings-section">
          <h4>终端字体</h4>
          <div class="form-row">
            <label>字体族</label>
            <select :value="theme.fontFamily" @change="theme.setFontFamily($event.target.value)">
              <option v-for="f in theme.terminalFonts" :key="f.name" :value="f.value">{{ f.name }}</option>
            </select>
          </div>
          <div class="form-row">
            <label>字号</label>
            <div class="size-selector">
              <button v-for="s in theme.terminalFontSizes" :key="s" class="size-btn" :class="{ active: theme.fontSize === s }" @click="theme.setFontSize(s)">{{ s }}</button>
            </div>
          </div>
          <div class="form-row">
            <label>Scrollback 行数</label>
            <select :value="theme.scrollback" @change="theme.setScrollback(Number($event.target.value))">
              <option :value="1000">1,000</option>
              <option :value="5000">5,000</option>
              <option :value="10000">10,000</option>
              <option :value="50000">50,000</option>
            </select>
          </div>
          <div class="font-preview" :style="{ fontFamily: theme.fontFamily, fontSize: theme.fontSize + 'px' }">
            Sample: ls -la /home/user $ git status
          </div>
        </section>

        <!-- ═══ 连接设置 ═══ -->
        <section class="settings-section">
          <h4>连接</h4>
          <div class="form-row">
            <label>默认 SSH 端口</label>
            <input type="number" :value="theme.defaultPort" min="1" max="65535" @change="theme.setDefaultPort(Number($event.target.value))" class="input-sm" />
          </div>
          <div class="form-row">
            <label>连接超时</label>
            <select :value="theme.connectTimeout" @change="theme.setConnectTimeout(Number($event.target.value))">
              <option :value="5">5 秒</option>
              <option :value="10">10 秒</option>
              <option :value="15">15 秒</option>
              <option :value="30">30 秒</option>
            </select>
          </div>
        </section>
      </div>
    </div>

    <!-- 自定义配色创建器 -->
    <CustomSchemeCreator v-if="showCustomCreator" @close="showCustomCreator = false" />
  </div>
</template>

<script setup>
import { ref, computed } from "vue";
import { useThemeStore } from "../stores/theme";
import { useTerminalStore } from "../stores/terminal";
import { appThemes, terminalSchemes } from "../themes/index";
import CustomSchemeCreator from "./CustomSchemeCreator.vue";

defineEmits(["close"]);
const theme = useThemeStore();
const termStore = useTerminalStore();
const showCustomCreator = ref(false);

const darkSchemes = computed(() => theme.allTerminalSchemes.filter(s => s.group === "dark"));
const lightSchemes = computed(() => theme.allTerminalSchemes.filter(s => s.group === "light"));
const customSchemes = computed(() => theme.allTerminalSchemes.filter(s => !s.builtin));

function previewColors(colors) {
  return {
    black: colors.black, red: colors.red, green: colors.green,
    yellow: colors.yellow, blue: colors.blue, magenta: colors.magenta,
    cyan: colors.cyan, white: colors.white,
  };
}
</script>

<style scoped>
.modal-mask {
  position: fixed; inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex; align-items: center; justify-content: center;
  z-index: 1000;
}
.settings-panel {
  background: var(--surface); color: var(--text-primary);
  border: 1px solid var(--border); border-radius: 12px;
  width: 520px; max-height: 80vh;
  display: flex; flex-direction: column;
  box-shadow: 0 12px 40px rgba(0,0,0,0.5);
}
.settings-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 16px 20px; border-bottom: 1px solid var(--border);
}
.settings-header h3 {
  font-size: 15px; font-weight: 600;
  display: flex; align-items: center; gap: 8px;
}
.close-btn {
  width: 28px; height: 28px; border-radius: 6px;
  display: flex; align-items: center; justify-content: center;
  color: var(--text-muted); transition: background 0.12s;
}
.close-btn:hover { background: var(--elevated); color: var(--text-primary); }

.settings-body {
  overflow-y: auto; padding: 16px 20px;
  scrollbar-width: thin; scrollbar-color: var(--active) transparent;
}
.settings-body::-webkit-scrollbar { width: 6px; }
.settings-body::-webkit-scrollbar-thumb { background: var(--active); border-radius: 3px; }

.settings-section { margin-bottom: 24px; }
.settings-section h4 {
  font-size: 12px; font-weight: 600; text-transform: uppercase;
  letter-spacing: 0.5px; color: var(--text-muted); margin-bottom: 12px;
}

/* App 主题选择 */
.theme-grid { display: flex; gap: 8px; }
.theme-card {
  flex: 1; padding: 12px; border-radius: 8px;
  border: 1px solid var(--border); background: var(--panel);
  display: flex; flex-direction: column; align-items: center; gap: 6px;
  font-size: 12px; color: var(--text-secondary); cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}
.theme-card:hover { border-color: var(--text-muted); }
.theme-card.active { border-color: var(--accent); color: var(--accent); background: var(--accent-dim); }
.theme-card i { font-size: 18px; }

/* 终端配色 */
.scheme-group { margin-bottom: 12px; }
.scheme-group-label {
  font-size: 11px; color: var(--text-muted); margin-bottom: 8px;
  font-weight: 500;
}
.scheme-grid { display: flex; flex-wrap: wrap; gap: 6px; }
.scheme-card {
  position: relative;
  padding: 8px 10px; border-radius: 8px;
  border: 1px solid var(--border); background: var(--panel);
  cursor: pointer; transition: border-color 0.15s;
  display: flex; flex-direction: column; align-items: flex-start; gap: 4px;
  min-width: 120px;
}
.scheme-card:hover { border-color: var(--text-muted); }
.scheme-card.active { border-color: var(--accent); }
.scheme-card.custom { padding-right: 28px; }
.scheme-card-body { display: flex; flex-direction: column; gap: 4px; width: 100%; }
.scheme-delete {
  position: absolute; top: 6px; right: 6px;
  width: 18px; height: 18px; border-radius: 4px;
  display: flex; align-items: center; justify-content: center;
  font-size: 9px; color: var(--text-muted);
  opacity: 0; transition: opacity 0.12s, background 0.12s;
}
.scheme-card:hover .scheme-delete { opacity: 1; }
.scheme-delete:hover { background: var(--error); color: #fff; }
.scheme-preview { display: flex; gap: 2px; }
.color-dot { width: 10px; height: 10px; border-radius: 50%; border: 1px solid rgba(255,255,255,0.1); }
.scheme-name { font-size: 11px; color: var(--text-secondary); }

.add-scheme-btn {
  margin-top: 8px; padding: 8px 12px; border-radius: 6px;
  border: 1px dashed var(--border); background: none;
  color: var(--text-muted); font-size: 12px;
  display: flex; align-items: center; gap: 6px;
  transition: border-color 0.12s, color 0.12s;
}
.add-scheme-btn:hover { border-color: var(--accent); color: var(--accent); }

/* 表单 */
.form-row {
  display: flex; align-items: center; gap: 12px;
  margin-bottom: 10px;
}
.form-row label {
  font-size: 12px; color: var(--text-secondary);
  min-width: 100px; flex-shrink: 0;
}
.form-row select, .form-row input {
  flex: 1; padding: 6px 10px; border-radius: 6px;
  border: 1px solid var(--border); background: var(--panel);
  color: var(--text-primary); font-size: 12px;
  font-family: inherit;
}
.input-sm { width: 80px; flex: none; }
.size-selector { display: flex; gap: 4px; flex-wrap: wrap; }
.size-btn {
  width: 32px; height: 28px; border-radius: 4px;
  border: 1px solid var(--border); background: var(--panel);
  color: var(--text-secondary); font-size: 11px;
  display: flex; align-items: center; justify-content: center;
  transition: border-color 0.12s, color 0.12s;
}
.size-btn:hover { border-color: var(--text-muted); }
.size-btn.active { border-color: var(--accent); color: var(--accent); background: var(--accent-dim); }

.font-preview {
  margin-top: 10px; padding: 10px 12px; border-radius: 6px;
  background: var(--panel); border: 1px solid var(--border);
  color: var(--text-secondary); font-size: 14px;
  white-space: pre;
}
</style>
