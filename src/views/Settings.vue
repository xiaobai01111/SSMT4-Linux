<script setup lang="ts">
import { appSettings } from '../store'
import { openFileDialog } from '../api';
import { useI18n } from 'vue-i18n';

const { t } = useI18n()

const selectCacheDir = async () => {
  const selected = await openFileDialog({
    directory: true,
    multiple: false,
    title: t('settings.selectcachedir')
  });

  if (selected && typeof selected === 'string') {
    appSettings.cacheDir = selected;
  }
};

const selectDataDir = async () => {
  const selected = await openFileDialog({
    directory: true,
    multiple: false,
    title: t('settings.selectdatadir')
  });

  if (selected && typeof selected === 'string') {
    appSettings.dataDir = selected;
  }
};
</script>

<template>
  <div class="page-container" style="padding: 24px 24px 56px 24px;">

    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ t('settings.basicsettings') }}</span>
        </div>
      </template>
      <el-form label-width="140px">
        <el-form-item :label="t('settings.language')">
          <el-select v-model="appSettings.locale" placeholder="Select language" style="width: 200px">
            <el-option label="简体中文" value="zhs" />
            <el-option label="繁體中文" value="zht" />
            <el-option label="English" value="en" />
          </el-select>
        </el-form-item>
        <el-form-item :label="t('settings.datadir')">
          <div style="display: flex; gap: 10px; width: 100%;">
            <el-input v-model="appSettings.dataDir" :placeholder="t('settings.datadir_placeholder')" />
            <el-button @click="selectDataDir">{{ t('settings.selectfolder') }}</el-button>
          </div>
          <div style="font-size: 12px; color: rgba(255,255,255,0.4); margin-top: 4px; line-height: 1.5;">
            {{ t('settings.datadir_hint') }}
          </div>
        </el-form-item>
        <el-form-item :label="t('settings.cachedir')">
          <div style="display: flex; gap: 10px; width: 100%;">
            <el-input v-model="appSettings.cacheDir" :placeholder="t('settings.cachedir_placeholder')" />
            <el-button @click="selectCacheDir">{{ t('settings.selectfolder') }}</el-button>
          </div>
        </el-form-item>
        <el-form-item :label="t('settings.github_token')">
          <el-input v-model="appSettings.githubToken" :placeholder="t('settings.github_token_placeholder')" type="password"
            show-password />
        </el-form-item>
      </el-form>
    </el-card>

    <br />

    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ t('settings.appearance') }}</span>
        </div>
      </template>

      <el-form label-width="140px">
        <div class="settings-divider">{{ t('settings.content_style') }}</div>
        <el-form-item :label="t('settings.opacity')">
          <el-slider v-model="appSettings.contentOpacity" :min="0" :max="1" :step="0.01" show-input />
        </el-form-item>
        <el-form-item :label="t('settings.blur')">
          <el-slider v-model="appSettings.contentBlur" :min="0" :max="50" :step="1" show-input />
        </el-form-item>
      </el-form>
    </el-card>

    <br />

    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ t('settings.page_display') }}</span>
        </div>
      </template>
      <el-form label-width="140px">
        <el-form-item :label="t('settings.modpage')">
          <el-switch v-model="appSettings.showMods" />
        </el-form-item>
        <el-form-item :label="t('settings.websitepage')">
          <el-switch v-model="appSettings.showWebsites" />
        </el-form-item>
        <el-form-item :label="t('settings.docpage')">
          <el-switch v-model="appSettings.showDocuments" />
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>

<style scoped>
.settings-divider {
  display: flex;
  align-items: center;
  margin: 25px 0 15px 0;
  color: #e0e0e0;
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.settings-divider::after {
  content: '';
  flex: 1;
  height: 1px;
  background: linear-gradient(to right, rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.05));
  margin-left: 15px;
}
</style>
