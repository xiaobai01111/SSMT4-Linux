<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { appSettings } from '../store'
import {
  downloadProton,
  fetchRemoteProtonGrouped,
  getProtonCatalog,
  openFileDialog,
  openLogWindow,
  saveProtonCatalog,
  scanLocalProtonGrouped,
  showMessage,
  type ProtonCatalog,
  type ProtonFamily,
  type ProtonFamilyLocalGroup,
  type ProtonFamilyRemoteGroup,
  type ProtonRemoteVersionItem,
  type ProtonSource,
} from '../api';
import { useI18n } from 'vue-i18n';

const { t } = useI18n()

const activeMenu = ref('basic')

const protonCatalog = ref<ProtonCatalog>({ families: [], sources: [] });
const localGroups = ref<ProtonFamilyLocalGroup[]>([]);
const remoteGroups = ref<ProtonFamilyRemoteGroup[]>([]);

const isCatalogLoading = ref(false);
const isCatalogSaving = ref(false);
const isLocalLoading = ref(false);
const isRemoteLoading = ref(false);
const isDownloading = ref(false);
const downloadingFamilyKey = ref('');
const downloadingTag = ref('');
const protonLoaded = ref(false);

const selectedLocalByFamily = reactive<Record<string, string>>({});
const selectedRemoteByFamily = reactive<Record<string, string>>({});

type EditableProtonFamily = ProtonFamily & { detect_patterns_text: string };
const editableFamilies = ref<EditableProtonFamily[]>([]);
const editableSources = ref<ProtonSource[]>([]);

const tr = (key: string, fallback: string) => {
  const value = t(key);
  return value === key ? fallback : value;
};

const remoteItemKey = (item: ProtonRemoteVersionItem) => `${item.tag}@@${item.source_repo}`;

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

const hasSourceForFamily = (familyKey: string) => {
  return protonCatalog.value.sources.some(
    (source) => source.family_key === familyKey && source.enabled,
  );
};

const selectedLocalItem = (familyKey: string) => {
  const group = localGroups.value.find((g) => g.family_key === familyKey);
  if (!group) return null;
  return group.items.find((item) => item.id === selectedLocalByFamily[familyKey]) ?? null;
};

const selectedRemoteItem = (familyKey: string) => {
  const group = remoteGroups.value.find((g) => g.family_key === familyKey);
  if (!group) return null;
  const key = selectedRemoteByFamily[familyKey];
  return group.items.find((item) => remoteItemKey(item) === key) ?? null;
};

const ensureLocalSelections = () => {
  for (const group of localGroups.value) {
    if (group.items.length === 0) {
      delete selectedLocalByFamily[group.family_key];
      continue;
    }
    const existing = selectedLocalByFamily[group.family_key];
    const hasExisting = group.items.some((item) => item.id === existing);
    if (!hasExisting) {
      selectedLocalByFamily[group.family_key] = group.items[0].id;
    }
  }
};

const ensureRemoteSelections = () => {
  for (const group of remoteGroups.value) {
    if (group.items.length === 0) {
      delete selectedRemoteByFamily[group.family_key];
      continue;
    }
    const existing = selectedRemoteByFamily[group.family_key];
    const hasExisting = group.items.some((item) => remoteItemKey(item) === existing);
    if (!hasExisting) {
      selectedRemoteByFamily[group.family_key] = remoteItemKey(group.items[0]);
    }
  }
};

const toEditableFamily = (family: ProtonFamily): EditableProtonFamily => ({
  ...family,
  detect_patterns_text: family.detect_patterns.join('\n'),
});

const normalizeDetectPatterns = (text: string): string[] => {
  return text
    .split('\n')
    .map((v) => v.trim())
    .filter((v) => v.length > 0);
};

const loadCatalog = async () => {
  if (isCatalogLoading.value) return;
  try {
    isCatalogLoading.value = true;
    const catalog = await getProtonCatalog();
    protonCatalog.value = {
      families: [...catalog.families].sort((a, b) => a.sort_order - b.sort_order),
      sources: [...catalog.sources],
    };
    editableFamilies.value = protonCatalog.value.families.map(toEditableFamily);
    editableSources.value = protonCatalog.value.sources.map((source) => ({ ...source }));
  } catch (e) {
    await showMessage(`加载 Proton 目录失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isCatalogLoading.value = false;
  }
};

const refreshLocalGrouped = async () => {
  if (isLocalLoading.value) return;
  try {
    isLocalLoading.value = true;
    localGroups.value = await scanLocalProtonGrouped();
    ensureLocalSelections();
  } catch (e) {
    await showMessage(`获取本地 Proton 失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isLocalLoading.value = false;
  }
};

const refreshRemoteGrouped = async () => {
  if (isRemoteLoading.value) return;
  try {
    isRemoteLoading.value = true;
    remoteGroups.value = await fetchRemoteProtonGrouped();
    ensureRemoteSelections();
  } catch (e) {
    await showMessage(`获取远程 Proton 列表失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isRemoteLoading.value = false;
  }
};

const buildCatalogPayload = (): ProtonCatalog | null => {
  const families: ProtonFamily[] = editableFamilies.value.map((family) => ({
    family_key: family.family_key.trim(),
    display_name: family.display_name.trim(),
    enabled: family.enabled,
    sort_order: Number(family.sort_order) || 0,
    detect_patterns: normalizeDetectPatterns(family.detect_patterns_text),
    builtin: family.builtin,
  }));

  const sources: ProtonSource[] = editableSources.value.map((source) => ({
    id: source.id ?? null,
    family_key: source.family_key.trim(),
    provider: source.provider.trim(),
    repo: source.repo.trim(),
    endpoint: source.endpoint.trim(),
    url_template: source.url_template.trim(),
    asset_index: Number(source.asset_index) >= 0 ? Number(source.asset_index) : -1,
    asset_pattern: source.asset_pattern.trim(),
    tag_pattern: source.tag_pattern.trim(),
    max_count: Number(source.max_count) || 15,
    include_prerelease: source.include_prerelease,
    enabled: source.enabled,
    note: source.note.trim(),
  }));

  const familyKeySet = new Set<string>();
  for (const family of families) {
    if (!family.family_key || !family.display_name) {
      showMessage(tr('settings.proton_empty_fields', '家族 key 和显示名不能为空'), { title: '错误', kind: 'error' });
      return null;
    }
    if (!/^[a-zA-Z0-9_-]+$/.test(family.family_key)) {
      showMessage(tr('settings.proton_invalid_family_key', `非法 family_key: ${family.family_key}`), { title: '错误', kind: 'error' });
      return null;
    }
    const lower = family.family_key.toLowerCase();
    if (familyKeySet.has(lower)) {
      showMessage(`${tr('settings.proton_invalid_family_key', 'family_key 重复')}: ${family.family_key}`, { title: '错误', kind: 'error' });
      return null;
    }
    familyKeySet.add(lower);
  }

  for (const source of sources) {
    if (!source.family_key || !familyKeySet.has(source.family_key.toLowerCase())) {
      showMessage(`source family_key 不存在: ${source.family_key}`, { title: '错误', kind: 'error' });
      return null;
    }
    if (!source.provider) source.provider = 'github_releases';
    const needRepo = source.provider === 'github_releases';
    const needEndpoint = source.provider === 'forgejo_releases' || source.provider === 'github_actions';
    if (needRepo && !source.repo && !source.endpoint) {
      showMessage(tr('settings.proton_empty_fields', 'github_releases 需要 repo 或 endpoint'), { title: '错误', kind: 'error' });
      return null;
    }
    if (needEndpoint && !source.endpoint) {
      showMessage(tr('settings.proton_empty_fields', `${source.provider} 需要 endpoint`), { title: '错误', kind: 'error' });
      return null;
    }
    if (source.provider === 'github_actions' && !source.url_template) {
      showMessage(tr('settings.proton_empty_fields', 'github_actions 需要 url_template'), { title: '错误', kind: 'error' });
      return null;
    }
    if (!Number.isInteger(source.asset_index) || source.asset_index < -1 || source.asset_index > 100) {
      showMessage(tr('settings.proton_empty_fields', 'asset_index 必须在 -1 到 100 之间'), { title: '错误', kind: 'error' });
      return null;
    }
  }

  return { families, sources };
};

const saveCatalogChanges = async () => {
  if (isCatalogSaving.value) return;

  const payload = buildCatalogPayload();
  if (!payload) return;

  try {
    isCatalogSaving.value = true;
    await saveProtonCatalog(payload);
    await loadCatalog();
    await Promise.all([refreshLocalGrouped(), refreshRemoteGrouped()]);
    await showMessage(tr('settings.proton_editor_saved', 'Proton 目录已保存'), { title: '成功', kind: 'info' });
  } catch (e) {
    await showMessage(`保存 Proton 目录失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isCatalogSaving.value = false;
  }
};

const reloadCatalogEditor = async () => {
  await loadCatalog();
  await showMessage(tr('settings.proton_editor_reloaded', '已重载 Proton 目录'), { title: '提示', kind: 'info' });
};

const addFamily = () => {
  editableFamilies.value.push({
    family_key: '',
    display_name: '',
    enabled: true,
    sort_order: editableFamilies.value.length * 10 + 10,
    detect_patterns: [],
    detect_patterns_text: '',
    builtin: false,
  });
};

const removeFamily = (index: number) => {
  const family = editableFamilies.value[index];
  if (!family) return;
  editableSources.value = editableSources.value.filter((source) => source.family_key !== family.family_key);
  editableFamilies.value.splice(index, 1);
};

const addSource = () => {
  const defaultFamily = editableFamilies.value[0]?.family_key || '';
  editableSources.value.push({
    id: null,
    family_key: defaultFamily,
    provider: 'github_releases',
    repo: '',
    endpoint: '',
    url_template: '',
    asset_index: -1,
    asset_pattern: '(?i)\\.tar\\.(gz|xz)$',
    tag_pattern: '.*',
    max_count: 15,
    include_prerelease: false,
    enabled: true,
    note: '',
  });
};

const removeSource = (index: number) => {
  editableSources.value.splice(index, 1);
};

const installSelectedForFamily = async (familyKey: string) => {
  if (isDownloading.value) return;

  const item = selectedRemoteItem(familyKey);
  if (!item || item.installed) return;

  try {
    isDownloading.value = true;
    downloadingFamilyKey.value = familyKey;
    downloadingTag.value = item.tag;
    const message = await downloadProton(item.download_url, item.tag, familyKey);
    await showMessage(message, { title: '下载完成', kind: 'info' });
    await Promise.all([refreshLocalGrouped(), refreshRemoteGrouped()]);
  } catch (e) {
    await showMessage(`下载 Proton 失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isDownloading.value = false;
    downloadingFamilyKey.value = '';
    downloadingTag.value = '';
  }
};

const familyCards = computed(() => {
  const localMap = new Map(localGroups.value.map((group) => [group.family_key, group]));
  const remoteMap = new Map(remoteGroups.value.map((group) => [group.family_key, group]));

  return protonCatalog.value.families
    .filter((family) => family.enabled)
    .sort((a, b) => a.sort_order - b.sort_order)
    .map((family) => ({
      ...family,
      local: localMap.get(family.family_key) ?? {
        family_key: family.family_key,
        display_name: family.display_name,
        items: [],
      },
      remote: remoteMap.get(family.family_key) ?? {
        family_key: family.family_key,
        display_name: family.display_name,
        items: [],
      },
    }));
});

const selectCacheDir = async () => {
  const selected = await openFileDialog({
    directory: true,
    multiple: false,
    title: t('settings.selectcachedir')
  });

  if (selected && typeof selected === 'string') {
    appSettings.cacheDir = selected;
  }
};

const selectDataDir = async () => {
  const selected = await openFileDialog({
    directory: true,
    multiple: false,
    title: t('settings.selectdatadir')
  });

  if (selected && typeof selected === 'string') {
    appSettings.dataDir = selected;
  }
};

watch(
  () => activeMenu.value,
  async (menu) => {
    if (menu !== 'proton') return;
    if (protonLoaded.value) return;

    await loadCatalog();
    await refreshLocalGrouped();
    await refreshRemoteGrouped();
    protonLoaded.value = true;
  },
  { immediate: true }
);
</script>

<template>
  <div class="settings-layout">
    <!-- 左侧菜单 -->
    <div class="settings-menu">
      <el-menu
        :default-active="activeMenu"
        @select="(index: string) => activeMenu = index"
        class="settings-el-menu"
      >
        <el-menu-item index="basic">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg></el-icon>
          <span>{{ t('settings.basicsettings') }}</span>
        </el-menu-item>
        <el-menu-item index="appearance">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2.69l5.66 5.66a8 8 0 1 1-11.31 0z"/></svg></el-icon>
          <span>{{ t('settings.appearance') }}</span>
        </el-menu-item>
        <el-menu-item index="display">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg></el-icon>
          <span>{{ t('settings.page_display') }}</span>
        </el-menu-item>
        <el-menu-item index="proton">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 16V8a2 2 0 0 0-1-1.73l-6-3.46a2 2 0 0 0-2 0L5 6.27A2 2 0 0 0 4 8v8a2 2 0 0 0 1 1.73l6 3.46a2 2 0 0 0 2 0l6-3.46A2 2 0 0 0 20 16z"></path><polyline points="7.5 4.21 12 6.81 16.5 4.21"></polyline><polyline points="7.5 19.79 7.5 14.6 3 12"></polyline><polyline points="21 12 16.5 14.6 16.5 19.79"></polyline><polyline points="12 22.08 12 16.9 7.5 14.3"></polyline><polyline points="12 16.9 16.5 14.3"></polyline><polyline points="12 6.81 12 12"></polyline></svg></el-icon>
          <span>{{ tr('settings.proton_manage_title', 'Proton 管理') }}</span>
        </el-menu-item>
      </el-menu>
    </div>

    <!-- 右侧内容区 -->
    <div class="settings-content">
      <!-- 基础设置 -->
      <div v-show="activeMenu === 'basic'" class="settings-panel">
        <div class="panel-title">{{ t('settings.basicsettings') }}</div>
        <el-form label-width="140px">
          <el-form-item :label="t('settings.language')">
            <el-select v-model="appSettings.locale" placeholder="Select language" style="width: 200px">
              <el-option label="简体中文" value="zhs" />
              <el-option label="繁體中文" value="zht" />
              <el-option label="English" value="en" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('settings.datadir')">
            <div class="form-item-vertical">
              <div style="display: flex; gap: 10px; width: 100%;">
                <el-input v-model="appSettings.dataDir" :placeholder="t('settings.datadir_placeholder')" />
                <el-button @click="selectDataDir">{{ t('settings.selectfolder') }}</el-button>
              </div>
              <div class="form-item-hint">
                {{ t('settings.datadir_hint') }}
              </div>
            </div>
          </el-form-item>
          <el-form-item :label="t('settings.cachedir')">
            <div style="display: flex; gap: 10px; width: 100%;">
              <el-input v-model="appSettings.cacheDir" :placeholder="t('settings.cachedir_placeholder')" />
              <el-button @click="selectCacheDir">{{ t('settings.selectfolder') }}</el-button>
            </div>
          </el-form-item>
          <el-form-item :label="t('settings.github_token')">
            <el-input v-model="appSettings.githubToken" :placeholder="t('settings.github_token_placeholder')" type="password"
              show-password />
          </el-form-item>
          <el-form-item label="尘白下载源策略">
            <div class="form-item-vertical">
              <el-select v-model="appSettings.snowbreakSourcePolicy" style="width: 260px">
                <el-option label="官方优先（失败后回退社区）" value="official_first" />
                <el-option label="社区优先（失败后回退官方）" value="community_first" />
              </el-select>
              <div class="form-item-hint">
                推荐保持"官方优先"，网络异常时会自动回退到另一来源。
              </div>
            </div>
          </el-form-item>
          <el-form-item label="日志查看器">
            <div class="form-item-vertical">
              <el-button @click="openLogWindow()">打开日志窗口</el-button>
              <div class="form-item-hint">
                在新窗口中查看软件运行日志，便于排查问题时提供给开发者。
              </div>
            </div>
          </el-form-item>
        </el-form>
      </div>

      <!-- 外观设置 -->
      <div v-show="activeMenu === 'appearance'" class="settings-panel">
        <div class="panel-title">{{ t('settings.appearance') }}</div>
        <el-form label-width="140px">
          <div class="settings-divider">{{ t('settings.content_style') }}</div>
          <el-form-item :label="t('settings.opacity')">
            <el-slider v-model="appSettings.contentOpacity" :min="0" :max="1" :step="0.01" show-input />
          </el-form-item>
          <el-form-item :label="t('settings.blur')">
            <el-slider v-model="appSettings.contentBlur" :min="0" :max="50" :step="1" show-input />
          </el-form-item>
        </el-form>
      </div>

      <!-- 页面显示设置 -->
      <div v-show="activeMenu === 'display'" class="settings-panel">
        <div class="panel-title">{{ t('settings.page_display') }}</div>
        <el-form label-width="140px">
          <el-form-item :label="t('settings.modpage')">
            <el-switch v-model="appSettings.showMods" />
          </el-form-item>
          <el-form-item :label="t('settings.websitepage')">
            <el-switch v-model="appSettings.showWebsites" />
          </el-form-item>
          <el-form-item :label="t('settings.docpage')">
            <el-switch v-model="appSettings.showDocuments" />
          </el-form-item>
        </el-form>
      </div>

      <!-- Proton 管理 -->
      <div v-show="activeMenu === 'proton'" class="settings-panel proton-panel">
        <div class="panel-title">{{ tr('settings.proton_manage_title', 'Proton 管理') }}</div>

        <div class="section-block">
          <div class="section-header">
            <div class="section-title">{{ tr('settings.proton_tab_hint', '每个 Proton 家族独立管理（本地/远程双下拉）') }}</div>
            <div class="toolbar-actions">
              <el-button size="small" @click="refreshLocalGrouped" :loading="isLocalLoading">
                {{ isLocalLoading ? tr('settings.proton_refreshing', '刷新中...') : tr('settings.proton_refresh_local', '刷新本地') }}
              </el-button>
              <el-button size="small" @click="refreshRemoteGrouped" :loading="isRemoteLoading">
                {{ isRemoteLoading ? tr('settings.proton_fetching', '获取中...') : tr('settings.proton_refresh_remote', '刷新远程') }}
              </el-button>
            </div>
          </div>
          <div class="section-hint">
            {{ tr('settings.proton_local_remote_hint', '本页仅管理 Proton 版本下载与目录，不直接修改单个游戏的运行环境。') }}
          </div>

          <div v-for="family in familyCards" :key="family.family_key" class="family-card">
            <div class="family-header">
              <div class="family-title">{{ family.display_name }}</div>
              <div class="family-key">{{ family.family_key }}</div>
            </div>

            <div class="family-row">
              <div class="row-label">{{ tr('settings.proton_local_versions', '本地已安装') }}</div>
              <el-select
                v-model="selectedLocalByFamily[family.family_key]"
                :placeholder="tr('settings.proton_select_local', '选择本地版本')"
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
            <div class="row-sub" v-if="selectedLocalItem(family.family_key)">
              {{ tr('settings.proton_selected_path', '路径') }}: {{ selectedLocalItem(family.family_key)?.path }}
            </div>
            <div class="row-sub" v-else>
              {{ tr('settings.proton_no_local', '该家族暂无本地版本') }}
            </div>

            <div class="family-row" style="margin-top: 10px;">
              <div class="row-label">{{ tr('settings.proton_remote_versions', '远程可下载') }}</div>
              <el-select
                v-model="selectedRemoteByFamily[family.family_key]"
                :placeholder="tr('settings.proton_select_remote', '选择远程版本')"
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
                  !selectedRemoteItem(family.family_key)
                  || selectedRemoteItem(family.family_key)?.installed
                  || isDownloading
                "
                :loading="isDownloading && downloadingFamilyKey === family.family_key"
                @click="installSelectedForFamily(family.family_key)"
              >
                {{
                  isDownloading && downloadingFamilyKey === family.family_key
                    ? `${tr('settings.proton_downloading', '下载中...')} ${downloadingTag}`
                    : selectedRemoteItem(family.family_key)?.installed
                      ? tr('settings.proton_installed', '已安装')
                      : tr('settings.proton_download', '下载选中版本')
                }}
              </el-button>
            </div>
            <div class="row-sub" v-if="selectedRemoteItem(family.family_key)">
              {{ tr('settings.proton_selected_version', '版本') }}:
              {{ selectedRemoteItem(family.family_key)?.version }} ·
              {{ formatDate(selectedRemoteItem(family.family_key)?.published_at || '') }}
            </div>
            <div class="row-sub" v-else-if="!hasSourceForFamily(family.family_key)">
              {{ tr('settings.proton_missing_source_hint', '该家族未配置来源，请在下方目录编辑器新增 source。') }}
            </div>
            <div class="row-sub" v-else>
              {{ tr('settings.proton_no_remote', '该家族暂无远程版本（可尝试刷新远程）') }}
            </div>
          </div>
        </div>

        <div class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">{{ tr('settings.proton_catalog_editor', 'Proton 目录可视化维护') }}</div>
              <div class="section-hint">{{ tr('settings.proton_catalog_hint', '修改家族/来源后保存即可生效，无需改代码。') }}</div>
            </div>
            <div class="toolbar-actions">
              <el-button size="small" @click="reloadCatalogEditor" :loading="isCatalogLoading">
                {{ tr('settings.proton_reload_catalog', '重载目录') }}
              </el-button>
              <el-button type="primary" size="small" @click="saveCatalogChanges" :loading="isCatalogSaving">
                {{ tr('settings.proton_save_catalog', '保存目录') }}
              </el-button>
            </div>
          </div>

          <div class="editor-subtitle">{{ tr('settings.proton_families', '家族定义') }}</div>
          <div class="editor-list">
            <div v-for="(family, idx) in editableFamilies" :key="`${family.family_key}-${idx}`" class="editor-row family-editor-row">
              <el-input v-model="family.family_key" :placeholder="tr('settings.proton_family_key', 'family_key')" />
              <el-input v-model="family.display_name" :placeholder="tr('settings.proton_family_name', '显示名称')" />
              <el-input-number v-model="family.sort_order" :min="0" :max="9999" :step="10" controls-position="right" />
              <el-switch v-model="family.enabled" :active-text="tr('settings.proton_enabled', '启用')" />
              <el-switch v-model="family.builtin" :active-text="tr('settings.proton_builtin', '内置')" />
              <el-button text type="danger" @click="removeFamily(idx)">
                {{ tr('settings.proton_remove', '删除') }}
              </el-button>
              <el-input
                v-model="family.detect_patterns_text"
                type="textarea"
                :rows="2"
                :placeholder="tr('settings.proton_detect_patterns', '检测规则，每行一个（支持 regex）')"
                class="patterns-input"
              />
            </div>
            <el-button size="small" @click="addFamily">{{ tr('settings.proton_add_family', '新增家族') }}</el-button>
          </div>

          <div class="editor-subtitle" style="margin-top: 14px;">{{ tr('settings.proton_sources', '来源定义') }}</div>
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
              <el-input v-model="source.repo" :placeholder="tr('settings.proton_source_repo', 'owner/repo')" />
              <el-input v-model="source.endpoint" :placeholder="tr('settings.proton_source_endpoint', '完整 API endpoint（可选）')" />
              <el-input v-model="source.url_template" :placeholder="tr('settings.proton_source_url_template', '下载 URL 模板（actions 用，含 {id}）')" />
              <el-input-number v-model="source.asset_index" :min="-1" :max="100" controls-position="right" />
              <el-input v-model="source.asset_pattern" :placeholder="tr('settings.proton_source_asset_pattern', '资产匹配 regex')" />
              <el-input v-model="source.tag_pattern" :placeholder="tr('settings.proton_source_tag_pattern', 'tag 匹配 regex')" />
              <el-input-number v-model="source.max_count" :min="1" :max="100" controls-position="right" />
              <el-switch v-model="source.include_prerelease" :active-text="tr('settings.proton_source_prerelease', '预发布')" />
              <el-switch v-model="source.enabled" :active-text="tr('settings.proton_enabled', '启用')" />
              <el-input v-model="source.note" :placeholder="tr('settings.proton_source_note', '备注')" />
              <el-button text type="danger" @click="removeSource(idx)">
                {{ tr('settings.proton_remove', '删除') }}
              </el-button>
            </div>
            <el-button size="small" @click="addSource">{{ tr('settings.proton_add_source', '新增来源') }}</el-button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-layout {
  display: flex;
  height: 100%;
  overflow: hidden;
}

.settings-menu {
  width: 180px;
  min-width: 180px;
  border-right: 1px solid rgba(255, 255, 255, 0.08);
  overflow-y: auto;
  padding-top: 12px;
}

.settings-el-menu {
  border-right: none;
  background-color: transparent;
}

.settings-el-menu .el-menu-item {
  height: 46px;
  line-height: 46px;
  margin: 2px 8px;
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.7);
  font-size: 14px;
}

.settings-el-menu .el-menu-item:hover {
  background-color: rgba(255, 255, 255, 0.06);
  color: #fff;
}

.settings-el-menu .el-menu-item.is-active {
  background-color: rgba(64, 158, 255, 0.15);
  color: #409eff;
}

.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: 24px 28px 56px 28px;
}

.settings-panel {
  max-width: 720px;
}

.panel-title {
  font-size: 20px;
  font-weight: 600;
  color: #e0e0e0;
  margin-bottom: 24px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.form-item-vertical {
  display: flex;
  flex-direction: column;
  width: 100%;
}

.form-item-hint {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
  margin-top: 4px;
  line-height: 1.5;
}

.settings-divider {
  display: flex;
  align-items: center;
  margin: 25px 0 15px 0;
  color: #e0e0e0;
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.settings-divider::after {
  content: '';
  flex: 1;
  height: 1px;
  background: linear-gradient(to right, rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.05));
  margin-left: 15px;
}

.proton-panel {
  max-width: 1080px;
}

.section-block {
  margin-bottom: 20px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.02);
  padding: 14px;
}

.section-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.toolbar-actions {
  display: flex;
  gap: 8px;
}

.section-title {
  font-size: 15px;
  font-weight: 600;
  color: #f2f2f2;
}

.section-hint {
  margin-top: 6px;
  color: rgba(255, 255, 255, 0.55);
  font-size: 12px;
}

.family-card {
  margin-top: 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.18);
  padding: 12px;
}

.family-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 10px;
}

.family-title {
  font-size: 14px;
  color: #fff;
  font-weight: 600;
}

.family-key {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.55);
}

.family-row {
  display: grid;
  grid-template-columns: 120px 1fr auto;
  gap: 10px;
  align-items: center;
  margin-top: 10px;
}

.row-label {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.75);
}

.family-select {
  width: 100%;
}

.row-sub {
  margin-top: 6px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.58);
  word-break: break-all;
}

.remote-option-row {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
}

.remote-option-meta {
  color: rgba(255, 255, 255, 0.55);
  font-size: 12px;
}

.editor-subtitle {
  font-size: 14px;
  color: #e8e8e8;
  font-weight: 600;
  margin-bottom: 8px;
}

.editor-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.editor-row {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.14);
  padding: 10px;
  display: grid;
  gap: 8px;
}

.family-editor-row {
  grid-template-columns: 180px 200px 140px 90px 90px auto;
}

.source-editor-row {
  grid-template-columns: 190px 170px 200px 260px 260px 130px 180px 180px 130px 100px 90px 180px auto;
}

.patterns-input {
  grid-column: 1 / -1;
}

.source-family-select {
  width: 100%;
}

.provider-select {
  width: 100%;
}

@media (max-width: 1280px) {
  .family-editor-row,
  .source-editor-row {
    grid-template-columns: 1fr;
  }

  .family-row {
    grid-template-columns: 1fr;
  }
}
</style>
