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
  <div class="mod-list-card w-full">
    <div class="panel-top">
      <div class="panel-title">{{ tr('mods.modListTitle', 'Mod 列表') }}</div>
      <div class="panel-tools">
        <el-input
          v-model="modKeywordModel"
          class="mod-search"
          clearable
          :placeholder="tr('mods.searchMod', '搜索 Mod 名称、路径...')"
        >
          <template #prefix>
            <i class="el-icon-search"></i>
          </template>
        </el-input>
        <el-radio-group v-model="modStatusFilterModel">
          <el-radio-button value="all">{{ tr('mods.filterAll', '全部') }}</el-radio-button>
          <el-radio-button value="enabled">{{ tr('mods.filterEnabled', '已启用') }}</el-radio-button>
          <el-radio-button value="disabled">{{ tr('mods.filterDisabled', '已禁用') }}</el-radio-button>
        </el-radio-group>
      </div>
    </div>

    <div class="empty-state-wrapper" v-if="!selectedGameName">
      <div class="empty-state-box">
        {{ tr('mods.selectGameFirst', '请先在上方选择一个游戏。') }}
      </div>
    </div>

    <div class="empty-state-wrapper" v-else-if="!selectedState && !isLoadingSelectedMods">
      <div class="empty-state-box">
        <span class="text-warning"><i class="el-icon-warning-outline mr-1"></i>{{ tr('mods.noSelectedState', '当前无法读取所选游戏的 Mod 状态。') }}</span>
      </div>
    </div>

    <div class="empty-state-wrapper" v-else-if="selectedGameSummary && !selectedGameSummary.migotoSupported">
      <div class="empty-state-box">
        {{ tr('mods.unsupportedNoMods', '当前游戏暂不支持 3DMigoto / Mod 管理。') }}
      </div>
    </div>

    <div class="empty-state-wrapper" v-else-if="selectedState && filteredModEntries.length === 0 && !isLoadingSelectedMods">
      <div class="empty-state-box">
        <template v-if="selectedState.entries.length === 0">
          {{ tr('mods.noMods', '当前游戏的 Mod 目录还没有可管理的条目。你可以先打开目录并放入 Mod。') }}
        </template>
        <template v-else>
          {{ tr('mods.noModsFiltered', '没有符合当前筛选条件的 Mod。') }}
        </template>
      </div>
    </div>

    <div v-else class="table-wrapper">
      <el-table
        :data="filteredModEntries"
        v-loading="isLoadingSelectedMods || isBulkOperating"
        class="mod-table"
        stripe
        empty-text="暂无数据"
        max-height="600"
      >
        <el-table-column prop="displayName" :label="tr('mods.columnName', '名称')" min-width="260">
          <template #default="{ row }">
            <div class="mod-name-cell">
              <div class="mod-name">{{ row.displayName }}</div>
              <div class="mod-sub">{{ row.relativeName }}</div>
            </div>
          </template>
        </el-table-column>

        <el-table-column prop="entryType" :label="tr('mods.columnType', '类型')" width="100">
          <template #default="{ row }">
            <el-tag size="small" :type="row.entryType === 'directory' ? 'info' : 'success'" effect="plain">
              {{ row.entryType === 'directory' ? tr('mods.typeDir', '目录') : tr('mods.typeFile', '文件') }}
            </el-tag>
          </template>
        </el-table-column>

        <el-table-column prop="sizeBytes" :label="tr('mods.columnSize', '大小')" width="110">
          <template #default="{ row }">
            <span class="text-secondary text-sm">{{ formatBytes(row.sizeBytes) }}</span>
          </template>
        </el-table-column>

        <el-table-column prop="modifiedUnix" :label="tr('mods.columnModified', '修改时间')" width="170">
          <template #default="{ row }">
            <span class="text-secondary text-sm">{{ formatModified(row.modifiedUnix) }}</span>
          </template>
        </el-table-column>

        <el-table-column :label="tr('mods.columnStatus', '加载状态')" width="120">
          <template #default="{ row }">
            <el-tag size="small" :type="row.enabled ? 'success' : 'info'" :effect="row.enabled ? 'light' : 'plain'">
              {{ row.enabled ? tr('mods.loaded', '已加载') : tr('mods.unloaded', '已禁用') }}
            </el-tag>
          </template>
        </el-table-column>

        <el-table-column :label="tr('mods.columnActions', '操作')" width="180" fixed="right">
          <template #default="{ row }">
            <div class="table-actions">
              <el-switch
                :model-value="row.enabled"
                :loading="activeModEntryName === row.relativeName"
                :disabled="!!activeModEntryName || isBulkOperating"
                @update:model-value="emit('toggle-mod-entry', row, $event)"
              />
              <el-button size="small" plain @click="emit('open-entry-location', row)">
                {{ tr('mods.openLocation', '打开位置') }}
              </el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </div>
  </div>
</template>

<style scoped>
/* 主卡片容器 */
.mod-list-card {
  border: 1px solid var(--el-border-color-lighter);
  background-color: var(--el-bg-color-overlay);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
}
.w-full {
  width: 100%;
}

/* 顶部控制栏 */
.panel-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  flex-wrap: wrap;
}

.panel-title {
  color: var(--el-text-color-primary);
  font-size: 16px;
  font-weight: 600;
}

.panel-tools {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
}

.mod-search {
  width: 280px;
}

/* 颜色工具类 */
.text-secondary { color: var(--el-text-color-secondary); }
.text-warning { color: var(--el-color-warning); }
.text-sm { font-size: 13px; }
.mr-1 { margin-right: 4px; }

/* 空状态容器 */
.empty-state-wrapper {
  padding: 40px 20px;
  display: flex;
  justify-content: center;
  align-items: center;
}
.empty-state-box {
  padding: 32px;
  border-radius: 8px;
  border: 2px dashed var(--el-border-color-lighter);
  color: var(--el-text-color-secondary);
  font-size: 14px;
  text-align: center;
  max-width: 600px;
  background-color: var(--el-fill-color-blank);
}

/* 表格区域 */
.table-wrapper {
  width: 100%;
}
.mod-table {
  width: 100%;
  border-radius: 0 0 8px 8px;
}
.mod-table :deep(th.el-table__cell) {
  background-color: var(--el-fill-color-light);
  color: var(--el-text-color-regular);
  font-weight: 500;
}

/* 表格内部排版 */
.mod-name-cell {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.mod-name {
  color: var(--el-text-color-primary);
  font-weight: 600;
  font-size: 14px;
}
.mod-sub {
  color: var(--el-text-color-secondary);
  font-size: 12px;
  font-family: monospace;
  word-break: break-all;
}

.table-actions {
  display: flex;
  align-items: center;
  gap: 16px;
}

/* 响应式调整 */
@media (max-width: 900px) {
  .panel-top {
    flex-direction: column;
    align-items: stretch;
  }
  .panel-tools {
    justify-content: space-between;
  }
  .mod-search {
    width: 100%;
    max-width: none;
    flex: 1;
  }
}
</style>
