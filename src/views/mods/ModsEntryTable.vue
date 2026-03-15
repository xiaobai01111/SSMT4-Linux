<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { GameModDirectoryState, ManagedModEntry } from '../../api';
import type { ModGameSummary, ModStatusFilter } from './types';

const props = defineProps<{
  selectedGameName: string;
  selectedGameSummary: ModGameSummary | null;
  selectedState: GameModDirectoryState | null;
  filteredModEntries: ManagedModEntry[];
  isLoadingSelectedMods: boolean;
  isBulkOperating: boolean;
  activeModEntryName: string;
  modKeyword: string;
  modStatusFilter: ModStatusFilter;
  formatBytes: (bytes: number) => string;
  formatModified: (unix?: number | null) => string;
}>();

const emit = defineEmits<{
  (event: 'update:modKeyword', value: string): void;
  (event: 'update:modStatusFilter', value: ModStatusFilter): void;
  (event: 'toggle-mod-entry', entry: ManagedModEntry, value: string | number | boolean): void;
  (event: 'open-entry-location', entry: ManagedModEntry): void;
}>();

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);

const modKeywordModel = computed({
  get: () => props.modKeyword,
  set: (value: string) => emit('update:modKeyword', value),
});

const modStatusFilterModel = computed({
  get: () => props.modStatusFilter,
  set: (value: ModStatusFilter) => emit('update:modStatusFilter', value),
});
</script>

<template>
  <div class="mod-list-panel">
    <div class="panel-top">
      <div class="panel-title">{{ tr('mods.modListTitle', 'Mod 列表') }}</div>
      <div class="panel-tools">
        <el-input
          v-model="modKeywordModel"
          class="mod-search"
          clearable
          :placeholder="tr('mods.searchMod', '搜索 Mod 名称、路径...')"
        />
        <el-radio-group v-model="modStatusFilterModel" size="small">
          <el-radio-button label="all">{{ tr('mods.filterAll', '全部') }}</el-radio-button>
          <el-radio-button label="enabled">{{ tr('mods.filterEnabled', '已启用') }}</el-radio-button>
          <el-radio-button label="disabled">{{ tr('mods.filterDisabled', '已禁用') }}</el-radio-button>
        </el-radio-group>
      </div>
    </div>

    <div v-if="!selectedGameName" class="empty-state">
      {{ tr('mods.selectGameFirst', '请先选择一个游戏。') }}
    </div>

    <div v-else-if="!selectedState && !isLoadingSelectedMods" class="empty-state">
      {{ tr('mods.noSelectedState', '当前无法读取所选游戏的 Mod 状态。') }}
    </div>

    <div
      v-else-if="selectedGameSummary && !selectedGameSummary.migotoSupported"
      class="empty-state"
    >
      {{ tr('mods.unsupportedNoMods', '当前游戏暂不支持 3DMigoto / Mod 管理。') }}
    </div>

    <div
      v-else-if="selectedState && filteredModEntries.length === 0 && !isLoadingSelectedMods"
      class="empty-state"
    >
      <template v-if="selectedState.entries.length === 0">
        {{ tr('mods.noMods', '当前游戏的 Mod 目录还没有可管理的条目。你可以先打开目录并放入 Mod。') }}
      </template>
      <template v-else>
        {{ tr('mods.noModsFiltered', '没有符合当前筛选条件的 Mod。') }}
      </template>
    </div>

    <el-table
      v-else
      :data="filteredModEntries"
      v-loading="isLoadingSelectedMods || isBulkOperating"
      class="mod-table"
      empty-text=""
      max-height="500"
    >
      <el-table-column prop="displayName" :label="tr('mods.columnName', '名称')" min-width="240">
        <template #default="{ row }">
          <div class="mod-name-cell">
            <div class="mod-name">{{ row.displayName }}</div>
            <div class="mod-sub">{{ row.relativeName }}</div>
          </div>
        </template>
      </el-table-column>

      <el-table-column prop="entryType" :label="tr('mods.columnType', '类型')" width="110">
        <template #default="{ row }">
          <el-tag size="small" :type="row.entryType === 'directory' ? 'info' : 'success'">
            {{ row.entryType === 'directory' ? tr('mods.typeDir', '目录') : tr('mods.typeFile', '文件') }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column prop="sizeBytes" :label="tr('mods.columnSize', '大小')" width="120">
        <template #default="{ row }">
          {{ formatBytes(row.sizeBytes) }}
        </template>
      </el-table-column>

      <el-table-column prop="modifiedUnix" :label="tr('mods.columnModified', '修改时间')" min-width="180">
        <template #default="{ row }">
          {{ formatModified(row.modifiedUnix) }}
        </template>
      </el-table-column>

      <el-table-column :label="tr('mods.columnStatus', '加载状态')" width="140">
        <template #default="{ row }">
          <el-tag size="small" :type="row.enabled ? 'success' : 'warning'">
            {{ row.enabled ? tr('mods.loaded', '已加载') : tr('mods.unloaded', '已禁用') }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column :label="tr('mods.columnActions', '操作')" min-width="240">
        <template #default="{ row }">
          <div class="table-actions">
            <el-switch
              :model-value="row.enabled"
              :loading="activeModEntryName === row.relativeName"
              :disabled="!!activeModEntryName || isBulkOperating"
              @update:model-value="emit('toggle-mod-entry', row, $event)"
            />
            <el-button text @click="emit('open-entry-location', row)">{{ tr('mods.openLocation', '打开位置') }}</el-button>
          </div>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<style scoped>
.mod-list-panel {
  border: 1px solid rgba(0, 240, 255, 0.18);
  background: rgba(0, 8, 14, 0.55);
  border-radius: 10px;
  padding: 18px;
}

.panel-top {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: center;
  margin-bottom: 16px;
}

.panel-title {
  color: #fff;
  font-size: 18px;
  font-weight: 700;
}

.panel-tools {
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
}

.mod-search {
  min-width: 300px;
}

:deep(.mod-search .el-input__wrapper) {
  background-color: rgba(10, 15, 20, 0.6) !important;
  border: 1px solid rgba(0, 240, 255, 0.3) !important;
  box-shadow: none !important;
}

.empty-state {
  padding: 18px;
  border-radius: 8px;
  border: 1px dashed rgba(255, 255, 255, 0.14);
  color: rgba(255, 255, 255, 0.62);
}

.mod-table {
  width: 100%;
}

:deep(.mod-table) {
  --el-table-border-color: rgba(255, 255, 255, 0.08);
  --el-table-header-bg-color: rgba(255, 255, 255, 0.03);
  --el-table-tr-bg-color: transparent;
  --el-table-row-hover-bg-color: rgba(0, 240, 255, 0.06);
  --el-table-text-color: rgba(255, 255, 255, 0.88);
  --el-table-header-text-color: rgba(255, 255, 255, 0.72);
  --el-fill-color-blank: transparent;
  background: transparent;
}

.mod-name-cell {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.mod-name {
  color: #fff;
  font-weight: 600;
}

.mod-sub {
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
  word-break: break-all;
}

.table-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

@media (max-width: 900px) {
  .panel-top {
    flex-direction: column;
    align-items: stretch;
  }

  .mod-search {
    max-width: none;
    min-width: 0;
    width: 100%;
  }
}
</style>
