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
  guideHint: tr(
    'settings.proton_guide_hint',
    '请先在此下载并安装至少一个 Proton 版本，然后回到主页启动游戏。',
  ),
  sectionHint: tr(
    'settings.proton_tab_hint',
    '每个 Proton 家族独立管理（本地/远程双下拉）',
  ),
  refreshing: tr('settings.proton_refreshing', '刷新中...'),
  refreshLocal: tr('settings.proton_refresh_local', '刷新本地'),
  fetching: tr('settings.proton_fetching', '获取中...'),
  refreshRemote: tr('settings.proton_refresh_remote', '刷新远程'),
  localRemoteHint: tr(
    'settings.proton_local_remote_hint',
    '本页仅管理 Proton 版本下载与目录，不直接修改单个游戏的运行环境。',
  ),
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
  missingSourceHint: tr(
    'settings.proton_missing_source_hint',
    '该家族未配置来源，请在下方目录编辑器新增 source。',
  ),
  noRemote: tr(
    'settings.proton_no_remote',
    '该家族暂无远程版本（可尝试刷新远程）',
  ),
  catalogEditor: tr(
    'settings.proton_catalog_editor',
    'Proton 目录可视化维护',
  ),
  catalogHint: tr(
    'settings.proton_catalog_hint',
    '修改家族/来源后保存即可生效，无需改代码。',
  ),
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
  detectPatterns: tr(
    'settings.proton_detect_patterns',
    '检测规则，每行一个（支持 regex）',
  ),
  addFamily: tr('settings.proton_add_family', '新增家族'),
  sources: tr('settings.proton_sources', '来源定义'),
  sourceRepo: tr('settings.proton_source_repo', 'owner/repo'),
  sourceEndpoint: tr(
    'settings.proton_source_endpoint',
    '完整 API endpoint（可选）',
  ),
  sourceUrlTemplate: tr(
    'settings.proton_source_url_template',
    '下载 URL 模板（actions 用，含 {id}）',
  ),
  sourceAssetPattern: tr(
    'settings.proton_source_asset_pattern',
    '资产匹配 regex',
  ),
  sourceTagPattern: tr(
    'settings.proton_source_tag_pattern',
    'tag 匹配 regex',
  ),
  sourcePrerelease: tr('settings.proton_source_prerelease', '预发布'),
  sourceNote: tr('settings.proton_source_note', '备注'),
  addSource: tr('settings.proton_add_source', '新增来源'),
  editorCollapsedHint: tr(
    'settings.proton_editor_collapsed_hint',
    '目录编辑器已折叠（推荐保持折叠以提升滚动性能）。',
  ),
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
  <div class="settings-panel proton-panel" data-onboarding="settings-proton-panel">
    <div class="panel-title">{{ text.panelTitle }}</div>
    <div v-if="guideMenu === 'proton'" class="settings-guide-banner">
      {{ text.guideHint }}
    </div>

    <div class="section-block">
      <div class="section-header">
        <div class="section-title">{{ text.sectionHint }}</div>
        <div class="toolbar-actions">
          <el-button size="small" @click="refreshLocalGrouped" :loading="isLocalLoading">
            {{ isLocalLoading ? text.refreshing : text.refreshLocal }}
          </el-button>
          <el-button size="small" @click="refreshRemoteGrouped" :loading="isRemoteLoading">
            {{ isRemoteLoading ? text.fetching : text.refreshRemote }}
          </el-button>
        </div>
      </div>
      <div class="section-hint">
        {{ text.localRemoteHint }}
      </div>

      <div v-for="family in familyCards" :key="family.family_key" class="family-card">
        <div class="family-header">
          <div class="family-title">{{ family.display_name }}</div>
          <div class="family-key">{{ family.family_key }}</div>
        </div>

        <div class="family-row">
          <div class="row-label">{{ text.localVersions }}</div>
          <el-select
            v-model="selectedLocalByFamily[family.family_key]"
            :placeholder="text.selectLocal"
            class="family-select"
            filterable
          >
            <el-option
              v-for="item in family.local.items"
              :key="item.id"
              :label="`${item.name} (${item.version})`"
              :value="item.id"
            />
          </el-select>
        </div>
        <div class="row-sub row-sub-path" v-if="selectedLocalItems[family.family_key]">
          <span>{{ text.path }}: {{ selectedLocalItems[family.family_key]?.path }}</span>
          <el-button
            v-if="isManagedProtonItem(selectedLocalItems[family.family_key])"
            text
            type="danger"
            size="small"
            :loading="!!deletingProtonIds[selectedLocalItems[family.family_key]?.id || '']"
            @click="removeLocalProtonItem(selectedLocalItems[family.family_key]!)"
          >
            {{ text.delete }}
          </el-button>
        </div>
        <div class="row-sub" v-else>
          {{ text.noLocal }}
        </div>

        <div class="family-row" style="margin-top: 10px;">
          <div class="row-label">{{ text.remoteVersions }}</div>
          <el-select
            v-model="selectedRemoteByFamily[family.family_key]"
            :placeholder="text.selectRemote"
            class="family-select"
            filterable
            :disabled="family.remote.items.length === 0"
          >
            <el-option
              v-for="item in family.remote.items"
              :key="remoteItemKey(item)"
              :label="`${item.tag} · ${formatBytes(item.file_size)} · ${item.source_repo}`"
              :value="remoteItemKey(item)"
            >
              <div class="remote-option-row">
                <span>{{ item.tag }}</span>
                <span class="remote-option-meta">{{ formatDate(item.published_at) }}</span>
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
        <div class="row-sub" v-if="selectedRemoteItems[family.family_key]">
          {{ text.version }}:
          {{ selectedRemoteItems[family.family_key]?.version }} ·
          {{ formatDate(selectedRemoteItems[family.family_key]?.published_at || '') }}
        </div>
        <div class="row-sub" v-else-if="!hasSourceByFamily[family.family_key]">
          {{ text.missingSourceHint }}
        </div>
        <div class="row-sub" v-else>
          {{ text.noRemote }}
        </div>
      </div>
    </div>

    <div class="section-block">
      <div class="section-header">
        <div>
          <div class="section-title">{{ text.catalogEditor }}</div>
          <div class="section-hint">{{ text.catalogHint }}</div>
        </div>
        <div class="toolbar-actions">
          <el-button size="small" @click="toggleCatalogEditor">
            {{ showCatalogEditor ? text.editorCollapse : text.editorExpand }}
          </el-button>
          <template v-if="showCatalogEditor">
            <el-button size="small" @click="reloadCatalogEditor" :loading="isCatalogLoading">
              {{ text.reloadCatalog }}
            </el-button>
            <el-button type="primary" size="small" @click="saveCatalogChanges" :loading="isCatalogSaving">
              {{ text.saveCatalog }}
            </el-button>
          </template>
        </div>
      </div>

      <div v-if="showCatalogEditor" class="proton-editor-wrap">
        <div class="editor-subtitle">{{ text.families }}</div>
        <div class="editor-list">
          <div v-for="(family, idx) in editableFamilies" :key="`${family.family_key}-${idx}`" class="editor-row family-editor-row">
            <el-input v-model="family.family_key" :placeholder="text.familyKey" />
            <el-input v-model="family.display_name" :placeholder="text.familyName" />
            <el-input-number v-model="family.sort_order" :min="0" :max="9999" :step="10" controls-position="right" />
            <el-switch v-model="family.enabled" :active-text="text.enabled" />
            <el-switch v-model="family.builtin" :active-text="text.builtin" />
            <el-button text type="danger" @click="removeFamily(idx)">
              {{ text.remove }}
            </el-button>
            <el-input
              v-model="family.detect_patterns_text"
              type="textarea"
              :rows="2"
              :placeholder="text.detectPatterns"
              class="patterns-input"
            />
          </div>
          <el-button size="small" @click="addFamily">{{ text.addFamily }}</el-button>
        </div>

        <div class="editor-subtitle" style="margin-top: 14px;">{{ text.sources }}</div>
        <div class="editor-list">
          <div v-for="(source, idx) in editableSources" :key="`${source.repo}-${idx}`" class="editor-row source-editor-row">
            <el-select v-model="source.family_key" class="source-family-select" filterable>
              <el-option
                v-for="family in editableFamilies"
                :key="family.family_key"
                :label="`${family.display_name || family.family_key} (${family.family_key || '-'})`"
                :value="family.family_key"
              />
            </el-select>
            <el-select v-model="source.provider" class="provider-select" filterable>
              <el-option label="github_releases" value="github_releases" />
              <el-option label="forgejo_releases" value="forgejo_releases" />
              <el-option label="github_actions" value="github_actions" />
            </el-select>
            <el-input v-model="source.repo" :placeholder="text.sourceRepo" />
            <el-input v-model="source.endpoint" :placeholder="text.sourceEndpoint" />
            <el-input v-model="source.url_template" :placeholder="text.sourceUrlTemplate" />
            <el-input-number v-model="source.asset_index" :min="-1" :max="100" controls-position="right" />
            <el-input v-model="source.asset_pattern" :placeholder="text.sourceAssetPattern" />
            <el-input v-model="source.tag_pattern" :placeholder="text.sourceTagPattern" />
            <el-input-number v-model="source.max_count" :min="1" :max="100" controls-position="right" />
            <el-switch v-model="source.include_prerelease" :active-text="text.sourcePrerelease" />
            <el-switch v-model="source.enabled" :active-text="text.enabled" />
            <el-input v-model="source.note" :placeholder="text.sourceNote" />
            <el-button text type="danger" @click="removeSource(idx)">
              {{ text.remove }}
            </el-button>
          </div>
          <el-button size="small" @click="addSource">{{ text.addSource }}</el-button>
        </div>
      </div>
      <div v-else class="row-sub" style="margin-top: 10px;">
        {{ text.editorCollapsedHint }}
      </div>
    </div>
  </div>
</template>
