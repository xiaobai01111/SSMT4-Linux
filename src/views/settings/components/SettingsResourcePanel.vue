<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { VersionCheckInfo } from '../../../api';

defineProps<{
  resourceInfo: VersionCheckInfo | null;
  isResourceChecking: boolean;
  isResourcePulling: boolean;
  checkResourceInfo: () => void | Promise<void>;
  pullResources: () => void | Promise<void>;
}>();

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => {
  return te(key) ? String(t(key)) : fallback;
};
</script>

<template>
  <div class="settings-panel version-panel" data-onboarding="settings-resource-panel">
    <div class="panel-title">{{ tr('settings.resource.title', '资源更新') }}</div>

    <div class="section-block">
      <div class="section-header">
        <div>
          <div class="section-title">{{ tr('settings.resource.sectionTitle', 'data-linux 资源版本') }}</div>
          <div class="section-hint">
            {{ tr('settings.resource.sectionHint', '优先从 GitHub 检查 data-linux（旧名 Data-parameters）版本；GitHub 异常时会自动回退到 Gitee 镜像，并可一键拉取更新。') }}
          </div>
        </div>
        <div class="toolbar-actions">
          <el-button size="small" @click="checkResourceInfo" :loading="isResourceChecking">
            {{ isResourceChecking ? tr('settings.resource.checking', '检查中...') : tr('settings.resource.checkAction', '检查资源版本') }}
          </el-button>
          <el-button type="primary" size="small" @click="pullResources" :loading="isResourcePulling">
            {{ isResourcePulling ? tr('settings.resource.pulling', '拉取中...') : tr('settings.resource.pullAction', '拉取资源更新') }}
          </el-button>
        </div>
      </div>

      <div v-if="resourceInfo" class="version-grid">
        <div class="version-row">
          <div class="version-label">{{ tr('settings.resource.currentVersion', '本地资源版本') }}</div>
          <div class="version-value">{{ resourceInfo.currentVersion }}</div>
        </div>
        <div class="version-row">
          <div class="version-label">{{ tr('settings.resource.latestVersion', '远程资源版本') }}</div>
          <div class="version-value">{{ resourceInfo.latestVersion }}</div>
        </div>
        <div class="version-row">
          <div class="version-label">{{ tr('settings.resource.status', '更新状态') }}</div>
          <div class="version-value">
            <el-tag v-if="resourceInfo.hasUpdate" type="warning">{{ tr('settings.resource.hasUpdate', '有可用资源更新') }}</el-tag>
            <el-tag v-else type="success">{{ tr('settings.resource.upToDate', '资源已是最新') }}</el-tag>
          </div>
        </div>
        <div class="version-row version-log-row">
          <div class="version-label">{{ tr('settings.resource.logLabel', '检查信息') }}</div>
          <div class="version-value">
            <pre class="version-log-content">{{ resourceInfo.updateLog || tr('settings.resource.logEmpty', '暂无检查信息') }}</pre>
          </div>
        </div>
      </div>

      <div v-else class="row-sub">
        {{ tr('settings.resource.notLoaded', '尚未获取资源版本信息，请点击“检查资源版本”。') }}
      </div>
    </div>
  </div>
</template>
