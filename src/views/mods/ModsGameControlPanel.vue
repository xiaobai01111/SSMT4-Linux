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
  <div class="mod-control-card w-full">
    
    <div class="panel-row">
      <el-input
        v-model="gameKeywordModel"
        class="game-search"
        clearable
        :placeholder="tr('mods.searchGame', '搜索游戏...')"
      >
        <template #prefix>
          <i class="el-icon-search"></i>
        </template>
      </el-input>
      
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
        <el-tag v-if="selectedGameSummary.migotoSupported" type="info" effect="plain" class="ml-2">
          {{ selectedGameSummary.importer }}
        </el-tag>
        <el-tag v-else type="warning" effect="plain" class="ml-2">
          {{ tr('mods.unsupportedStatus', '暂不支持') }}
        </el-tag>
      </template>
    </div>

    <div v-if="selectedGameSummary" class="selected-game-section">
      
      <div class="selected-game-head flex-between">
        <div class="game-meta flex-row align-center">
          <img v-if="selectedGameSummary.iconPath" :src="selectedGameSummary.iconPath" class="game-icon" alt="" />
          <div v-else class="game-icon-placeholder"><i class="el-icon-picture-outline"></i></div>
          <div class="game-info-text">
            <div class="game-title">{{ selectedGameSummary.displayName }}</div>
            <div class="game-sub">{{ selectedGameSummary.name }}</div>
          </div>
        </div>
        <div class="game-tags">
          <el-tag size="small" :type="selectedGameSummary.migotoEnabled ? 'success' : 'warning'" effect="light">
            {{ selectedGameSummary.migotoEnabled ? tr('mods.enabled', '已启用') : tr('mods.disabled', '未启用') }}
          </el-tag>
          <el-tag size="small" type="danger" effect="plain">{{ tr('mods.experimental', '实验性') }}</el-tag>
        </div>
      </div>

      <el-alert v-if="selectedGameSummary.loadError" type="error" show-icon :closable="false" class="mt-4 custom-alert">
        {{ tr('mods.loadError', '读取配置失败') }}: {{ selectedGameSummary.loadError }}
      </el-alert>

      <el-alert v-if="!selectedGameSummary.migotoSupported" type="warning" show-icon :closable="false" class="mt-4 custom-alert">
        {{ tr('mods.unsupportedGame', '当前游戏暂不支持 3DMigoto / Mod 管理，因此无法开启。') }}
      </el-alert>

      <div class="path-grid">
        <div class="path-block">
          <div class="path-label">{{ tr('mods.modFolder', 'Mod 目录') }}</div>
          <div class="path-value">{{ selectedGameSummary.modFolder || '-' }}</div>
          <div class="path-meta">
            <span :class="selectedGameSummary.modFolderExists ? 'text-success' : 'text-warning'">
              <i :class="selectedGameSummary.modFolderExists ? 'el-icon-circle-check' : 'el-icon-warning-outline'"></i>
              {{ selectedGameSummary.modFolderExists ? tr('mods.folderExists', '目录已存在') : tr('mods.folderMissing', '目录不存在，打开时会自动创建') }}
            </span>
          </div>
        </div>
        
        <div class="path-block">
          <div class="path-label">{{ tr('mods.shaderFolder', 'ShaderFixes 目录') }}</div>
          <div class="path-value">{{ selectedGameSummary.shaderFixesFolder || '-' }}</div>
          <div class="path-meta">
            <span :class="selectedGameSummary.shaderFixesFolderExists ? 'text-success' : 'text-warning'">
              <i :class="selectedGameSummary.shaderFixesFolderExists ? 'el-icon-circle-check' : 'el-icon-warning-outline'"></i>
              {{ selectedGameSummary.shaderFixesFolderExists ? tr('mods.folderExists', '目录已存在') : tr('mods.folderMissing', '目录不存在，打开时会自动创建') }}
            </span>
          </div>
        </div>
      </div>

      <div class="selected-actions">
        <el-button
          type="primary"
          :disabled="!selectedGameSummary.migotoSupported"
          @click="emit('open-selected-mods-folder')"
        >
          <i class="el-icon-folder-opened mr-1"></i> {{ tr('mods.openMods', '打开 Mod 目录') }}
        </el-button>
        <el-button
          :disabled="!selectedGameSummary.migotoSupported"
          @click="emit('open-selected-shader-fixes-folder')"
        >
          <i class="el-icon-folder-opened mr-1"></i> {{ tr('mods.openShaderFixes', '打开 ShaderFixes') }}
        </el-button>
        <div class="spacer"></div>
        <el-button
          type="success"
          plain
          :disabled="isBulkOperating || !selectedGameSummary.migotoSupported || !selectedEntryCount"
          @click="emit('toggle-all', true)"
        >
          <i class="el-icon-check mr-1"></i> {{ tr('mods.enableAll', '全部启用') }}
        </el-button>
        <el-button
          type="warning"
          plain
          :disabled="isBulkOperating || !selectedGameSummary.migotoSupported || !selectedEntryCount"
          @click="emit('toggle-all', false)"
        >
          <i class="el-icon-close mr-1"></i> {{ tr('mods.disableAll', '全部禁用') }}
        </el-button>
      </div>
      
    </div>
  </div>
</template>

<style scoped>
/* 主卡片容器 */
.mod-control-card {
  border: 1px solid var(--el-border-color-lighter);
  background-color: var(--el-bg-color-overlay);
  border-radius: 8px;
  padding: 20px;
  margin-bottom: 20px;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
}
.w-full {
  width: 100%;
}

/* 顶部操作栏 */
.panel-row {
  display: flex;
  gap: 16px;
  align-items: center;
  flex-wrap: wrap;
}

.game-search {
  width: 240px;
}

.game-select {
  flex: 1;
  min-width: 240px;
  max-width: 400px;
}

.game-toggle-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-left: auto; /* 将开关推向右侧 */
}

.game-toggle-label {
  color: var(--el-text-color-regular);
  font-size: 14px;
  font-weight: 500;
}

/* 分割线与选中详情区 */
.selected-game-section {
  margin-top: 20px;
  padding-top: 20px;
  border-top: 1px solid var(--el-border-color-lighter);
}

/* 游戏头部信息 */
.selected-game-head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
  flex-wrap: wrap;
}

.game-meta {
  display: flex;
  align-items: center;
  gap: 16px;
}

.game-icon {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  object-fit: cover;
  border: 1px solid var(--el-border-color-lighter);
}

.game-icon-placeholder {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  background-color: var(--el-fill-color-light);
  border: 1px dashed var(--el-border-color);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  color: var(--el-text-color-placeholder);
}

.game-info-text {
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.game-title {
  color: var(--el-text-color-primary);
  font-size: 18px;
  font-weight: 600;
}

.game-sub {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  margin-top: 4px;
  font-family: monospace;
  word-break: break-all;
}

.game-tags {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

/* 自定义警告框 */
.custom-alert {
  background-color: var(--el-color-danger-light-9);
  border: 1px solid var(--el-color-danger-light-7);
}

/* 路径网格展示 */
.path-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  margin-top: 20px;
}

.path-block {
  padding: 16px;
  border-radius: 8px;
  border: 1px solid var(--el-border-color-lighter);
  background-color: var(--el-fill-color-light);
  display: flex;
  flex-direction: column;
}

.path-label {
  color: var(--el-text-color-regular);
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
}

.path-value {
  font-family: monospace;
  font-size: 13px;
  color: var(--el-text-color-primary);
  line-height: 1.5;
  word-break: break-all;
  background-color: var(--el-bg-color);
  padding: 8px 10px;
  border-radius: 4px;
  border: 1px solid var(--el-border-color-lighter);
}

.path-meta {
  margin-top: 10px;
  font-size: 12px;
}

/* 底部操作按钮 */
.selected-actions {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  margin-top: 24px;
  align-items: center;
}

/* 工具类 */
.spacer { flex-grow: 1; }
.flex-row { display: flex; gap: 8px; align-items: center; }
.flex-between { display: flex; justify-content: space-between; align-items: center; }
.text-success { color: var(--el-color-success); }
.text-warning { color: var(--el-color-warning); }
.ml-2 { margin-left: 8px; }
.mr-1 { margin-right: 4px; }
.mt-4 { margin-top: 16px; }

/* 响应式调整 */
@media (max-width: 1024px) {
  .path-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 768px) {
  .panel-row {
    flex-direction: column;
    align-items: stretch;
  }
  .game-search, .game-select {
    width: 100%;
    max-width: none;
  }
  .game-toggle-wrap {
    margin-left: 0;
    justify-content: space-between;
  }
  .selected-actions {
    justify-content: center;
  }
  .spacer { display: none; }
}
</style>