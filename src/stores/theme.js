import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { appThemes, terminalSchemes, terminalFonts, terminalFontSizes } from "../themes/index";

function loadSetting(key, fallback) {
  try {
    const v = localStorage.getItem(key);
    return v !== null ? v : fallback;
  } catch {
    return fallback;
  }
}

function saveSetting(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {}
}

export const useThemeStore = defineStore("theme", () => {
  // ── App 主题 ──
  const appThemeId = ref(loadSetting("kaiterm_app_theme", "dark"));
  const appTheme = computed(() => appThemes[appThemeId.value] || appThemes.dark);

  function setAppTheme(id) {
    if (!appThemes[id]) return;
    appThemeId.value = id;
    saveSetting("kaiterm_app_theme", id);
    applyAppTheme();
  }

  function applyAppTheme() {
    const root = document.documentElement;
    const vars = appTheme.value.vars;
    for (const [k, v] of Object.entries(vars)) {
      root.style.setProperty(k, v);
    }
  }

  // ── 终端配色方案 ──
  const terminalSchemeId = ref(loadSetting("kaiterm_terminal_scheme", "default-dark"));
  const terminalScheme = computed(() => terminalSchemes[terminalSchemeId.value] || terminalSchemes["default-dark"]);

  // 自定义配色方案列表
  const customSchemes = ref(JSON.parse(loadSetting("kaiterm_custom_schemes", "[]")));

  const allTerminalSchemes = computed(() => {
    const builtins = Object.entries(terminalSchemes).map(([id, s]) => ({
      id, name: s.name, group: s.group, colors: s.colors, builtin: true,
    }));
    const customs = customSchemes.value.map((s, i) => ({
      id: `custom-${i}`, name: s.name, group: s.group, colors: s.colors, builtin: false,
    }));
    return [...builtins, ...customs];
  });

  function setTerminalScheme(id) {
    terminalSchemeId.value = id;
    saveSetting("kaiterm_terminal_scheme", id);
  }

  function addCustomScheme(scheme) {
    customSchemes.value.push(scheme);
    saveSetting("kaiterm_custom_schemes", JSON.stringify(customSchemes.value));
  }

  function removeCustomScheme(index) {
    customSchemes.value.splice(index, 1);
    saveSetting("kaiterm_custom_schemes", JSON.stringify(customSchemes.value));
    // 如果当前选中的是被删除的方案，回退到默认
    if (terminalSchemeId.value.startsWith(`custom-${index}`)) {
      setTerminalScheme("default-dark");
    }
  }

  // 获取当前终端 xterm theme
  const xtermTheme = computed(() => terminalScheme.value.colors);

  // ── 终端字体 ──
  const fontFamily = ref(loadSetting("kaiterm_term_font", terminalFonts[0].value));
  const fontSize = ref(Number(loadSetting("kaiterm_term_font_size", "14")));
  const scrollback = ref(Number(loadSetting("kaiterm_term_scrollback", "5000")));

  function setFontFamily(v) {
    fontFamily.value = v;
    saveSetting("kaiterm_term_font", v);
  }

  function setFontSize(v) {
    fontSize.value = v;
    saveSetting("kaiterm_term_font_size", v);
  }

  function setScrollback(v) {
    scrollback.value = v;
    saveSetting("kaiterm_term_scrollback", v);
  }

  // ── 连接设置 ──
  const defaultPort = ref(Number(loadSetting("kaiterm_default_port", "22")));
  const connectTimeout = ref(Number(loadSetting("kaiterm_connect_timeout", "15")));

  function setDefaultPort(v) {
    defaultPort.value = v;
    saveSetting("kaiterm_default_port", v);
  }

  function setConnectTimeout(v) {
    connectTimeout.value = v;
    saveSetting("kaiterm_connect_timeout", v);
  }

  // ── 设置面板显示状态 ──
  const showSettings = ref(false);

  return {
    // Settings panel
    showSettings,
    // App theme
    appThemeId, appTheme, setAppTheme, applyAppTheme,
    // Terminal scheme
    terminalSchemeId, terminalScheme, allTerminalSchemes,
    setTerminalScheme, addCustomScheme, removeCustomScheme, xtermTheme,
    // Font
    fontFamily, fontSize, scrollback,
    setFontFamily, setFontSize, setScrollback,
    // Connection
    defaultPort, connectTimeout, setDefaultPort, setConnectTimeout,
    // Constants
    terminalFonts, terminalFontSizes,
  };
});
