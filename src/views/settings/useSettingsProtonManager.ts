import { computed, reactive, ref, shallowRef } from 'vue';
import {
  deleteLocalProton,
  downloadProton,
  fetchRemoteProtonGrouped,
  getProtonCatalog,
  saveProtonCatalog,
  scanLocalProtonGrouped,
  type ProtonCatalog,
  type ProtonFamily,
  type ProtonFamilyLocalGroup,
  type ProtonLocalVersionItem,
  type ProtonFamilyRemoteGroup,
  type ProtonRemoteVersionItem,
  type ProtonSource,
} from '../../api';

type ToastFn = (
  kind: 'success' | 'warning' | 'info' | 'error',
  title: string,
  message: string,
) => Promise<void>;

type TaskMessage<T> = string | ((value: T) => string);

type RunDownloadTask = <T>(options: {
  taskId: string;
  title: string;
  pendingMessage?: string;
  componentKey?: string;
  run: () => Promise<T>;
  successMessage: TaskMessage<T>;
  errorMessage: TaskMessage<unknown>;
  refresh?: () => Promise<void>;
}) => Promise<T>;

type RunDeleteTask = <T>(options: {
  taskId: string;
  title?: string;
  run: () => Promise<T>;
  successMessage: TaskMessage<T>;
  errorMessage: TaskMessage<unknown>;
  refresh?: () => Promise<void>;
}) => Promise<T>;

type TranslateFn = (
  key: string,
  fallback: string,
  params?: Record<string, unknown>,
) => string;

export type EditableProtonFamily = ProtonFamily & { detect_patterns_text: string };

export function useSettingsProtonManager({
  tr,
  toast,
  runDownloadTask,
  runDeleteTask,
  getDataDir,
}: {
  tr: TranslateFn;
  toast: ToastFn;
  runDownloadTask: RunDownloadTask;
  runDeleteTask: RunDeleteTask;
  getDataDir: () => string;
}) {
  const protonCatalog = shallowRef<ProtonCatalog>({ families: [], sources: [] });
  const localGroups = shallowRef<ProtonFamilyLocalGroup[]>([]);
  const remoteGroups = shallowRef<ProtonFamilyRemoteGroup[]>([]);

  const isCatalogLoading = ref(false);
  const isCatalogSaving = ref(false);
  const isLocalLoading = ref(false);
  const isRemoteLoading = ref(false);
  const isDownloading = ref(false);
  const downloadingFamilyKey = ref('');
  const downloadingTag = ref('');
  const showProtonCatalogEditor = ref(false);

  const selectedLocalByFamily = reactive<Record<string, string>>({});
  const selectedRemoteByFamily = reactive<Record<string, string>>({});
  const editableFamilies = ref<EditableProtonFamily[]>([]);
  const editableSources = ref<ProtonSource[]>([]);
  const deletingProtonIds = reactive<Record<string, boolean>>({});

  const remoteItemKey = (item: ProtonRemoteVersionItem) =>
    `${item.tag}@@${item.source_repo}`;

  const hasSourceForFamily = (familyKey: string) =>
    protonCatalog.value.sources.some(
      (source) => source.family_key === familyKey && source.enabled,
    );

  const hasSourceByFamily = computed<Record<string, boolean>>(() => {
    const sourceMap: Record<string, boolean> = {};
    for (const source of protonCatalog.value.sources) {
      if (!source.family_key || !source.enabled) continue;
      sourceMap[source.family_key] = true;
    }
    return sourceMap;
  });

  const selectedLocalItem = (familyKey: string) => {
    const group = localGroups.value.find((g) => g.family_key === familyKey);
    if (!group) return null;
    return (
      group.items.find((item) => item.id === selectedLocalByFamily[familyKey]) ??
      null
    );
  };

  const selectedRemoteItem = (familyKey: string) => {
    const group = remoteGroups.value.find((g) => g.family_key === familyKey);
    if (!group) return null;
    const key = selectedRemoteByFamily[familyKey];
    return group.items.find((item) => remoteItemKey(item) === key) ?? null;
  };

  const selectedLocalItems = computed<Record<string, ProtonLocalVersionItem | null>>(() => {
    const items: Record<string, ProtonLocalVersionItem | null> = {};
    for (const group of localGroups.value) {
      const selectedId = selectedLocalByFamily[group.family_key];
      items[group.family_key] =
        group.items.find((item) => item.id === selectedId) ?? null;
    }
    return items;
  });

  const selectedRemoteItems = computed<Record<string, ProtonRemoteVersionItem | null>>(() => {
    const items: Record<string, ProtonRemoteVersionItem | null> = {};
    for (const group of remoteGroups.value) {
      const selectedKey = selectedRemoteByFamily[group.family_key];
      items[group.family_key] =
        group.items.find((item) => remoteItemKey(item) === selectedKey) ?? null;
    }
    return items;
  });

  const protonFamilyLabel = (familyKey: string) => {
    const family = protonCatalog.value.families.find(
      (item) => item.family_key === familyKey,
    );
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
      const hasExisting = group.items.some(
        (item) => remoteItemKey(item) === existing,
      );
      if (!hasExisting) {
        selectedRemoteByFamily[group.family_key] = remoteItemKey(group.items[0]);
      }
    }
  };

  const clearRemoteSelections = () => {
    for (const familyKey of Object.keys(selectedRemoteByFamily)) {
      delete selectedRemoteByFamily[familyKey];
    }
  };

  const syncRemoteInstalledState = (
    groups: ProtonFamilyRemoteGroup[],
  ): ProtonFamilyRemoteGroup[] => {
    const localMap = new Map(
      localGroups.value.map((group) => [group.family_key, group.items]),
    );

    return groups.map((group) => {
      const localItems = localMap.get(group.family_key) ?? [];
      const localVersions = new Set(
        localItems
          .map((item) => item.version.trim().toLowerCase())
          .filter((item) => item.length > 0),
      );
      const localNames = localItems
        .map((item) => item.name.trim().toLowerCase())
        .filter((item) => item.length > 0);

      return {
        ...group,
        items: group.items.map((item) => {
          const tag = item.tag.trim().toLowerCase();
          const version = item.version.trim().toLowerCase();
          const installed =
            localVersions.has(version) ||
            localVersions.has(tag) ||
            localNames.some((name) => name === tag || name.includes(tag));

          return {
            ...item,
            installed,
          };
        }),
      };
    });
  };

  const toEditableFamily = (family: ProtonFamily): EditableProtonFamily => ({
    ...family,
    detect_patterns_text: family.detect_patterns.join('\n'),
  });

  const normalizeDetectPatterns = (text: string): string[] =>
    text
      .split('\n')
      .map((v) => v.trim())
      .filter((v) => v.length > 0);

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
      editableSources.value = protonCatalog.value.sources.map((source) => ({
        ...source,
      }));
    } catch (e) {
      await toast(
        'error',
        tr('settings.messages.title.error', '错误'),
        tr('settings.messages.protonCatalogLoadFailed', `加载 Proton 目录失败: ${e}`),
      );
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
      remoteGroups.value = syncRemoteInstalledState(remoteGroups.value);
      ensureRemoteSelections();
    } catch (e) {
      await toast(
        'error',
        tr('settings.messages.title.error', '错误'),
        tr('settings.messages.protonLocalFetchFailed', `获取本地 Proton 失败: ${e}`),
      );
    } finally {
      isLocalLoading.value = false;
    }
  };

  const refreshRemoteGrouped = async () => {
    if (isRemoteLoading.value) return;
    try {
      isRemoteLoading.value = true;
      remoteGroups.value = syncRemoteInstalledState(
        await fetchRemoteProtonGrouped(),
      );
      ensureRemoteSelections();
    } catch (e) {
      await toast(
        'error',
        tr('settings.messages.title.error', '错误'),
        tr('settings.messages.protonRemoteFetchFailed', `获取远程 Proton 列表失败: ${e}`),
      );
    } finally {
      isRemoteLoading.value = false;
    }
  };

  const refreshProtonState = async () => {
    await refreshLocalGrouped();
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
        void toast(
          'error',
          tr('settings.messages.title.error', '错误'),
          tr('settings.proton_empty_fields', '家族 key 和显示名不能为空'),
        );
        return null;
      }
      if (!/^[a-zA-Z0-9_-]+$/.test(family.family_key)) {
        void toast(
          'error',
          tr('settings.messages.title.error', '错误'),
          tr(
            'settings.proton_invalid_family_key',
            `非法 family_key: ${family.family_key}`,
          ),
        );
        return null;
      }
      const lower = family.family_key.toLowerCase();
      if (familyKeySet.has(lower)) {
        void toast(
          'error',
          tr('settings.messages.title.error', '错误'),
          `${tr('settings.proton_invalid_family_key', 'family_key 重复')}: ${family.family_key}`,
        );
        return null;
      }
      familyKeySet.add(lower);
    }

    for (const source of sources) {
      if (!source.family_key || !familyKeySet.has(source.family_key.toLowerCase())) {
        void toast(
          'error',
          tr('settings.messages.title.error', '错误'),
          `source family_key 不存在: ${source.family_key}`,
        );
        return null;
      }
      if (!source.provider) source.provider = 'github_releases';
      const needRepo = source.provider === 'github_releases';
      const needEndpoint =
        source.provider === 'forgejo_releases' ||
        source.provider === 'github_actions';
      if (needRepo && !source.repo && !source.endpoint) {
        void toast(
          'error',
          tr('settings.messages.title.error', '错误'),
          tr(
            'settings.proton_empty_fields',
            'github_releases 需要 repo 或 endpoint',
          ),
        );
        return null;
      }
      if (needEndpoint && !source.endpoint) {
        void toast(
          'error',
          tr('settings.messages.title.error', '错误'),
          tr(
            'settings.proton_empty_fields',
            `${source.provider} 需要 endpoint`,
          ),
        );
        return null;
      }
      if (source.provider === 'github_actions' && !source.url_template) {
        void toast(
          'error',
          tr('settings.messages.title.error', '错误'),
          tr(
            'settings.proton_empty_fields',
            'github_actions 需要 url_template',
          ),
        );
        return null;
      }
      if (
        !Number.isInteger(source.asset_index) ||
        source.asset_index < -1 ||
        source.asset_index > 100
      ) {
        void toast(
          'error',
          tr('settings.messages.title.error', '错误'),
          tr(
            'settings.proton_empty_fields',
            'asset_index 必须在 -1 到 100 之间',
          ),
        );
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
      remoteGroups.value = [];
      clearRemoteSelections();
      await refreshLocalGrouped();
      await toast(
        'success',
        tr('settings.messages.title.success', '成功'),
        tr('settings.proton_editor_saved', 'Proton 目录已保存'),
      );
    } catch (e) {
      await toast(
        'error',
        tr('settings.messages.title.error', '错误'),
        tr('settings.messages.protonCatalogSaveFailed', `保存 Proton 目录失败: ${e}`),
      );
    } finally {
      isCatalogSaving.value = false;
    }
  };

  const reloadCatalogEditor = async () => {
    await loadCatalog();
    await toast(
      'info',
      tr('settings.messages.title.info', '提示'),
      tr('settings.proton_editor_reloaded', '已重载 Proton 目录'),
    );
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
    editableSources.value = editableSources.value.filter(
      (source) => source.family_key !== family.family_key,
    );
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

  const normalizeFsPath = (value: string) =>
    value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();

  const normalizedDataDir = computed(() => {
    const value = String(getDataDir() || '').trim();
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
    return managedProtonRoots.value.some(
      (root) => path === root || path.startsWith(`${root}/`),
    );
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
        pendingMessage: tr(
          'settings.messages.downloadLabelBody',
          `正在下载 ${familyLabel} ${item.tag}，请稍候...`,
        )
          .replace('{label}', familyLabel)
          .replace('{version}', item.tag),
        run: () => downloadProton(item.download_url, item.tag, familyKey),
        successMessage: tr(
          'settings.messages.downloadLabelDone',
          `${familyLabel} ${item.tag} 下载完成`,
        )
          .replace('{label}', familyLabel)
          .replace('{version}', item.tag),
        errorMessage: (e) =>
          tr('settings.messages.downloadFailed', `下载失败: ${e}`).replace(
            '{error}',
            String(e),
          ),
        refresh: refreshProtonState,
      });
    } finally {
      isDownloading.value = false;
      downloadingFamilyKey.value = '';
      downloadingTag.value = '';
    }
  };

  const familyCards = computed(() => {
    const localMap = new Map(
      localGroups.value.map((group) => [group.family_key, group]),
    );
    const remoteMap = new Map(
      remoteGroups.value.map((group) => [group.family_key, group]),
    );

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

  const removeLocalProtonItem = async (item: {
    id: string;
    path: string;
    name: string;
  }) => {
    if (!isManagedProtonItem(item) || deletingProtonIds[item.id]) return;
    const taskId = `settings-proton-delete-${item.id}`;
    const target = item.name || item.path;
    try {
      deletingProtonIds[item.id] = true;
      await runDeleteTask({
        taskId,
        run: () => deleteLocalProton(item.path),
        successMessage: tr('settings.messages.deleteTargetDone', `${target} 已删除`).replace(
          '{target}',
          target,
        ),
        errorMessage: (e) =>
          tr('settings.messages.deleteFailed', `删除失败: ${e}`).replace(
            '{error}',
            String(e),
          ),
        refresh: refreshProtonState,
      });
    } finally {
      deletingProtonIds[item.id] = false;
    }
  };

  return {
    protonCatalog,
    localGroups,
    remoteGroups,
    isCatalogLoading,
    isCatalogSaving,
    isLocalLoading,
    isRemoteLoading,
    isDownloading,
    downloadingFamilyKey,
    downloadingTag,
    showProtonCatalogEditor,
    selectedLocalByFamily,
    selectedRemoteByFamily,
    editableFamilies,
    editableSources,
    deletingProtonIds,
    remoteItemKey,
    hasSourceForFamily,
    hasSourceByFamily,
    selectedLocalItem,
    selectedRemoteItem,
    selectedLocalItems,
    selectedRemoteItems,
    loadCatalog,
    refreshLocalGrouped,
    refreshRemoteGrouped,
    refreshProtonState,
    saveCatalogChanges,
    reloadCatalogEditor,
    addFamily,
    removeFamily,
    addSource,
    removeSource,
    isManagedProtonItem,
    installSelectedForFamily,
    familyCards,
    removeLocalProtonItem,
  };
}
