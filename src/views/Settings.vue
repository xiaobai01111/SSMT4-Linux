<script setup lang="ts">
import { computed, inject, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { appSettings, startFeatureOnboarding } from '../store'
import {
  deleteLocalDxvk,
  deleteLocalProton,
  deleteLocalVkd3d,
  downloadDxvk,
  downloadVkd3d,
  downloadProton,
  fetchDxvkVersions,
  fetchVkd3dVersions,
  fetchRemoteProtonGrouped,
  getResourceVersionInfo,
  getVersionCheckInfo,
  getProtonCatalog,
  openFileDialog,
  openLogWindow,
  pullResourceUpdates,
  saveProtonCatalog,
  scanLocalDxvk,
  scanLocalVkd3d,
  scanLocalProtonGrouped,
  showMessage,
  type DxvkLocalVersion,
  type DxvkRemoteVersion,
  type Vkd3dLocalVersion,
  type Vkd3dRemoteVersion,
  type ProtonCatalog,
  type ProtonFamily,
  type ProtonFamilyLocalGroup,
  type ProtonFamilyRemoteGroup,
  type ProtonRemoteVersionItem,
  type ProtonSource,
  type VersionCheckInfo,
  type GameInfo,
  getXxmiPackageSources,
  scanLocalXxmiPackages,
  fetchXxmiRemoteVersions,
  downloadXxmiPackage,
  deployXxmiPackage,
  deleteLocalXxmiPackage,
  type XxmiPackageSource,
  type XxmiRemoteVersion,
  type XxmiLocalPackage,
} from '../api';
import { useI18n } from 'vue-i18n';
import { useSettingsResourceTasks } from '../composables/useSettingsResourceTasks';
import { useSettingsMigotoConfig } from '../composables/useSettingsMigotoConfig';
import { NOTIFY_KEY } from '../types/notify';

const { t, te } = useI18n()
const route = useRoute();
const router = useRouter();
const notify = inject(NOTIFY_KEY, null);

const activeMenu = ref('basic')
const guideMenu = ref('');
let guideMenuTimer: ReturnType<typeof setTimeout> | null = null;

const VALID_MENUS = new Set([
  'basic',
  'appearance',
  'display',
  'version',
  'resource',
  'proton',
  'dxvk',
  'vkd3d',
  'migoto',
]);

const applyMenuFromRoute = () => {
  const menu = String(route.query.menu || '').trim();
  const guide = String(route.query.guide || '').trim();
  if (VALID_MENUS.has(menu)) {
    activeMenu.value = menu;
    if (guide === '1') {
      guideMenu.value = menu;
      if (guideMenuTimer) {
        clearTimeout(guideMenuTimer);
      }
      guideMenuTimer = setTimeout(() => {
        guideMenu.value = '';
        guideMenuTimer = null;
      }, 2600);
    }
  }
};
const versionInfo = ref<VersionCheckInfo | null>(null);
const isVersionChecking = ref(false);
const versionCheckLoaded = ref(false);
const resourceInfo = ref<VersionCheckInfo | null>(null);
const isResourceChecking = ref(false);
const isResourcePulling = ref(false);
const resourceCheckLoaded = ref(false);

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
const showProtonCatalogEditor = ref(false);

const selectedLocalByFamily = reactive<Record<string, string>>({});
const selectedRemoteByFamily = reactive<Record<string, string>>({});

type EditableProtonFamily = ProtonFamily & { detect_patterns_text: string };
const editableFamilies = ref<EditableProtonFamily[]>([]);
const editableSources = ref<ProtonSource[]>([]);

const tr = (key: string, fallback: string, params?: Record<string, unknown>) => {
  if (!te(key)) return fallback;
  return params ? t(key, params) : t(key);
};
const { runDownloadTask, runDeleteTask } = useSettingsResourceTasks(tr);

const toast = async (
  kind: 'success' | 'warning' | 'info' | 'error',
  title: string,
  message: string,
) => {
  const handler = notify?.[kind];
  if (typeof handler === 'function') {
    handler(title, message);
    return;
  }
  await showMessage(message, { title, kind });
};

const checkVersionInfo = async () => {
  if (isVersionChecking.value) return;
  try {
    isVersionChecking.value = true;
    versionInfo.value = await getVersionCheckInfo();
    versionCheckLoaded.value = true;
  } catch (e) {
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.versionCheckFailed', `版本检查失败: ${e}`));
  } finally {
    isVersionChecking.value = false;
  }
};

const checkResourceInfo = async () => {
  if (isResourceChecking.value) return;
  try {
    isResourceChecking.value = true;
    resourceInfo.value = await getResourceVersionInfo();
    resourceCheckLoaded.value = true;
  } catch (e) {
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.resourceCheckFailed', `资源检查失败: ${e}`));
  } finally {
    isResourceChecking.value = false;
  }
};

const pullResources = async () => {
  if (isResourcePulling.value) return;
  try {
    isResourcePulling.value = true;
    const msg = await pullResourceUpdates();
    await checkResourceInfo();
    await toast('info', tr('settings.resource.title', '资源更新'), msg);
  } catch (e) {
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.resourcePullFailed', `拉取资源失败: ${e}`));
  } finally {
    isResourcePulling.value = false;
  }
};

const reenterOnboarding = async () => {
  startFeatureOnboarding(0);
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

const protonFamilyLabel = (familyKey: string) => {
  const family = protonCatalog.value.families.find((item) => item.family_key === familyKey);
  return family?.display_name || familyKey;
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
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.protonCatalogLoadFailed', `加载 Proton 目录失败: ${e}`));
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
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.protonLocalFetchFailed', `获取本地 Proton 失败: ${e}`));
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
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.protonRemoteFetchFailed', `获取远程 Proton 列表失败: ${e}`));
  } finally {
    isRemoteLoading.value = false;
  }
};

const refreshProtonState = async () => {
  await Promise.all([refreshLocalGrouped(), refreshRemoteGrouped()]);
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
      toast('error', tr('settings.messages.title.error', '错误'), tr('settings.proton_empty_fields', '家族 key 和显示名不能为空'));
      return null;
    }
    if (!/^[a-zA-Z0-9_-]+$/.test(family.family_key)) {
      toast('error', tr('settings.messages.title.error', '错误'), tr('settings.proton_invalid_family_key', `非法 family_key: ${family.family_key}`));
      return null;
    }
    const lower = family.family_key.toLowerCase();
    if (familyKeySet.has(lower)) {
      toast('error', tr('settings.messages.title.error', '错误'), `${tr('settings.proton_invalid_family_key', 'family_key 重复')}: ${family.family_key}`);
      return null;
    }
    familyKeySet.add(lower);
  }

  for (const source of sources) {
    if (!source.family_key || !familyKeySet.has(source.family_key.toLowerCase())) {
      toast('error', tr('settings.messages.title.error', '错误'), `source family_key 不存在: ${source.family_key}`);
      return null;
    }
    if (!source.provider) source.provider = 'github_releases';
    const needRepo = source.provider === 'github_releases';
    const needEndpoint = source.provider === 'forgejo_releases' || source.provider === 'github_actions';
    if (needRepo && !source.repo && !source.endpoint) {
      toast('error', tr('settings.messages.title.error', '错误'), tr('settings.proton_empty_fields', 'github_releases 需要 repo 或 endpoint'));
      return null;
    }
    if (needEndpoint && !source.endpoint) {
      toast('error', tr('settings.messages.title.error', '错误'), tr('settings.proton_empty_fields', `${source.provider} 需要 endpoint`));
      return null;
    }
    if (source.provider === 'github_actions' && !source.url_template) {
      toast('error', tr('settings.messages.title.error', '错误'), tr('settings.proton_empty_fields', 'github_actions 需要 url_template'));
      return null;
    }
    if (!Number.isInteger(source.asset_index) || source.asset_index < -1 || source.asset_index > 100) {
      toast('error', tr('settings.messages.title.error', '错误'), tr('settings.proton_empty_fields', 'asset_index 必须在 -1 到 100 之间'));
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
    await toast('success', tr('settings.messages.title.success', '成功'), tr('settings.proton_editor_saved', 'Proton 目录已保存'));
  } catch (e) {
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.protonCatalogSaveFailed', `保存 Proton 目录失败: ${e}`));
  } finally {
    isCatalogSaving.value = false;
  }
};

const reloadCatalogEditor = async () => {
  await loadCatalog();
  await toast('info', tr('settings.messages.title.info', '提示'), tr('settings.proton_editor_reloaded', '已重载 Proton 目录'));
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

const deletingProtonIds = reactive<Record<string, boolean>>({});
const deletingDxvkKeys = reactive<Record<string, boolean>>({});
const deletingVkd3dVersions = reactive<Record<string, boolean>>({});

const normalizeFsPath = (value: string) =>
  value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();

const normalizedDataDir = computed(() => {
  const value = String(appSettings.dataDir || '').trim();
  return value ? normalizeFsPath(value) : '';
});

const managedProtonRoots = computed(() => {
  const baseDir = normalizedDataDir.value;
  if (!baseDir) return [];

  const roots = [`${baseDir}/proton`, `${baseDir}/runners/wine`].map(
    normalizeFsPath,
  );
  return Array.from(new Set(roots));
});

const isManagedProtonItem = (item: { path: string } | null | undefined) => {
  if (!item?.path) return false;
  const path = normalizeFsPath(item.path);
  return managedProtonRoots.value.some((root) => path === root || path.startsWith(`${root}/`));
};

const installSelectedForFamily = async (familyKey: string) => {
  if (isDownloading.value) return;

  const item = selectedRemoteItem(familyKey);
  if (!item || item.installed) return;

  const taskId = `settings-proton-download-${familyKey}`;
  const familyLabel = protonFamilyLabel(familyKey);
  const componentKey = `${familyKey.startsWith('Wine') ? 'proton:wine' : 'proton:proton'}:${item.tag}`;
  try {
    isDownloading.value = true;
    downloadingFamilyKey.value = familyKey;
    downloadingTag.value = item.tag;
    await runDownloadTask({
      taskId,
      componentKey,
      title: tr('settings.messages.downloadLabelTitle', `下载 ${familyLabel}`).replace('{label}', familyLabel),
      pendingMessage: tr('settings.messages.downloadLabelBody', `正在下载 ${familyLabel} ${item.tag}，请稍候...`)
        .replace('{label}', familyLabel)
        .replace('{version}', item.tag),
      run: () => downloadProton(item.download_url, item.tag, familyKey),
      successMessage: tr('settings.messages.downloadLabelDone', `${familyLabel} ${item.tag} 下载完成`)
        .replace('{label}', familyLabel)
        .replace('{version}', item.tag),
      errorMessage: (e) =>
        tr('settings.messages.downloadFailed', `下载失败: ${e}`).replace('{error}', String(e)),
      refresh: refreshProtonState,
    });
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

const removeLocalProtonItem = async (item: { id: string; path: string; name: string }) => {
  if (!isManagedProtonItem(item) || deletingProtonIds[item.id]) return;
  const taskId = `settings-proton-delete-${item.id}`;
  const target = item.name || item.path;
  try {
    deletingProtonIds[item.id] = true;
    await runDeleteTask({
      taskId,
      run: () => deleteLocalProton(item.path),
      successMessage: tr('settings.messages.deleteTargetDone', `${target} 已删除`).replace('{target}', target),
      errorMessage: (e) =>
        tr('settings.messages.deleteFailed', `删除失败: ${e}`).replace('{error}', String(e)),
      refresh: refreshProtonState,
    });
  } finally {
    deletingProtonIds[item.id] = false;
  }
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

const dxvkVariantLabel = (variant: string) => {
  const labels: Record<string, string> = {
    dxvk: tr('settings.dxvk_variant_official', 'Official DXVK'),
    gplasync: 'DXVK-GPLAsync',
    async: 'DXVK-Async',
    sarek: 'DXVK-Sarek',
    sarekasync: 'DXVK-Sarek-Async',
  };
  return labels[variant] || `DXVK-${variant}`;
};

const dxvkVariantShortLabel = (variant: string) => {
  const labels: Record<string, string> = {
    dxvk: tr('settings.dxvk_variant_short_official', 'Official'),
    gplasync: 'GPLAsync',
    async: 'Async',
    sarek: 'Sarek',
    sarekasync: 'Sarek-Async',
  };
  return labels[variant] || variant;
};

const dxvkGroupedList = computed(() => {
  const groups = new Map<string, DxvkVersionItem[]>();
  for (const item of dxvkVersionList.value) {
    const list = groups.get(item.variant) || [];
    list.push(item);
    groups.set(item.variant, list);
  }
  return Array.from(groups.entries())
    .sort((a, b) => {
      if (a[0] === 'dxvk') return -1;
      if (b[0] === 'dxvk') return 1;
      return a[0].localeCompare(b[0]);
    })
    .map(([variant, items]) => ({
      variant,
      label: dxvkVariantLabel(variant),
      items,
    }));
});

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
    if (dxvkRemoteVersions.value.length === 0) {
      dxvkFetchWarning.value = tr('settings.messages.dxvkFetchWarning', '未获取到远程版本，请稍后重试。');
    }
  } catch (e) {
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.dxvkFetchFailed', `获取 DXVK 版本列表失败: ${e}`).replace('{error}', String(e)));
  } finally {
    isDxvkFetching.value = false;
  }
};

const refreshDxvkState = async () => {
  const [, remote] = await Promise.all([refreshDxvkLocal(), fetchDxvkVersions()]);
  dxvkRemoteVersions.value = remote;
};

const doDownloadDxvk = async () => {
  const item = selectedDxvkItem.value;
  if (isDxvkDownloading.value || !item) return;
  const label = dxvkVariantLabel(item.variant);
  const taskId = `settings-dxvk-download-${item.variant}-${item.version}`;
  const componentKey = `dxvk:${item.variant}:${item.version}`;
  try {
    isDxvkDownloading.value = true;
    await runDownloadTask({
      taskId,
      componentKey,
      title: tr('settings.messages.downloadLabelTitle', `下载 ${label}`).replace('{label}', label),
      pendingMessage: tr('settings.messages.downloadLabelBody', `正在下载 ${label} ${item.version}，请稍候...`).replace('{label}', label).replace('{version}', item.version),
      run: () => downloadDxvk(item.version, item.variant),
      successMessage: tr('settings.messages.downloadLabelDone', `${label} ${item.version} 下载完成`)
        .replace('{label}', label)
        .replace('{version}', item.version),
      errorMessage: (e) =>
        tr('settings.messages.downloadFailed', `下载失败: ${e}`).replace('{error}', String(e)),
      refresh: refreshDxvkState,
    });
  } finally {
    isDxvkDownloading.value = false;
  }
};

const dxvkLocalCount = computed(() => dxvkLocalVersions.value.length);

const removeLocalDxvkItem = async (version: string, variant: string) => {
  const key = `${version}|${variant}`;
  if (deletingDxvkKeys[key]) return;
  const label = dxvkVariantLabel(variant);
  const taskId = `settings-dxvk-delete-${variant}-${version}`;
  const target = `${label} ${version}`;
  try {
    deletingDxvkKeys[key] = true;
    await runDeleteTask({
      taskId,
      run: () => deleteLocalDxvk(version, variant),
      successMessage: tr('settings.messages.deleteTargetDone', `${target} 已删除`).replace('{target}', target),
      errorMessage: (e) =>
        tr('settings.messages.deleteFailed', `删除失败: ${e}`).replace('{error}', String(e)),
      refresh: refreshDxvkState,
    });
  } finally {
    deletingDxvkKeys[key] = false;
  }
};

// ============================================================
// VKD3D 管理
// ============================================================
const vkd3dLocalVersions = ref<Vkd3dLocalVersion[]>([]);
const vkd3dRemoteVersions = ref<Vkd3dRemoteVersion[]>([]);
const vkd3dSelectedVersion = ref('');
const isVkd3dFetching = ref(false);
const isVkd3dDownloading = ref(false);
const vkd3dLoaded = ref(false);
const vkd3dFetchWarning = ref('');

interface Vkd3dVersionItem {
  version: string;
  isLocal: boolean;
  isRemote: boolean;
  fileSize: number;
  publishedAt: string;
}

const vkd3dVersionList = computed<Vkd3dVersionItem[]>(() => {
  const map = new Map<string, Vkd3dVersionItem>();

  for (const rv of vkd3dRemoteVersions.value) {
    map.set(rv.version, {
      version: rv.version,
      isLocal: rv.is_local,
      isRemote: true,
      fileSize: rv.file_size,
      publishedAt: rv.published_at,
    });
  }

  for (const lv of vkd3dLocalVersions.value) {
    if (!map.has(lv.version)) {
      map.set(lv.version, {
        version: lv.version,
        isLocal: true,
        isRemote: false,
        fileSize: 0,
        publishedAt: '',
      });
    }
  }

  return Array.from(map.values()).sort((a, b) => b.version.localeCompare(a.version));
});

const selectedVkd3dItem = computed(() =>
  vkd3dVersionList.value.find(v => v.version === vkd3dSelectedVersion.value)
);

const refreshVkd3dLocal = async () => {
  try {
    vkd3dLocalVersions.value = await scanLocalVkd3d();
  } catch (e) {
    console.warn('[vkd3d] 扫描本地版本失败:', e);
  }
};

const refreshVkd3dRemote = async () => {
  if (isVkd3dFetching.value) return;
  vkd3dFetchWarning.value = '';
  try {
    isVkd3dFetching.value = true;
    vkd3dRemoteVersions.value = await fetchVkd3dVersions();
    if (!vkd3dSelectedVersion.value && vkd3dRemoteVersions.value.length > 0) {
      vkd3dSelectedVersion.value = vkd3dRemoteVersions.value[0].version;
    }
    if (vkd3dRemoteVersions.value.length === 0) {
      vkd3dFetchWarning.value = tr('settings.messages.vkd3dFetchWarning', '未获取到远程版本，请稍后重试。');
    }
  } catch (e) {
    await toast('error', tr('settings.messages.title.error', '错误'), tr('settings.messages.vkd3dFetchFailed', `获取 VKD3D 版本列表失败: ${e}`).replace('{error}', String(e)));
  } finally {
    isVkd3dFetching.value = false;
  }
};

const refreshVkd3dState = async () => {
  const [, remote] = await Promise.all([refreshVkd3dLocal(), fetchVkd3dVersions()]);
  vkd3dRemoteVersions.value = remote;
};

const doDownloadVkd3d = async () => {
  const item = selectedVkd3dItem.value;
  if (isVkd3dDownloading.value || !item) return;
  const taskId = `settings-vkd3d-download-${item.version}`;
  const label = 'VKD3D-Proton';
  const componentKey = `vkd3d:${item.version}`;
  try {
    isVkd3dDownloading.value = true;
    await runDownloadTask({
      taskId,
      componentKey,
      title: tr('settings.messages.downloadLabelTitle', `下载 ${label}`).replace('{label}', label),
      pendingMessage: tr('settings.messages.downloadLabelBody', `正在下载 ${label} ${item.version}，请稍候...`)
        .replace('{label}', label)
        .replace('{version}', item.version),
      run: () => downloadVkd3d(item.version),
      successMessage: tr('settings.messages.downloadLabelDone', `${label} ${item.version} 下载完成`)
        .replace('{label}', label)
        .replace('{version}', item.version),
      errorMessage: (e) =>
        tr('settings.messages.downloadFailed', `下载失败: ${e}`).replace('{error}', String(e)),
      refresh: refreshVkd3dState,
    });
  } finally {
    isVkd3dDownloading.value = false;
  }
};

const vkd3dLocalCount = computed(() => vkd3dLocalVersions.value.length);

const removeLocalVkd3dItem = async (version: string) => {
  if (deletingVkd3dVersions[version]) return;
  const taskId = `settings-vkd3d-delete-${version}`;
  const target = `VKD3D-Proton ${version}`;
  try {
    deletingVkd3dVersions[version] = true;
    await runDeleteTask({
      taskId,
      run: () => deleteLocalVkd3d(version),
      successMessage: tr('settings.messages.deleteTargetDone', `${target} 已删除`).replace('{target}', target),
      errorMessage: (e) =>
        tr('settings.messages.deleteFailed', `删除失败: ${e}`).replace('{error}', String(e)),
      refresh: refreshVkd3dState,
    });
  } finally {
    deletingVkd3dVersions[version] = false;
  }
};

// ============================================================
// 3DMIGOTO 管理
// ============================================================
const {
  ensureMigotoLoaded,
  globalMigotoEnabled,
  handleMigotoGlobalToggle,
  isMigotoImporterLocked,
  isMigotoInjectionLocked,
  isMigotoPathOverridden,
  isMigotoSaving,
  isMigotoTogglePending,
  isMigotoWwmi,
  getMigotoAutoPathDescription,
  getMigotoPathDisplayValue,
  loadMigotoGameConfig,
  migotoAvailableImporterOptions,
  migotoConfig,
  migotoGamesList,
  migotoImporterHint,
  migotoInjectionHint,
  migotoRiskStatement,
  migotoSelectedGame,
  migotoStartArgsHint,
  restoreMigotoPathAuto,
  saveMigotoGameConfig,
  selectMigotoPath,
  showMigotoRiskRestatement,
} = useSettingsMigotoConfig({ t, tr, toast });

const openDocumentsDoc = async (docId: string) => {
  if (!appSettings.showDocuments) {
    appSettings.showDocuments = true;
  }
  await router.push({
    name: 'Documents',
    query: {
      doc: docId,
    },
  });
};

const getLocalizedGameName = (game: Pick<GameInfo, 'name'> | string) => {
  const gameName = typeof game === 'string' ? game : game.name;
  const fallback = gameName;
  return te(`games.${gameName}`) ? t(`games.${gameName}`) : fallback;
};

// ============================================================
// XXMI 资源包下载管理
// ============================================================
const xxmiSources = ref<XxmiPackageSource[]>([]);
const xxmiSelectedSource = ref('xxmi-libs');
const xxmiRemoteVersions = ref<XxmiRemoteVersion[]>([]);
const xxmiLocalPackages = ref<XxmiLocalPackage[]>([]);
const isXxmiFetching = ref(false);
const isXxmiDownloading = ref(false);
const xxmiDownloadingVersion = ref('');
const xxmiMessage = ref('');
const xxmiMessageType = ref<'success' | 'error' | ''>('');

const loadXxmiSources = async () => {
  try {
    xxmiSources.value = await getXxmiPackageSources();
  } catch (e) {
    console.warn('[xxmi] 获取包源列表失败:', e);
  }
};

const refreshXxmiLocal = async () => {
  try {
    const status = await scanLocalXxmiPackages();
    xxmiLocalPackages.value = status.packages;
  } catch (e) {
    console.warn('[xxmi] 扫描本地包失败:', e);
  }
};

const refreshXxmiRemote = async () => {
  if (isXxmiFetching.value) return;
  try {
    isXxmiFetching.value = true;
    xxmiMessage.value = '';
    xxmiRemoteVersions.value = await fetchXxmiRemoteVersions(xxmiSelectedSource.value);
  } catch (e) {
    xxmiMessage.value = tr('settings.messages.xxmiFetchFailed', `获取远程版本失败: ${e}`).replace('{error}', String(e));
    xxmiMessageType.value = 'error';
  } finally {
    isXxmiFetching.value = false;
  }
};

const refreshXxmiState = async () => {
  await Promise.all([refreshXxmiLocal(), refreshXxmiRemote()]);
};

const runXxmiAction = async <T>({
  pendingMessage,
  run,
  successMessage,
  errorMessage,
  refresh,
}: {
  pendingMessage: string;
  run: () => Promise<T>;
  successMessage: (result: T) => string;
  errorMessage: (error: unknown) => string;
  refresh?: () => Promise<void>;
}) => {
  try {
    xxmiMessage.value = pendingMessage;
    xxmiMessageType.value = '';
    const result = await run();
    xxmiMessage.value = successMessage(result);
    xxmiMessageType.value = 'success';
    if (refresh) {
      await refresh();
    }
    return result;
  } catch (e) {
    xxmiMessage.value = errorMessage(e);
    xxmiMessageType.value = 'error';
    throw e;
  }
};

const doDownloadXxmi = async (ver: XxmiRemoteVersion) => {
  if (isXxmiDownloading.value) return;
  try {
    isXxmiDownloading.value = true;
    xxmiDownloadingVersion.value = ver.version;
    await runXxmiAction({
      pendingMessage: tr('settings.messages.xxmiDownloading', `正在下载 ${ver.source_name} ${ver.version}...`)
        .replace('{source}', ver.source_name)
        .replace('{version}', ver.version),
      run: () => downloadXxmiPackage(ver.source_id, ver.version, ver.download_url),
      successMessage: (msg) => msg,
      errorMessage: (e) =>
        tr('settings.messages.downloadFailed', `下载失败: ${e}`).replace('{error}', String(e)),
      refresh: refreshXxmiState,
    });
  } finally {
    isXxmiDownloading.value = false;
    xxmiDownloadingVersion.value = '';
  }
};

const doDeployXxmi = async (pkg: XxmiLocalPackage) => {
  const targetDir = getMigotoPathDisplayValue('importer_folder');
  if (!targetDir) {
    xxmiMessage.value = t('settings.migoto.xxmiDeployNoTarget');
    xxmiMessageType.value = 'error';
    return;
  }
  await runXxmiAction({
    pendingMessage: tr('settings.messages.taskDeploying', '正在部署，请稍候...'),
    run: () => deployXxmiPackage(pkg.source_id, pkg.version, targetDir),
    successMessage: (msg) => msg,
    errorMessage: (e) =>
      tr('settings.messages.xxmiDeployFailed', `部署失败: ${e}`).replace('{error}', String(e)),
  });
};

const doDeleteXxmi = async (pkg: XxmiLocalPackage) => {
  await runXxmiAction({
    pendingMessage: tr('settings.messages.taskDeleting', '正在删除，请稍候...'),
    run: () => deleteLocalXxmiPackage(pkg.source_id, pkg.version),
    successMessage: (msg) => msg,
    errorMessage: (e) =>
      tr('settings.messages.xxmiDeleteFailed', `删除失败: ${e}`).replace('{error}', String(e)),
    refresh: refreshXxmiState,
  });
};

watch(() => xxmiSelectedSource.value, async () => {
  xxmiRemoteVersions.value = [];
  await refreshXxmiRemote();
});

const xxmiFilteredLocal = computed(() =>
  xxmiLocalPackages.value.filter(p => p.source_id === xxmiSelectedSource.value)
);

// Feature #4: 游戏名 → XXMI 包源 ID 映射（核心库 xxmi-libs 始终显示）
const gameToXxmiSourceMap: Record<string, string> = {
  WutheringWaves: 'wwmi',
  ZenlessZoneZero: 'zzmi',
  HonkaiStarRail: 'srmi',
  GenshinImpact: 'gimi',
  Genshin: 'gimi',
  HonkaiImpact3rd: 'himi',
  Honkai3rd: 'himi',
  ArknightsEndfield: 'efmi',
};

// Feature #4: 仅显示核心库 + 当前游戏对应的包源
const xxmiFilteredSources = computed(() => {
  const game = migotoSelectedGame.value;
  if (!game) return xxmiSources.value; // 未选择游戏时显示全部
  const gameSourceId = gameToXxmiSourceMap[game];
  if (!gameSourceId) return xxmiSources.value; // 未知游戏时显示全部
  return xxmiSources.value.filter(s => s.id === 'xxmi-libs' || s.id === gameSourceId);
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
  () => [activeMenu.value, globalMigotoEnabled.value] as const,
  async ([menu, migotoEnabled]) => {
    if (menu === 'version' && !versionCheckLoaded.value) {
      await checkVersionInfo();
    }
    if (menu === 'resource' && !resourceCheckLoaded.value) {
      await checkResourceInfo();
    }
    if (menu === 'proton' && !protonLoaded.value) {
      await loadCatalog();
      await Promise.all([refreshLocalGrouped(), refreshRemoteGrouped()]);
      protonLoaded.value = true;
    }
    if (menu === 'proton') {
      // 默认折叠重型编辑器，避免滚动时大量表单常驻渲染
      showProtonCatalogEditor.value = false;
    }
    if (menu === 'dxvk' && !dxvkLoaded.value) {
      await Promise.all([refreshDxvkLocal(), refreshDxvkRemote()]);
      dxvkLoaded.value = true;
    }
    if (menu === 'vkd3d' && !vkd3dLoaded.value) {
      await Promise.all([refreshVkd3dLocal(), refreshVkd3dRemote()]);
      vkd3dLoaded.value = true;
    }
    if (menu === 'migoto' && migotoEnabled) {
      await Promise.all([ensureMigotoLoaded(), loadXxmiSources(), refreshXxmiLocal()]);
    }
  },
  { immediate: true }
);

watch(
  () => [route.query.menu, route.query.guide, route.query.t],
  () => applyMenuFromRoute(),
  { immediate: true },
);
</script>

<template>
  <div class="settings-layout">
    <!-- 左侧菜单 -->
    <div class="settings-menu" data-onboarding="settings-menu">
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
        <el-menu-item index="resource">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 14a8 8 0 0 1 8-8h8"/><path d="M20 10V6h-4"/><path d="M20 10a8 8 0 0 1-8 8H4"/><path d="M4 14v4h4"/></svg></el-icon>
          <span>{{ tr('settings.resource.title', '资源更新') }}</span>
        </el-menu-item>
        <el-menu-item index="proton">
          <span v-if="guideMenu === 'proton'" class="menu-guide-dot"></span>
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 16V8a2 2 0 0 0-1-1.73l-6-3.46a2 2 0 0 0-2 0L5 6.27A2 2 0 0 0 4 8v8a2 2 0 0 0 1 1.73l6 3.46a2 2 0 0 0 2 0l6-3.46A2 2 0 0 0 20 16z"></path><polyline points="7.5 4.21 12 6.81 16.5 4.21"></polyline><polyline points="7.5 19.79 7.5 14.6 3 12"></polyline><polyline points="21 12 16.5 14.6 16.5 19.79"></polyline><polyline points="12 22.08 12 16.9 7.5 14.3"></polyline><polyline points="12 16.9 16.5 14.3"></polyline><polyline points="12 6.81 12 12"></polyline></svg></el-icon>
          <span>{{ tr('settings.proton_manage_title', 'Proton 管理') }}</span>
        </el-menu-item>
        <el-menu-item index="dxvk">
          <span v-if="guideMenu === 'dxvk'" class="menu-guide-dot"></span>
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 20h20"/><path d="M5 20V8l7-5 7 5v12"/><path d="M9 20v-4h6v4"/><path d="M9 12h6"/><path d="M9 16h6"/></svg></el-icon>
          <span>{{ tr('settings.dxvk_manage_title', 'DXVK 管理') }}</span>
        </el-menu-item>
        <el-menu-item index="vkd3d">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 20h16"/><path d="M6 20V8l6-4 6 4v12"/><path d="M9 12h6"/><path d="M9 16h6"/></svg></el-icon>
          <span>{{ tr('settings.vkd3d_manage_title', 'VKD3D 管理') }}</span>
        </el-menu-item>
        <el-menu-item index="migoto">
          <el-icon><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg></el-icon>
          <span>{{ tr('settings.migoto_manage_title', '3DMIGOTO 管理') }}</span>
        </el-menu-item>
      </el-menu>
    </div>

    <!-- 右侧内容区 -->
    <div class="settings-content">
      <!-- 基础设置 -->
      <div v-if="activeMenu === 'basic'" class="settings-panel" data-onboarding="settings-basic-panel">
        <div class="panel-title">{{ t('settings.basicsettings') }}</div>
        <el-form label-width="140px">
          <el-form-item :label="t('settings.language')">
            <el-select v-model="appSettings.locale" :placeholder="tr('settings.language_select_placeholder', 'Select language')" style="width: 200px">
              <el-option :label="tr('settings.language_options.zhs', '简体中文')" value="zhs" />
              <el-option :label="tr('settings.language_options.zht', '繁體中文')" value="zht" />
              <el-option :label="tr('settings.language_options.en', 'English')" value="en" />
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
          <el-form-item :label="tr('settings.snowbreakSourcePolicy.label', '尘白下载源策略')">
            <div class="form-item-vertical">
              <el-select v-model="appSettings.snowbreakSourcePolicy" style="width: 260px">
                <el-option :label="tr('settings.snowbreakSourcePolicy.officialFirst', '官方优先（失败后回退社区）')" value="official_first" />
                <el-option :label="tr('settings.snowbreakSourcePolicy.communityFirst', '社区优先（失败后回退官方）')" value="community_first" />
              </el-select>
              <div class="form-item-hint">
                {{ tr('settings.snowbreakSourcePolicy.hint', '推荐保持\"官方优先\"，网络异常时会自动回退到另一来源。') }}
              </div>
            </div>
          </el-form-item>
          <el-form-item :label="tr('settings.logviewer.label', '日志查看器')">
            <div class="form-item-vertical">
              <el-button @click="openLogWindow()">{{ tr('settings.logviewer.open', '打开日志窗口') }}</el-button>
              <div class="form-item-hint">
                {{ tr('settings.logviewer.hint', '在新窗口中查看软件运行日志，便于排查问题时提供给开发者。') }}
              </div>
            </div>
          </el-form-item>
          <el-form-item :label="tr('settings.onboarding.label', '新手引导')">
            <div class="form-item-vertical">
              <el-button @click="reenterOnboarding">{{ tr('settings.onboarding.reenter', '重新进入新手引导') }}</el-button>
              <div class="form-item-hint">
                {{ tr('settings.onboarding.hint', '仅重新展示功能导览（主页、游戏库、运行环境等），不会重置初始化设置。') }}
              </div>
            </div>
          </el-form-item>
        </el-form>
      </div>

      <!-- 外观设置 -->
      <div v-if="activeMenu === 'appearance'" class="settings-panel">
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
      <div v-if="activeMenu === 'display'" class="settings-panel" data-onboarding="settings-display-panel">
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
      <div v-if="activeMenu === 'version'" class="settings-panel version-panel" data-onboarding="settings-version-panel">
        <div class="panel-title">{{ tr('settings.version_check_title', '版本检查') }}</div>

        <div class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">{{ tr('settings.version_check_overview', '当前版本信息') }}</div>
              <div class="section-hint">
                {{ tr('settings.version_check_hint', '优先从 GitHub 检查主程序 version 和 version-log；GitHub 异常时会自动回退到 Gitee，最后再回退到本地打包文件。') }}
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

      <!-- 资源检查 -->
      <div v-if="activeMenu === 'resource'" class="settings-panel version-panel" data-onboarding="settings-resource-panel">
        <div class="panel-title">{{ tr('settings.resource.title', '资源更新') }}</div>

        <div class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">{{ tr('settings.resource.sectionTitle', 'data-linux 资源版本') }}</div>
              <div class="section-hint">
                {{ tr('settings.resource.sectionHint', '优先从 GitHub 检查 data-linux（旧名 Data-parameters）版本；GitHub 异常时会自动回退到 Gitee 镜像，并可一键拉取更新。') }}
              </div>
            </div>
            <div class="toolbar-actions">
              <el-button size="small" @click="checkResourceInfo" :loading="isResourceChecking">
                {{ isResourceChecking ? tr('settings.resource.checking', '检查中...') : tr('settings.resource.checkAction', '检查资源版本') }}
              </el-button>
              <el-button type="primary" size="small" @click="pullResources" :loading="isResourcePulling">
                {{ isResourcePulling ? tr('settings.resource.pulling', '拉取中...') : tr('settings.resource.pullAction', '拉取资源更新') }}
              </el-button>
            </div>
          </div>

          <div v-if="resourceInfo" class="version-grid">
            <div class="version-row">
              <div class="version-label">{{ tr('settings.resource.currentVersion', '本地资源版本') }}</div>
              <div class="version-value">{{ resourceInfo.currentVersion }}</div>
            </div>
            <div class="version-row">
              <div class="version-label">{{ tr('settings.resource.latestVersion', '远程资源版本') }}</div>
              <div class="version-value">{{ resourceInfo.latestVersion }}</div>
            </div>
            <div class="version-row">
              <div class="version-label">{{ tr('settings.resource.status', '更新状态') }}</div>
              <div class="version-value">
                <el-tag v-if="resourceInfo.hasUpdate" type="warning">{{ tr('settings.resource.hasUpdate', '有可用资源更新') }}</el-tag>
                <el-tag v-else type="success">{{ tr('settings.resource.upToDate', '资源已是最新') }}</el-tag>
              </div>
            </div>
            <div class="version-row version-log-row">
              <div class="version-label">{{ tr('settings.resource.logLabel', '检查信息') }}</div>
              <div class="version-value">
                <pre class="version-log-content">{{ resourceInfo.updateLog || tr('settings.resource.logEmpty', '暂无检查信息') }}</pre>
              </div>
            </div>
          </div>

          <div v-else class="row-sub">
            {{ tr('settings.resource.notLoaded', '尚未获取资源版本信息，请点击“检查资源版本”。') }}
          </div>
        </div>
      </div>

      <!-- Proton 管理 -->
      <div v-if="activeMenu === 'proton'" class="settings-panel proton-panel" data-onboarding="settings-proton-panel">
        <div class="panel-title">{{ tr('settings.proton_manage_title', 'Proton 管理') }}</div>
        <div v-if="guideMenu === 'proton'" class="settings-guide-banner">
          {{ tr('settings.proton_guide_hint', '请先在此下载并安装至少一个 Proton 版本，然后回到主页启动游戏。') }}
        </div>

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
            <div class="row-sub row-sub-path" v-if="selectedLocalItem(family.family_key)">
              <span>{{ tr('settings.proton_selected_path', '路径') }}: {{ selectedLocalItem(family.family_key)?.path }}</span>
              <el-button
                v-if="isManagedProtonItem(selectedLocalItem(family.family_key))"
                text
                type="danger"
                size="small"
                :loading="!!deletingProtonIds[selectedLocalItem(family.family_key)?.id || '']"
                @click="removeLocalProtonItem(selectedLocalItem(family.family_key)!)"
              >
                {{ tr('settings.actions.delete', '删除') }}
              </el-button>
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
              <el-button size="small" @click="showProtonCatalogEditor = !showProtonCatalogEditor">
                {{ showProtonCatalogEditor ? tr('settings.proton_editor_collapse', '收起目录编辑器') : tr('settings.proton_editor_expand', '展开目录编辑器') }}
              </el-button>
              <template v-if="showProtonCatalogEditor">
                <el-button size="small" @click="reloadCatalogEditor" :loading="isCatalogLoading">
                  {{ tr('settings.proton_reload_catalog', '重载目录') }}
                </el-button>
                <el-button type="primary" size="small" @click="saveCatalogChanges" :loading="isCatalogSaving">
                  {{ tr('settings.proton_save_catalog', '保存目录') }}
                </el-button>
              </template>
            </div>
          </div>

          <div v-if="showProtonCatalogEditor" class="proton-editor-wrap">
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
          <div v-else class="row-sub" style="margin-top: 10px;">
            {{ tr('settings.proton_editor_collapsed_hint', '目录编辑器已折叠（推荐保持折叠以提升滚动性能）。') }}
          </div>
        </div>
      </div>

      <!-- DXVK 管理 -->
      <div v-if="activeMenu === 'dxvk'" class="settings-panel dxvk-panel" data-onboarding="settings-dxvk-panel">
        <div class="panel-title">{{ tr('settings.dxvk_manage_title', 'DXVK 管理') }}</div>
        <div v-if="guideMenu === 'dxvk'" class="settings-guide-banner">
          {{ tr('settings.dxvk_guide_hint', '请先在此下载 DXVK 版本；下载后可在“游戏设置 -> 运行环境”里应用到当前 Prefix。') }}
        </div>

        <div class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">{{ tr('settings.dxvk_section_title', 'DXVK (DirectX → Vulkan)') }}</div>
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
                <el-tag :type="lv.variant === 'dxvk' ? 'info' : 'warning'" size="small">
                  {{ dxvkVariantShortLabel(lv.variant) }}
                </el-tag>
                <el-tag v-if="lv.extracted" type="success" size="small">{{ tr('settings.dxvk_extracted', '已解压') }}</el-tag>
                <el-tag v-else type="info" size="small">{{ tr('settings.dxvk_archive_only', '仅存档') }}</el-tag>
                <div class="dxvk-local-path">{{ lv.path }}</div>
                <div class="dxvk-local-actions">
                  <el-button
                    text
                    type="danger"
                    size="small"
                    :loading="!!deletingDxvkKeys[`${lv.version}|${lv.variant}`]"
                    @click="removeLocalDxvkItem(lv.version, lv.variant)"
                  >
                    {{ tr('settings.actions.delete', '删除') }}
                  </el-button>
                </div>
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
                  :key="group.variant"
                  :label="group.label"
                >
                  <el-option
                    v-for="v in group.items"
                    :key="v.key"
                    :label="`${v.version}${v.isLocal ? ' [本地]' : ''}${v.fileSize > 0 ? ` (${formatBytes(v.fileSize)})` : ''}`"
                    :value="v.key"
                  >
                    <div class="remote-option-row">
                      <span>
                        <el-tag
                          :type="v.variant === 'dxvk' ? 'info' : 'warning'"
                          size="small"
                          style="margin-right: 6px;"
                        >
                          {{ dxvkVariantShortLabel(v.variant) }}
                        </el-tag>
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

      <!-- VKD3D 管理 -->
      <div v-if="activeMenu === 'vkd3d'" class="settings-panel dxvk-panel" data-onboarding="settings-vkd3d-panel">
        <div class="panel-title">{{ tr('settings.vkd3d_manage_title', 'VKD3D 管理') }}</div>

        <div class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">{{ tr('settings.vkd3d_section_title', 'VKD3D-Proton (Direct3D 12 → Vulkan)') }}</div>
              <div class="section-hint">
                {{ tr('settings.vkd3d_hint', '在此下载和管理 VKD3D-Proton 版本，可用于 Direct3D 12 转译。') }}
              </div>
            </div>
            <div class="toolbar-actions">
              <el-button size="small" @click="refreshVkd3dLocal">
                {{ tr('settings.vkd3d_refresh_local', '刷新本地') }}
              </el-button>
              <el-button size="small" @click="refreshVkd3dRemote" :loading="isVkd3dFetching">
                {{ isVkd3dFetching ? tr('settings.vkd3d_fetching', '获取中...') : tr('settings.vkd3d_refresh_remote', '获取可用版本') }}
              </el-button>
            </div>
          </div>

          <!-- 本地已缓存版本 -->
          <div class="dxvk-section">
            <div class="editor-subtitle" style="margin-top: 14px;">
              {{ tr('settings.vkd3d_local_title', '本地已缓存') }}
              <span class="dxvk-count">({{ vkd3dLocalCount }} {{ tr('settings.vkd3d_versions', '个版本') }})</span>
            </div>
            <div v-if="vkd3dLocalVersions.length === 0" class="row-sub" style="margin-top: 8px;">
              {{ tr('settings.vkd3d_no_local', '暂无本地缓存版本，请先获取可用版本并下载。') }}
            </div>
            <div v-else class="dxvk-local-list">
              <div v-for="lv in vkd3dLocalVersions" :key="lv.version" class="dxvk-local-item">
                <div class="dxvk-local-ver">{{ lv.version }}</div>
                <el-tag v-if="lv.extracted" type="success" size="small">{{ tr('settings.vkd3d_extracted', '已解压') }}</el-tag>
                <el-tag v-else type="info" size="small">{{ tr('settings.vkd3d_archive_only', '仅存档') }}</el-tag>
                <div class="dxvk-local-path">{{ lv.path }}</div>
                <div class="dxvk-local-actions">
                  <el-button
                    text
                    type="danger"
                    size="small"
                    :loading="!!deletingVkd3dVersions[lv.version]"
                    @click="removeLocalVkd3dItem(lv.version)"
                  >
                    {{ tr('settings.actions.delete', '删除') }}
                  </el-button>
                </div>
              </div>
            </div>
          </div>

          <!-- 版本选择 + 下载 -->
          <div class="dxvk-section" style="margin-top: 16px;">
            <div class="editor-subtitle">{{ tr('settings.vkd3d_download_title', '下载 VKD3D-Proton 版本') }}</div>
            <div v-if="vkd3dFetchWarning" class="dxvk-fetch-warning" style="margin-bottom: 8px;">
              ⚠ {{ vkd3dFetchWarning }}
            </div>
            <div class="dxvk-download-row">
              <el-select
                v-model="vkd3dSelectedVersion"
                :placeholder="tr('settings.vkd3d_select_version', '选择版本...')"
                class="dxvk-version-select"
                filterable
              >
                <el-option
                  v-for="v in vkd3dVersionList"
                  :key="v.version"
                  :label="`${v.version}${v.isLocal ? ' [本地]' : ''}${v.fileSize > 0 ? ` (${formatBytes(v.fileSize)})` : ''}`"
                  :value="v.version"
                >
                  <div class="remote-option-row">
                    <span>
                      {{ v.version }}
                      <el-tag v-if="v.isLocal" type="success" size="small" style="margin-left: 6px;">{{ tr('settings.vkd3d_cached', '已缓存') }}</el-tag>
                    </span>
                    <span class="remote-option-meta">
                      {{ v.fileSize > 0 ? formatBytes(v.fileSize) : '' }}
                      {{ v.publishedAt ? `· ${formatDate(v.publishedAt)}` : '' }}
                    </span>
                  </div>
                </el-option>
              </el-select>
              <el-button
                type="primary"
                :disabled="!selectedVkd3dItem || isVkd3dDownloading || selectedVkd3dItem?.isLocal"
                :loading="isVkd3dDownloading"
                @click="doDownloadVkd3d"
              >
                {{
                  isVkd3dDownloading
                    ? tr('settings.vkd3d_downloading', '下载中...')
                    : selectedVkd3dItem?.isLocal
                      ? tr('settings.vkd3d_already_cached', '已缓存')
                      : tr('settings.vkd3d_download', '下载')
                }}
              </el-button>
            </div>
          </div>
        </div>
      </div>

      <!-- 3DMIGOTO 管理 -->
      <div v-if="activeMenu === 'migoto'" class="settings-panel dxvk-panel" data-onboarding="settings-migoto-panel">
        <div class="panel-title-row">
          <div class="panel-title panel-title-inline">{{ $t('settings.migoto.panelTitle') }}</div>
          <el-tag type="danger" effect="dark" size="small" class="panel-title-badge">
            {{ tr('settings.migoto.experimentalBadge', '实验性') }}
          </el-tag>
        </div>

        <div class="section-block migoto-risk-block">
          <div class="section-header">
            <div>
              <div class="section-title migoto-risk-title">{{ tr('settings.migoto.riskTitle', '严重警告') }}</div>
              <div class="section-hint migoto-risk-text">
                {{ migotoRiskStatement }}
              </div>
            </div>
          </div>
          <div class="migoto-risk-actions">
            <el-button type="danger" plain @click="openDocumentsDoc('terms')">
              {{ tr('settings.migoto.viewTerms', '查看《服务条款》') }}
            </el-button>
            <el-button plain @click="openDocumentsDoc('risk')">
              {{ tr('settings.migoto.viewProjectRisk', '查看项目风险与要求') }}
            </el-button>
            <el-button type="warning" plain @click="showMigotoRiskRestatement">
              {{ tr('settings.migoto.restateRisk', '再次查看风险声明') }}
            </el-button>
          </div>
        </div>

        <div class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">{{ tr('settings.migoto.globalToggleTitle', '3DMigoto 全局开关') }}</div>
              <div class="section-hint">
                {{ tr('settings.migoto.globalToggleHint', '关闭后将全局禁用 3DMigoto 的相关配置、桥接和注入；游戏设置页也不会显示 3DMigoto 入口。') }}
              </div>
            </div>
          </div>
          <el-form label-width="160px" class="migoto-form" style="margin-top: 16px;">
            <el-form-item :label="tr('settings.migoto.globalToggleLabel', '启用 3DMigoto')">
              <el-switch
                :model-value="globalMigotoEnabled"
                :loading="isMigotoTogglePending"
                :disabled="isMigotoTogglePending"
                @update:model-value="handleMigotoGlobalToggle"
              />
            </el-form-item>
          </el-form>
        </div>

        <div v-if="!globalMigotoEnabled" class="section-block">
          <div class="row-sub">
            {{ tr('settings.migoto.disabledSummary', '3DMigoto 当前已全局禁用。现有游戏配置会被保留，但启动时不会加载桥接、不会注入，也不会在游戏设置中显示 3DMigoto 管理。') }}
          </div>
        </div>

        <div v-else class="section-block">
          <div class="section-header">
            <div>
              <div class="section-title">{{ $t('settings.migoto.gameConfigTitle') }}</div>
              <div class="section-hint">
                {{ $t('settings.migoto.gameConfigHint') }}
              </div>
            </div>
          </div>

          <!-- 游戏选择 -->
          <div class="migoto-game-select" style="margin-top: 16px;">
            <div class="editor-subtitle">{{ $t('settings.migoto.selectGame') }}</div>
            <el-select
              v-model="migotoSelectedGame"
              :placeholder="$t('settings.migoto.selectGamePlaceholder')"
              class="dxvk-version-select"
              filterable
              style="width: 100%; max-width: 400px;"
            >
              <el-option
                v-for="g in migotoGamesList"
                :key="g.name"
                :label="getLocalizedGameName(g)"
                :value="g.name"
              />
            </el-select>
          </div>

          <template v-if="migotoSelectedGame">

            <!-- 路径配置 -->
            <div class="dxvk-section" style="margin-top: 20px;">
              <div class="editor-subtitle">{{ $t('settings.migoto.pathConfig') }}</div>
              <div class="section-hint" style="margin-bottom: 12px;">
                {{ $t('settings.migoto.pathConfigHint') }}
              </div>
              <el-form label-width="160px" class="migoto-form">

                <el-form-item :label="$t('settings.migoto.migotoPath')">
                  <div style="display: flex; gap: 8px; width: 100%;">
                    <el-input v-model="migotoConfig.migoto_path" :placeholder="$t('settings.migoto.migotoPathPlaceholder')" />
                    <el-button size="small" @click="selectMigotoPath('migoto_path')">{{ $t('settings.migoto.browse') }}</el-button>
                  </div>
                  <div class="form-item-hint">{{ $t('settings.migoto.migotoPathHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.importerFolder')">
                  <div style="display: flex; gap: 8px; width: 100%;">
                    <el-input
                      :model-value="getMigotoPathDisplayValue('importer_folder')"
                      :readonly="!isMigotoPathOverridden('importer_folder')"
                      :placeholder="$t('settings.migoto.importerFolderPlaceholder')"
                    />
                    <el-button size="small" @click="selectMigotoPath('importer_folder')">{{ $t('settings.migoto.browse') }}</el-button>
                    <el-button
                      v-if="isMigotoPathOverridden('importer_folder')"
                      size="small"
                      text
                      @click="restoreMigotoPathAuto('importer_folder')"
                    >
                      {{ $t('settings.migoto.restoreAuto') }}
                    </el-button>
                  </div>
                  <div class="form-item-hint">{{ $t('settings.migoto.importerFolderHint') }}</div>
                  <div v-if="!isMigotoPathOverridden('importer_folder')" class="form-item-hint">{{ getMigotoAutoPathDescription('importer_folder') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.modFolder')">
                  <div style="display: flex; gap: 8px; width: 100%;">
                    <el-input
                      :model-value="getMigotoPathDisplayValue('mod_folder')"
                      :readonly="!isMigotoPathOverridden('mod_folder')"
                      :placeholder="$t('settings.migoto.modFolderPlaceholder')"
                    />
                    <el-button size="small" @click="selectMigotoPath('mod_folder')">{{ $t('settings.migoto.browse') }}</el-button>
                    <el-button
                      v-if="isMigotoPathOverridden('mod_folder')"
                      size="small"
                      text
                      @click="restoreMigotoPathAuto('mod_folder')"
                    >
                      {{ $t('settings.migoto.restoreAuto') }}
                    </el-button>
                  </div>
                  <div class="form-item-hint">{{ $t('settings.migoto.modFolderHint') }}</div>
                  <div v-if="!isMigotoPathOverridden('mod_folder')" class="form-item-hint">{{ getMigotoAutoPathDescription('mod_folder') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.shaderFixesFolder')">
                  <div style="display: flex; gap: 8px; width: 100%;">
                    <el-input
                      :model-value="getMigotoPathDisplayValue('shader_fixes_folder')"
                      :readonly="!isMigotoPathOverridden('shader_fixes_folder')"
                      :placeholder="$t('settings.migoto.shaderFixesFolderPlaceholder')"
                    />
                    <el-button size="small" @click="selectMigotoPath('shader_fixes_folder')">{{ $t('settings.migoto.browse') }}</el-button>
                    <el-button
                      v-if="isMigotoPathOverridden('shader_fixes_folder')"
                      size="small"
                      text
                      @click="restoreMigotoPathAuto('shader_fixes_folder')"
                    >
                      {{ $t('settings.migoto.restoreAuto') }}
                    </el-button>
                  </div>
                  <div class="form-item-hint">{{ $t('settings.migoto.shaderFixesFolderHint') }}</div>
                  <div v-if="!isMigotoPathOverridden('shader_fixes_folder')" class="form-item-hint">{{ getMigotoAutoPathDescription('shader_fixes_folder') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.d3dxIniPath')">
                  <div style="display: flex; gap: 8px; width: 100%;">
                    <el-input
                      :model-value="getMigotoPathDisplayValue('d3dx_ini_path')"
                      :readonly="!isMigotoPathOverridden('d3dx_ini_path')"
                      :placeholder="$t('settings.migoto.d3dxIniPathPlaceholder')"
                    />
                    <el-button size="small" @click="selectMigotoPath('d3dx_ini_path')">{{ $t('settings.migoto.browse') }}</el-button>
                    <el-button
                      v-if="isMigotoPathOverridden('d3dx_ini_path')"
                      size="small"
                      text
                      @click="restoreMigotoPathAuto('d3dx_ini_path')"
                    >
                      {{ $t('settings.migoto.restoreAuto') }}
                    </el-button>
                  </div>
                  <div class="form-item-hint">{{ $t('settings.migoto.d3dxIniPathHint') }}</div>
                  <div v-if="!isMigotoPathOverridden('d3dx_ini_path')" class="form-item-hint">{{ getMigotoAutoPathDescription('d3dx_ini_path') }}</div>
                </el-form-item>

              </el-form>
            </div>

            <!-- Mod 导入器 + 注入方式 -->
            <div class="dxvk-section" style="margin-top: 20px;">
              <div class="editor-subtitle">{{ $t('settings.migoto.importerAndInjection') }}</div>
              <el-form label-width="160px" class="migoto-form">

                <el-form-item :label="$t('settings.migoto.importerLabel')">
                  <el-select v-model="migotoConfig.importer" :disabled="isMigotoImporterLocked" style="width: 280px;">
                    <el-option
                      v-for="opt in migotoAvailableImporterOptions"
                      :key="opt.value"
                      :label="opt.label"
                      :value="opt.value"
                    />
                  </el-select>
                  <div class="form-item-hint">{{ migotoImporterHint }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.injectionLabel')">
                  <el-radio-group v-model="migotoConfig.use_hook" :disabled="isMigotoInjectionLocked">
                    <el-radio :value="true">{{ $t('settings.migoto.injectionHook') }}</el-radio>
                    <el-radio :value="false">{{ $t('settings.migoto.injectionDirect') }}</el-radio>
                  </el-radio-group>
                  <div class="form-item-hint">{{ migotoInjectionHint }}</div>
                </el-form-item>

              </el-form>
            </div>

            <!-- 高级选项 -->
            <div class="dxvk-section" style="margin-top: 20px;">
              <div class="editor-subtitle">{{ $t('settings.migoto.advancedOptions') }}</div>
              <el-form label-width="160px" class="migoto-form">

                <el-form-item :label="$t('settings.migoto.enforceRendering')">
                  <el-switch v-model="migotoConfig.enforce_rendering" />
                  <div class="form-item-hint">{{ $t('settings.migoto.enforceRenderingHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.muteWarnings')">
                  <el-switch v-model="migotoConfig.mute_warnings" />
                  <div class="form-item-hint">{{ $t('settings.migoto.muteWarningsHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.enableHunting')">
                  <el-switch v-model="migotoConfig.enable_hunting" />
                  <div class="form-item-hint">{{ $t('settings.migoto.enableHuntingHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.dumpShaders')">
                  <el-switch v-model="migotoConfig.dump_shaders" />
                  <div class="form-item-hint">{{ $t('settings.migoto.dumpShadersHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.callsLogging')">
                  <el-switch v-model="migotoConfig.calls_logging" />
                  <div class="form-item-hint">{{ $t('settings.migoto.callsLoggingHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.debugLogging')">
                  <el-switch v-model="migotoConfig.debug_logging" />
                  <div class="form-item-hint">{{ $t('settings.migoto.debugLoggingHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.processTimeout')">
                  <el-input-number v-model="migotoConfig.process_timeout" :min="5" :max="120" :step="5" style="width: 160px;" />
                  <div class="form-item-hint">{{ $t('settings.migoto.processTimeoutHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.unsafeMode')">
                  <el-switch v-model="migotoConfig.unsafe_mode" />
                  <div class="form-item-hint">{{ $t('settings.migoto.unsafeModeHint') }}</div>
                  <div v-if="migotoConfig.unsafe_mode" style="color: #f56c6c; font-size: 12px; margin-top: 4px;">
                    {{ $t('settings.migoto.unsafeModeWarn') }}
                  </div>
                </el-form-item>

              </el-form>
            </div>

            <!-- WWMI 专属设置 -->
            <div v-if="isMigotoWwmi" class="dxvk-section" style="margin-top: 20px;">
              <div class="editor-subtitle">{{ $t('settings.migoto.wwmiTitle') }}</div>
              <el-form label-width="160px" class="migoto-form">

                <el-form-item :label="$t('settings.migoto.wwmiConfigureGame')">
                  <el-switch v-model="migotoConfig.wwmi_configure_game" />
                  <div class="form-item-hint">{{ $t('settings.migoto.wwmiConfigureGameHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.wwmiUnlockFps')">
                  <el-switch v-model="migotoConfig.wwmi_unlock_fps" />
                  <div class="form-item-hint">{{ $t('settings.migoto.wwmiUnlockFpsHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.wwmiPerfTweaks')">
                  <el-switch v-model="migotoConfig.wwmi_perf_tweaks" />
                  <div class="form-item-hint">{{ $t('settings.migoto.wwmiPerfTweaksHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.wwmiDisableWoundedFx')">
                  <el-switch v-model="migotoConfig.wwmi_disable_wounded_fx" />
                  <div class="form-item-hint">{{ $t('settings.migoto.wwmiDisableWoundedFxHint') }}</div>
                </el-form-item>

              </el-form>
            </div>

            <!-- 中间层 Bridge 配置 -->
            <div class="dxvk-section" style="margin-top: 20px;">
              <div class="editor-subtitle">{{ $t('settings.migoto.bridgeConfig') }}</div>
              <div class="section-hint" style="margin-bottom: 12px;">
                {{ $t('settings.migoto.bridgeConfigHint') }}
              </div>
              <el-form label-width="180px" class="migoto-form">

                <el-form-item :label="$t('settings.migoto.bridgeExe')">
                  <div style="display: flex; gap: 8px; width: 100%;">
                    <el-input v-model="migotoConfig.bridge_exe_path" :placeholder="$t('settings.migoto.bridgeExePlaceholder')" />
                    <el-button size="small" @click="selectMigotoPath('bridge_exe_path')">{{ $t('settings.migoto.browse') }}</el-button>
                  </div>
                  <div class="form-item-hint">{{ $t('settings.migoto.bridgeExeHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.startArgs')">
                  <el-input v-model="migotoConfig.start_args" :placeholder="$t('settings.migoto.startArgsPlaceholder')" />
                  <div class="form-item-hint">{{ migotoStartArgsHint }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.processStartMethod')">
                  <el-select v-model="migotoConfig.process_start_method" style="width: 200px;">
                    <el-option value="Native" :label="$t('settings.migoto.startMethodNative')" />
                    <el-option value="CreateProcess" :label="$t('settings.migoto.startMethodCreateProcess')" />
                    <el-option value="ShellExecute" :label="$t('settings.migoto.startMethodShellExecute')" />
                  </el-select>
                  <div class="form-item-hint">{{ $t('settings.migoto.processStartMethodHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.processPriority')">
                  <el-select v-model="migotoConfig.process_priority" style="width: 200px;">
                    <el-option value="Normal" :label="$t('settings.migoto.priorityNormal')" />
                    <el-option value="AboveNormal" :label="$t('settings.migoto.priorityAboveNormal')" />
                    <el-option value="High" :label="$t('settings.migoto.priorityHigh')" />
                    <el-option value="Realtime" :label="$t('settings.migoto.priorityRealtime')" />
                    <el-option value="BelowNormal" :label="$t('settings.migoto.priorityBelowNormal')" />
                    <el-option value="Idle" :label="$t('settings.migoto.priorityIdle')" />
                  </el-select>
                  <div class="form-item-hint">{{ $t('settings.migoto.processPriorityHint') }}</div>
                </el-form-item>

                <el-form-item :label="$t('settings.migoto.dllInitDelay')">
                  <el-input-number v-model="migotoConfig.xxmi_dll_init_delay" :min="0" :max="5000" :step="50" style="width: 180px;" />
                  <span style="margin-left: 8px; color: rgba(255,255,255,0.5); font-size: 12px;">{{ $t('settings.migoto.dllInitDelayUnit') }}</span>
                  <div class="form-item-hint">{{ $t('settings.migoto.dllInitDelayHint') }}</div>
                </el-form-item>

              </el-form>
            </div>

            <!-- 自定义启动命令 -->
            <div class="dxvk-section" style="margin-top: 20px;">
              <div class="editor-subtitle">{{ $t('settings.migoto.customLaunch') }}</div>
              <el-form label-width="180px" class="migoto-form">

                <el-form-item :label="$t('settings.migoto.customLaunchEnable')">
                  <el-switch v-model="migotoConfig.custom_launch_enabled" />
                  <div class="form-item-hint">{{ $t('settings.migoto.customLaunchEnableHint') }}</div>
                </el-form-item>

                <template v-if="migotoConfig.custom_launch_enabled">
                  <el-form-item :label="$t('settings.migoto.customLaunchCmd')">
                    <el-input v-model="migotoConfig.custom_launch_cmd" :placeholder="$t('settings.migoto.customLaunchCmdPlaceholder')" />
                    <div class="form-item-hint">{{ $t('settings.migoto.customLaunchCmdHint') }}</div>
                  </el-form-item>

                  <el-form-item :label="$t('settings.migoto.customLaunchInjectMode')">
                    <el-select v-model="migotoConfig.custom_launch_inject_mode" style="width: 200px;">
                      <el-option value="Hook" :label="$t('settings.migoto.customLaunchInjectHook')" />
                      <el-option value="Direct" :label="$t('settings.migoto.customLaunchInjectDirect')" />
                    </el-select>
                    <div class="form-item-hint">{{ $t('settings.migoto.customLaunchInjectHint') }}</div>
                  </el-form-item>
                </template>

              </el-form>
            </div>

            <!-- 启动前/加载后脚本 -->
            <div class="dxvk-section" style="margin-top: 20px;">
              <div class="editor-subtitle">{{ $t('settings.migoto.scriptHooks') }}</div>
              <div class="section-hint" style="margin-bottom: 12px;">
                {{ $t('settings.migoto.scriptHooksHint') }}
              </div>
              <el-form label-width="180px" class="migoto-form">

                <el-form-item :label="$t('settings.migoto.preLaunchScript')">
                  <el-switch v-model="migotoConfig.pre_launch_enabled" />
                </el-form-item>
                <template v-if="migotoConfig.pre_launch_enabled">
                  <el-form-item :label="$t('settings.migoto.preLaunchCmd')">
                    <el-input v-model="migotoConfig.pre_launch_cmd" :placeholder="$t('settings.migoto.preLaunchCmdPlaceholder')" />
                  </el-form-item>
                  <el-form-item :label="$t('settings.migoto.preLaunchWait')">
                    <el-switch v-model="migotoConfig.pre_launch_wait" />
                    <div class="form-item-hint">{{ $t('settings.migoto.preLaunchWaitHint') }}</div>
                  </el-form-item>
                </template>

                <el-form-item :label="$t('settings.migoto.postLoadScript')">
                  <el-switch v-model="migotoConfig.post_load_enabled" />
                </el-form-item>
                <template v-if="migotoConfig.post_load_enabled">
                  <el-form-item :label="$t('settings.migoto.postLoadCmd')">
                    <el-input v-model="migotoConfig.post_load_cmd" :placeholder="$t('settings.migoto.postLoadCmdPlaceholder')" />
                  </el-form-item>
                  <el-form-item :label="$t('settings.migoto.postLoadWait')">
                    <el-switch v-model="migotoConfig.post_load_wait" />
                    <div class="form-item-hint">{{ $t('settings.migoto.postLoadWaitHint') }}</div>
                  </el-form-item>
                </template>

              </el-form>
            </div>

            <!-- 额外库加载 -->
            <div class="dxvk-section" style="margin-top: 20px;">
              <div class="editor-subtitle">{{ $t('settings.migoto.extraLibraries') }}</div>
              <el-form label-width="180px" class="migoto-form">

                <el-form-item :label="$t('settings.migoto.extraLibrariesEnable')">
                  <el-switch v-model="migotoConfig.extra_libraries_enabled" />
                  <div class="form-item-hint">{{ $t('settings.migoto.extraLibrariesEnableHint') }}</div>
                </el-form-item>

                <el-form-item v-if="migotoConfig.extra_libraries_enabled" :label="$t('settings.migoto.extraLibrariesPaths')">
                  <el-input
                    v-model="migotoConfig.extra_libraries_paths"
                    type="textarea"
                    :rows="4"
                    :placeholder="$t('settings.migoto.extraLibrariesPathsPlaceholder')"
                  />
                  <div class="form-item-hint">{{ $t('settings.migoto.extraLibrariesPathsHint') }}</div>
                </el-form-item>

              </el-form>
            </div>

            <!-- 保存按钮 -->
            <div style="margin-top: 24px; display: flex; gap: 12px;">
              <el-button type="primary" @click="saveMigotoGameConfig" :loading="isMigotoSaving">
                {{ isMigotoSaving ? $t('settings.migoto.saving') : $t('settings.migoto.saveConfig') }}
              </el-button>
              <el-button @click="loadMigotoGameConfig">{{ $t('settings.migoto.reload') }}</el-button>
            </div>

          </template>
        </div>

        <!-- XXMI 资源包下载 -->
        <div v-if="globalMigotoEnabled" class="section-block" style="margin-top: 28px;">
          <div class="section-header">
            <div>
              <div class="section-title">{{ $t('settings.migoto.xxmiTitle') }}</div>
              <div class="section-hint">
                {{ $t('settings.migoto.xxmiHint') }}
              </div>
            </div>
          </div>

          <!-- 包源选择 -->
          <div style="margin-top: 16px; display: flex; align-items: center; gap: 12px; flex-wrap: wrap;">
            <div class="editor-subtitle" style="margin: 0;">{{ $t('settings.migoto.xxmiSource') }}</div>
            <el-select
              v-model="xxmiSelectedSource"
              style="width: 320px;"
              class="dxvk-version-select"
            >
              <el-option
                v-for="src in xxmiFilteredSources"
                :key="src.id"
                :label="src.display_name"
                :value="src.id"
              />
            </el-select>
            <el-button size="small" @click="refreshXxmiRemote" :loading="isXxmiFetching">
              {{ isXxmiFetching ? $t('settings.migoto.xxmiFetching') : $t('settings.migoto.xxmiRefresh') }}
            </el-button>
          </div>

          <!-- 状态消息 -->
          <div
            v-if="xxmiMessage"
            :style="{
              marginTop: '12px', padding: '8px 12px', borderRadius: '4px', fontSize: '13px',
              background: xxmiMessageType === 'success' ? 'rgba(103,194,58,0.1)' : xxmiMessageType === 'error' ? 'rgba(245,108,108,0.1)' : 'rgba(144,147,153,0.1)',
              color: xxmiMessageType === 'success' ? '#67c23a' : xxmiMessageType === 'error' ? '#f56c6c' : '#909399',
            }"
          >
            {{ xxmiMessage }}
          </div>

          <!-- 本地已下载 -->
          <div v-if="xxmiFilteredLocal.length > 0" style="margin-top: 16px;">
            <div class="editor-subtitle">{{ $t('settings.migoto.xxmiLocalTitle') }}</div>
            <div class="xxmi-pkg-list">
              <div v-for="pkg in xxmiFilteredLocal" :key="`${pkg.source_id}-${pkg.version}`" class="xxmi-pkg-item">
                <div class="xxmi-pkg-info">
                  <span class="xxmi-pkg-version">{{ pkg.version }}</span>
                  <span class="xxmi-pkg-size">{{ formatBytes(pkg.size_bytes) }}</span>
                </div>
                <div class="xxmi-pkg-actions">
                  <el-button size="small" type="primary" @click="doDeployXxmi(pkg)">
                    {{ $t('settings.migoto.xxmiDeploy') }}
                  </el-button>
                  <el-button size="small" type="danger" @click="doDeleteXxmi(pkg)">
                    {{ $t('settings.migoto.xxmiDelete') }}
                  </el-button>
                </div>
              </div>
            </div>
          </div>

          <!-- 远程可用版本 -->
          <div v-if="xxmiRemoteVersions.length > 0" style="margin-top: 16px;">
            <div class="editor-subtitle">{{ $t('settings.migoto.xxmiRemoteTitle') }}</div>
            <div class="xxmi-pkg-list">
              <div v-for="ver in xxmiRemoteVersions" :key="`${ver.source_id}-${ver.version}`" class="xxmi-pkg-item">
                <div class="xxmi-pkg-info">
                  <span class="xxmi-pkg-version">{{ ver.version }}</span>
                  <span class="xxmi-pkg-size">{{ formatBytes(ver.asset_size) }}</span>
                  <el-tag v-if="ver.installed" type="success" size="small" style="margin-left: 6px;">{{ $t('settings.migoto.xxmiInstalled') }}</el-tag>
                </div>
                <div class="xxmi-pkg-actions">
                  <el-button
                    v-if="!ver.installed"
                    size="small"
                    type="primary"
                    :loading="isXxmiDownloading && xxmiDownloadingVersion === ver.version"
                    :disabled="isXxmiDownloading"
                    @click="doDownloadXxmi(ver)"
                  >
                    {{ isXxmiDownloading && xxmiDownloadingVersion === ver.version ? $t('settings.migoto.xxmiDownloading') : $t('settings.migoto.xxmiDownload') }}
                  </el-button>
                  <span v-if="ver.published_at" class="xxmi-pkg-date">
                    {{ ver.published_at.substring(0, 10) }}
                  </span>
                </div>
              </div>
            </div>
          </div>

          <!-- 空状态 -->
          <div v-else-if="!isXxmiFetching && xxmiSources.length > 0" style="margin-top: 16px; color: #909399; font-size: 13px;">
            {{ $t('settings.migoto.xxmiEmpty') }}
          </div>
        </div>
      </div>

    </div>

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
  background: rgba(10, 15, 20, 0.92);
  will-change: transform;
  contain: layout style;
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

.menu-guide-dot {
  position: absolute;
  left: 8px;
  top: 50%;
  width: 8px;
  height: 8px;
  margin-top: -4px;
  border-radius: 999px;
  background: #f59e0b;
  animation: menuGuideBlink 0.7s ease-in-out 0s 6;
}

@keyframes menuGuideBlink {
  0% { opacity: 0.3; transform: scale(0.9); }
  50% { opacity: 1; transform: scale(1.25); }
  100% { opacity: 0.35; transform: scale(0.9); }
}

.settings-el-menu .el-menu-item:hover {
  background-color: rgba(0, 240, 255, 0.1);
  color: #fff;
}

.settings-el-menu .el-menu-item.is-active {
  background-color: rgba(0, 240, 255, 0.15);
  color: #00f0ff; /* Glowing cyan text */
  font-weight: 600;
  border-left: 4px solid #00f0ff;
}

.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: 32px 40px 60px 40px;
  will-change: scroll-position;
}

.settings-panel {
  max-width: 800px;
  animation: fadeIn 0.15s ease-out;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
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
}

.panel-title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin-bottom: 32px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
}

.panel-title-inline {
  margin-bottom: 0;
  padding-bottom: 0;
  border-bottom: none;
}

.panel-title-badge {
  letter-spacing: 0.5px;
}

.migoto-risk-block {
  border: 1px solid rgba(255, 99, 71, 0.45);
  background: linear-gradient(135deg, rgba(120, 12, 12, 0.32), rgba(55, 12, 12, 0.22));
}

.migoto-risk-title {
  color: #ff9b8a;
}

.migoto-risk-text {
  color: rgba(255, 226, 220, 0.92);
  line-height: 1.7;
}

.migoto-risk-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 16px;
}

.settings-guide-banner {
  margin-bottom: 14px;
  border-radius: 6px;
  border: 1px solid rgba(245, 158, 11, 0.45);
  background: rgba(245, 158, 11, 0.14);
  color: #fbbf24;
  padding: 10px 12px;
  font-size: 13px;
  animation: guideBannerPulse 0.8s ease-in-out 0s 4;
}

@keyframes guideBannerPulse {
  0% { opacity: 0.65; }
  50% { opacity: 1; }
  100% { opacity: 0.68; }
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
  padding: 20px;
}

.section-block:hover, .family-card:hover {
  background: rgba(15, 20, 25, 0.8);
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

.family-card {
  content-visibility: auto;
  contain-intrinsic-size: 240px;
}

.proton-editor-wrap {
  content-visibility: auto;
  contain-intrinsic-size: 720px;
}

.family-title {
  font-size: 16px;
  color: #fff;
  font-weight: 600;
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

.dxvk-local-actions {
  flex: 0 0 auto;
}

.row-sub-path {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
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
  border: 1px solid rgba(255, 255, 255, 0.2) !important;
  box-shadow: none !important;
  border-radius: 4px;
}

:deep(.el-input__wrapper:hover), :deep(.el-select__wrapper:hover) {
  border-color: rgba(0, 240, 255, 0.5) !important;
  box-shadow: none !important;
}

:deep(.el-input__wrapper.is-focus), :deep(.el-select__wrapper.is-focus) {
  border-color: #00f0ff !important;
  box-shadow: none !important;
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
  text-transform: uppercase;
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

:deep(.el-button:hover:not(.is-disabled)) {
  background-color: #00f0ff;
  color: #000;
  border-color: #00f0ff;
}

:deep(.el-button--primary) {
  background-color: rgba(0, 240, 255, 0.2);
  color: #00f0ff;
  border: 1px solid #00f0ff;
}

:deep(.el-button--primary:hover:not(.is-disabled)) {
  background-color: #00f0ff;
  color: #000;
}

:deep(.el-button.is-disabled) {
  background-color: rgba(255, 255, 255, 0.05);
  color: rgba(255, 255, 255, 0.2);
  border-color: rgba(255, 255, 255, 0.1);
}

/* XXMI 资源包列表 */
.xxmi-pkg-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
}

.xxmi-pkg-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  background: rgba(0, 240, 255, 0.03);
  border: 1px solid rgba(0, 240, 255, 0.15);
  border-radius: 4px;
  transition: border-color 0.2s;
}

.xxmi-pkg-item:hover {
  border-color: rgba(0, 240, 255, 0.4);
}

.xxmi-pkg-info {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
  min-width: 0;
}

.xxmi-pkg-version {
  font-weight: 600;
  color: #00f0ff;
  font-size: 14px;
  font-family: 'JetBrains Mono', monospace;
}

.xxmi-pkg-size {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
}

.xxmi-pkg-date {
  color: rgba(255, 255, 255, 0.4);
  font-size: 12px;
  margin-left: 8px;
}

.xxmi-pkg-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
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
