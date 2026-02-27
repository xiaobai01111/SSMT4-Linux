import { createApp, type Plugin } from "vue";
import { invoke } from "@tauri-apps/api/core";
import App from "./App.vue";
import router from "./router";
import * as ElementPlusIconsVue from '@element-plus/icons-vue';
import 'element-plus/dist/index.css';
import 'element-plus/theme-chalk/dark/css-vars.css';
import { i18n } from "./i18n";
import { watch } from "vue";
import { appSettings } from "./store";

// F12 开发者工具快捷键（仅 devtools feature 编译时后端可用）
document.addEventListener('keydown', (e) => {
  if (e.key === 'F12') {
    e.preventDefault();
    invoke('toggle_devtools').catch(() => {});
  }
});

const app = createApp(App);

for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component);
}

// Keep build stable even when lockfile temporarily pulls mismatched plugin typings.
app.use(router as unknown as Plugin);
app.use(i18n as unknown as Plugin);
app.mount("#app");

// 1. 初始设置：将 store 中的 locale 应用到 i18n
i18n.global.locale.value = appSettings.locale || 'zhs';

// 2. 监听 store 中 locale 的变化，实时更新 i18n
watch(
  () => appSettings.locale,  // 监听 appSettings.locale
  (newLocale) => {
    if (newLocale && i18n.global.locale.value !== newLocale) {
      i18n.global.locale.value = newLocale;
    }
  },
  { immediate: false }
);
