<script setup>
import { ref, onMounted, computed } from "vue";
import { safeInvoke } from "../stores/safeInvoke";
import { useConnectionStore } from "../stores/connection";

const emit = defineEmits(["close"]);
const conn = useConnectionStore();

const panelWidth = computed({
  get: () => conn.sysinfoPanelWidth,
  set: (v) => conn.setSysinfoPanelWidth(v),
});

// --- Resize ---
let resizeStartX = 0;
let resizeStartW = 440;
const MIN_WIDTH = 280;

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

const tabs = [
  { key: "system", label: "系统", icon: "fa-server" },
  { key: "cpu", label: "CPU", icon: "fa-microchip" },
  { key: "memory", label: "内存", icon: "fa-memory" },
  { key: "disk", label: "磁盘", icon: "fa-hdd" },
  { key: "network", label: "网卡", icon: "fa-network-wired" },
  { key: "processes", label: "进程", icon: "fa-list" },
  { key: "ports", label: "端口", icon: "fa-plug" },
];

const activeTab = ref("system");
const data = ref(null);
const loading = ref(false);
const error = ref(null);

async function fetchData() {
  if (!conn.activeSessionId) return;
  loading.value = true;
  error.value = null;
  data.value = null;
  try {
    data.value = await safeInvoke("sysinfo_get", {
      sessionId: conn.activeSessionId,
    });
  } catch (e) {
    error.value = e;
  } finally {
    loading.value = false;
  }
}

onMounted(fetchData);

function fmtMb(mb) {
  if (mb > 1024) return (mb / 1024).toFixed(1) + " GiB";
  return mb.toFixed(0) + " MiB";
}

function barColor(pct) {
  if (pct >= 90) return "var(--error)";
  if (pct >= 70) return "#e6a817";
  return "var(--accent)";
}
</script>

<template>
  <div class="panel panel--sysinfo" :style="{ width: panelWidth + 'px' }">
    <div class="resize-handle" @mousedown.prevent="onResizeStart"></div>
    <div class="panel-header">
      <div class="panel-tabs">
        <button class="tab active"><i class="fas fa-chart-bar"></i> 系统信息</button>
      </div>
      <div class="panel-actions">
        <button title="刷新" @click="fetchData" :disabled="loading"><i class="fas fa-sync-alt" :class="{ spin: loading }"></i></button>
        <span class="action-sep"></span>
        <button title="关闭" @click="$emit('close')" class="btn-close-panel"><i class="fas fa-times"></i></button>
      </div>
    </div>

    <div class="sys-tabs">
      <button
        v-for="t in tabs"
        :key="t.key"
        class="sys-tab"
        :class="{ active: activeTab === t.key }"
        @click="activeTab = t.key"
      >
        <i :class="'fas ' + t.icon"></i>
        {{ t.label }}
      </button>
    </div>

    <div class="sys-content">
      <div v-if="loading" class="sys-status"><i class="fas fa-spinner spin"></i> 采集数据中…</div>
      <div v-else-if="error" class="sys-status sys-error">{{ error }}</div>
      <div v-else-if="!data" class="sys-status">暂无数据</div>

      <!-- ═══ 系统 ═══ -->
      <template v-else-if="activeTab === 'system'">
        <table class="info-table"><tbody>
          <tr><td class="label">主机名</td><td>{{ data.hostname || '-' }}</td></tr>
          <tr><td class="label">操作系统</td><td>{{ data.os || '-' }}</td></tr>
          <tr><td class="label">内核版本</td><td>{{ data.kernel || '-' }}</td></tr>
        </tbody></table>
      </template>

      <!-- ═══ CPU ═══ -->
      <template v-else-if="activeTab === 'cpu'">
        <table class="info-table"><tbody>
          <tr><td class="label">型号</td><td>{{ data.cpu_model || '-' }}</td></tr>
          <tr><td class="label">核心数</td><td>{{ data.cpu_cores || '-' }}</td></tr>
          <tr v-if="data.cpu_freq_mhz > 0"><td class="label">当前频率</td><td>{{ data.cpu_freq_mhz.toFixed(0) }} MHz</td></tr>
          <tr v-if="data.cpu_max_mhz > 0"><td class="label">最大频率</td><td>{{ data.cpu_max_mhz.toFixed(0) }} MHz</td></tr>
        </tbody></table>
        <div class="bar-section">
          <div class="bar-label">CPU 使用率</div>
          <div class="bar-track">
            <div
              class="bar-fill"
              :style="{ width: data.cpu_usage_pct + '%', background: barColor(data.cpu_usage_pct) }"
            ></div>
          </div>
          <div class="bar-pct" :style="{ color: barColor(data.cpu_usage_pct) }">
            {{ data.cpu_usage_pct.toFixed(1) }}%
          </div>
        </div>
      </template>

      <!-- ═══ 内存 ═══ -->
      <template v-else-if="activeTab === 'memory'">
        <table class="info-table"><tbody>
          <tr><td class="label">总计</td><td>{{ fmtMb(data.memory.total_mb) }}</td></tr>
          <tr><td class="label">已用</td><td>{{ fmtMb(data.memory.used_mb) }}</td></tr>
          <tr><td class="label">可用</td><td>{{ fmtMb(data.memory.total_mb - data.memory.used_mb) }}</td></tr>
          <tr><td class="label">使用率</td><td :style="{ color: barColor(data.memory.pct) }">{{ data.memory.pct.toFixed(1) }}%</td></tr>
        </tbody></table>
        <div class="bar-section">
          <div class="bar-label">内存</div>
          <div class="bar-track">
            <div
              class="bar-fill"
              :style="{ width: data.memory.pct + '%', background: barColor(data.memory.pct) }"
            ></div>
          </div>
        </div>
        <div v-if="data.swap.total_mb > 0" class="bar-section" style="margin-top:12px">
          <div class="bar-label">交换分区</div>
          <div class="bar-track">
            <div
              class="bar-fill"
              :style="{ width: data.swap.pct + '%', background: barColor(data.swap.pct) }"
            ></div>
          </div>
          <div class="bar-pct" :style="{ color: barColor(data.swap.pct) }">
            {{ data.swap.pct.toFixed(1) }}%（{{ fmtMb(data.swap.used_mb) }} / {{ fmtMb(data.swap.total_mb) }}）
          </div>
        </div>
      </template>

      <!-- ═══ 磁盘 ═══ -->
      <template v-else-if="activeTab === 'disk'">
        <div v-if="data.disks.length === 0" class="sys-status">无磁盘信息</div>
        <div v-for="(d, i) in data.disks" :key="i" class="disk-row">
          <div class="disk-mount">{{ d.mount }}</div>
          <div class="bar-section" style="margin:0">
            <div class="bar-label">{{ d.filesystem }} — {{ d.used }} / {{ d.total }}</div>
            <div class="bar-track">
              <div
                class="bar-fill"
                :style="{ width: d.pct + '%', background: barColor(d.pct) }"
              ></div>
            </div>
            <div class="bar-pct" :style="{ color: barColor(d.pct) }">{{ d.pct.toFixed(0) }}%</div>
          </div>
        </div>
      </template>

      <!-- ═══ 网卡 ═══ -->
      <template v-else-if="activeTab === 'network'">
        <div v-if="data.interfaces.length === 0" class="sys-status">无网卡信息</div>
        <table class="info-table">
          <thead>
            <tr><th>接口</th><th>IP 地址</th><th>MAC 地址</th></tr>
          </thead>
          <tbody>
            <tr v-for="(n, i) in data.interfaces" :key="i">
              <td>{{ n.name }}</td>
              <td>{{ n.ip || '-' }}</td>
              <td class="mono">{{ n.mac || '-' }}</td>
            </tr>
          </tbody>
        </table>
      </template>

      <!-- ═══ 进程 ═══ -->
      <template v-else-if="activeTab === 'processes'">
        <div v-if="data.processes.length === 0" class="sys-status">无进程信息</div>
        <table class="info-table process-table">
          <thead>
            <tr><th>PID</th><th>CPU%</th><th>MEM%</th><th>命令</th></tr>
          </thead>
          <tbody>
            <tr v-for="(p, i) in data.processes" :key="i">
              <td class="mono">{{ p.pid }}</td>
              <td class="mono">{{ p.cpu_pct.toFixed(1) }}</td>
              <td class="mono">{{ p.mem_pct.toFixed(1) }}</td>
              <td class="cmd">{{ p.command }}</td>
            </tr>
          </tbody>
        </table>
      </template>

      <!-- ═══ 端口 ═══ -->
      <template v-else-if="activeTab === 'ports'">
        <div v-if="data.ports.length === 0" class="sys-status">无端口信息</div>
        <table class="info-table">
          <thead>
            <tr><th>端口</th><th>协议</th><th>进程</th></tr>
          </thead>
          <tbody>
            <tr v-for="(p, i) in data.ports" :key="i">
              <td class="mono">{{ p.port }}</td>
              <td>{{ p.protocol }}</td>
              <td>{{ p.process || '-' }}</td>
            </tr>
          </tbody>
        </table>
      </template>
    </div>
  </div>
</template>

<style scoped>
.panel--sysinfo {
  position: absolute; top: 0; right: 0; bottom: 0;
  display: flex; flex-direction: column;
  overflow: hidden; background: var(--surface);
  border-left: 1px solid var(--border); z-index: 20;
  box-shadow: -4px 0 20px rgba(0,0,0,0.25);
}
.resize-handle {
  position: absolute; left: -3px; top: 0; bottom: 0; width: 6px;
  cursor: col-resize; z-index: 5;
}
.resize-handle:hover, .resize-handle:active {
  background: var(--accent); opacity: 0.4;
}
.panel-header {
  height: var(--panel-header-h); background: var(--surface);
  display: flex; align-items: center;
  justify-content: space-between; padding: 0 4px 0 8px; flex-shrink: 0;
}
.panel-tabs { display: flex; align-items: center; gap: 2px; height: 100%; }
.tab {
  display: flex; align-items: center; gap: 6px;
  padding: 0 12px; height: 28px; border-radius: 6px; font-size: 12px;
  font-weight: 500; color: var(--text-secondary);
  background: none; border: none; cursor: pointer;
  transition: background 0.12s, color 0.12s; font-family: inherit;
}
.tab:hover { background: var(--elevated); color: var(--text-primary); }
.tab.active { background: var(--panel); color: var(--text-primary); }
.tab i { font-size: 11px; }
.panel-actions { display: flex; align-items: center; gap: 1px; }
.panel-actions button {
  width: 28px; height: 28px; border-radius: 4px;
  display: flex; align-items: center; justify-content: center;
  font-size: 12px; color: var(--text-muted);
  background: none; border: none; cursor: pointer;
  transition: background 0.12s, color 0.12s; font-family: inherit;
}
.panel-actions button:hover { background: var(--elevated); color: var(--text-primary); }
.panel-actions button:disabled { opacity: 0.3; cursor: not-allowed; }
.action-sep { width:1px; height:18px; background:var(--border); margin:0 3px; }
.btn-close-panel:hover { color: var(--error) !important; }

.sys-tabs {
  display: flex; flex-wrap: wrap; gap: 2px; padding: 6px 8px;
  background: var(--panel); border-bottom: 1px solid var(--border); flex-shrink: 0;
}
.sys-tab {
  padding: 4px 10px; border-radius: 4px; font-size: 11px; font-family: inherit;
  color: var(--text-muted); background: none; border: none; cursor: pointer;
  display: flex; align-items: center; gap: 4px;
  transition: background 0.12s, color 0.12s;
}
.sys-tab:hover { background: var(--elevated); color: var(--text-secondary); }
.sys-tab.active { background: var(--accent-dim); color: var(--accent); }
.sys-tab i { font-size: 10px; }

.sys-content {
  flex: 1; overflow-y: auto; padding: 12px 16px; scrollbar-width: thin;
  scrollbar-color: var(--active) transparent;
}
.sys-content::-webkit-scrollbar { width: 6px; }
.sys-content::-webkit-scrollbar-thumb { background: var(--active); border-radius: 3px; }

.sys-status { color: var(--text-muted); text-align: center; padding: 24px; font-size: 13px; }
.sys-error { color: var(--error); }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

/* ── Info table ── */
.info-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.info-table td, .info-table th { padding: 5px 8px; text-align: left; border-bottom: 1px solid var(--border); }
.info-table th { color: var(--text-muted); font-weight: 500; font-size: 11px; }
.info-table .label { color: var(--text-muted); width: 90px; white-space: nowrap; }
.info-table .mono { font-family: var(--font-mono); }
.info-table .cmd { font-family: var(--font-mono); font-size: 11px; max-width: 240px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.process-table { font-size: 11px; }
.process-table td, .process-table th { padding: 3px 6px; }

/* ── Stat card (uptime) ── */
.stat-card { text-align: center; padding: 24px 0; }
.stat-icon { font-size: 36px; color: var(--accent); margin-bottom: 12px; }
.stat-value { font-size: 20px; font-weight: 600; color: var(--text-primary); margin-bottom: 4px; }
.stat-label { font-size: 12px; color: var(--text-muted); }

/* ── Progress bar ── */
.bar-section { display: flex; align-items: center; gap: 8px; margin-top: 10px; }
.bar-label { font-size: 11px; color: var(--text-muted); white-space: nowrap; flex-shrink: 0; }
.bar-track { flex: 1; height: 8px; background: var(--elevated); border-radius: 4px; overflow: hidden; }
.bar-fill { height: 100%; border-radius: 4px; transition: width 0.3s ease; }
.bar-pct { font-size: 11px; font-weight: 600; width: 48px; text-align: right; flex-shrink: 0; font-family: var(--font-mono); }

/* ── Disk row ── */
.disk-row { padding: 8px 0; border-bottom: 1px solid var(--border); }
.disk-row:last-child { border-bottom: none; }
.disk-mount { font-size: 13px; font-weight: 500; color: var(--text-primary); margin-bottom: 4px; }
</style>



