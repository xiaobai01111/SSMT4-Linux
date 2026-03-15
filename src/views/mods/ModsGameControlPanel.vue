<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { GameModDirectoryState } from '../../api';
import type { ModGameSummary } from './types';

const props = defineProps<{
  gameKeyword: string;
  selectedGameName: string;
  filteredGameOptions: ModGameSummary[];
  selectedGameSummary: ModGameSummary | null;
  selectedState: GameModDirectoryState | null;
  isSavingGameToggle: boolean;
  isBulkOperating: boolean;
}>();

const emit = defineEmits<{
  (event: 'update:gameKeyword', value: string): void;
  (event: 'update:selectedGameName', value: string): void;
  (event: 'toggle-game-migoto', value: string | number | boolean): void;
  (event: 'open-selected-mods-folder'): void;
  (event: 'open-selected-shader-fixes-folder'): void;
  (event: 'toggle-all', enabled: boolean): void;
}>();

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);

const gameKeywordModel = computed({
  get: () => props.gameKeyword,
  set: (value: string) => emit('update:gameKeyword', value),
});

const selectedGameNameModel = computed({
  get: () => props.selectedGameName,
  set: (value: string) => emit('update:selectedGameName', value),
});

const selectedEntryCount = computed(() => props.selectedState?.entries?.length || 0);
</script>

<template>
  <div class="control-panel">
    <div class="panel-row">
      <el-input
        v-model="gameKeywordModel"
        class="game-search"
        clearable
        :placeholder="tr('mods.searchGame', '搜索游戏...')"
      />
      <el-select
        v-model="selectedGameNameModel"
        class="game-select"
        filterable
        :placeholder="tr('mods.selectGame', '选择一个游戏')"
      >
        <el-option
          v-for="entry in filteredGameOptions"
          :key="entry.name"
          :label="entry.displayName"
          :value="entry.name"
        />
      </el-select>

      <template v-if="selectedGameSummary">
        <div class="game-toggle-wrap">
          <span class="game-toggle-label">{{ tr('mods.gameToggle', '本游戏启用 3DMigoto') }}</span>
          <el-switch
            :model-value="selectedGameSummary.migotoEnabled"
            :loading="isSavingGameToggle"
            :disabled="isSavingGameToggle || !selectedGameSummary.migotoSupported"
            @update:model-value="emit('toggle-game-migoto', $event)"
          />
        </div>
        <el-tag v-if="selectedGameSummary.migotoSupported" type="info">
          {{ selectedGameSummary.importer }}
        </el-tag>
        <el-tag v-else type="warning">
          {{ tr('mods.unsupportedStatus', '暂不支持') }}
        </el-tag>
      </template>
    </div>

    <div v-if="selectedGameSummary" class="selected-game-card">
      <div class="selected-game-head">
        <div class="game-meta">
          <img v-if="selectedGameSummary.iconPath" :src="selectedGameSummary.iconPath" class="game-icon" alt="" />
          <div>
            <div class="game-title">{{ selectedGameSummary.displayName }}</div>
            <div class="game-sub">{{ selectedGameSummary.name }}</div>
          </div>
        </div>
        <div class="game-tags">
          <el-tag size="small" :type="selectedGameSummary.migotoEnabled ? 'success' : 'warning'">
            {{ selectedGameSummary.migotoEnabled ? tr('mods.enabled', '已启用') : tr('mods.disabled', '未启用') }}
          </el-tag>
          <el-tag size="small" type="danger">{{ tr('mods.experimental', '实验性') }}</el-tag>
        </div>
      </div>

      <div v-if="selectedGameSummary.loadError" class="card-error">
        {{ tr('mods.loadError', '读取配置失败') }}: {{ selectedGameSummary.loadError }}
      </div>

      <div v-if="!selectedGameSummary.migotoSupported" class="card-error">
        {{ tr('mods.unsupportedGame', '当前游戏暂不支持 3DMigoto / Mod 管理，因此无法开启。') }}
      </div>

      <div class="path-grid">
        <div class="path-block">
          <div class="path-label">{{ tr('mods.modFolder', 'Mod 目录') }}</div>
          <div class="path-value">{{ selectedGameSummary.modFolder || '-' }}</div>
          <div class="path-meta">
            {{ selectedGameSummary.modFolderExists ? tr('mods.folderExists', '目录已存在') : tr('mods.folderMissing', '目录不存在，打开时会自动创建') }}
          </div>
        </div>
        <div class="path-block">
          <div class="path-label">{{ tr('mods.shaderFolder', 'ShaderFixes 目录') }}</div>
          <div class="path-value">{{ selectedGameSummary.shaderFixesFolder || '-' }}</div>
          <div class="path-meta">
            {{ selectedGameSummary.shaderFixesFolderExists ? tr('mods.folderExists', '目录已存在') : tr('mods.folderMissing', '目录不存在，打开时会自动创建') }}
          </div>
        </div>
      </div>

      <div class="selected-actions">
        <el-button
          type="primary"
          :disabled="!selectedGameSummary.migotoSupported"
          @click="emit('open-selected-mods-folder')"
        >
          {{ tr('mods.openMods', '打开 Mod 目录') }}
        </el-button>
        <el-button
          :disabled="!selectedGameSummary.migotoSupported"
          @click="emit('open-selected-shader-fixes-folder')"
        >
          {{ tr('mods.openShaderFixes', '打开 ShaderFixes') }}
        </el-button>
        <el-button
          type="success"
          plain
          :disabled="isBulkOperating || !selectedGameSummary.migotoSupported || !selectedEntryCount"
          @click="emit('toggle-all', true)"
        >
          {{ tr('mods.enableAll', '全部启用') }}
        </el-button>
        <el-button
          type="warning"
          plain
          :disabled="isBulkOperating || !selectedGameSummary.migotoSupported || !selectedEntryCount"
          @click="emit('toggle-all', false)"
        >
          {{ tr('mods.disableAll', '全部禁用') }}
        </el-button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.control-panel {
  border: 1px solid rgba(0, 240, 255, 0.18);
  background: rgba(0, 8, 14, 0.55);
  border-radius: 10px;
  padding: 18px;
  margin-bottom: 18px;
}

.panel-row {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
}

.game-search {
  max-width: 220px;
}

.game-select {
  min-width: 320px;
  max-width: 420px;
}

.game-toggle-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
}

.game-toggle-label {
  color: rgba(255, 255, 255, 0.72);
  font-size: 13px;
  font-weight: 600;
}

.selected-game-card {
  margin-top: 18px;
  padding-top: 18px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.selected-game-head {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
}

.game-meta {
  display: flex;
  align-items: center;
  gap: 12px;
}

.game-icon {
  width: 44px;
  height: 44px;
  border-radius: 8px;
  object-fit: cover;
  border: 1px solid rgba(255, 255, 255, 0.12);
}

.game-title {
  color: #fff;
  font-size: 18px;
  font-weight: 700;
}

.game-sub {
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
  margin-top: 4px;
  word-break: break-all;
}

.game-tags {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.card-error {
  margin-top: 14px;
  color: #f56c6c;
  font-size: 13px;
  line-height: 1.6;
}

.path-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
  margin-top: 16px;
}

.path-block {
  padding: 14px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(255, 255, 255, 0.03);
}

.path-label {
  color: rgba(255, 255, 255, 0.72);
  font-size: 13px;
  font-weight: 600;
}

.path-value {
  margin-top: 8px;
  font-family: monospace;
  font-size: 13px;
  color: #d7f9ff;
  line-height: 1.6;
  word-break: break-all;
}

.path-meta {
  margin-top: 8px;
  color: rgba(255, 255, 255, 0.48);
  font-size: 12px;
}

.selected-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 16px;
}

:deep(.game-search .el-input__wrapper),
:deep(.game-select .el-input__wrapper) {
  background-color: rgba(10, 15, 20, 0.6) !important;
  border: 1px solid rgba(0, 240, 255, 0.3) !important;
  box-shadow: none !important;
}

@media (max-width: 1180px) {
  .path-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 900px) {
  .panel-row {
    align-items: stretch;
  }

  .game-search,
  .game-select {
    max-width: none;
    min-width: 0;
    width: 100%;
  }
}
</style>
