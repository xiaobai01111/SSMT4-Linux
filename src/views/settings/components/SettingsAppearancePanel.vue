<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { AppSettings } from '../../../types/ipc';

defineProps<{
  appSettings: AppSettings;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="settings-panel">
    <div class="panel-title">{{ t('settings.appearance') }}</div>
    <el-form label-width="140px">
      <div class="settings-divider">{{ t('settings.content_style') }}</div>
      <el-form-item :label="t('settings.opacity')">
        <el-slider v-model="appSettings.contentOpacity" :min="0" :max="1" :step="0.01" show-input />
      </el-form-item>
      <el-form-item :label="t('settings.blur')">
        <el-slider v-model="appSettings.contentBlur" :min="0" :max="50" :step="1" show-input />
      </el-form-item>
    </el-form>
  </div>
</template>
