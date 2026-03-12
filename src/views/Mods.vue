<script setup lang="ts">
import { computed, inject, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { appSettings, gamesList, loadGames } from '../store';
import {
  ensureDirectory,
  getAppDataDirPath,
  loadGameConfig,
  openInExplorer,
  pathExists,
  saveGameConfig,
  scanGameMods,
  setAllGameModEntriesEnabled,
  setGameModEntryEnabled,
  showMessage,
  type GameConfig,
  type GameModDirectoryState,
  type ManagedModEntry,
} from '../api';
import { useI18n } from 'vue-i18n';
import { messages } from '../i18n';
import { NOTIFY_KEY } from '../types/notify';
import { detectMigotoResolvedPaths, resolveMigotoImporter } from '../utils/migotoLayout';

type ModGameSummary = {
  name: string;
  displayName: string;
  fallbackDisplayName: string;
  searchNames: string[];
  iconPath: string;
  importer: string;
  migotoEnabled: boolean;
  modFolder: string;
  shaderFixesFolder: string;
  modFolderExists: boolean;
  shaderFixesFolderExists: boolean;
  loadError: string;
};

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);
const router = useRouter();
const notify = inject(NOTIFY_KEY, null);

const appDataDir = ref('');
const gameKeyword = ref('');
const modKeyword = ref('');
const modStatusFilter = ref<'all' | 'enabled' | 'disabled'>('all');
const gameSummaries = ref<ModGameSummary[]>([]);
const selectedGameName = ref('');
const selectedState = ref<GameModDirectoryState | null>(null);

const isLoadingGames = ref(false);
const isLoadingSelectedMods = ref(false);
const isSavingGameToggle = ref(false);
const isBulkOperating = ref(false);
const activeModEntryName = ref('');
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

const getGameLocaleName = (localeKey: (typeof gameNameLocales)[number], gameName: string) => {
  const value = (messages[localeKey] as Record<string, any>)?.games?.[gameName];
  return typeof value === 'string' ? value.trim() : '';
};

const getLocalizedGameName = (
  game: Pick<ModGameSummary, 'name' | 'fallbackDisplayName'> | Pick<(typeof gamesList)[number], 'name' | 'displayName'>,
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

const buildSummaryFolders = async (gameName: string, config: GameConfig) => {
  const migoto = (config.other?.migoto || {}) as Record<string, unknown>;
  const resolved = await detectMigotoResolvedPaths({
    gameName,
    config: {
      importer: trimValue(migoto.importer),
      migoto_path: trimValue(migoto.migoto_path),
      importer_folder: trimValue(migoto.importer_folder),
      mod_folder: trimValue(migoto.mod_folder),
      shader_fixes_folder: trimValue(migoto.shader_fixes_folder),
    },
    defaultMigotoPath: joinPathString(appDataDir.value, '3Dmigoto-data'),
    pathExistsAt: pathExists,
  });

  return {
    importer: resolved.importer,
    migotoEnabled: Boolean(migoto.enabled),
    modFolder: resolved.modFolder,
    shaderFixesFolder: resolved.shaderFixesFolder,
  };
};

const buildSummary = async (game: (typeof gamesList)[number]): Promise<ModGameSummary> => {
  try {
    const config = await loadGameConfig(game.name);
    const folders = await buildSummaryFolders(game.name, config);
    const [modFolderExists, shaderFixesFolderExists] = await Promise.all([
      folders.modFolder ? pathExists(folders.modFolder) : Promise.resolve(false),
      folders.shaderFixesFolder ? pathExists(folders.shaderFixesFolder) : Promise.resolve(false),
    ]);

    return {
      name: game.name,
      displayName: '',
      fallbackDisplayName: trimValue(game.displayName) || game.name,
      searchNames: [],
      iconPath: game.iconPath || '',
      importer: folders.importer,
      migotoEnabled: folders.migotoEnabled,
      modFolder: folders.modFolder,
      shaderFixesFolder: folders.shaderFixesFolder,
      modFolderExists,
      shaderFixesFolderExists,
      loadError: '',
    };
  } catch (error) {
    return {
      name: game.name,
      displayName: '',
      fallbackDisplayName: trimValue(game.displayName) || game.name,
      searchNames: [],
      iconPath: game.iconPath || '',
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
const enabledGameCount = computed(() => gameSummaries.value.filter((entry) => entry.migotoEnabled).length);
const selectedEnabledModCount = computed(() => {
  return selectedState.value?.entries.filter((entry) => entry.enabled).length || 0;
});
const selectedDisabledModCount = computed(() => {
  return selectedState.value?.entries.filter((entry) => !entry.enabled).length || 0;
});

const syncSummaryFromState = (state: GameModDirectoryState) => {
  const summary = gameSummaries.value.find((entry) => entry.name === state.gameName);
  if (!summary) return;
  summary.importer = state.importer;
  summary.migotoEnabled = state.migotoEnabled;
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

const loadGameSummaries = async () => {
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
    await loadGames();
    const summaries = await Promise.all(gamesList.map((game) => buildSummary(game)));
    gameSummaries.value = sortGameSummaries(summaries.map((summary) => localizeGameSummary(summary)));

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
    await toast('warning', tr('mods.infoTitle', '提示'), tr('mods.pathMissing', '当前没有可用的目录路径。'));
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
  await openFolder(selectedState.value?.shaderFixesFolder || selectedGameSummary.value?.shaderFixesFolder || '');
};

const toggleGameMigoto = async (nextValue: string | number | boolean) => {
  const summary = selectedGameSummary.value;
  if (!summary || isSavingGameToggle.value) return;

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
    void loadSelectedGameMods();
  },
);

watch(
  () => appSettings.migotoEnabled,
  (enabled) => {
    if (!enabled) {
      void router.replace('/');
      return;
    }
    void loadGameSummaries();
  },
);

watch(
  () => appSettings.locale,
  () => {
    if (!gameSummaries.value.length) return;
    gameSummaries.value = sortGameSummaries(gameSummaries.value.map((summary) => localizeGameSummary(summary)));
  },
);

onMounted(() => {
  void loadGameSummaries();
});
</script>

<template>
  <div class="mods-page">
    <div class="mods-header">
      <div>
        <div class="title-row">
          <h1 class="title">{{ tr('mods.title', 'Mod 管理') }}</h1>
          <el-tag type="danger" effect="dark" size="small">{{ tr('mods.experimental', '实验性') }}</el-tag>
        </div>
        <p class="desc">
          {{ tr('mods.descAdvanced', '按游戏集中管理 3DMigoto Mod。你可以选择游戏、查看有效目录、单个或批量启用/禁用 Mod，并快速打开 Mod / ShaderFixes 目录。') }}
        </p>
      </div>
      <div class="header-actions">
        <el-button @click="openMigotoSettings">{{ tr('mods.gotoSettings', '前往 3DMIGOTO 管理') }}</el-button>
        <el-button type="primary" :loading="isLoadingGames" @click="loadGameSummaries">
          {{ isLoadingGames ? tr('mods.refreshing', '刷新中...') : tr('mods.refresh', '刷新') }}
        </el-button>
      </div>
    </div>

    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statGames', '游戏总数') }}</div>
        <div class="stat-value">{{ totalGameCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statEnabledGames', '已启用 3DMigoto 的游戏') }}</div>
        <div class="stat-value">{{ enabledGameCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statEnabledMods', '当前游戏已加载 Mod') }}</div>
        <div class="stat-value">{{ selectedEnabledModCount }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">{{ tr('mods.statDisabledMods', '当前游戏已禁用 Mod') }}</div>
        <div class="stat-value">{{ selectedDisabledModCount }}</div>
      </div>
    </div>

    <div class="control-panel">
      <div class="panel-row">
        <el-input
          v-model="gameKeyword"
          class="game-search"
          clearable
          :placeholder="tr('mods.searchGame', '搜索游戏...')"
        />
        <el-select
          v-model="selectedGameName"
          class="game-select"
          filterable
          :placeholder="tr('mods.selectGame', '选择一个游戏')"
        >
          <el-option
            v-for="entry in filteredGameOptions"
            :key="entry.name"
            :label="entry.displayName"
            :value="entry.name"
          />
        </el-select>

        <template v-if="selectedGameSummary">
          <div class="game-toggle-wrap">
            <span class="game-toggle-label">{{ tr('mods.gameToggle', '本游戏启用 3DMigoto') }}</span>
            <el-switch
              :model-value="selectedGameSummary.migotoEnabled"
              :loading="isSavingGameToggle"
              :disabled="isSavingGameToggle"
              @update:model-value="toggleGameMigoto"
            />
          </div>
          <el-tag type="info">{{ selectedGameSummary.importer }}</el-tag>
        </template>
      </div>

      <div v-if="selectedGameSummary" class="selected-game-card">
        <div class="selected-game-head">
          <div class="game-meta">
            <img v-if="selectedGameSummary.iconPath" :src="selectedGameSummary.iconPath" class="game-icon" alt="" />
            <div>
              <div class="game-title">{{ selectedGameSummary.displayName }}</div>
              <div class="game-sub">{{ selectedGameSummary.name }}</div>
            </div>
          </div>
          <div class="game-tags">
            <el-tag size="small" :type="selectedGameSummary.migotoEnabled ? 'success' : 'warning'">
              {{ selectedGameSummary.migotoEnabled ? tr('mods.enabled', '已启用') : tr('mods.disabled', '未启用') }}
            </el-tag>
            <el-tag size="small" type="danger">{{ tr('mods.experimental', '实验性') }}</el-tag>
          </div>
        </div>

        <div v-if="selectedGameSummary.loadError" class="card-error">
          {{ tr('mods.loadError', '读取配置失败') }}: {{ selectedGameSummary.loadError }}
        </div>

        <div class="path-grid">
          <div class="path-block">
            <div class="path-label">{{ tr('mods.modFolder', 'Mod 目录') }}</div>
            <div class="path-value">{{ selectedGameSummary.modFolder || '-' }}</div>
            <div class="path-meta">
              {{ selectedGameSummary.modFolderExists ? tr('mods.folderExists', '目录已存在') : tr('mods.folderMissing', '目录不存在，打开时会自动创建') }}
            </div>
          </div>
          <div class="path-block">
            <div class="path-label">{{ tr('mods.shaderFolder', 'ShaderFixes 目录') }}</div>
            <div class="path-value">{{ selectedGameSummary.shaderFixesFolder || '-' }}</div>
            <div class="path-meta">
              {{ selectedGameSummary.shaderFixesFolderExists ? tr('mods.folderExists', '目录已存在') : tr('mods.folderMissing', '目录不存在，打开时会自动创建') }}
            </div>
          </div>
        </div>

        <div class="selected-actions">
          <el-button type="primary" @click="openSelectedModsFolder">{{ tr('mods.openMods', '打开 Mod 目录') }}</el-button>
          <el-button @click="openSelectedShaderFixesFolder">{{ tr('mods.openShaderFixes', '打开 ShaderFixes') }}</el-button>
          <el-button type="success" plain :disabled="isBulkOperating || !selectedState?.entries?.length" @click="toggleAllMods(true)">
            {{ tr('mods.enableAll', '全部启用') }}
          </el-button>
          <el-button type="warning" plain :disabled="isBulkOperating || !selectedState?.entries?.length" @click="toggleAllMods(false)">
            {{ tr('mods.disableAll', '全部禁用') }}
          </el-button>
        </div>
      </div>
    </div>

    <div class="mod-list-panel">
      <div class="panel-top">
        <div class="panel-title">{{ tr('mods.modListTitle', 'Mod 列表') }}</div>
        <div class="panel-tools">
          <el-input
            v-model="modKeyword"
            class="mod-search"
            clearable
            :placeholder="tr('mods.searchMod', '搜索 Mod 名称、路径...')"
          />
          <el-radio-group v-model="modStatusFilter" size="small">
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
                @update:model-value="toggleModEntry(row, $event)"
              />
              <el-button text @click="openEntryLocation(row)">{{ tr('mods.openLocation', '打开位置') }}</el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </div>
  </div>
</template>

<style scoped>
.mods-page {
  padding: 32px 40px 60px 40px;
  animation: fadeIn 0.15s ease-out;
  background: rgba(10, 15, 20, 0.92);
  width: 100%;
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.mods-header {
  display: flex;
  justify-content: space-between;
  gap: 20px;
  align-items: flex-start;
  margin-bottom: 28px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
}

.title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.title {
  margin: 0;
  font-size: 28px;
  font-weight: 700;
  color: #00f0ff;
  letter-spacing: 1px;
  text-transform: uppercase;
}

.desc {
  margin: 10px 0 0 0;
  color: rgba(255, 255, 255, 0.65);
  max-width: 840px;
  line-height: 1.7;
}

.header-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
  margin-bottom: 18px;
}

.stat-card {
  border: 1px solid rgba(0, 240, 255, 0.18);
  background: rgba(0, 8, 14, 0.55);
  border-radius: 8px;
  padding: 16px 18px;
}

.stat-label {
  color: rgba(255, 255, 255, 0.58);
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.6px;
}

.stat-value {
  margin-top: 8px;
  color: #fff;
  font-size: 24px;
  font-weight: 700;
}

.control-panel,
.mod-list-panel {
  border: 1px solid rgba(0, 240, 255, 0.18);
  background: rgba(0, 8, 14, 0.55);
  border-radius: 10px;
}

.control-panel {
  padding: 18px;
  margin-bottom: 18px;
}

.panel-row {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
}

.game-search {
  max-width: 220px;
}

.game-select {
  min-width: 320px;
  max-width: 420px;
}

.game-toggle-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
}

.game-toggle-label {
  color: rgba(255, 255, 255, 0.72);
  font-size: 13px;
  font-weight: 600;
}

.selected-game-card {
  margin-top: 18px;
  padding-top: 18px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.selected-game-head {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
}

.game-meta {
  display: flex;
  align-items: center;
  gap: 12px;
}

.game-icon {
  width: 44px;
  height: 44px;
  border-radius: 8px;
  object-fit: cover;
  border: 1px solid rgba(255, 255, 255, 0.12);
}

.game-title {
  color: #fff;
  font-size: 18px;
  font-weight: 700;
}

.game-sub {
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
  margin-top: 4px;
  word-break: break-all;
}

.game-tags {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.card-error {
  margin-top: 14px;
  color: #f56c6c;
  font-size: 13px;
  line-height: 1.6;
}

.path-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
  margin-top: 16px;
}

.path-block {
  padding: 14px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(255, 255, 255, 0.03);
}

.path-label {
  color: rgba(255, 255, 255, 0.72);
  font-size: 13px;
  font-weight: 600;
}

.path-value {
  margin-top: 8px;
  font-family: monospace;
  font-size: 13px;
  color: #d7f9ff;
  line-height: 1.6;
  word-break: break-all;
}

.path-meta {
  margin-top: 8px;
  color: rgba(255, 255, 255, 0.48);
  font-size: 12px;
}

.selected-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 16px;
}

.mod-list-panel {
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

:deep(.game-search .el-input__wrapper),
:deep(.mod-search .el-input__wrapper),
:deep(.game-select .el-input__wrapper) {
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

@media (max-width: 1180px) {
  .stats-grid {
    grid-template-columns: 1fr 1fr;
  }

  .path-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 900px) {
  .mods-page {
    padding: 24px 20px 48px 20px;
  }

  .mods-header,
  .panel-top {
    flex-direction: column;
    align-items: stretch;
  }

  .stats-grid {
    grid-template-columns: 1fr;
  }

  .panel-row {
    align-items: stretch;
  }

  .game-search,
  .game-select,
  .mod-search {
    max-width: none;
    min-width: 0;
    width: 100%;
  }
}
</style>
