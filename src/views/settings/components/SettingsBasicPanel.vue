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
  <div class="settings-panel" data-onboarding="settings-basic-panel">
    <div class="panel-title">{{ t('settings.basicsettings') }}</div>
    <el-form label-width="140px">
      <el-form-item :label="t('settings.language')">
        <el-select
          :model-value="appSettings.locale"
          :placeholder="tr('settings.language_select_placeholder', 'Select language')"
          style="width: 200px"
          @update:model-value="handleLocaleChange"
        >
          <el-option :label="tr('settings.language_options.zhs', '简体中文')" value="zhs" />
          <el-option :label="tr('settings.language_options.zht', '繁體中文')" value="zht" />
          <el-option :label="tr('settings.language_options.en', 'English')" value="en" />
        </el-select>
      </el-form-item>
      <el-form-item :label="t('settings.datadir')">
        <div class="form-item-vertical">
          <div style="display: flex; gap: 10px; width: 100%;">
            <el-input v-model="appSettings.dataDir" :placeholder="t('settings.datadir_placeholder')" />
            <el-button @click="selectDataDir">{{ t('settings.selectfolder') }}</el-button>
          </div>
          <div class="form-item-hint">
            {{ t('settings.datadir_hint') }}
          </div>
        </div>
      </el-form-item>
      <el-form-item :label="t('settings.cachedir')">
        <div style="display: flex; gap: 10px; width: 100%;">
          <el-input v-model="appSettings.cacheDir" :placeholder="t('settings.cachedir_placeholder')" />
          <el-button @click="selectCacheDir">{{ t('settings.selectfolder') }}</el-button>
        </div>
      </el-form-item>
      <el-form-item :label="t('settings.github_token')">
        <el-input v-model="appSettings.githubToken" :placeholder="t('settings.github_token_placeholder')" type="password" show-password />
      </el-form-item>
      <el-form-item :label="tr('settings.snowbreakSourcePolicy.label', '尘白下载源策略')">
        <div class="form-item-vertical">
          <el-select v-model="appSettings.snowbreakSourcePolicy" style="width: 260px">
            <el-option :label="tr('settings.snowbreakSourcePolicy.officialFirst', '官方优先（失败后回退社区）')" value="official_first" />
            <el-option :label="tr('settings.snowbreakSourcePolicy.communityFirst', '社区优先（失败后回退官方）')" value="community_first" />
          </el-select>
          <div class="form-item-hint">
            {{ tr('settings.snowbreakSourcePolicy.hint', '推荐保持\"官方优先\"，网络异常时会自动回退到另一来源。') }}
          </div>
        </div>
      </el-form-item>
      <el-form-item :label="tr('settings.logviewer.label', '日志查看器')">
        <div class="form-item-vertical">
          <el-button @click="openLogWindow">{{ tr('settings.logviewer.open', '打开日志窗口') }}</el-button>
          <div class="form-item-hint">
            {{ tr('settings.logviewer.hint', '在新窗口中查看软件运行日志，便于排查问题时提供给开发者。') }}
          </div>
        </div>
      </el-form-item>
      <el-form-item :label="tr('settings.onboarding.label', '新手引导')">
        <div class="form-item-vertical">
          <el-button @click="reenterOnboarding">{{ tr('settings.onboarding.reenter', '重新进入新手引导') }}</el-button>
          <div class="form-item-hint">
            {{ tr('settings.onboarding.hint', '仅重新展示功能导览（主页、游戏库、运行环境等），不会重置初始化设置。') }}
          </div>
        </div>
      </el-form-item>
    </el-form>
  </div>
</template>
