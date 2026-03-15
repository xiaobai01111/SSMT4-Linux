<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type {
  ProtonCatalog,
  ProtonFamily,
  ProtonFamilyLocalGroup,
  ProtonLocalVersionItem,
  ProtonFamilyRemoteGroup,
  ProtonRemoteVersionItem,
  ProtonSource,
} from '../../../api';

type EditableProtonFamily = ProtonFamily & { detect_patterns_text: string };
type ProtonFamilyCard = ProtonFamily & {
  local: ProtonFamilyLocalGroup;
  remote: ProtonFamilyRemoteGroup;
};

const props = defineProps<{
  guideMenu: string;
  protonCatalog: ProtonCatalog;
  familyCards: ProtonFamilyCard[];
  selectedLocalByFamily: Record<string, string>;
  selectedRemoteByFamily: Record<string, string>;
  showCatalogEditor: boolean;
  isCatalogLoading: boolean;
  isCatalogSaving: boolean;
  isLocalLoading: boolean;
  isRemoteLoading: boolean;
  isDownloading: boolean;
  downloadingFamilyKey: string;
  downloadingTag: string;
  editableFamilies: EditableProtonFamily[];
  editableSources: ProtonSource[];
  deletingProtonIds: Record<string, boolean>;
  refreshLocalGrouped: () => void | Promise<void>;
  refreshRemoteGrouped: () => void | Promise<void>;
  selectedLocalItems: Record<string, ProtonLocalVersionItem | null>;
  selectedRemoteItems: Record<string, ProtonRemoteVersionItem | null>;
  remoteItemKey: (item: ProtonRemoteVersionItem) => string;
  hasSourceByFamily: Record<string, boolean>;
  isManagedProtonItem: (item: { path: string } | null | undefined) => boolean;
  installSelectedForFamily: (familyKey: string) => void | Promise<void>;
  removeLocalProtonItem: (item: ProtonLocalVersionItem) => void | Promise<void>;
  reloadCatalogEditor: () => void | Promise<void>;
  saveCatalogChanges: () => void | Promise<void>;
  addFamily: () => void;
  removeFamily: (index: number) => void;
  addSource: () => void;
  removeSource: (index: number) => void;
}>();

const emit = defineEmits<{
  (event: 'update:showCatalogEditor', value: boolean): void;
}>();

const { t, te } = useI18n();

const tr = (key: string, fallback: string) => {
  return te(key) ? String(t(key)) : fallback;
};

const text = computed(() => ({
  panelTitle: tr('settings.proton_manage_title', 'Proton 管理'),
  guideHint: tr('settings.proton_guide_hint', '请先在此下载并安装至少一个 Proton 版本，然后回到主页启动游戏。'),
  sectionHint: tr('settings.proton_tab_hint', '每个 Proton 家族独立管理（本地/远程双下拉）'),
  refreshing: tr('settings.proton_refreshing', '刷新中...'),
  refreshLocal: tr('settings.proton_refresh_local', '刷新本地'),
  fetching: tr('settings.proton_fetching', '获取中...'),
  refreshRemote: tr('settings.proton_refresh_remote', '刷新远程'),
  localRemoteHint: tr('settings.proton_local_remote_hint', '本页仅管理 Proton 版本下载与目录，不直接修改单个游戏的运行环境。'),
  localVersions: tr('settings.proton_local_versions', '本地已安装'),
  selectLocal: tr('settings.proton_select_local', '选择本地版本'),
  path: tr('settings.proton_selected_path', '路径'),
  delete: tr('settings.actions.delete', '删除'),
  noLocal: tr('settings.proton_no_local', '该家族暂无本地版本'),
  remoteVersions: tr('settings.proton_remote_versions', '远程可下载'),
  selectRemote: tr('settings.proton_select_remote', '选择远程版本'),
  downloading: tr('settings.proton_downloading', '下载中...'),
  installed: tr('settings.proton_installed', '已安装'),
  download: tr('settings.proton_download', '下载选中版本'),
  version: tr('settings.proton_selected_version', '版本'),
  missingSourceHint: tr('settings.proton_missing_source_hint', '该家族未配置来源，请在下方目录编辑器新增 source。'),
  noRemote: tr('settings.proton_no_remote', '该家族暂无远程版本（可尝试刷新远程）'),
  catalogEditor: tr('settings.proton_catalog_editor', 'Proton 目录可视化维护'),
  catalogHint: tr('settings.proton_catalog_hint', '修改家族/来源后保存即可生效，无需改代码。'),
  editorCollapse: tr('settings.proton_editor_collapse', '收起目录编辑器'),
  editorExpand: tr('settings.proton_editor_expand', '展开目录编辑器'),
  reloadCatalog: tr('settings.proton_reload_catalog', '重载目录'),
  saveCatalog: tr('settings.proton_save_catalog', '保存目录'),
  families: tr('settings.proton_families', '家族定义'),
  familyKey: tr('settings.proton_family_key', 'family_key'),
  familyName: tr('settings.proton_family_name', '显示名称'),
  enabled: tr('settings.proton_enabled', '启用'),
  builtin: tr('settings.proton_builtin', '内置'),
  remove: tr('settings.proton_remove', '删除'),
  detectPatterns: tr('settings.proton_detect_patterns', '检测规则，每行一个（支持 regex）'),
  addFamily: tr('settings.proton_add_family', '新增家族'),
  sources: tr('settings.proton_sources', '来源定义'),
  sourceRepo: tr('settings.proton_source_repo', 'owner/repo'),
  sourceEndpoint: tr('settings.proton_source_endpoint', '完整 API endpoint（可选）'),
  sourceUrlTemplate: tr('settings.proton_source_url_template', '下载 URL 模板（actions 用，含 {id}）'),
  sourceAssetPattern: tr('settings.proton_source_asset_pattern', '资产匹配 regex'),
  sourceTagPattern: tr('settings.proton_source_tag_pattern', 'tag 匹配 regex'),
  sourcePrerelease: tr('settings.proton_source_prerelease', '预发布'),
  sourceNote: tr('settings.proton_source_note', '备注'),
  addSource: tr('settings.proton_add_source', '新增来源'),
  editorCollapsedHint: tr('settings.proton_editor_collapsed_hint', '目录编辑器已折叠（推荐保持折叠以提升滚动性能）。'),
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

const toggleCatalogEditor = () => {
  emit('update:showCatalogEditor', !props.showCatalogEditor);
};
</script>

<template>
  <div class="settings-panel proton-panel w-full" data-onboarding="settings-proton-panel">
    <div class="panel-header">
      <h2 class="panel-title">{{ text.panelTitle }}</h2>
    </div>

    <el-alert v-if="guideMenu === 'proton'" type="warning" show-icon :closable="false" class="mt-4 custom-alert">
      {{ text.guideHint }}
    </el-alert>

    <div class="section-header mt-6 mb-4">
      <div>
        <div class="section-title">{{ text.sectionHint }}</div>
        <div class="section-hint mt-1">{{ text.localRemoteHint }}</div>
      </div>
      <div class="flex-row">
        <el-button size="small" @click="refreshLocalGrouped" :loading="isLocalLoading">
          <i class="el-icon-refresh mr-1" v-if="!isLocalLoading"></i>
          {{ isLocalLoading ? text.refreshing : text.refreshLocal }}
        </el-button>
        <el-button size="small" type="primary" plain @click="refreshRemoteGrouped" :loading="isRemoteLoading">
          <i class="el-icon-download mr-1" v-if="!isRemoteLoading"></i>
          {{ isRemoteLoading ? text.fetching : text.refreshRemote }}
        </el-button>
      </div>
    </div>

    <div class="family-list">
      <el-card v-for="family in familyCards" :key="family.family_key" class="setting-card full-width-card family-card" shadow="never">
        <template #header>
          <div class="flex-row align-center">
            <span class="card-header-title text-primary">{{ family.display_name }}</span>
            <el-tag size="small" type="info" effect="plain" class="ml-2">{{ family.family_key }}</el-tag>
          </div>
        </template>

        <div class="two-column-grid">
          
          <div class="column-block">
            <div class="column-title">{{ text.localVersions }}</div>
            <div class="flex-row mt-3">
              <el-select
                v-model="selectedLocalByFamily[family.family_key]"
                :placeholder="text.selectLocal"
                class="flex-1"
                filterable
              >
                <el-option
                  v-for="item in family.local.items"
                  :key="item.id"
                  :label="`${item.name} (${item.version})`"
                  :value="item.id"
                />
              </el-select>
              <el-button
                v-if="selectedLocalItems[family.family_key] && isManagedProtonItem(selectedLocalItems[family.family_key])"
                type="danger"
                plain
                :loading="!!deletingProtonIds[selectedLocalItems[family.family_key]?.id || '']"
                @click="removeLocalProtonItem(selectedLocalItems[family.family_key]!)"
              >
                {{ text.delete }}
              </el-button>
            </div>
            <div class="column-hint mt-2">
              <template v-if="selectedLocalItems[family.family_key]">
                <span class="text-secondary">{{ text.path }}:</span>
                <span class="font-mono text-xs ml-1 path-text break-all">{{ selectedLocalItems[family.family_key]?.path }}</span>
              </template>
              <template v-else>
                <span class="text-secondary">{{ text.noLocal }}</span>
              </template>
            </div>
          </div>

          <div class="column-block">
            <div class="column-title">{{ text.remoteVersions }}</div>
            <div class="flex-row mt-3">
              <el-select
                v-model="selectedRemoteByFamily[family.family_key]"
                :placeholder="text.selectRemote"
                class="flex-1"
                filterable
                :disabled="family.remote.items.length === 0"
              >
                <el-option
                  v-for="item in family.remote.items"
                  :key="remoteItemKey(item)"
                  :label="`${item.tag} · ${formatBytes(item.file_size)} · ${item.source_repo}`"
                  :value="remoteItemKey(item)"
                >
                  <div class="flex-between w-full">
                    <span class="font-bold">{{ item.tag }}</span>
                    <span class="text-secondary text-xs ml-3">{{ formatDate(item.published_at) }}</span>
                  </div>
                </el-option>
              </el-select>
              <el-button
                type="primary"
                :disabled="
                  !selectedRemoteItems[family.family_key]
                  || selectedRemoteItems[family.family_key]?.installed
                  || isDownloading
                "
                :loading="isDownloading && downloadingFamilyKey === family.family_key"
                @click="installSelectedForFamily(family.family_key)"
              >
                {{
                  isDownloading && downloadingFamilyKey === family.family_key
                    ? `${text.downloading} ${downloadingTag}`
                    : selectedRemoteItems[family.family_key]?.installed
                      ? text.installed
                      : text.download
                }}
              </el-button>
            </div>
            <div class="column-hint mt-2">
              <template v-if="selectedRemoteItems[family.family_key]">
                <span class="text-secondary">{{ text.version }}:</span>
                <span class="font-bold ml-1">{{ selectedRemoteItems[family.family_key]?.version }}</span>
                <span class="text-secondary ml-1">· {{ formatDate(selectedRemoteItems[family.family_key]?.published_at || '') }}</span>
              </template>
              <template v-else-if="!hasSourceByFamily[family.family_key]">
                <span class="text-warning"><i class="el-icon-warning-outline mr-1"></i>{{ text.missingSourceHint }}</span>
              </template>
              <template v-else>
                <span class="text-secondary">{{ text.noRemote }}</span>
              </template>
            </div>
          </div>

        </div>
      </el-card>
    </div>

    <el-card class="setting-card mt-8 full-width-card" shadow="never">
      <template #header>
        <div class="flex-between flex-wrap gap-4">
          <div>
            <div class="card-header-title">{{ text.catalogEditor }}</div>
            <div class="setting-desc mt-1">{{ text.catalogHint }}</div>
          </div>
          <div class="flex-row">
            <el-button size="small" @click="toggleCatalogEditor">
              {{ showCatalogEditor ? text.editorCollapse : text.editorExpand }}
            </el-button>
            <template v-if="showCatalogEditor">
              <el-button size="small" @click="reloadCatalogEditor" :loading="isCatalogLoading">
                {{ text.reloadCatalog }}
              </el-button>
              <el-button type="success" size="small" plain @click="saveCatalogChanges" :loading="isCatalogSaving">
                <i class="el-icon-check mr-1" v-if="!isCatalogSaving"></i> {{ text.saveCatalog }}
              </el-button>
            </template>
          </div>
        </div>
      </template>

      <div v-if="showCatalogEditor" class="editor-wrap">
        
        <div class="editor-section">
          <div class="flex-between mb-4 border-b pb-2">
            <span class="font-bold text-base">{{ text.families }}</span>
            <el-button size="small" type="primary" plain @click="addFamily">+ {{ text.addFamily }}</el-button>
          </div>
          
          <div class="editor-list">
            <div v-for="(family, idx) in editableFamilies" :key="`${family.family_key}-${idx}`" class="editor-row-card">
              <div class="editor-grid-family">
                <el-input v-model="family.family_key" :placeholder="text.familyKey" />
                <el-input v-model="family.display_name" :placeholder="text.familyName" />
                <div class="flex-row align-center">
                  <span class="text-xs text-secondary whitespace-nowrap">排序</span>
                  <el-input-number v-model="family.sort_order" :min="0" :max="9999" :step="10" controls-position="right" class="flex-1" />
                </div>
                <el-switch v-model="family.enabled" :active-text="text.enabled" />
                <el-switch v-model="family.builtin" :active-text="text.builtin" />
                <el-button type="danger" text bg @click="removeFamily(idx)" class="justify-self-end">{{ text.remove }}</el-button>
              </div>
              <el-input
                v-model="family.detect_patterns_text"
                type="textarea"
                :rows="2"
                :placeholder="text.detectPatterns"
                class="mt-3 font-mono text-sm patterns-input"
              />
            </div>
          </div>
        </div>

        <div class="editor-section mt-8">
          <div class="flex-between mb-4 border-b pb-2">
            <span class="font-bold text-base">{{ text.sources }}</span>
            <el-button size="small" type="primary" plain @click="addSource">+ {{ text.addSource }}</el-button>
          </div>
          
          <div class="editor-list">
            <div v-for="(source, idx) in editableSources" :key="`${source.repo}-${idx}`" class="editor-row-card">
              
              <div class="editor-grid-source mb-3">
                <el-select v-model="source.family_key" filterable placeholder="选择家族">
                  <el-option
                    v-for="family in editableFamilies"
                    :key="family.family_key"
                    :label="`${family.display_name || family.family_key} (${family.family_key || '-'})`"
                    :value="family.family_key"
                  />
                </el-select>
                <el-select v-model="source.provider" filterable placeholder="提供商">
                  <el-option label="github_releases" value="github_releases" />
                  <el-option label="forgejo_releases" value="forgejo_releases" />
                  <el-option label="github_actions" value="github_actions" />
                </el-select>
                <el-input v-model="source.repo" :placeholder="text.sourceRepo" />
                <el-input v-model="source.endpoint" :placeholder="text.sourceEndpoint" />
              </div>

              <div class="editor-grid-source-2 mb-3">
                <el-input v-model="source.url_template" :placeholder="text.sourceUrlTemplate" class="span-2" />
                <el-input v-model="source.asset_pattern" :placeholder="text.sourceAssetPattern" />
                <el-input v-model="source.tag_pattern" :placeholder="text.sourceTagPattern" />
              </div>

              <div class="editor-grid-source-3">
                <div class="flex-row align-center">
                  <span class="text-xs text-secondary whitespace-nowrap">Asset Index</span>
                  <el-input-number v-model="source.asset_index" :min="-1" :max="100" controls-position="right" class="flex-1" />
                </div>
                <div class="flex-row align-center">
                  <span class="text-xs text-secondary whitespace-nowrap">Max Count</span>
                  <el-input-number v-model="source.max_count" :min="1" :max="100" controls-position="right" class="flex-1" />
                </div>
                <el-input v-model="source.note" :placeholder="text.sourceNote" />
                <div class="flex-row gap-4 align-center">
                  <el-switch v-model="source.include_prerelease" :active-text="text.sourcePrerelease" />
                  <el-switch v-model="source.enabled" :active-text="text.enabled" />
                </div>
                <el-button type="danger" text bg @click="removeSource(idx)" class="justify-self-end">{{ text.remove }}</el-button>
              </div>

            </div>
          </div>
        </div>

      </div>
      
      <div v-else class="empty-hint">
        {{ text.editorCollapsedHint }}
      </div>
    </el-card>
  </div>
</template>

<style scoped>
/* 强制覆盖全局限制，确保面板 100% 宽度 */
.proton-panel {
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  color: var(--el-text-color-primary);
  width: 100% !important;
  max-width: none !important;
  flex-grow: 1;
}

.w-full { width: 100%; }
.full-width-card { 
  width: 100% !important; 
  max-width: none !important;
  box-sizing: border-box; 
}

/* 颜色与字体工具类 */
.text-primary { color: var(--el-color-primary); }
.text-danger { color: var(--el-color-danger); }
.text-warning { color: var(--el-color-warning); }
.text-secondary { color: var(--el-text-color-secondary); }
.font-mono { font-family: monospace; }
.font-bold { font-weight: 600; }
.text-sm { font-size: 13px; }
.text-xs { font-size: 12px; }
.text-base { font-size: 15px; }

/* 间距工具类 */
.mt-1 { margin-top: 4px; }
.mt-2 { margin-top: 8px; }
.mt-3 { margin-top: 12px; }
.mt-4 { margin-top: 16px; }
.mt-6 { margin-top: 24px; }
.mt-8 { margin-top: 32px; }
.mb-3 { margin-bottom: 12px; }
.mb-4 { margin-bottom: 16px; }
.ml-1 { margin-left: 4px; }
.ml-2 { margin-left: 8px; }
.ml-3 { margin-left: 12px; }
.mr-1 { margin-right: 4px; }
.pb-2 { padding-bottom: 8px; }
.flex-1 { flex: 1; min-width: 0; }
.gap-4 { gap: 16px; }

/* 排版工具类 */
.flex-row { display: flex; gap: 8px; align-items: center; }
.flex-between { display: flex; justify-content: space-between; align-items: center; }
.flex-wrap { flex-wrap: wrap; }
.break-all { word-break: break-all; }
.whitespace-nowrap { white-space: nowrap; }
.justify-self-end { justify-self: flex-end; }
.border-b { border-bottom: 1px solid var(--el-border-color-lighter); }

/* 头部样式 */
.panel-header {
  display: flex; align-items: center; gap: 12px;
  padding-bottom: 12px; border-bottom: 1px solid var(--el-border-color-lighter);
}
.panel-title { font-size: 22px; font-weight: 600; margin: 0; }
.section-header { display: flex; justify-content: space-between; align-items: flex-end; flex-wrap: wrap; gap: 12px; }
.section-title { font-size: 18px; font-weight: 600; }
.section-hint { font-size: 13px; color: var(--el-text-color-secondary); }

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

/* 家族管理块 */
.family-list {
  display: flex; flex-direction: column; gap: 20px;
}
.family-card {
  transition: border-color 0.2s, box-shadow 0.2s;
}
.family-card:hover {
  border-color: var(--el-color-primary-light-5);
}

/* 双列网格 (左本地、右远程) */
.two-column-grid {
  display: grid; grid-template-columns: 1fr 1fr; gap: 32px;
}
@media (max-width: 900px) {
  .two-column-grid { grid-template-columns: 1fr; gap: 20px; }
}
.column-block { display: flex; flex-direction: column; }
.column-title { font-size: 14px; font-weight: 500; color: var(--el-text-color-regular); }
.column-hint { font-size: 13px; min-height: 20px; display: flex; align-items: center; }
.path-text { color: var(--el-text-color-placeholder); }

/* 目录可视化编辑器样式 */
.editor-list {
  display: flex; flex-direction: column; gap: 16px;
}
.editor-row-card {
  background-color: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px; padding: 16px;
}

/* 给编辑器里的多行输入框加深背景 */
.patterns-input :deep(.el-textarea__inner) {
  background-color: var(--el-fill-color-light);
}

/* 家族表单网格排版 */
.editor-grid-family {
  display: grid; gap: 12px; align-items: center;
  grid-template-columns: 2fr 2fr 1fr auto auto auto;
}

/* 来源表单网格排版 */
.editor-grid-source {
  display: grid; gap: 12px; align-items: center;
  grid-template-columns: 1.5fr 1.5fr 2fr 2fr;
}
.editor-grid-source-2 {
  display: grid; gap: 12px; align-items: center;
  grid-template-columns: 2fr 1fr 1fr;
}
.span-2 { grid-column: span 1; }

.editor-grid-source-3 {
  display: grid; gap: 12px; align-items: center;
  grid-template-columns: 1.2fr 1.2fr 2fr auto auto;
}

/* 响应式断点 */
@media (max-width: 1200px) {
  .editor-grid-family { grid-template-columns: 1fr 1fr 1fr; }
  .editor-grid-source, .editor-grid-source-2, .editor-grid-source-3 {
    grid-template-columns: 1fr 1fr;
  }
}
@media (max-width: 768px) {
  .editor-grid-family, .editor-grid-source, .editor-grid-source-2, .editor-grid-source-3 {
    grid-template-columns: 1fr;
  }
}

/* 空状态 */
.empty-hint {
  text-align: center; padding: 40px 20px;
  color: var(--el-text-color-secondary); font-size: 13px;
  border: 2px dashed var(--el-border-color-lighter);
  border-radius: 8px; background-color: var(--el-bg-color);
}
</style>