import { computed, inject, onMounted, ref, shallowRef, watch } from 'vue';
import { useRouter } from 'vue-router';
import { appSettings, gamesList, loadGames } from '../../store';
import {
  ensureDirectory,
  getAppDataDirPath,
  loadGameConfig,
  openInExplorer,
  saveGameConfig,
  scanGameMods,
  setAllGameModEntriesEnabled,
  setGameModEntryEnabled,
  showMessage,
  type GameConfig,
  type GameModDirectoryState,
  type ManagedModEntry,
} from '../../api';
import { useI18n } from 'vue-i18n';
import { messages } from '../../i18n';
import { NOTIFY_KEY, type NotifyApi } from '../../types/notify';
import {
  buildMigotoResolvedPaths,
  resolveMigotoImporter,
} from '../../utils/migotoLayout';
import type { ModGameSummary, ModStatusFilter } from './types';

const gameNameLocales = ['zhs', 'zht', 'en'] as const;
const localeSortMap: Record<string, string> = {
  zhs: 'zh-Hans-CN',
  zht: 'zh-Hant-TW',
  en: 'en',
};

const trimValue = (value: unknown) => String(value ?? '').trim();

const joinPathString = (...parts: string[]) => {
  return parts
    .map((part, index) => {
      const normalized = trimValue(part);
      if (!normalized) return '';
      if (index === 0) return normalized.replace(/[\\/]+$/g, '');
      return normalized.replace(/^[/\\]+/g, '').replace(/[\\/]+$/g, '');
    })
    .filter(Boolean)
    .join('/');
};

const getParentPath = (path: string) => {
  const normalized = trimValue(path).replace(/[\\/]+$/g, '');
  const segments = normalized.split(/[\\/]+/).filter(Boolean);
  if (segments.length <= 1) return normalized;
  return segments.slice(0, -1).join('/');
};

const formatBytes = (bytes: number) => {
  if (!Number.isFinite(bytes) || bytes <= 0) return '-';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};

const formatModified = (unix?: number | null) => {
  if (!unix) return '-';
  const date = new Date(unix * 1000);
  if (Number.isNaN(date.getTime())) return '-';
  return date.toLocaleString();
};

const waitNextFrame = () =>
  new Promise<void>((resolve) => {
    if (
      typeof window !== 'undefined'
      && typeof window.requestAnimationFrame === 'function'
    ) {
      window.requestAnimationFrame(() => resolve());
      return;
    }
    setTimeout(resolve, 0);
  });

export function useModsView() {
  const { t, te } = useI18n();
  const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);
  const router = useRouter();
  const notify = inject<NotifyApi | null>(NOTIFY_KEY, null);

  const appDataDir = ref('');
  const gameKeyword = ref('');
  const modKeyword = ref('');
  const modStatusFilter = ref<ModStatusFilter>('all');
  const gameSummaries = shallowRef<ModGameSummary[]>([]);
  const selectedGameName = ref('');
  const selectedState = shallowRef<GameModDirectoryState | null>(null);

  const isLoadingGames = ref(false);
  const isLoadingSelectedMods = ref(false);
  const isSavingGameToggle = ref(false);
  const isBulkOperating = ref(false);
  const activeModEntryName = ref('');

  const getGameLocaleName = (localeKey: (typeof gameNameLocales)[number], gameName: string) => {
    const value = (messages[localeKey] as Record<string, unknown>)?.games as
      | Record<string, unknown>
      | undefined;
    const localized = value?.[gameName];
    return typeof localized === 'string' ? localized.trim() : '';
  };

  const getLocalizedGameName = (
    game:
      | Pick<ModGameSummary, 'name' | 'fallbackDisplayName'>
      | Pick<(typeof gamesList)[number], 'name' | 'displayName'>,
  ) => {
    const fallback = trimValue(
      'fallbackDisplayName' in game ? game.fallbackDisplayName : game.displayName,
    ) || game.name;
    return te(`games.${game.name}`) ? String(t(`games.${game.name}`)) : fallback;
  };

  const buildGameSearchNames = (gameName: string, fallbackDisplayName: string) => {
    const names = new Set<string>();
    for (const candidate of [
      gameName,
      fallbackDisplayName,
      ...gameNameLocales.map((localeKey) => getGameLocaleName(localeKey, gameName)),
    ]) {
      const normalized = trimValue(candidate);
      if (normalized) {
        names.add(normalized);
      }
    }
    return [...names];
  };

  const sortGameSummaries = (summaries: ModGameSummary[]) => {
    const localeKey = localeSortMap[appSettings.locale] || 'en';
    return [...summaries].sort((a, b) => a.displayName.localeCompare(b.displayName, localeKey));
  };

  const localizeGameSummary = (summary: ModGameSummary): ModGameSummary => {
    const displayName = getLocalizedGameName(summary);
    return {
      ...summary,
      displayName,
      searchNames: buildGameSearchNames(summary.name, summary.fallbackDisplayName),
    };
  };

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

  const buildSummaryPreview = (gameName: string, config: GameConfig) => {
    const migoto = (config.other?.migoto || {}) as Record<string, unknown>;
    const resolved = buildMigotoResolvedPaths({
      gameName,
      config: {
        importer: trimValue(migoto.importer),
        migoto_path: trimValue(migoto.migoto_path),
        importer_folder: trimValue(migoto.importer_folder),
        mod_folder: trimValue(migoto.mod_folder),
        shader_fixes_folder: trimValue(migoto.shader_fixes_folder),
      },
      defaultMigotoPath: joinPathString(appDataDir.value, '3Dmigoto-data'),
    });

    return {
      migotoEnabled: Boolean(migoto.enabled),
      importer: resolved.importer,
      modFolder: resolved.modFolder,
      shaderFixesFolder: resolved.shaderFixesFolder,
    };
  };

  const buildSummary = async (game: (typeof gamesList)[number]): Promise<ModGameSummary> => {
    if (!game.migotoSupported) {
      return {
        name: game.name,
        displayName: '',
        fallbackDisplayName: trimValue(game.displayName) || game.name,
        searchNames: [],
        iconPath: game.iconPath || '',
        migotoSupported: false,
        importer: '',
        migotoEnabled: false,
        modFolder: '',
        shaderFixesFolder: '',
        modFolderExists: false,
        shaderFixesFolderExists: false,
        loadError: '',
      };
    }

    try {
      const config = await loadGameConfig(game.name);
      const folders = buildSummaryPreview(game.name, config);

      return {
        name: game.name,
        displayName: '',
        fallbackDisplayName: trimValue(game.displayName) || game.name,
        searchNames: [],
        iconPath: game.iconPath || '',
        migotoSupported: true,
        importer: folders.importer,
        migotoEnabled: folders.migotoEnabled,
        modFolder: folders.modFolder,
        shaderFixesFolder: folders.shaderFixesFolder,
        modFolderExists: false,
        shaderFixesFolderExists: false,
        loadError: '',
      };
    } catch (error) {
      return {
        name: game.name,
        displayName: '',
        fallbackDisplayName: trimValue(game.displayName) || game.name,
        searchNames: [],
        iconPath: game.iconPath || '',
        migotoSupported: game.migotoSupported,
        importer: resolveMigotoImporter(game.name, ''),
        migotoEnabled: false,
        modFolder: '',
        shaderFixesFolder: '',
        modFolderExists: false,
        shaderFixesFolderExists: false,
        loadError: String(error),
      };
    }
  };

  const selectedGameSummary = computed(() => {
    return gameSummaries.value.find((item) => item.name === selectedGameName.value) ?? null;
  });

  const filteredGameOptions = computed(() => {
    const normalized = gameKeyword.value.trim().toLowerCase();
    if (!normalized) return gameSummaries.value;
    return gameSummaries.value.filter((entry) => {
      return (
        entry.searchNames.some((name) => name.toLowerCase().includes(normalized))
        || entry.name.toLowerCase().includes(normalized)
        || entry.importer.toLowerCase().includes(normalized)
      );
    });
  });

  const filteredModEntries = computed(() => {
    let entries = selectedState.value?.entries || [];
    if (modStatusFilter.value === 'enabled') {
      entries = entries.filter((entry) => entry.enabled);
    } else if (modStatusFilter.value === 'disabled') {
      entries = entries.filter((entry) => !entry.enabled);
    }

    const normalized = modKeyword.value.trim().toLowerCase();
    if (!normalized) return entries;

    return entries.filter((entry) => {
      return (
        entry.displayName.toLowerCase().includes(normalized)
        || entry.relativeName.toLowerCase().includes(normalized)
        || entry.entryType.toLowerCase().includes(normalized)
        || entry.path.toLowerCase().includes(normalized)
      );
    });
  });

  const totalGameCount = computed(() => gameSummaries.value.length);
  const enabledGameCount = computed(
    () => gameSummaries.value.filter((entry) => entry.migotoEnabled).length,
  );
  const selectedEnabledModCount = computed(() => {
    return selectedState.value?.entries.filter((entry) => entry.enabled).length || 0;
  });
  const selectedDisabledModCount = computed(() => {
    return selectedState.value?.entries.filter((entry) => !entry.enabled).length || 0;
  });

  const syncSummaryFromState = (state: GameModDirectoryState) => {
    const summary = gameSummaries.value.find((entry) => entry.name === state.gameName);
    if (!summary) return;
    summary.migotoSupported = state.migotoSupported;
    summary.importer = state.importer;
    summary.migotoEnabled = state.migotoSupported && state.migotoEnabled;
    summary.modFolder = state.modFolder;
    summary.modFolderExists = state.modFolderExists;
    summary.shaderFixesFolder = state.shaderFixesFolder;
    summary.shaderFixesFolderExists = state.shaderFixesFolderExists;
  };

  const loadSelectedGameMods = async () => {
    if (!selectedGameName.value || !appSettings.migotoEnabled) {
      selectedState.value = null;
      return;
    }

    try {
      isLoadingSelectedMods.value = true;
      selectedState.value = null;
      const state = await scanGameMods(selectedGameName.value);
      selectedState.value = state;
      syncSummaryFromState(state);
    } catch (error) {
      selectedState.value = null;
      await toast(
        'error',
        tr('mods.errorTitle', '错误'),
        tr('mods.scanFailed', `读取当前游戏 Mod 列表失败: ${error}`).replace('{error}', String(error)),
      );
    } finally {
      isLoadingSelectedMods.value = false;
    }
  };

  const loadGameSummaries = async (forceRescan = true) => {
    if (!appSettings.migotoEnabled) {
      gameSummaries.value = [];
      selectedGameName.value = '';
      selectedState.value = null;
      return;
    }

    const previousSelection = selectedGameName.value;

    try {
      isLoadingGames.value = true;
      appDataDir.value = await getAppDataDirPath();
      if (forceRescan || gamesList.length === 0) {
        await loadGames();
      }
      const summaries: ModGameSummary[] = [];
      for (let index = 0; index < gamesList.length; index += 1) {
        summaries.push(await buildSummary(gamesList[index]));
        if ((index + 1) % 4 === 0) {
          await waitNextFrame();
        }
      }
      gameSummaries.value = sortGameSummaries(
        summaries.map((summary) => localizeGameSummary(summary)),
      );

      if (gameSummaries.value.length === 0) {
        selectedGameName.value = '';
        selectedState.value = null;
        return;
      }

      const nextSelection = gameSummaries.value.some((entry) => entry.name === previousSelection)
        ? previousSelection
        : gameSummaries.value[0].name;
      const selectionChanged = nextSelection !== previousSelection;
      selectedGameName.value = nextSelection;

      if (!selectionChanged) {
        await waitNextFrame();
        await loadSelectedGameMods();
      }
    } catch (error) {
      await toast(
        'error',
        tr('mods.errorTitle', '错误'),
        tr('mods.loadFailed', `加载 Mod 管理信息失败: ${error}`).replace('{error}', String(error)),
      );
    } finally {
      isLoadingGames.value = false;
    }
  };

  const openFolder = async (targetPath: string) => {
    if (!trimValue(targetPath)) {
      await toast(
        'warning',
        tr('mods.infoTitle', '提示'),
        tr('mods.pathMissing', '当前没有可用的目录路径。'),
      );
      return;
    }
    await ensureDirectory(targetPath);
    await openInExplorer(targetPath);
  };

  const openEntryLocation = async (entry: ManagedModEntry) => {
    const target = entry.entryType === 'directory' ? entry.path : getParentPath(entry.path);
    await openFolder(target || entry.path);
  };

  const openSelectedModsFolder = async () => {
    await openFolder(selectedState.value?.modFolder || selectedGameSummary.value?.modFolder || '');
  };

  const openSelectedShaderFixesFolder = async () => {
    await openFolder(
      selectedState.value?.shaderFixesFolder || selectedGameSummary.value?.shaderFixesFolder || '',
    );
  };

  const toggleGameMigoto = async (nextValue: string | number | boolean) => {
    const summary = selectedGameSummary.value;
    if (!summary || isSavingGameToggle.value) return;
    if (!summary.migotoSupported) {
      await toast(
        'warning',
        tr('mods.infoTitle', '提示'),
        tr(
          'mods.unsupportedGame',
          '当前游戏暂不支持 3DMigoto / Mod 管理，因此无法开启。',
        ),
      );
      return;
    }

    const enabled = Boolean(nextValue);
    if (enabled === summary.migotoEnabled) return;

    try {
      isSavingGameToggle.value = true;
      const config = await loadGameConfig(summary.name);
      if (!config.other) config.other = {};
      if (!config.other.migoto) config.other.migoto = {};
      config.other.migoto.enabled = enabled;
      await saveGameConfig(summary.name, config);
      summary.migotoEnabled = enabled;
      if (selectedState.value && selectedState.value.gameName === summary.name) {
        selectedState.value.migotoEnabled = enabled;
      }
      await toast(
        'success',
        tr('mods.savedTitle', '保存成功'),
        enabled
          ? tr('mods.enabledGame', '已启用该游戏的 3DMigoto / Mod 支持')
          : tr('mods.disabledGame', '已禁用该游戏的 3DMigoto / Mod 支持'),
      );
    } catch (error) {
      await toast('error', tr('mods.saveFailedTitle', '保存失败'), String(error));
    } finally {
      isSavingGameToggle.value = false;
    }
  };

  const toggleModEntry = async (entry: ManagedModEntry, nextValue: string | number | boolean) => {
    if (!selectedGameName.value || activeModEntryName.value) return;

    const enabled = Boolean(nextValue);
    if (enabled === entry.enabled) return;

    try {
      activeModEntryName.value = entry.relativeName;
      await setGameModEntryEnabled(selectedGameName.value, entry.relativeName, enabled);
      await loadSelectedGameMods();
      await toast(
        'success',
        tr('mods.savedTitle', '保存成功'),
        enabled
          ? tr('mods.modEnabled', 'Mod 已启用并参与加载')
          : tr('mods.modDisabled', 'Mod 已禁用并停止加载'),
      );
    } catch (error) {
      await toast('error', tr('mods.saveFailedTitle', '保存失败'), String(error));
    } finally {
      activeModEntryName.value = '';
    }
  };

  const toggleAllMods = async (enabled: boolean) => {
    if (!selectedGameName.value || isBulkOperating.value) return;

    try {
      isBulkOperating.value = true;
      const result = await setAllGameModEntriesEnabled(selectedGameName.value, enabled);
      await loadSelectedGameMods();

      const skippedText = result.skipped.length > 0
        ? `\n${tr('mods.bulkSkipped', '以下条目因名称冲突被跳过')}: ${result.skipped.join(', ')}`
        : '';

      await toast(
        'success',
        tr('mods.savedTitle', '保存成功'),
        (
          enabled
            ? tr('mods.bulkEnabled', '已批量启用 {count} 个 Mod')
            : tr('mods.bulkDisabled', '已批量禁用 {count} 个 Mod')
        ).replace('{count}', String(result.changed)) + skippedText,
      );
    } catch (error) {
      await toast('error', tr('mods.saveFailedTitle', '保存失败'), String(error));
    } finally {
      isBulkOperating.value = false;
    }
  };

  const openMigotoSettings = async () => {
    await router.push({
      path: '/settings',
      query: {
        menu: 'migoto',
      },
    });
  };

  watch(
    () => selectedGameName.value,
    (value, oldValue) => {
      if (!value || value === oldValue) return;
      void waitNextFrame().then(() => loadSelectedGameMods());
    },
  );

  watch(
    () => appSettings.migotoEnabled,
    (enabled) => {
      if (!enabled) {
        void router.replace('/');
        return;
      }
      void loadGameSummaries(false);
    },
  );

  watch(
    () => appSettings.locale,
    () => {
      if (!gameSummaries.value.length) return;
      gameSummaries.value = sortGameSummaries(
        gameSummaries.value.map((summary) => localizeGameSummary(summary)),
      );
    },
  );

  onMounted(() => {
    void loadGameSummaries(false);
  });

  return {
    activeModEntryName,
    appSettings,
    enabledGameCount,
    filteredGameOptions,
    filteredModEntries,
    formatBytes,
    formatModified,
    gameKeyword,
    gameSummaries,
    isBulkOperating,
    isLoadingGames,
    isLoadingSelectedMods,
    isSavingGameToggle,
    loadGameSummaries,
    modKeyword,
    modStatusFilter,
    openEntryLocation,
    openMigotoSettings,
    openSelectedModsFolder,
    openSelectedShaderFixesFolder,
    selectedDisabledModCount,
    selectedEnabledModCount,
    selectedGameName,
    selectedGameSummary,
    selectedState,
    toggleAllMods,
    toggleGameMigoto,
    toggleModEntry,
    totalGameCount,
  };
}
