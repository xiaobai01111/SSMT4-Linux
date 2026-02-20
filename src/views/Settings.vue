<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { appSettings } from '../store'
import {
  downloadDxvk,
  downloadProton,
  fetchDxvkVersions,
  fetchRemoteProtonGrouped,
  getVersionCheckInfo,
  getProtonCatalog,
  openFileDialog,
  openLogWindow,
  saveProtonCatalog,
  scanLocalDxvk,
  scanLocalProtonGrouped,
  showMessage,
  type DxvkLocalVersion,
  type DxvkRemoteVersion,
  type ProtonCatalog,
  type ProtonFamily,
  type ProtonFamilyLocalGroup,
  type ProtonFamilyRemoteGroup,
  type ProtonRemoteVersionItem,
  type ProtonSource,
  type VersionCheckInfo,
} from '../api';
import { useI18n } from 'vue-i18n';

const { t } = useI18n()

const activeMenu = ref('basic')
const versionInfo = ref<VersionCheckInfo | null>(null);
const isVersionChecking = ref(false);
const versionCheckLoaded = ref(false);

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

const checkVersionInfo = async () => {
  if (isVersionChecking.value) return;
  try {
    isVersionChecking.value = true;
    versionInfo.value = await getVersionCheckInfo();
    versionCheckLoaded.value = true;
  } catch (e) {
    await showMessage(`版本检查失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isVersionChecking.value = false;
  }
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
    showDlDialog('下载 Proton', `正在下载 ${item.tag}，请稍候...`);
    const message = await downloadProton(item.download_url, item.tag, familyKey);
    dlDialogStatus.value = 'success';
    dlDialogMessage.value = message;
    await Promise.all([refreshLocalGrouped(), refreshRemoteGrouped()]);
  } catch (e) {
    dlDialogStatus.value = 'error';
    dlDialogMessage.value = `下载 Proton 失败: ${e}`;
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

// ============================================================
// 下载弹窗状态
// ============================================================
const dlDialogVisible = ref(false);
const dlDialogTitle = ref('');
const dlDialogMessage = ref('');
const dlDialogStatus = ref<'downloading' | 'success' | 'error'>('downloading');

const showDlDialog = (title: string, message: string) => {
  dlDialogTitle.value = title;
  dlDialogMessage.value = message;
  dlDialogStatus.value = 'downloading';
  dlDialogVisible.value = true;
};

const closeDlDialog = () => {
  dlDialogVisible.value = false;
};

// ============================================================
// DXVK 管理
// ============================================================
const dxvkLocalVersions = ref<DxvkLocalVersion[]>([]);
const dxvkRemoteVersions = ref<DxvkRemoteVersion[]>([]);
const dxvkSelectedKey = ref('');  // "version|variant" 格式
const isDxvkFetching = ref(false);
const isDxvkDownloading = ref(false);
const dxvkLoaded = ref(false);

interface DxvkVersionItem {
  version: string;
  variant: string;
  key: string;       // "version|variant"
  isLocal: boolean;
  isRemote: boolean;
  fileSize: number;
  publishedAt: string;
}

const dxvkVersionList = computed<DxvkVersionItem[]>(() => {
  const map = new Map<string, DxvkVersionItem>();

  for (const rv of dxvkRemoteVersions.value) {
    const key = `${rv.version}|${rv.variant}`;
    map.set(key, {
      version: rv.version,
      variant: rv.variant,
      key,
      isLocal: rv.is_local,
      isRemote: true,
      fileSize: rv.file_size,
      publishedAt: rv.published_at,
    });
  }

  for (const lv of dxvkLocalVersions.value) {
    const key = `${lv.version}|${lv.variant}`;
    if (!map.has(key)) {
      map.set(key, {
        version: lv.version,
        variant: lv.variant,
        key,
        isLocal: true,
        isRemote: false,
        fileSize: 0,
        publishedAt: '',
      });
    }
  }

  return Array.from(map.values()).sort((a, b) => {
    const cmp = b.version.localeCompare(a.version);
    return cmp !== 0 ? cmp : a.variant.localeCompare(b.variant);
  });
});

const selectedDxvkItem = computed(() =>
  dxvkVersionList.value.find(v => v.key === dxvkSelectedKey.value)
);

const dxvkGroupedList = computed(() => [
  {
    label: 'DXVK (官方)',
    items: dxvkVersionList.value.filter(v => v.variant === 'dxvk'),
  },
  {
    label: 'DXVK-GPLAsync',
    items: dxvkVersionList.value.filter(v => v.variant === 'gplasync'),
  },
].filter(g => g.items.length > 0));

const refreshDxvkLocal = async () => {
  try {
    dxvkLocalVersions.value = await scanLocalDxvk();
  } catch (e) {
    console.warn('[dxvk] 扫描本地版本失败:', e);
  }
};

const dxvkFetchWarning = ref('');

const refreshDxvkRemote = async () => {
  if (isDxvkFetching.value) return;
  dxvkFetchWarning.value = '';
  try {
    isDxvkFetching.value = true;
    dxvkRemoteVersions.value = await fetchDxvkVersions();
    if (!dxvkSelectedKey.value && dxvkRemoteVersions.value.length > 0) {
      const first = dxvkRemoteVersions.value[0];
      dxvkSelectedKey.value = `${first.version}|${first.variant}`;
    }
    // 检查是否有缺失的 variant
    const hasDxvk = dxvkRemoteVersions.value.some(v => v.variant === 'dxvk');
    const hasGpl = dxvkRemoteVersions.value.some(v => v.variant === 'gplasync');
    const missing: string[] = [];
    if (!hasDxvk) missing.push('官方 DXVK (GitHub API 限流)');
    if (!hasGpl) missing.push('DXVK-GPLAsync');
    if (missing.length > 0) {
      dxvkFetchWarning.value = `部分版本获取失败: ${missing.join('、')}，请稍后重试。`;
    }
  } catch (e) {
    await showMessage(`获取 DXVK 版本列表失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isDxvkFetching.value = false;
  }
};

const doDownloadDxvk = async () => {
  const item = selectedDxvkItem.value;
  if (isDxvkDownloading.value || !item) return;
  try {
    isDxvkDownloading.value = true;
    const label = item.variant === 'gplasync' ? 'DXVK-GPLAsync' : 'DXVK';
    showDlDialog(`下载 ${label}`, `正在下载 ${label} ${item.version}，请稍候...`);
    const result = await downloadDxvk(item.version, item.variant);
    dlDialogStatus.value = 'success';
    dlDialogMessage.value = result;
    await refreshDxvkLocal();
    dxvkRemoteVersions.value = await fetchDxvkVersions();
  } catch (e) {
    dlDialogStatus.value = 'error';
    dlDialogMessage.value = `下载失败: ${e}`;
  } finally {
    isDxvkDownloading.value = false;
  }
};

const dxvkLocalCount = computed(() => dxvkLocalVersions.value.length);

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
    if (menu === 'version' && !versionCheckLoaded.value) {
      await checkVersionInfo();
    }
    if (menu === 'proton' && !protonLoaded.value) {
      await loadCatalog();
      await refreshLocalGrouped();
      await refreshRemoteGrouped();
      protonLoaded.value = true;
    }
    if (menu === 'dxvk' && !dxvkLoaded.value) {
      await refreshDxvkLocal();
      await refreshDxvkRemote();
      dxvkLoaded.value = true;
    }
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
        <el-menu-item index="version">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 6v6l4 2"/><circle cx="12" cy="12" r="9"/></svg></el-icon>
          <span>{{ tr('settings.version_check_title', '版本检查') }}</span>
        </el-menu-item>
        <el-menu-item index="proton">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 16V8a2 2 0 0 0-1-1.73l-6-3.46a2 2 0 0 0-2 0L5 6.27A2 2 0 0 0 4 8v8a2 2 0 0 0 1 1.73l6 3.46a2 2 0 0 0 2 0l6-3.46A2 2 0 0 0 20 16z"></path><polyline points="7.5 4.21 12 6.81 16.5 4.21"></polyline><polyline points="7.5 19.79 7.5 14.6 3 12"></polyline><polyline points="21 12 16.5 14.6 16.5 19.79"></polyline><polyline points="12 22.08 12 16.9 7.5 14.3"></polyline><polyline points="12 16.9 16.5 14.3"></polyline><polyline points="12 6.81 12 12"></polyline></svg></el-icon>
          <span>{{ tr('settings.proton_manage_title', 'Proton 管理') }}</span>
        </el-menu-item>
        <el-menu-item index="dxvk">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 20h20"/><path d="M5 20V8l7-5 7 5v12"/><path d="M9 20v-4h6v4"/><path d="M9 12h6"/><path d="M9 16h6"/></svg></el-icon>
          <span>{{ tr('settings.dxvk_manage_title', 'DXVK 管理') }}</span>
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
          <el-form-item :label="t('settings.websitepage')">
            <el-switch v-model="appSettings.showWebsites" />
          </el-form-item>
          <el-form-item :label="t('settings.docpage')">
            <el-switch v-model="appSettings.showDocuments" />
          </el-form-item>
        </el-form>
      </div>

      <!-- 版本检查 -->
      <div v-show="activeMenu === 'version'" class="settings-panel version-panel">
        <div class="panel-title">{{ tr('settings.version_check_title', '版本检查') }}</div>

        <div class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">{{ tr('settings.version_check_overview', '当前版本信息') }}</div>
              <div class="section-hint">
                {{ tr('settings.version_check_hint', '从项目根目录 version 和 version-log 读取最新版本与更新日志。') }}
              </div>
            </div>
            <div class="toolbar-actions">
              <el-button type="primary" size="small" @click="checkVersionInfo" :loading="isVersionChecking">
                {{ isVersionChecking ? tr('settings.version_checking', '检查中...') : tr('settings.version_check_action', '检查更新') }}
              </el-button>
            </div>
          </div>

          <div v-if="versionInfo" class="version-grid">
            <div class="version-row">
              <div class="version-label">{{ tr('settings.version_current', '当前版本') }}</div>
              <div class="version-value">{{ versionInfo.currentVersion }}</div>
            </div>
            <div class="version-row">
              <div class="version-label">{{ tr('settings.version_latest', '最新版本') }}</div>
              <div class="version-value">{{ versionInfo.latestVersion }}</div>
            </div>
            <div class="version-row">
              <div class="version-label">{{ tr('settings.version_status', '更新状态') }}</div>
              <div class="version-value">
                <el-tag v-if="versionInfo.hasUpdate" type="warning">{{ tr('settings.version_has_update', '有可用更新') }}</el-tag>
                <el-tag v-else type="success">{{ tr('settings.version_up_to_date', '已是最新') }}</el-tag>
              </div>
            </div>
            <div class="version-row version-log-row">
              <div class="version-label">{{ tr('settings.version_log', '更新日志') }}</div>
              <div class="version-value">
                <pre class="version-log-content">{{ versionInfo.updateLog || tr('settings.version_log_empty', '暂无更新日志') }}</pre>
              </div>
            </div>
          </div>

          <div v-else class="row-sub">
            {{ tr('settings.version_not_loaded', '尚未获取版本信息，请点击“检查更新”。') }}
          </div>
        </div>
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

      <!-- DXVK 管理 -->
      <div v-show="activeMenu === 'dxvk'" class="settings-panel dxvk-panel">
        <div class="panel-title">{{ tr('settings.dxvk_manage_title', 'DXVK 管理') }}</div>

        <div class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">DXVK (DirectX → Vulkan)</div>
              <div class="section-hint">
                {{ tr('settings.dxvk_hint', '在此下载和管理 DXVK 版本，并安装到游戏的 Wine Prefix 中。') }}
              </div>
            </div>
            <div class="toolbar-actions">
              <el-button size="small" @click="refreshDxvkLocal">
                {{ tr('settings.dxvk_refresh_local', '刷新本地') }}
              </el-button>
              <el-button size="small" @click="refreshDxvkRemote" :loading="isDxvkFetching">
                {{ isDxvkFetching ? tr('settings.dxvk_fetching', '获取中...') : tr('settings.dxvk_refresh_remote', '获取可用版本') }}
              </el-button>
            </div>
          </div>

          <!-- 本地已缓存版本 -->
          <div class="dxvk-section">
            <div class="editor-subtitle" style="margin-top: 14px;">
              {{ tr('settings.dxvk_local_title', '本地已缓存') }}
              <span class="dxvk-count">({{ dxvkLocalCount }} {{ tr('settings.dxvk_versions', '个版本') }})</span>
            </div>
            <div v-if="dxvkLocalVersions.length === 0" class="row-sub" style="margin-top: 8px;">
              {{ tr('settings.dxvk_no_local', '暂无本地缓存版本，请先获取可用版本并下载。') }}
            </div>
            <div v-else class="dxvk-local-list">
              <div v-for="lv in dxvkLocalVersions" :key="`${lv.version}|${lv.variant}`" class="dxvk-local-item">
                <div class="dxvk-local-ver">{{ lv.version }}</div>
                <el-tag v-if="lv.variant === 'gplasync'" type="warning" size="small">GPLAsync</el-tag>
                <el-tag v-if="lv.extracted" type="success" size="small">{{ tr('settings.dxvk_extracted', '已解压') }}</el-tag>
                <el-tag v-else type="info" size="small">{{ tr('settings.dxvk_archive_only', '仅存档') }}</el-tag>
                <div class="dxvk-local-path">{{ lv.path }}</div>
              </div>
            </div>
          </div>

          <!-- 版本选择 + 下载 -->
          <div class="dxvk-section" style="margin-top: 16px;">
            <div class="editor-subtitle">{{ tr('settings.dxvk_download_title', '下载 DXVK 版本') }}</div>
            <div v-if="dxvkFetchWarning" class="dxvk-fetch-warning" style="margin-bottom: 8px;">
              ⚠ {{ dxvkFetchWarning }}
            </div>
            <div class="dxvk-download-row">
              <el-select
                v-model="dxvkSelectedKey"
                :placeholder="tr('settings.dxvk_select_version', '选择版本...')"
                class="dxvk-version-select"
                filterable
              >
                <el-option-group
                  v-for="group in dxvkGroupedList"
                  :key="group.label"
                  :label="group.label"
                >
                  <el-option
                    v-for="v in group.items"
                    :key="v.key"
                    :label="`${v.variant === 'gplasync' ? '[GPLAsync] ' : ''}${v.version}${v.isLocal ? ' [本地]' : ''}${v.fileSize > 0 ? ` (${formatBytes(v.fileSize)})` : ''}`"
                    :value="v.key"
                  >
                    <div class="remote-option-row">
                      <span>
                        {{ v.version }}
                        <el-tag v-if="v.isLocal" type="success" size="small" style="margin-left: 6px;">{{ tr('settings.dxvk_cached', '已缓存') }}</el-tag>
                      </span>
                      <span class="remote-option-meta">
                        {{ v.fileSize > 0 ? formatBytes(v.fileSize) : '' }}
                        {{ v.publishedAt ? `· ${formatDate(v.publishedAt)}` : '' }}
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
                    ? tr('settings.dxvk_downloading', '下载中...')
                    : selectedDxvkItem?.isLocal
                      ? tr('settings.dxvk_already_cached', '已缓存')
                      : tr('settings.dxvk_download', '下载')
                }}
              </el-button>
            </div>
          </div>
        </div>
      </div>

    </div>

    <!-- 下载进度弹窗 -->
    <el-dialog
      v-model="dlDialogVisible"
      :title="dlDialogTitle"
      width="420px"
      :close-on-click-modal="dlDialogStatus !== 'downloading'"
      :close-on-press-escape="dlDialogStatus !== 'downloading'"
      :show-close="dlDialogStatus !== 'downloading'"
      align-center
    >
      <div class="dl-dialog-body">
        <div v-if="dlDialogStatus === 'downloading'" class="dl-dialog-loading">
          <el-icon class="dl-dialog-spinner"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="2" x2="12" y2="6"/><line x1="12" y1="18" x2="12" y2="22"/><line x1="4.93" y1="4.93" x2="7.76" y2="7.76"/><line x1="16.24" y1="16.24" x2="19.07" y2="19.07"/><line x1="2" y1="12" x2="6" y2="12"/><line x1="18" y1="12" x2="22" y2="12"/><line x1="4.93" y1="19.07" x2="7.76" y2="16.24"/><line x1="16.24" y1="7.76" x2="19.07" y2="4.93"/></svg></el-icon>
          <div class="dl-dialog-text">{{ dlDialogMessage }}</div>
        </div>
        <div v-else-if="dlDialogStatus === 'success'" class="dl-dialog-result dl-dialog-success">
          <el-icon style="font-size: 32px; color: #67c23a;"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg></el-icon>
          <div class="dl-dialog-text">{{ dlDialogMessage }}</div>
        </div>
        <div v-else class="dl-dialog-result dl-dialog-error">
          <el-icon style="font-size: 32px; color: #f56c6c;"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg></el-icon>
          <div class="dl-dialog-text">{{ dlDialogMessage }}</div>
        </div>
      </div>
      <template #footer>
        <el-button v-if="dlDialogStatus !== 'downloading'" type="primary" @click="closeDlDialog">
          确定
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.settings-layout {
  display: flex;
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: relative;
  /* Tech Glass Wrapper */
  background: rgba(10, 15, 20, 0.75);
  backdrop-filter: blur(12px);
}

.settings-menu {
  width: 220px;
  min-width: 220px;
  border-right: 1px solid rgba(0, 240, 255, 0.3); /* Tech Cyan Line */
  background: rgba(0, 5, 10, 0.4);
  overflow-y: auto;
  padding: 24px 12px;
}

.settings-el-menu {
  border-right: none;
  background-color: transparent;
}

.settings-el-menu .el-menu-item {
  height: 48px;
  line-height: 48px;
  margin: 4px 0;
  border-radius: 4px; /* Sharp */
  color: rgba(255, 255, 255, 0.65);
  font-size: 14px;
  font-weight: 500;
  transition: all 0.2s ease;
  position: relative;
  overflow: hidden;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.settings-el-menu .el-menu-item:hover {
  background-color: rgba(0, 240, 255, 0.1);
  color: #fff;
}

.settings-el-menu .el-menu-item.is-active {
  background-color: rgba(0, 240, 255, 0.15);
  color: #00f0ff; /* Glowing cyan text */
  font-weight: 600;
  text-shadow: 0 0 10px rgba(0, 240, 255, 0.5);
  box-shadow: inset 4px 0 0 #00f0ff; /* Sharp cyan left marker */
}

.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: 32px 40px 60px 40px;
}

.settings-panel {
  max-width: 800px;
  animation: fadeIn 0.4s ease-out;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.panel-title {
  font-size: 24px;
  font-weight: 600;
  color: #00f0ff; /* Tech cyan */
  margin-bottom: 32px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
  letter-spacing: 1px;
  text-transform: uppercase;
  text-shadow: 0 0 12px rgba(0, 240, 255, 0.4);
}

.form-item-vertical {
  display: flex;
  flex-direction: column;
  width: 100%;
}

.form-item-hint {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.45);
  margin-top: 6px;
  line-height: 1.5;
}

.settings-divider {
  display: flex;
  align-items: center;
  margin: 30px 0 20px 0;
  color: #fff;
  font-size: 15px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.settings-divider::after {
  content: '';
  flex: 1;
  height: 1px;
  background: linear-gradient(to right, rgba(255, 255, 255, 0.15), transparent);
  margin-left: 20px;
}

.proton-panel {
  max-width: 1100px;
}

/* Glass Panels / Cards / Tech blocks */
.section-block, .family-card, .dxvk-status-card {
  margin-bottom: 24px;
  border: 1px solid rgba(0, 240, 255, 0.2);
  border-radius: 4px; /* Sharper */
  background: rgba(10, 15, 20, 0.6); /* Solid translucent */
  box-shadow: 0 4px 20px rgba(0, 240, 255, 0.05), inset 0 0 10px rgba(0, 240, 255, 0.02);
  padding: 20px;
  transition: transform 0.2s ease, box-shadow 0.2s ease;
}

.section-block:hover, .family-card:hover {
  background: rgba(15, 20, 25, 0.8);
  box-shadow: 0 8px 30px rgba(0, 240, 255, 0.1), inset 0 1px 0 rgba(0, 240, 255, 0.2);
}

.section-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.toolbar-actions {
  display: flex;
  gap: 12px;
}

.section-title {
  font-size: 16px;
  font-weight: 600;
  color: #fff;
}

.section-hint {
  margin-top: 6px;
  color: rgba(255, 255, 255, 0.55);
  font-size: 13px;
  line-height: 1.5;
}

.version-panel {
  max-width: 900px;
}

.version-grid {
  margin-top: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.version-row {
  display: grid;
  grid-template-columns: 140px 1fr;
  gap: 16px;
  align-items: start;
}

.version-label {
  font-size: 14px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.7);
}

.version-value {
  font-size: 14px;
  color: #fff;
}

.version-log-row {
  align-items: stretch;
}

.version-log-content {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 13px;
  line-height: 1.6;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  padding: 16px;
  color: rgba(255, 255, 255, 0.85);
  font-family: 'Fira Code', monospace;
}

.family-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 12px;
}

.family-title {
  font-size: 16px;
  color: #fff;
  font-weight: 600;
  text-shadow: 0 0 8px rgba(255, 255, 255, 0.2);
}

.family-key {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.45);
  background: rgba(0, 0, 0, 0.3);
  padding: 2px 8px;
  border-radius: 12px;
}

.family-row {
  display: grid;
  grid-template-columns: 130px 1fr auto;
  gap: 12px;
  align-items: center;
  margin-top: 16px;
}

.row-label {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.75);
}

.family-select {
  width: 100%;
}

.row-sub {
  margin-top: 8px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
  word-break: break-all;
}

.remote-option-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
}

.remote-option-meta {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
}

.editor-subtitle {
  font-size: 15px;
  color: #fff;
  font-weight: 600;
  margin-bottom: 12px;
}

.editor-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.editor-row {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  padding: 12px;
  display: grid;
  gap: 12px;
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

.source-family-select, .provider-select {
  width: 100%;
}

.dxvk-panel {
  max-width: 900px;
}

.dxvk-section {
  margin-top: 12px;
}

.dxvk-local-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 12px;
}

.dxvk-local-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  transition: background 0.2s;
}

.dxvk-local-item:hover {
  background: rgba(255, 255, 255, 0.05);
}

.dxvk-local-ver {
  font-size: 15px;
  font-weight: 600;
  color: #fff;
  min-width: 70px;
}

.dxvk-local-path {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.45);
  word-break: break-all;
  flex: 1;
}

.dxvk-count {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
  font-weight: 400;
  margin-left: 8px;
}

.dxvk-download-row {
  display: flex;
  gap: 12px;
  align-items: center;
}

.dxvk-version-select {
  flex: 1;
}

.dxvk-status-row {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 14px;
  color: #fff;
}

.dxvk-status-label {
  min-width: 80px;
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
}

.text-ok { color: #67c23a; font-weight: 600;}
.text-err { color: #f56c6c; font-weight: 600;}

/* Dialog Customization */
.dl-dialog-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 20px 0;
}

.dl-dialog-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 20px;
}

.dl-dialog-spinner {
  font-size: 40px;
  color: #fff;
  animation: dl-spin 1.2s cubic-bezier(0.5, 0, 0.5, 1) infinite;
  text-shadow: 0 0 15px rgba(255,255,255,0.5);
}

@keyframes dl-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.dl-dialog-result {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.dl-dialog-text {
  font-size: 15px;
  color: #ececec;
  text-align: center;
  line-height: 1.6;
  word-break: break-all;
}

.dxvk-fetch-warning {
  font-size: 13px;
  color: #e6a23c;
  background: rgba(230, 162, 60, 0.15);
  border: 1px solid rgba(230, 162, 60, 0.3);
  border-radius: 6px;
  padding: 8px 12px;
}

/* 
  Deep customization for Element Plus components inside settings 
  to match the Bright Tech HUD theme.
*/
:deep(.el-form-item__label) {
  color: rgba(255, 255, 255, 0.85);
  font-weight: 500;
}

:deep(.el-input__wrapper), :deep(.el-select__wrapper) {
  background-color: rgba(0, 0, 0, 0.5) !important;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.2) inset !important;
  border-radius: 4px; /* Sharp corners */
  transition: all 0.2s;
}

:deep(.el-input__wrapper:hover), :deep(.el-select__wrapper:hover) {
  box-shadow: 0 0 0 1px rgba(0, 240, 255, 0.5) inset !important;
}

:deep(.el-input__wrapper.is-focus), :deep(.el-select__wrapper.is-focus) {
  box-shadow: 0 0 0 1px #00f0ff inset, 0 0 10px rgba(0, 240, 255, 0.3) !important;
  background-color: rgba(0, 240, 255, 0.05) !important;
}

:deep(.el-input__inner) {
  color: #fff !important;
}

/* Switches as Mechanical Toggles */
:deep(.el-switch__core) {
  background-color: rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 4px;
}
:deep(.el-switch.is-checked .el-switch__core) {
  background-color: #00f0ff;
  border-color: #00f0ff;
  box-shadow: 0 0 10px rgba(0, 240, 255, 0.4);
}
:deep(.el-switch.is-checked .el-switch__core .el-switch__action) {
  background-color: #000;
  border-radius: 2px;
}
:deep(.el-switch__core .el-switch__action) {
  border-radius: 2px;
}

/* Tech Buttons */
:deep(.el-button) {
  background-color: rgba(0, 240, 255, 0.05);
  border: 1px solid rgba(0, 240, 255, 0.5);
  color: #00f0ff;
  border-radius: 4px;
  transition: all 0.2s ease;
  text-transform: uppercase;
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

:deep(.el-button:hover:not(.is-disabled)) {
  background-color: #00f0ff;
  color: #000;
  box-shadow: 0 0 15px rgba(0, 240, 255, 0.6);
  border-color: #00f0ff;
}

:deep(.el-button--primary) {
  background-color: rgba(0, 240, 255, 0.2);
  color: #00f0ff;
  border: 1px solid #00f0ff;
  box-shadow: inset 0 0 8px rgba(0, 240, 255, 0.3);
}

:deep(.el-button--primary:hover:not(.is-disabled)) {
  background-color: #00f0ff;
  color: #000;
  box-shadow: 0 0 20px rgba(0, 240, 255, 0.8);
}

:deep(.el-button.is-disabled) {
  background-color: rgba(255, 255, 255, 0.05);
  color: rgba(255, 255, 255, 0.2);
  border-color: rgba(255, 255, 255, 0.1);
}

@media (max-width: 1280px) {
  .family-editor-row,
  .source-editor-row,
  .family-row,
  .version-row {
    grid-template-columns: 1fr;
    gap: 12px;
  }
}
</style>
