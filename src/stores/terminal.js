import { defineStore } from "pinia";
import { safeInvoke } from "./safeInvoke";
import { listen } from "@tauri-apps/api/event";
import { ref } from "vue";
import { Terminal } from "xterm";
import { FitAddon } from "xterm-addon-fit";
import { SearchAddon } from "xterm-addon-search";
import { WebLinksAddon } from "xterm-addon-web-links";
import { Unicode11Addon } from "xterm-addon-unicode11";
import { ClipboardAddon } from "@xterm/addon-clipboard";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useConnectionStore } from "./connection";
import { useThemeStore } from "./theme";

export const useTerminalStore = defineStore("terminal", () => {
  const terminals = ref({});
  const fitAddons = ref({});
  const searchAddons = ref({});
  const searchResults = ref({});

  // 登录初始化期间（钩子注入 + clear）产生的输出先缓冲，不写屏。
  // 仅当目录跟随开启时（syncDir=true）才启用此缓冲；目录跟随关闭时直接写屏。
  // 前端在缓冲里监听 clear 动作：一旦检测到清屏序列，就丢弃此前所有内容
  // （它们已被 clear 擦掉），等 clear 之后的新提示符到达再一次性展示，
  // 这样用户直接看到干净提示符，不会闪过钩子脚本文本或多出来的提示符。
  const started = ref({});
  const outBuffer = ref({});
  const clearSeen = ref({});

  function createTerminal(sessionId, container) {
    if (terminals.value[sessionId]) {
      terminals.value[sessionId].dispose();
    }

    const conn = useConnectionStore();
    const session = conn.sessions.find((s) => s.id === sessionId);
    const isLocal = session?.type === "local";
    const needsInitBuffering = !isLocal && session?.syncDir;

    const themeStore = useThemeStore();
    const t = themeStore.xtermTheme;

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontSize: themeStore.fontSize,
      fontFamily: themeStore.fontFamily,
      scrollback: themeStore.scrollback,
      theme: t,
      allowTransparency: false,
      disableStdin: false,
    });

    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    const webLinksAddon = new WebLinksAddon((event, uri) => {
      event.preventDefault();
      const url = uri.includes("://") ? uri : `https://${uri}`;
      openUrl(url);
    });
    const unicodeAddon = new Unicode11Addon();
    const clipboardAddon = new ClipboardAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);
    term.loadAddon(webLinksAddon);
    term.loadAddon(unicodeAddon);
    term.loadAddon(clipboardAddon);
    term.unicode.activeVersion = '11';

    searchAddons.value[sessionId] = searchAddon;
    searchAddon.onDidChangeResults((r) => {
      searchResults.value[sessionId] = { resultIndex: r.resultIndex, resultCount: r.resultCount };
    });

    term.open(container);
    fitAddon.fit();

    const resizeCmd = isLocal ? "resize_local_terminal" : "resize_terminal";
    const sendResize = () => {
      const { cols, rows } = term;
      safeInvoke(resizeCmd, { sessionId, cols, rows }).catch(console.error);
    };
    term.onResize(sendResize);
    sendResize();

    const writeCmd = isLocal ? "write_local_stdin" : "write_stdin";
    term.onData((data) => {
      const adjusted = isLocal ? data.replace(/\x7f/g, "\x08") : data;
      safeInvoke(writeCmd, { sessionId, data: adjusted }).catch(console.error);
    });

    terminals.value[sessionId] = term;
    fitAddons.value[sessionId] = fitAddon;

    // 目录跟随开启时，后端会注入钩子+clear，输出先缓冲不写屏；
    // 前端监听 clear 动作完成后丢弃旧内容并展示新提示符。
    // 目录跟随关闭时，直接写屏，无缓冲。
    if (needsInitBuffering) {
      started.value[sessionId] = false;
      outBuffer.value[sessionId] = "";
      clearSeen.value[sessionId] = false;
      setTimeout(() => markStarted(sessionId), 2000);
    } else {
      started.value[sessionId] = true;
      outBuffer.value[sessionId] = "";
      clearSeen.value[sessionId] = true;
    }

    return term;
  }

  // 清屏动作标记：clear 命令回车 (\nclear\n) 或转义清屏 (ESC[2J)
  const CLEAR_MARK = /(\nclear\n|\x1b\[2J)/;

  function writeToTerminal(sessionId, data) {
    const term = terminals.value[sessionId];
    if (!term) return;
    // 未开始渲染前，先缓冲；检测到 clear 动作后丢弃此前内容并准备展示
    if (!started.value[sessionId]) {
      const buf = (outBuffer.value[sessionId] || "") + data;
      if (!clearSeen.value[sessionId] && CLEAR_MARK.test(buf)) {
        // clear 已执行：它之前的所有内容（钩子文本/旧提示符/clear 本身）
        // 都已被擦掉，只保留 clear 之后的内容（即新提示符）。
        const idx = buf.search(CLEAR_MARK);
        let after = buf.slice(idx).replace(CLEAR_MARK, "");
        outBuffer.value[sessionId] = after;
        clearSeen.value[sessionId] = true;
        // 稍等让 clear 之后的提示符到达，再一次性展示
        setTimeout(() => markStarted(sessionId), 120);
      } else {
        outBuffer.value[sessionId] = buf;
      }
      return;
    }
    // 已开始但缓冲还没flush（终端刚创建好）时，先把缓冲写出来
    const buf = outBuffer.value[sessionId];
    if (buf) {
      term.write(buf);
      outBuffer.value[sessionId] = "";
    }
    term.write(data);
  }

  function markStarted(sessionId) {
    started.value[sessionId] = true;
    const buf = outBuffer.value[sessionId];
    const term = terminals.value[sessionId];
    if (buf && term) {
      term.write(buf);
      outBuffer.value[sessionId] = "";
    }
  }

  function fitTerminal(sessionId) {
    const fit = fitAddons.value[sessionId];
    if (fit) {
      try {
        fit.fit();
      } catch (e) {
        // ignore
      }
    }
  }

  function resizeTerminal(sessionId, cols, rows) {
    safeInvoke("resize_terminal", { sessionId, cols, rows }).catch(console.error);
  }

  function destroyTerminal(sessionId) {
    const term = terminals.value[sessionId];
    if (term) {
      term.dispose();
      delete terminals.value[sessionId];
      delete fitAddons.value[sessionId];
    }
    delete searchAddons.value[sessionId];
    delete searchResults.value[sessionId];
    delete started.value[sessionId];
    delete outBuffer.value[sessionId];
    delete clearSeen.value[sessionId];
  }

  function updateAllTerminals() {
    const themeStore = useThemeStore();
    const t = themeStore.xtermTheme;
    Object.values(terminals.value).forEach((term) => {
      term.options.theme = t;
      term.options.fontSize = themeStore.fontSize;
      term.options.fontFamily = themeStore.fontFamily;
      term.options.scrollback = themeStore.scrollback;
    });
    // fit 所有已创建的终端以适配新字体大小
    Object.keys(fitAddons.value).forEach((sid) => fitTerminal(sid));
  }

  return {
    terminals,
    searchAddons,
    searchResults,
    createTerminal,
    writeToTerminal,
    markStarted,
    fitTerminal,
    resizeTerminal,
    destroyTerminal,
    updateAllTerminals,
  };
});

