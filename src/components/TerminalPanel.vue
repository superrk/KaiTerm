<script setup>
import { ref, watch, onMounted, onUnmounted, nextTick } from "vue";
import { useConnectionStore } from "../stores/connection";
import { useTerminalStore } from "../stores/terminal";
import { listen } from "@tauri-apps/api/event";


const conn = useConnectionStore();
const termStore = useTerminalStore();
const termContainers = ref({});
const unlisteners = [];

const showSearch = ref(false);
const searchQuery = ref("");
const matchIndex = ref(0);
const matchCount = ref(0);
const searchInput = ref(null);

function toggleSearch() {
  showSearch.value = !showSearch.value;
  if (showSearch.value) {
    nextTick(() => searchInput.value?.focus());
  } else {
    searchQuery.value = "";
    matchIndex.value = 0;
    matchCount.value = 0;
  }
}

function doSearch(dir) {
  const sid = conn.activeSessionId;
  if (!sid || !searchQuery.value) return;
  const sa = termStore.searchAddons[sid];
  if (!sa) return;
  if (dir === "next") {
    sa.findNext(searchQuery.value, { incremental: false });
  } else {
    sa.findPrevious(searchQuery.value, { incremental: false });
  }
}

function onSearchInput() {
  const sid = conn.activeSessionId;
  if (!sid) return;
  const sa = termStore.searchAddons[sid];
  if (!sa) return;
  if (!searchQuery.value) {
    sa.clearActiveDecoration();
    matchIndex.value = 0;
    matchCount.value = 0;
    return;
  }
  sa.findNext(searchQuery.value, { incremental: true });
}

watch(() => termStore.searchResults[conn.activeSessionId], (r) => {
  if (r) {
    matchIndex.value = r.resultIndex + 1;
    matchCount.value = r.resultCount;
  }
}, { immediate: true });

// 屏幕缩放 / 窗口尺寸变化后，重新计算终端列行数并通知后端 resize。
// 缩放变化来自 Tauri 的 scaleChange 事件；窗口尺寸变化用 ResizeObserver 兜底。
function refitActive() {
  const sid = conn.activeSessionId;
  if (!sid) return;
  const el = termContainers.value[sid];
  if (!el) return;
  termStore.fitTerminal(sid);
}

function onKeydown(e) {
  if (e.ctrlKey && e.shiftKey && (e.key === "F" || e.key === "f")) {
    e.preventDefault();
    toggleSearch();
    return;
  }
  if (e.key === "Escape" && showSearch.value) {
    e.preventDefault();
    toggleSearch();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);
  unlisteners.push(() => window.removeEventListener("keydown", onKeydown));
  unlisteners.push(
    await listen("terminal-output", (event) => {
      const { session_id, data } = event.payload;
      termStore.writeToTerminal(session_id, data);
    })
  );
  unlisteners.push(
    await listen("terminal-error", (event) => {
      const sid = conn.activeSessionId;
      if (sid) termStore.writeToTerminal(sid, `\r\n\x1b[1;31m[错误] ${event.payload}\x1b[0m\r\n`);
    })
  );

  // 监听窗口/屏幕缩放变化：Tauri v2 通过 webview 的 scaleChange 事件广播
  unlisteners.push(
    await listen("scaleChange", () => refitActive())
  );
  // 兜底：浏览器原生 resize（含 DPI 改变触发的 resize）
  const onResize = () => refitActive();
  window.addEventListener("resize", onResize);
  unlisteners.push(() => window.removeEventListener("resize", onResize));
  // 兜底：终端容器尺寸变化（如面板拖动、文件面板开合）
  const ro = new ResizeObserver(() => refitActive());
  ro.observe(document.getElementById("terminalContainer") || document.body);
  unlisteners.push(() => ro.disconnect());

  await nextTick();
  ensureActiveTerminal();
});

onUnmounted(() => {
  unlisteners.forEach((fn) => fn());
  unlisteners.length = 0;
});

function ensureActiveTerminal() {
  const sid = conn.activeSessionId;
  if (!sid) return;
  const el = termContainers.value[sid];
  if (el && !termStore.terminals[sid]) {
    termStore.createTerminal(sid, el);
    nextTick(() => termStore.fitTerminal(sid));
  }
}

watch(() => conn.sessions.length, async () => {
  await nextTick();
  // New session added — create terminal if container is ready
  conn.sessions.forEach((s) => {
    const el = termContainers.value[s.id];
    if (el && !termStore.terminals[s.id]) {
      termStore.createTerminal(s.id, el);
    }
  });
});

watch(() => conn.activeSessionId, async (newId, oldId) => {
  if (oldId) {
    const oldEl = termContainers.value[oldId];
    if (oldEl) oldEl.style.display = "none";
  }
  if (newId) {
    const el = termContainers.value[newId];
    if (el) {
      el.style.display = "block";
      if (!termStore.terminals[newId]) {
        await nextTick();
        termStore.createTerminal(newId, el);
      }
      nextTick(() => termStore.fitTerminal(newId));
    }
  }
});

function setContainerRef(sid, el) {
  if (el) termContainers.value[sid] = el;
}
</script>

<template>
  <div class="terminal-container">
    <div
      v-for="s in conn.sessions"
      :key="s.id"
      :ref="(el) => setContainerRef(s.id, el)"
      class="terminal-instance"
      :style="{ display: s.id === conn.activeSessionId ? 'block' : 'none' }"
    ></div>
    <div v-show="showSearch" class="search-bar" @click.stop>
      <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="11" cy="11" r="8"/>
        <path d="m21 21-4.35-4.35"/>
      </svg>
      <input
        ref="searchInput"
        v-model="searchQuery"
        type="text"
        class="search-input"
        placeholder="查找..."
        @input="onSearchInput"
        @keydown.enter="doSearch('next')"
        @keydown.shift.enter="doSearch('prev')"
      />
      <span v-if="searchQuery" class="match-count">
        {{ matchCount > 0 ? `${matchIndex}/${matchCount}` : '0/0' }}
      </span>
      <button class="search-btn" title="上一个 (Shift+Enter)" @click="doSearch('prev')">
        <svg viewBox="0 0 24 24" fill="currentColor"><path d="M7.41 15.41L12 10.83l4.59 4.58L18 14l-6-6-6 6z"/></svg>
      </button>
      <button class="search-btn" title="下一个 (Enter)" @click="doSearch('next')">
        <svg viewBox="0 0 24 24" fill="currentColor"><path d="M7.41 8.59L12 13.17l4.59-4.58L18 10l-6 6-6-6z"/></svg>
      </button>
      <button class="search-close" title="关闭 (Esc)" @click="toggleSearch">
        <svg viewBox="0 0 24 24" fill="currentColor"><path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.terminal-container {
  width: 100%;
  height: 100%;
  position: relative;
}
.terminal-instance {
  width: 100%;
  height: 100%;
  background: var(--panel);
  padding: 4px;
  box-sizing: border-box;
}
</style>

<style>
.xterm-viewport {
  overflow-y: auto !important;
  scrollbar-width: thin;
  scrollbar-color: rgba(255,255,255,0.08) transparent;
}
.xterm-viewport::-webkit-scrollbar {
  width: 6px;
}
.xterm-viewport::-webkit-scrollbar-track {
  background: transparent;
}
.xterm-viewport::-webkit-scrollbar-thumb {
  background: rgba(255,255,255,0.08);
  border-radius: 3px;
  min-height: 30px;
}
.xterm-viewport::-webkit-scrollbar-thumb:hover {
  background: rgba(255,255,255,0.15);
}
.xterm-viewport::-webkit-scrollbar-corner {
  background: transparent;
}
.xterm {
  padding: 0;
  height: 100%;
}
.xterm-screen {
  height: 100%;
}

.search-bar {
  position: absolute;
  bottom: 8px;
  right: 8px;
  display: flex;
  align-items: center;
  gap: 4px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 4px 6px;
  z-index: 10;
  box-shadow: 0 4px 12px rgba(0,0,0,0.3);
}
.search-icon {
  width: 14px;
  height: 14px;
  color: var(--text-muted);
  flex-shrink: 0;
}
.search-input {
  width: 180px;
  background: transparent;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
}
.search-input::placeholder {
  color: var(--text-muted);
}
.match-count {
  font-size: 11px;
  color: var(--text-muted);
  min-width: 30px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}
.search-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 3px;
  padding: 0;
}
.search-btn:hover {
  background: var(--elevated);
  color: var(--text-primary);
}
.search-btn svg {
  width: 16px;
  height: 16px;
}
.search-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  border-radius: 3px;
  padding: 0;
}
.search-close:hover {
  background: var(--elevated);
  color: var(--text-primary);
}
.search-close svg {
  width: 14px;
  height: 14px;
}
</style>