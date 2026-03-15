<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { AppSettings } from '../../../types/ipc';

defineProps<{
  appSettings: AppSettings;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="settings-panel" data-onboarding="settings-display-panel">
    <div class="panel-title">{{ t('settings.page_display') }}</div>
    <el-form label-width="140px">
      <el-form-item :label="t('settings.websitepage')">
        <el-switch v-model="appSettings.showWebsites" />
      </el-form-item>
      <el-form-item :label="t('settings.docpage')">
        <el-switch v-model="appSettings.showDocuments" />
      </el-form-item>
    </el-form>
  </div>
</template>
