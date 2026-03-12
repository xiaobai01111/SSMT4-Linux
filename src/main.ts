import { createApp, type Plugin } from "vue";
import { invoke } from "@tauri-apps/api/core";
import App from "./App.vue";
import router from "./router";
import * as ElementPlusIconsVue from '@element-plus/icons-vue';
import 'element-plus/theme-chalk/dark/css-vars.css';
import { i18n } from "./i18n";
import { watch } from "vue";
import { appSettings, bootstrapStore, startStorePostBootstrapTasks } from "./store";
import { bootstrapDownloadStore } from "./downloadStore";

if (import.meta.env.DEV) {
  // F12 开发者工具快捷键（仅开发态注册，后端由 devtools feature 提供）
  document.addEventListener('keydown', (e) => {
    if (e.key === 'F12') {
      e.preventDefault();
      invoke('toggle_devtools').catch(() => {});
    }
  });
}

async function bootstrapApplication() {
  await bootstrapStore();
  await bootstrapDownloadStore();

  // 初始设置：启动前先将 store 中的 locale 应用到 i18n，避免首屏闪动到默认语言
  i18n.global.locale.value = appSettings.locale || 'zhs';

  const app = createApp(App);

  for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
    app.component(key, component);
  }

  // Keep build stable even when lockfile temporarily pulls mismatched plugin typings.
  app.use(router as unknown as Plugin);
  app.use(i18n as unknown as Plugin);
  app.mount("#app");
  startStorePostBootstrapTasks();

  watch(
    () => appSettings.locale,
    (newLocale) => {
      if (newLocale && i18n.global.locale.value !== newLocale) {
        i18n.global.locale.value = newLocale;
      }
    },
    { immediate: false },
  );
}

void bootstrapApplication().catch((error) => {
  console.error('Failed to bootstrap application:', error);
});
