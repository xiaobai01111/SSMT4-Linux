<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { VersionCheckInfo } from '../../../api';

defineProps<{
  versionInfo: VersionCheckInfo | null;
  isVersionChecking: boolean;
  checkVersionInfo: () => void | Promise<void>;
}>();

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => {
  return te(key) ? String(t(key)) : fallback;
};
</script>

<template>
  <div class="settings-panel version-panel" data-onboarding="settings-version-panel">
    <div class="panel-title">{{ tr('settings.version_check_title', '版本检查') }}</div>

    <div class="section-block">
      <div class="section-header">
        <div>
          <div class="section-title">{{ tr('settings.version_check_overview', '当前版本信息') }}</div>
          <div class="section-hint">
            {{ tr('settings.version_check_hint', '优先从 GitHub 检查主程序 version 和 version-log；GitHub 异常时会自动回退到 Gitee，最后再回退到本地打包文件。') }}
          </div>
        </div>
        <div class="toolbar-actions">
          <el-button type="primary" size="small" @click="checkVersionInfo" :loading="isVersionChecking">
            {{ isVersionChecking ? tr('settings.version_checking', '检查中...') : tr('settings.version_check_action', '检查更新') }}
          </el-button>
        </div>
      </div>

      <div v-if="versionInfo" class="version-grid">
        <div class="version-row">
          <div class="version-label">{{ tr('settings.version_current', '当前版本') }}</div>
          <div class="version-value">{{ versionInfo.currentVersion }}</div>
        </div>
        <div class="version-row">
          <div class="version-label">{{ tr('settings.version_latest', '最新版本') }}</div>
          <div class="version-value">{{ versionInfo.latestVersion }}</div>
        </div>
        <div class="version-row">
          <div class="version-label">{{ tr('settings.version_status', '更新状态') }}</div>
          <div class="version-value">
            <el-tag v-if="versionInfo.hasUpdate" type="warning">{{ tr('settings.version_has_update', '有可用更新') }}</el-tag>
            <el-tag v-else type="success">{{ tr('settings.version_up_to_date', '已是最新') }}</el-tag>
          </div>
        </div>
        <div class="version-row version-log-row">
          <div class="version-label">{{ tr('settings.version_log', '更新日志') }}</div>
          <div class="version-value">
            <pre class="version-log-content">{{ versionInfo.updateLog || tr('settings.version_log_empty', '暂无更新日志') }}</pre>
          </div>
        </div>
      </div>

      <div v-else class="row-sub">
        {{ tr('settings.version_not_loaded', '尚未获取版本信息，请点击“检查更新”。') }}
      </div>
    </div>
  </div>
</template>
