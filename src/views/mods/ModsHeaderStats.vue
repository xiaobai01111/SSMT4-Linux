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
  <div>
    <div class="mods-header">
      <div>
        <div class="title-row">
          <h1 class="title">{{ tr('mods.title', 'Mod 管理') }}</h1>
          <el-tag type="danger" effect="dark" size="small">{{ tr('mods.experimental', '实验性') }}</el-tag>
        </div>
        <p class="desc">
          {{ tr('mods.descAdvanced', '按游戏集中管理 3DMigoto Mod。你可以选择游戏、查看有效目录、单个或批量启用/禁用 Mod，并快速打开 Mod / ShaderFixes 目录。') }}
        </p>
      </div>
      <div class="header-actions">
        <el-button @click="$emit('open-settings')">{{ tr('mods.gotoSettings', '前往 3DMIGOTO 管理') }}</el-button>
        <el-button type="primary" :loading="isLoadingGames" @click="$emit('refresh')">
          {{ isLoadingGames ? tr('mods.refreshing', '刷新中...') : tr('mods.refresh', '刷新') }}
        </el-button>
      </div>
    </div>

    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statGames', '游戏总数') }}</div>
        <div class="stat-value">{{ totalGameCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statEnabledGames', '已启用 3DMigoto 的游戏') }}</div>
        <div class="stat-value">{{ enabledGameCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statEnabledMods', '当前游戏已加载 Mod') }}</div>
        <div class="stat-value">{{ selectedEnabledModCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statDisabledMods', '当前游戏已禁用 Mod') }}</div>
        <div class="stat-value">{{ selectedDisabledModCount }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mods-header {
  display: flex;
  justify-content: space-between;
  gap: 20px;
  align-items: flex-start;
  margin-bottom: 28px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
}

.title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.title {
  margin: 0;
  font-size: 28px;
  font-weight: 700;
  color: #00f0ff;
  letter-spacing: 1px;
  text-transform: uppercase;
}

.desc {
  margin: 10px 0 0 0;
  color: rgba(255, 255, 255, 0.65);
  max-width: 840px;
  line-height: 1.7;
}

.header-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
  margin-bottom: 18px;
}

.stat-card {
  border: 1px solid rgba(0, 240, 255, 0.18);
  background: rgba(0, 8, 14, 0.55);
  border-radius: 8px;
  padding: 16px 18px;
}

.stat-label {
  color: rgba(255, 255, 255, 0.58);
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.6px;
}

.stat-value {
  margin-top: 8px;
  color: #fff;
  font-size: 24px;
  font-weight: 700;
}

@media (max-width: 1180px) {
  .stats-grid {
    grid-template-columns: 1fr 1fr;
  }
}

@media (max-width: 900px) {
  .mods-header {
    flex-direction: column;
    align-items: stretch;
  }

  .stats-grid {
    grid-template-columns: 1fr;
  }
}
</style>
