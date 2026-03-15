<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{
  isLoadingGames: boolean;
  totalGameCount: number;
  enabledGameCount: number;
  selectedEnabledModCount: number;
  selectedDisabledModCount: number;
}>();

defineEmits<{
  (event: 'refresh'): void;
  (event: 'open-settings'): void;
}>();

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);
</script>

<template>
  <div class="mods-manage-header w-full">
    <div class="header-main flex-between flex-wrap gap-4">
      <div>
        <div class="flex-row align-center">
          <h1 class="panel-title">{{ tr('mods.title', 'Mod 管理') }}</h1>
          <el-tag type="danger" effect="dark" size="small" class="ml-3">{{ tr('mods.experimental', '实验性') }}</el-tag>
        </div>
        <p class="setting-desc mt-2">
          {{ tr('mods.descAdvanced', '按游戏集中管理 3DMigoto Mod。你可以选择游戏、查看有效目录、单个或批量启用/禁用 Mod，并快速打开 Mod / ShaderFixes 目录。') }}
        </p>
      </div>
      <div class="header-actions flex-row w-auto">
        <el-button @click="$emit('open-settings')">
          <i class="el-icon-setting mr-1"></i> {{ tr('mods.gotoSettings', '前往 3DMIGOTO 管理') }}
        </el-button>
        <el-button type="primary" plain :loading="isLoadingGames" @click="$emit('refresh')">
          <i class="el-icon-refresh mr-1" v-if="!isLoadingGames"></i>
          {{ isLoadingGames ? tr('mods.refreshing', '刷新中...') : tr('mods.refresh', '刷新') }}
        </el-button>
      </div>
    </div>

    <div class="stats-grid mt-6 mb-6">
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statGames', '游戏总数') }}</div>
        <div class="stat-value">{{ totalGameCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statEnabledGames', '已启用 3DMigoto 的游戏') }}</div>
        <div class="stat-value text-primary">{{ enabledGameCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statEnabledMods', '当前游戏已加载 Mod') }}</div>
        <div class="stat-value text-success">{{ selectedEnabledModCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statDisabledMods', '当前游戏已禁用 Mod') }}</div>
        <div class="stat-value text-regular">{{ selectedDisabledModCount }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 全局覆盖 */
.mods-manage-header {
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  color: var(--el-text-color-primary);
}

.w-full { width: 100%; }
.w-auto { width: auto; }

/* 颜色工具类 */
.text-primary { color: var(--el-color-primary); }
.text-success { color: var(--el-color-success); }
.text-regular { color: var(--el-text-color-regular); }

/* 间距工具类 */
.mt-2 { margin-top: 8px; }
.mt-6 { margin-top: 24px; }
.mb-6 { margin-bottom: 24px; }
.ml-3 { margin-left: 12px; }
.mr-1 { margin-right: 4px; }
.gap-4 { gap: 16px; }

/* 弹性布局工具类 */
.flex-row { display: flex; gap: 8px; align-items: center; }
.flex-between { display: flex; justify-content: space-between; align-items: flex-start; }
.flex-wrap { flex-wrap: wrap; }
.align-center { align-items: center; }

/* 头部文本 */
.header-main {
  padding-bottom: 16px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.panel-title {
  font-size: 24px;
  font-weight: 600;
  margin: 0;
}
.setting-desc {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  max-width: 840px;
  line-height: 1.5;
  margin-bottom: 0;
}

/* 统计卡片网格 */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
}

/* 美化后的数据卡片（统一主题） */
.stat-card {
  border: 1px solid var(--el-border-color-lighter);
  background-color: var(--el-bg-color-overlay);
  border-radius: 8px;
  padding: 16px 20px;
  transition: border-color 0.2s, transform 0.2s;
}
.stat-card:hover {
  border-color: var(--el-color-primary-light-5);
  transform: translateY(-2px);
}

.stat-label {
  color: var(--el-text-color-secondary);
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.stat-value {
  margin-top: 8px;
  font-size: 26px;
  font-weight: 700;
  font-family: monospace;
}

/* 响应式调整 */
@media (max-width: 1180px) {
  .stats-grid { grid-template-columns: 1fr 1fr; gap: 12px; }
}

@media (max-width: 768px) {
  .header-main { flex-direction: column; align-items: stretch; }
  .header-actions { margin-top: 8px; }
  .stats-grid { grid-template-columns: 1fr; }
}
</style>