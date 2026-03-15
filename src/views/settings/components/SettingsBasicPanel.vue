<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { AppSettings, Locale } from '../../../types/ipc';

const props = defineProps<{
  appSettings: AppSettings;
  selectDataDir: () => void | Promise<void>;
  selectCacheDir: () => void | Promise<void>;
  updateLocaleAndReload: (locale: Locale) => void | Promise<void>;
  openLogWindow: () => void | Promise<void>;
  reenterOnboarding: () => void | Promise<void>;
}>();

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => {
  return te(key) ? String(t(key)) : fallback;
};

const handleLocaleChange = async (value: string | number | boolean) => {
  const nextLocale = String(value) as Locale;
  await props.updateLocaleAndReload(nextLocale);
};
</script>

<template>
  <div class="settings-panel basic-panel w-full" data-onboarding="settings-basic-panel">
    <div class="panel-header">
      <h2 class="panel-title">{{ t('settings.basicsettings') }}</h2>
    </div>

    <el-card class="setting-card mt-5 full-width-card" shadow="never">
      <template #header>
        <div class="card-header-title text-primary">{{ tr('settings.general_settings', '常规设置') }}</div>
      </template>
      <div class="two-column-grid">
        <div class="input-group">
          <label class="input-label">{{ t('settings.language') }}</label>
          <el-select
            :model-value="appSettings.locale"
            :placeholder="tr('settings.language_select_placeholder', 'Select language')"
            class="flex-1 max-w-sm"
            @update:model-value="handleLocaleChange"
          >
            <el-option :label="tr('settings.language_options.zhs', '简体中文')" value="zhs" />
            <el-option :label="tr('settings.language_options.zht', '繁體中文')" value="zht" />
            <el-option :label="tr('settings.language_options.en', 'English')" value="en" />
          </el-select>
        </div>

        <div class="input-group">
          <label class="input-label">{{ tr('settings.snowbreakSourcePolicy.label', '尘白下载源策略') }}</label>
          <el-select v-model="appSettings.snowbreakSourcePolicy" class="flex-1 max-w-sm">
            <el-option :label="tr('settings.snowbreakSourcePolicy.officialFirst', '官方优先（失败后回退社区）')" value="official_first" />
            <el-option :label="tr('settings.snowbreakSourcePolicy.communityFirst', '社区优先（失败后回退官方）')" value="community_first" />
          </el-select>
          <div class="input-hint">{{ tr('settings.snowbreakSourcePolicy.hint', '推荐保持"官方优先"，网络异常时会自动回退到另一来源。') }}</div>
        </div>
      </div>
    </el-card>

    <el-card class="setting-card mt-6 full-width-card" shadow="never">
      <template #header>
        <div class="card-header-title text-primary">{{ tr('settings.directory_settings', '目录设置') }}</div>
      </template>
      <div class="path-grid">
        <div class="input-group">
          <label class="input-label">{{ t('settings.datadir') }}</label>
          <div class="flex-row">
            <el-input v-model="appSettings.dataDir" :placeholder="t('settings.datadir_placeholder')" class="flex-1" />
            <el-button @click="selectDataDir">{{ t('settings.selectfolder') }}</el-button>
          </div>
          <div class="input-hint">{{ t('settings.datadir_hint') }}</div>
        </div>

        <div class="input-group">
          <label class="input-label">{{ t('settings.cachedir') }}</label>
          <div class="flex-row">
            <el-input v-model="appSettings.cacheDir" :placeholder="t('settings.cachedir_placeholder')" class="flex-1" />
            <el-button @click="selectCacheDir">{{ t('settings.selectfolder') }}</el-button>
          </div>
        </div>
      </div>
    </el-card>

    <el-card class="setting-card mt-6 full-width-card mb-8" shadow="never">
      <template #header>
        <div class="card-header-title text-primary">{{ tr('settings.advanced_maintenance', '高级与维护') }}</div>
      </template>
      <div class="path-grid">
        
        <div class="input-group">
          <label class="input-label">{{ t('settings.github_token') }}</label>
          <el-input v-model="appSettings.githubToken" :placeholder="t('settings.github_token_placeholder')" type="password" show-password class="max-w-md" />
        </div>

        <el-divider border-style="dashed" class="my-2" />

        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">{{ tr('settings.logviewer.label', '日志查看器') }}</div>
            <div class="setting-desc">{{ tr('settings.logviewer.hint', '在新窗口中查看软件运行日志，便于排查问题时提供给开发者。') }}</div>
          </div>
          <div class="setting-control pl-4">
            <el-button @click="openLogWindow">{{ tr('settings.logviewer.open', '打开日志窗口') }}</el-button>
          </div>
        </div>

        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">{{ tr('settings.onboarding.label', '新手引导') }}</div>
            <div class="setting-desc">{{ tr('settings.onboarding.hint', '仅重新展示功能导览（主页、游戏库、运行环境等），不会重置初始化设置。') }}</div>
          </div>
          <div class="setting-control pl-4">
            <el-button @click="reenterOnboarding">{{ tr('settings.onboarding.reenter', '重新进入新手引导') }}</el-button>
          </div>
        </div>

      </div>
    </el-card>
  </div>
</template>

<style scoped>
/* 强制覆盖全局限制，确保面板 100% 宽度 */
.basic-panel {
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  color: var(--el-text-color-primary);
  width: 100% !important;
  max-width: none !important;
  flex-grow: 1;
}

.w-full { width: 100%; }
.full-width-card { width: 100% !important; max-width: none !important; box-sizing: border-box; }
.max-w-sm { max-width: 400px; }
.max-w-md { max-width: 600px; }

/* 颜色与字体工具类 */
.text-primary { color: var(--el-color-primary); }
.mt-1 { margin-top: 4px; }
.mt-5 { margin-top: 20px; }
.mt-6 { margin-top: 24px; }
.my-2 { margin: 8px 0; }
.mb-8 { margin-bottom: 32px; }
.pl-4 { padding-left: 16px; }
.flex-1 { flex: 1; }

/* Flex 布局 */
.flex-row { display: flex; gap: 8px; align-items: center; }

/* 头部样式 */
.panel-header {
  display: flex; align-items: center; gap: 12px;
  padding-bottom: 12px; border-bottom: 1px solid var(--el-border-color-lighter);
}
.panel-title { font-size: 22px; font-weight: 600; margin: 0; }

/* 卡片样式 */
.setting-card {
  border: 1px solid var(--el-border-color-lighter);
  background-color: var(--el-bg-color-overlay);
  border-radius: 8px;
}
.card-header-title { font-size: 16px; font-weight: 600; }

/* 行级设置项 */
.setting-row {
  display: flex; justify-content: space-between; align-items: center;
}
.setting-info { flex: 1; }
.setting-name { font-size: 15px; font-weight: 500; color: var(--el-text-color-primary); }
.setting-desc { font-size: 12px; color: var(--el-text-color-secondary); margin-top: 4px; line-height: 1.4; }

/* 表单与输入框 */
.input-group { display: flex; flex-direction: column; }
.input-label { font-size: 14px; font-weight: 500; margin-bottom: 8px; color: var(--el-text-color-regular); }
.input-hint { font-size: 12px; color: var(--el-text-color-secondary); margin-top: 6px; line-height: 1.4; }

/* 路径配置网格 */
.path-grid { display: flex; flex-direction: column; gap: 24px; }

/* 两列等宽网格 */
.two-column-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; }
@media (max-width: 900px) {
  .two-column-grid { grid-template-columns: 1fr; }
}
</style>