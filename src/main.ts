import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import * as ElementPlusIconsVue from '@element-plus/icons-vue';
import 'element-plus/dist/index.css';
import 'element-plus/theme-chalk/dark/css-vars.css';
import { i18n } from "./i18n";
import { watch } from "vue";
import { appSettings } from "./store";

const app = createApp(App);

for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component);
}

app.use(router);
app.use(i18n);
app.mount("#app");

// 1. 初始设置：将 store 中的 locale 应用到 i18n
i18n.global.locale.value = appSettings.locale || 'zhs';

// 2. 监听 store 中 locale 的变化，实时更新 i18n
watch(
  () => appSettings.locale,  // 监听 appSettings.locale
  (newLocale) => {
    console.log('[i18n] locale changed to:', newLocale);
    console.log('[国际化] 本地语言切换为了:', newLocale);
    if (newLocale && i18n.global.locale.value !== newLocale) {
      i18n.global.locale.value = newLocale;
    }
  },
  { immediate: false }
);