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
  <div class="settings-panel resource-panel w-full" data-onboarding="settings-resource-panel">
    <div class="panel-header">
      <h2 class="panel-title">{{ tr('settings.resource.title', '资源更新') }}</h2>
    </div>

    <el-card class="setting-card mt-5 full-width-card" shadow="never">
      <template #header>
        <div class="flex-between flex-wrap gap-4">
          <div>
            <div class="card-header-title text-primary">{{ tr('settings.resource.sectionTitle', 'data-linux 资源版本') }}</div>
            <div class="setting-desc mt-1">
              {{ tr('settings.resource.sectionHint', '优先从 GitHub 检查 data-linux（旧名 Data-parameters）版本；GitHub 异常时会自动回退到 Gitee 镜像，并可一键拉取更新。') }}
            </div>
          </div>
          <div class="flex-row w-auto">
            <el-button size="small" @click="checkResourceInfo" :loading="isResourceChecking">
              <i class="el-icon-refresh mr-1" v-if="!isResourceChecking"></i>
              {{ isResourceChecking ? tr('settings.resource.checking', '检查中...') : tr('settings.resource.checkAction', '检查资源版本') }}
            </el-button>
            <el-button type="primary" plain size="small" @click="pullResources" :loading="isResourcePulling">
              <i class="el-icon-download mr-1" v-if="!isResourcePulling"></i>
              {{ isResourcePulling ? tr('settings.resource.pulling', '拉取中...') : tr('settings.resource.pullAction', '拉取资源更新') }}
            </el-button>
          </div>
        </div>
      </template>

      <div v-if="resourceInfo" class="info-list mt-2">
        <div class="info-row">
          <div class="info-label">{{ tr('settings.resource.currentVersion', '本地资源版本') }}</div>
          <div class="info-value font-mono font-bold">{{ resourceInfo.currentVersion }}</div>
        </div>
        
        <div class="info-row">
          <div class="info-label">{{ tr('settings.resource.latestVersion', '远程资源版本') }}</div>
          <div class="info-value font-mono font-bold text-primary">{{ resourceInfo.latestVersion }}</div>
        </div>
        
        <div class="info-row">
          <div class="info-label">{{ tr('settings.resource.status', '更新状态') }}</div>
          <div class="info-value">
            <el-tag v-if="resourceInfo.hasUpdate" type="warning" effect="light">
              {{ tr('settings.resource.hasUpdate', '有可用资源更新') }}
            </el-tag>
            <el-tag v-else type="success" effect="light">
              {{ tr('settings.resource.upToDate', '资源已是最新') }}
            </el-tag>
          </div>
        </div>
        
        <div class="info-row log-row mt-4">
          <div class="info-label mb-2">{{ tr('settings.resource.logLabel', '检查信息') }}</div>
          <pre class="log-content">{{ resourceInfo.updateLog || tr('settings.resource.logEmpty', '暂无检查信息') }}</pre>
        </div>
      </div>

      <div v-else class="empty-hint">
        {{ tr('settings.resource.notLoaded', '尚未获取资源版本信息，请点击上方“检查资源版本”。') }}
      </div>
    </el-card>

  </div>
</template>

<style scoped>
/* 强制覆盖全局限制，确保面板 100% 宽度 */
.resource-panel {
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  color: var(--el-text-color-primary);
  width: 100% !important;
  max-width: none !important;
  flex-grow: 1;
}

.w-full { width: 100%; }
.w-auto { width: auto; }
.full-width-card { 
  width: 100% !important; 
  max-width: none !important;
  box-sizing: border-box; 
}

/* 颜色与字体工具类 */
.text-primary { color: var(--el-color-primary); }
.font-mono { font-family: monospace; }
.font-bold { font-weight: 600; }

/* 间距工具类 */
.mt-1 { margin-top: 4px; }
.mt-2 { margin-top: 8px; }
.mt-4 { margin-top: 16px; }
.mt-5 { margin-top: 20px; }
.mb-2 { margin-bottom: 8px; }
.mr-1 { margin-right: 4px; }
.gap-4 { gap: 16px; }

/* Flex 布局 */
.flex-row { display: flex; gap: 8px; align-items: center; }
.flex-between { display: flex; justify-content: space-between; align-items: center; }
.flex-wrap { flex-wrap: wrap; }

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
.setting-desc { font-size: 13px; color: var(--el-text-color-secondary); margin: 0; }

/* 列表展示信息 */
.info-list {
  display: flex;
  flex-direction: column;
}
.info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 0;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.info-row:first-child {
  padding-top: 8px;
}
.info-row:last-child {
  border-bottom: none;
  padding-bottom: 0;
}
.info-label {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-regular);
}
.info-value {
  font-size: 15px;
  color: var(--el-text-color-primary);
}

/* 日志框独占一行 */
.log-row {
  flex-direction: column;
  align-items: stretch;
}
.log-content {
  background-color: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  padding: 12px 16px;
  margin: 0;
  font-family: monospace;
  font-size: 13px;
  color: var(--el-text-color-regular);
  line-height: 1.5;
  white-space: pre-wrap; /* 允许日志自动换行 */
  word-break: break-all;
  max-height: 300px; /* 防止日志过长撑爆页面 */
  overflow-y: auto;
}

/* 自定义滚动条 */
.log-content::-webkit-scrollbar {
  width: 6px;
}
.log-content::-webkit-scrollbar-thumb {
  background: var(--el-border-color-darker);
  border-radius: 4px;
}

/* 空状态 */
.empty-hint {
  text-align: center; padding: 40px 20px;
  color: var(--el-text-color-secondary); font-size: 13px;
  border: 2px dashed var(--el-border-color-lighter);
  border-radius: 8px; background-color: var(--el-bg-color);
}
</style>