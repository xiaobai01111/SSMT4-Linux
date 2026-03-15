<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { DxvkLocalVersion } from '../../../api';

interface DxvkVersionItem {
  version: string;
  variant: string;
  key: string;
  isLocal: boolean;
  isRemote: boolean;
  fileSize: number;
  publishedAt: string;
}

defineProps<{
  guideMenu: string;
  dxvkLocalVersions: DxvkLocalVersion[];
  dxvkGroupedList: Array<{ variant: string; label: string; items: DxvkVersionItem[] }>;
  selectedKey: string;
  selectedDxvkItem: DxvkVersionItem | undefined;
  isDxvkFetching: boolean;
  isDxvkDownloading: boolean;
  dxvkFetchWarning: string;
  dxvkLocalCount: number;
  deletingDxvkKeys: Record<string, boolean>;
  refreshDxvkLocal: () => void | Promise<void>;
  refreshDxvkRemote: () => void | Promise<void>;
  doDownloadDxvk: () => void | Promise<void>;
  removeLocalDxvkItem: (version: string, variant: string) => void | Promise<void>;
}>();

const emit = defineEmits<{
  (event: 'update:selectedKey', value: string): void;
}>();

const { t, te } = useI18n();

const tr = (key: string, fallback: string) => {
  return te(key) ? String(t(key)) : fallback;
};

const text = computed(() => ({
  panelTitle: tr('settings.dxvk_manage_title', 'DXVK 管理'),
  guideHint: tr(
    'settings.dxvk_guide_hint',
    '请先在此下载 DXVK 版本；下载后可在“游戏设置 -> 运行环境”里应用到当前 Prefix。',
  ),
  sectionTitle: tr('settings.dxvk_section_title', 'DXVK (DirectX → Vulkan)'),
  hint: tr(
    'settings.dxvk_hint',
    '在此下载和管理 DXVK 版本，并安装到游戏的 Wine Prefix 中。',
  ),
  refreshLocal: tr('settings.dxvk_refresh_local', '刷新本地'),
  fetching: tr('settings.dxvk_fetching', '获取中...'),
  refreshRemote: tr('settings.dxvk_refresh_remote', '获取可用版本'),
  downloadTitle: tr('settings.dxvk_download_title', '下载 DXVK 版本'),
  selectVersion: tr('settings.dxvk_select_version', '选择版本...'),
  cached: tr('settings.dxvk_cached', '已缓存'),
  downloading: tr('settings.dxvk_downloading', '下载中...'),
  alreadyCached: tr('settings.dxvk_already_cached', '已缓存'),
  download: tr('settings.dxvk_download', '下载'),
  localTitle: tr('settings.dxvk_local_title', '本地已缓存'),
  extracted: tr('settings.dxvk_extracted', '已解压'),
  archiveOnly: tr('settings.dxvk_archive_only', '仅存档'),
  delete: tr('settings.actions.delete', '删除'),
  noLocal: tr(
    'settings.dxvk_no_local',
    '暂无本地缓存版本，请先获取可用版本并下载。',
  ),
  variantShortOfficial: tr('settings.dxvk_variant_short_official', 'Official'),
}));

const formatBytes = (bytes: number) => {
  if (!Number.isFinite(bytes) || bytes <= 0) return '-';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};

const formatDate = (raw: string) => {
  if (!raw) return '-';
  const d = new Date(raw);
  if (Number.isNaN(d.getTime())) return raw;
  return d.toLocaleString();
};

const dxvkVariantShortLabel = (variant: string) => {
  const labels: Record<string, string> = {
    dxvk: text.value.variantShortOfficial,
    gplasync: 'GPLAsync',
    async: 'Async',
    sarek: 'Sarek',
    sarekasync: 'Sarek-Async',
  };
  return labels[variant] || variant;
};
</script>

<template>
  <div class="settings-panel dxvk-panel w-full" data-onboarding="settings-dxvk-panel">
    <div class="panel-header">
      <h2 class="panel-title">{{ text.panelTitle }}</h2>
    </div>

    <el-alert v-if="guideMenu === 'dxvk'" type="warning" show-icon :closable="false" class="mt-4 custom-alert">
      {{ text.guideHint }}
    </el-alert>

    <el-card class="setting-card mt-5 full-width-card" shadow="never">
      <template #header>
        <div class="flex-between flex-wrap gap-4">
          <div>
            <div class="card-header-title text-primary">{{ text.sectionTitle }}</div>
            <div class="setting-desc mt-1">{{ text.hint }}</div>
          </div>
          <div class="flex-row w-auto">
            <el-button size="small" @click="refreshDxvkLocal">
              <i class="el-icon-refresh mr-1"></i> {{ text.refreshLocal }}
            </el-button>
            <el-button size="small" type="primary" plain @click="refreshDxvkRemote" :loading="isDxvkFetching">
              {{ isDxvkFetching ? text.fetching : text.refreshRemote }}
            </el-button>
          </div>
        </div>
      </template>

      <el-alert v-if="dxvkFetchWarning" type="warning" show-icon :closable="false" class="mb-6 custom-alert">
        {{ dxvkFetchWarning }}
      </el-alert>

      <div class="action-toolbar">
        <div class="toolbar-label">{{ text.downloadTitle }}</div>
        <el-select
          :model-value="selectedKey"
          @update:model-value="emit('update:selectedKey', String($event))"
          :placeholder="text.selectVersion"
          class="flex-1 version-select"
          filterable
        >
          <el-option-group
            v-for="group in dxvkGroupedList"
            :key="group.variant"
            :label="group.label"
          >
            <el-option
              v-for="v in group.items"
              :key="v.key"
              :label="`${v.version}${v.isLocal ? ' [本地]' : ''}${v.fileSize > 0 ? ` (${formatBytes(v.fileSize)})` : ''}`"
              :value="v.key"
            >
              <div class="flex-between w-full">
                <span>
                  <el-tag
                    :type="v.variant === 'dxvk' ? 'info' : 'warning'"
                    size="small"
                    effect="plain"
                    class="mr-2"
                  >
                    {{ dxvkVariantShortLabel(v.variant) }}
                  </el-tag>
                  <span class="font-mono font-bold">{{ v.version }}</span>
                  <el-tag v-if="v.isLocal" type="success" size="small" effect="light" class="ml-2">{{ text.cached }}</el-tag>
                </span>
                <span class="text-secondary text-sm">
                  {{ v.fileSize > 0 ? formatBytes(v.fileSize) : '' }}
                  <span v-if="v.publishedAt" class="ml-1">· {{ formatDate(v.publishedAt) }}</span>
                </span>
              </div>
            </el-option>
          </el-option-group>
        </el-select>
        <el-button
          type="primary"
          :disabled="!selectedDxvkItem || isDxvkDownloading || selectedDxvkItem?.isLocal"
          :loading="isDxvkDownloading"
          @click="doDownloadDxvk"
        >
          {{
            isDxvkDownloading
              ? text.downloading
              : selectedDxvkItem?.isLocal
                ? text.alreadyCached
                : text.download
          }}
        </el-button>
      </div>

      <div class="list-header mt-8">
        <span class="list-title">{{ text.localTitle }}</span>
        <el-tag size="small" type="info" effect="plain" round>{{ dxvkLocalCount }}</el-tag>
      </div>

      <el-card v-if="dxvkLocalVersions.length > 0" shadow="never" class="local-list-card" body-style="padding: 0;">
        <div v-for="(lv, index) in dxvkLocalVersions" :key="`${lv.version}|${lv.variant}`" class="local-item" :class="{'border-b': index !== dxvkLocalVersions.length - 1}">
          <div class="local-item-info">
            <el-tag :type="lv.variant === 'dxvk' ? 'info' : 'warning'" size="small" effect="dark" class="mr-3">
              {{ dxvkVariantShortLabel(lv.variant) }}
            </el-tag>
            <span class="font-mono font-bold text-base">{{ lv.version }}</span>
            <el-tag v-if="lv.extracted" type="success" size="small" effect="light" class="ml-3">{{ text.extracted }}</el-tag>
            <el-tag v-else type="info" size="small" effect="light" class="ml-3">{{ text.archiveOnly }}</el-tag>
            <span class="text-secondary text-xs ml-3 font-mono break-all path-text">{{ lv.path }}</span>
          </div>
          <div class="local-item-actions">
            <el-button
              size="small"
              type="danger"
              plain
              :loading="!!deletingDxvkKeys[`${lv.version}|${lv.variant}`]"
              @click="removeLocalDxvkItem(lv.version, lv.variant)"
            >
              {{ text.delete }}
            </el-button>
          </div>
        </div>
      </el-card>
      
      <div v-else class="empty-hint">
        {{ text.noLocal }}
      </div>

    </el-card>
  </div>
</template>

<style scoped>
/* 强制覆盖全局限制，确保面板 100% 宽度 */
.dxvk-panel {
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
.text-secondary { color: var(--el-text-color-secondary); }
.font-mono { font-family: monospace; }
.font-bold { font-weight: 600; }
.text-sm { font-size: 13px; }
.text-xs { font-size: 12px; }
.text-base { font-size: 15px; }

/* 间距工具类 */
.mt-1 { margin-top: 4px; }
.mt-4 { margin-top: 16px; }
.mt-5 { margin-top: 20px; }
.mt-8 { margin-top: 32px; }
.mb-6 { margin-bottom: 24px; }
.ml-1 { margin-left: 4px; }
.ml-2 { margin-left: 8px; }
.ml-3 { margin-left: 12px; }
.mr-1 { margin-right: 4px; }
.mr-2 { margin-right: 8px; }
.mr-3 { margin-right: 12px; }
.flex-1 { flex: 1; }
.gap-4 { gap: 16px; }
.break-all { word-break: break-all; }

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

.custom-alert {
  background-color: var(--el-color-warning-light-9);
  border: 1px solid var(--el-color-warning-light-7);
}

/* 操作工具栏 */
.action-toolbar {
  display: flex; align-items: center; gap: 12px;
  background-color: var(--el-bg-color);
  padding: 14px 16px; border-radius: 8px;
  border: 1px solid var(--el-border-color-lighter);
  flex-wrap: wrap;
}
.toolbar-label { font-weight: 500; font-size: 14px; white-space: nowrap; }
.version-select { min-width: 260px; max-width: 400px; }

/* 列表样式 */
.list-header {
  display: flex; align-items: center; gap: 8px;
  margin-bottom: 12px; padding-bottom: 8px;
}
.list-title { font-size: 15px; font-weight: 600; }

.local-list-card {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px; background-color: var(--el-bg-color);
}
.local-item {
  display: flex; justify-content: space-between; align-items: center;
  padding: 14px 16px; transition: background-color 0.2s;
  flex-wrap: wrap; gap: 12px;
}
.local-item:hover { background-color: var(--el-fill-color-light); }
.local-item-info { display: flex; align-items: center; flex: 1; min-width: 0; flex-wrap: wrap; }
.path-text { color: var(--el-text-color-placeholder); }
.border-b { border-bottom: 1px solid var(--el-border-color-lighter); }

/* 空状态 */
.empty-hint {
  text-align: center; padding: 40px 20px;
  color: var(--el-text-color-secondary); font-size: 13px;
  border: 2px dashed var(--el-border-color-lighter);
  border-radius: 8px; background-color: var(--el-bg-color);
}
</style>
