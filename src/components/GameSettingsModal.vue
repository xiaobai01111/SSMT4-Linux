<script setup lang="ts">
import { ref, watch, reactive, computed, nextTick } from 'vue';
import {
  loadGameConfig as apiLoadGameConfig,
  saveGameConfig as apiSaveGameConfig,
  createNewConfig as apiCreateNewConfig,
  deleteGameConfigFolder as apiDeleteGameConfigFolder,
  setGameIcon as apiSetGameIcon,
  setGameBackground as apiSetGameBackground,
  resetGameIcon as apiResetGameIcon,
  // updateGameBackground as apiUpdateGameBackground,
  resetGameBackground as apiResetGameBackground,
  openFileDialog,
  showMessage,
  askConfirm,
  scanWineVersions,
  getGameWineConfig,
  setGameWineConfig,
  setGamePrefixPath,
  checkVulkan,
  getDisplayInfo,
  getPrefixInfo,
  getJadeiteStatus,
  installJadeite,
  installDxvk,
  installVkd3d,
  uninstallDxvk,
  uninstallVkd3d,
  scanLocalDxvk,
  scanLocalVkd3d,
  detectDxvkStatus,
  detectVkd3dStatus,
  getLocalVersion,
  type WineVersion,
  type ProtonSettings,
  type PrefixInfo,
  type JadeiteStatus,
  type VulkanInfo,
  type DisplayInfo,
  type DxvkLocalVersion,
  type DxvkInstalledStatus,
  type Vkd3dLocalVersion,
  type Vkd3dInstalledStatus,
  type RuntimeEnv,
} from '../api';
import { appSettings, loadGames, gamesList, switchToGame } from '../store';
import { useI18n } from 'vue-i18n';
import { inject } from 'vue';
import { useGameInfoEditor } from '../composables/useGameInfoEditor';
import { useGameSettingsLifecycle } from '../composables/useGameSettingsLifecycle';
import { useGameSettingsManagedSections } from '../composables/useGameSettingsManagedSections';
import { useGameSettingsPersistence } from '../composables/useGameSettingsPersistence';
import type { GameSettingsOpenRequest, GameSettingsTab, RuntimeFocusTarget } from '../types/gameSettings';
import type { GameConfig } from '../types/ipc';
import { NOTIFY_KEY } from '../types/notify';
import GameInfoProfileSection from './game-info/GameInfoProfileSection.vue';
import GameInfoVersionSection from './game-info/GameInfoVersionSection.vue';
import GameInfoPresetSection from './game-info/GameInfoPresetSection.vue';
import GameInfoAssetsSection from './game-info/GameInfoAssetsSection.vue';

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);
const notify = inject(NOTIFY_KEY, null);

const props = defineProps<{
  modelValue: boolean;
  gameName: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const config = reactive<GameConfig>({
  basic: { gamePreset: props.gameName || 'WutheringWaves', runtimeEnv: 'wine' },
  other: {}
});

const configName = ref(''); // Separate UI state for the folder name

const gameInfoEditor = useGameInfoEditor();
const {
  infoConfig,
  presets: infoPresets,
  loading: infoLoading,
  saving: infoSaving,
  dirty: infoDirty,
  sectionErrors: infoSectionErrors,
  nameValidation,
  hasUnsavedChanges: hasUnsavedInfoChanges,
  load: loadGameInfoState,
  saveMeta: saveInfoMetaRaw,
  saveRuntime: saveInfoRuntimeRaw,
  validateName: validateInfoName,
  setPreset: setInfoPreset,
  setRuntimeEnv: setInfoRuntimeEnv,
  markDirty: markInfoDirty,
} = gameInfoEditor;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const asString = (value: unknown, fallback = ''): string =>
  typeof value === 'string' ? value : fallback;

const canonicalPreset = (value: string): string => {
  return value.trim();
};

const normalizeLoadedConfig = (raw: unknown): GameConfig => {
  const root = isRecord(raw) ? raw : {};
  const basicRaw = isRecord(root.basic) ? root.basic : {};
  const otherRaw = isRecord(root.other) ? root.other : {};
  const droppedLegacyModKeys = new Set([
    'threeDMigoto',
    '3DmigotoPath',
    'TargetPath',
    'LaunchPath',
    'LaunchArgs',
    'WorkSpace',
    'MigotoPackage',
    'AutoSetAnalyseOptions',
    'AutoSetAnalyseOptionsSelectedIndex',
    'GithubPackageVersion',
    'DllInitializationDelay',
    'DllReplaceSelectedIndex',
    'DllPreProcessSelectedIndex',
    'RunWithShell',
    'AutoRunIgnoreErrorGIPlugin',
    'Delay',
    'LaunchItems',
    'PureGameMode',
    'd3dxPath',
  ]);

  const gamePreset =
    asString(basicRaw.gamePreset) ||
    asString(basicRaw.GamePreset) ||
    asString(root.GamePreset) ||
    asString(root.LogicName) ||
    props.gameName ||
    'WutheringWaves';

  const runtimeEnvRaw =
    asString(basicRaw.runtimeEnv) || asString(root.runtimeEnv);
  const runtimeEnv: 'wine' | 'steam' | 'linux' =
    runtimeEnvRaw === 'steam' ? 'steam' : runtimeEnvRaw === 'linux' ? 'linux' : 'wine';

  const mergedOther: Record<string, unknown> = { ...otherRaw };
  for (const [key, value] of Object.entries(root)) {
    if (key === 'basic' || key === 'other' || droppedLegacyModKeys.has(key)) continue;
    if (mergedOther[key] === undefined) {
      mergedOther[key] = value;
    }
  }
  for (const key of droppedLegacyModKeys) {
    delete mergedOther[key];
  }

  const legacyGamePath =
    asString(mergedOther.gamePath) ||
    asString(mergedOther.game_path) ||
    asString(root.gamePath) ||
    asString(root.game_path);
  if (legacyGamePath) {
    mergedOther.gamePath = legacyGamePath;
  }

  return {
    basic: {
      gamePreset: canonicalPreset(gamePreset),
      runtimeEnv,
    },
    other: mergedOther,
  };
};

const createDefaultGameConfig = (gameName: string): GameConfig => ({
  basic: {
    gamePreset: gameName || 'WutheringWaves',
    runtimeEnv: 'wine',
  },
  other: {},
});

const normalizeGameName = (value: string | null | undefined): string =>
  String(value || '').trim();

const isEditableGameName = (value: string | null | undefined): boolean => {
  const gameName = normalizeGameName(value);
  return !!gameName && gameName !== 'Default';
};

const {
  isLoading,
  hasLoadedConfig,
  activeLoadSession,
  startLoadSession,
  isActiveLoadSession,
  loadManagedSectionGroups,
  saveManagedSections,
} = useGameSettingsManagedSections({
  normalizeGameName,
  isEditableGameName,
  isModalOpen: () => props.modelValue,
});

const { handleGameNameChange, requestClose } = useGameSettingsLifecycle({
  askConfirm,
  tr,
});

const syncSystemOptionsIntoConfig = () => {
  config.other.gpuIndex = selectedGpuIndex.value;
  config.other.gameLang = gameLang.value;
};

// Wine/Proton State
const wineVersions = ref<WineVersion[]>([]);
const selectedWineVersionId = ref('');
const protonSettings = reactive<ProtonSettings>({
  steam_app_id: '0',
  use_umu_run: false,
  use_pressure_vessel: true,
  proton_media_use_gst: false,
  proton_enable_wayland: false,
  proton_no_d3d12: false,
  mangohud: false,
  steam_deck_compat: false,
  steamos_compat: false,
  sandbox_enabled: false,
  sandbox_isolate_home: false,
  dxvk_hud: '',
  dxvk_async: false,
  dxvk_frame_rate: 0,
  disable_gpu_filter: false,
  custom_env: {},
});
const vulkanInfo = ref<VulkanInfo | null>(null);
const displayInfo = ref<DisplayInfo | null>(null);
const newEnvKey = ref('');
const newEnvValue = ref('');

// 系统选项状态
const selectedGpuIndex = ref(-1); // -1 = 自动
const gameLang = ref(''); // '' = 跟随系统

const loadWineState = async (gameName: string, sessionId: number) => {
  if (!isEditableGameName(gameName)) {
    wineVersions.value = [];
    selectedWineVersionId.value = '';
    return;
  }
  try {
    const [wines, wineConfig, vulkan, display] = await Promise.all([
      scanWineVersions(),
      getGameWineConfig(gameName),
      checkVulkan(),
      getDisplayInfo(),
    ]);
    if (!isActiveLoadSession(sessionId)) return;
    wineVersions.value = wines;
    if (wineConfig.wine_version_id) {
      selectedWineVersionId.value = wineConfig.wine_version_id;
    }
    Object.assign(protonSettings, wineConfig.proton_settings);
    vulkanInfo.value = vulkan;
    displayInfo.value = display;
  } catch (e) {
    if (!isActiveLoadSession(sessionId)) return;
    console.error('Failed to load wine state:', e);
  }
};

const addCustomEnv = () => {
  if (newEnvKey.value.trim()) {
    protonSettings.custom_env[newEnvKey.value.trim()] = newEnvValue.value;
    newEnvKey.value = '';
    newEnvValue.value = '';
  }
};

const removeCustomEnv = (key: string) => {
  delete protonSettings.custom_env[key];
};

const selectedWineVersion = computed(() =>
  wineVersions.value.find(v => v.id === selectedWineVersionId.value)
);

const variantLabel = (variant: string) => {
  const labels: Record<string, string> = {
    official: 'Proton (Official)',
    experimental: 'Proton Experimental',
    geproton: 'GE-Proton',
    dwproton: 'DW-Proton',
    protontkg: 'Proton-TKG',
    lutris: 'Lutris Wine',
    systemwine: 'System Wine',
    custom: 'Custom',
  };
  return labels[variant] || variant;
};

// Jadeite 状态
const jadeiteStatus = ref<JadeiteStatus | null>(null);
const isJadeiteInstalling = ref(false);
const prefixInfo = ref<PrefixInfo | null>(null);

const isHoyoverse = computed(() =>
  ['HonkaiStarRail', 'ZenlessZoneZero'].includes(
    config.basic.gamePreset,
  ),
);

const loadJadeiteState = async (gameName = props.gameName, sessionId?: number) => {
  const resolvedGameName = normalizeGameName(gameName);
  if (!resolvedGameName) return;
  try {
    const nextStatus = await getJadeiteStatus(resolvedGameName);
    if (sessionId !== undefined && !isActiveLoadSession(sessionId)) return;
    jadeiteStatus.value = nextStatus;
  } catch (e) {
    if (sessionId !== undefined && !isActiveLoadSession(sessionId)) return;
    jadeiteStatus.value = null;
  }
};

const doInstallJadeite = async () => {
  if (isJadeiteInstalling.value) return;
  try {
    isJadeiteInstalling.value = true;
    const result = await installJadeite(props.gameName);
    await showMessage(result, { title: 'Jadeite', kind: 'info' });
    await loadJadeiteState(props.gameName);
  } catch (e) {
    await showMessage(tr('gamesettingsmodal.messages.jadeiteInstallFailed', `安装 jadeite 失败: ${e}`).replace('{error}', String(e)), { title: tr('gamesettingsmodal.message.error.title', '错误'), kind: 'error' });
  } finally {
    isJadeiteInstalling.value = false;
  }
};

const loadPrefixState = async (gameName: string, sessionId: number) => {
  if (!isEditableGameName(gameName)) {
    prefixInfo.value = null;
    return;
  }
  try {
    const nextPrefixInfo = await getPrefixInfo(gameName);
    if (!isActiveLoadSession(sessionId)) return;
    prefixInfo.value = nextPrefixInfo;
  } catch (e) {
    if (!isActiveLoadSession(sessionId)) return;
    prefixInfo.value = {
      game_id: gameName,
      exists: false,
      path: '',
      size_bytes: 0,
      config: null,
    };
  }
};

// DXVK 版本管理
const dxvkLocalVersions = ref<DxvkLocalVersion[]>([]);
const dxvkInstalledStatus = ref<DxvkInstalledStatus | null>(null);
const dxvkSelectedKey = ref('');
const isDxvkBusy = ref(false);

const dxvkVariantLabel = (variant: string) => {
  const labels: Record<string, string> = {
    dxvk: 'DXVK (官方)',
    gplasync: 'DXVK-GPLAsync',
    async: 'DXVK-Async',
    sarek: 'DXVK-Sarek',
    sarekasync: 'DXVK-Sarek-Async',
  };
  return labels[variant] || `DXVK-${variant}`;
};

const dxvkGroupedLocalVersions = computed(() => {
  const groups = new Map<string, DxvkLocalVersion[]>();
  for (const item of dxvkLocalVersions.value) {
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

const buildEmptyDxvkStatus = (): DxvkInstalledStatus => ({
  installed: false,
  version: null,
  dlls_found: [],
});

const loadDxvkState = async (gameName: string, sessionId: number) => {
  if (!isEditableGameName(gameName)) {
    dxvkLocalVersions.value = [];
    dxvkInstalledStatus.value = buildEmptyDxvkStatus();
    dxvkSelectedKey.value = '';
    return;
  }
  try {
    const [local, status] = await Promise.all([
      scanLocalDxvk(),
      detectDxvkStatus(gameName),
    ]);
    if (!isActiveLoadSession(sessionId)) return;
    dxvkLocalVersions.value = local;
    dxvkInstalledStatus.value = status;

    if (status.installed && status.version) {
      const match = local.find(lv => lv.version === status.version);
      dxvkSelectedKey.value = match
        ? `${match.version}|${match.variant}`
        : `${status.version}|dxvk`;
    } else if (local.length > 0 && !dxvkSelectedKey.value) {
      dxvkSelectedKey.value = `${local[0].version}|${local[0].variant}`;
    }
  } catch (e) {
    if (!isActiveLoadSession(sessionId)) return;
    dxvkLocalVersions.value = [];
    dxvkInstalledStatus.value = buildEmptyDxvkStatus();
    dxvkSelectedKey.value = '';
  }
};

const doInstallDxvk = async () => {
  if (isDxvkBusy.value || !dxvkSelectedKey.value) return;
  const [version, variant] = dxvkSelectedKey.value.split('|');
  if (!version || !variant) return;
  try {
    isDxvkBusy.value = true;
    const label = dxvkVariantLabel(variant);
    notify?.info(label, tr('gamesettingsmodal.messages.applyingVersion', `正在应用 ${label} ${version}...`).replace('{label}', label).replace('{version}', version));
    const result = await installDxvk(props.gameName, version, variant);
    notify?.success(tr('gamesettingsmodal.messages.applyDone', `${label} 应用完成`).replace('{label}', label), result);
    await loadDxvkState(normalizeGameName(props.gameName), activeLoadSession.value);
  } catch (e) {
    notify?.error(tr('gamesettingsmodal.messages.dxvkApplyFailedTitle', 'DXVK 应用失败'), `${e}`);
  } finally {
    isDxvkBusy.value = false;
  }
};

const doUninstallDxvk = async () => {
  if (isDxvkBusy.value) return;
  const confirmed = await askConfirm(tr('gamesettingsmodal.messages.dxvkUninstallConfirm', '确定要从当前 Prefix 中卸载 DXVK 吗？'), { title: 'DXVK', kind: 'warning' });
  if (!confirmed) return;
  try {
    isDxvkBusy.value = true;
    const result = await uninstallDxvk(props.gameName);
    notify?.success(tr('gamesettingsmodal.messages.dxvkUninstallDone', 'DXVK 卸载完成'), result);
    await loadDxvkState(normalizeGameName(props.gameName), activeLoadSession.value);
  } catch (e) {
    notify?.error(tr('gamesettingsmodal.messages.dxvkUninstallFailed', 'DXVK 卸载失败'), `${e}`);
  } finally {
    isDxvkBusy.value = false;
  }
};

// VKD3D
const vkd3dLocalVersions = ref<Vkd3dLocalVersion[]>([]);
const vkd3dInstalledStatus = ref<Vkd3dInstalledStatus | null>(null);
const vkd3dSelectedVersion = ref('');
const isVkd3dBusy = ref(false);

const buildEmptyVkd3dStatus = (): Vkd3dInstalledStatus => ({
  installed: false,
  version: null,
  dlls_found: [],
});

const loadVkd3dState = async (gameName: string, sessionId: number) => {
  if (!isEditableGameName(gameName)) {
    vkd3dLocalVersions.value = [];
    vkd3dInstalledStatus.value = buildEmptyVkd3dStatus();
    vkd3dSelectedVersion.value = '';
    return;
  }
  try {
    const [local, status] = await Promise.all([
      scanLocalVkd3d(),
      detectVkd3dStatus(gameName),
    ]);
    if (!isActiveLoadSession(sessionId)) return;
    vkd3dLocalVersions.value = local;
    vkd3dInstalledStatus.value = status;

    if (status.installed && status.version) {
      vkd3dSelectedVersion.value = status.version;
    } else if (local.length > 0 && !vkd3dSelectedVersion.value) {
      vkd3dSelectedVersion.value = local[0].version;
    }
  } catch (e) {
    if (!isActiveLoadSession(sessionId)) return;
    vkd3dLocalVersions.value = [];
    vkd3dInstalledStatus.value = buildEmptyVkd3dStatus();
    vkd3dSelectedVersion.value = '';
  }
};

const doInstallVkd3d = async () => {
  if (isVkd3dBusy.value || !vkd3dSelectedVersion.value) return;
  try {
    isVkd3dBusy.value = true;
    notify?.info('VKD3D-Proton', tr('gamesettingsmodal.messages.vkd3dApplying', `正在应用 VKD3D-Proton ${vkd3dSelectedVersion.value}...`).replace('{version}', vkd3dSelectedVersion.value));
    const result = await installVkd3d(props.gameName, vkd3dSelectedVersion.value);
    notify?.success(tr('gamesettingsmodal.messages.vkd3dApplyDone', 'VKD3D 应用完成'), result);
    await loadVkd3dState(normalizeGameName(props.gameName), activeLoadSession.value);
  } catch (e) {
    notify?.error(tr('gamesettingsmodal.messages.vkd3dApplyFailed', 'VKD3D 应用失败'), `${e}`);
  } finally {
    isVkd3dBusy.value = false;
  }
};

const doUninstallVkd3d = async () => {
  if (isVkd3dBusy.value) return;
  const confirmed = await askConfirm(tr('gamesettingsmodal.messages.vkd3dUninstallConfirm', '确定要从当前 Prefix 中卸载 VKD3D 吗？'), { title: 'VKD3D', kind: 'warning' });
  if (!confirmed) return;
  try {
    isVkd3dBusy.value = true;
    const result = await uninstallVkd3d(props.gameName);
    notify?.success(tr('gamesettingsmodal.messages.vkd3dUninstallDone', 'VKD3D 卸载完成'), result);
    await loadVkd3dState(normalizeGameName(props.gameName), activeLoadSession.value);
  } catch (e) {
    notify?.error(tr('gamesettingsmodal.messages.vkd3dUninstallFailed', 'VKD3D 卸载失败'), `${e}`);
  } finally {
    isVkd3dBusy.value = false;
  }
};

// Tabs
const activeTab = ref('info');
const globalMigotoEnabled = computed(() => !!appSettings.migotoEnabled);
const currentGameMigotoSupported = computed(() => {
  const gameName =
    normalizeGameName(props.gameName) || normalizeGameName(config.basic.gamePreset);
  if (!gameName) return false;
  return gamesList.find((game) => game.name === gameName)?.migotoSupported ?? false;
});
const tabs = computed(() => {
  const baseTabs = [
    { id: 'info', label: tr('gamesettingsmodal.tabs.info', '游戏信息') },
    { id: 'game', label: tr('gamesettingsmodal.tabs.game', '游戏选项') },
    { id: 'runtime', label: tr('gamesettingsmodal.tabs.runtime', '运行环境') },
    { id: 'system', label: tr('gamesettingsmodal.tabs.system', '系统选项') },
  ];

  if (globalMigotoEnabled.value) {
    baseTabs.splice(2, 0, { id: 'migoto', label: t('gamesettingsmodal.migoto.tabLabel') });
  }

  return baseTabs;
});

const migotoEnabled = ref(false);

const loadMigotoConfig = () => {
  migotoEnabled.value = currentGameMigotoSupported.value && !!config.other?.migoto?.enabled;
};

const saveMigotoEnabled = async () => {
  const gameName = normalizeGameName(props.gameName);
  if (!isEditableGameName(gameName)) return;

  if (!config.other) config.other = {};
  if (!config.other.migoto || !isRecord(config.other.migoto)) {
    config.other.migoto = {};
  }
  if (!currentGameMigotoSupported.value) {
    config.other.migoto.enabled = false;
    migotoEnabled.value = false;
    notify?.warning(
      tr('gamesettingsmodal.migoto.unsupportedTitle', '当前游戏暂不支持'),
      tr(
        'gamesettingsmodal.migoto.unsupportedHint',
        '当前游戏暂不支持 3DMigoto / Mod 加载，因此无法开启。',
      ),
    );
    return;
  }

  try {
    const latestRaw = await apiLoadGameConfig(gameName);
    const latestConfig = normalizeLoadedConfig(latestRaw);
    if (!latestConfig.other) latestConfig.other = {};
    const latestMigoto = isRecord(latestConfig.other.migoto)
      ? latestConfig.other.migoto
      : {};
    latestConfig.other.migoto = {
      ...latestMigoto,
      enabled: migotoEnabled.value,
    };
    await apiSaveGameConfig(gameName, latestConfig);

    config.other.migoto = {
      ...(isRecord(config.other.migoto) ? config.other.migoto : {}),
      ...(latestConfig.other.migoto as Record<string, unknown>),
    };
    notify?.success(t('gamesettingsmodal.migoto.saved'), '');
  } catch (e) {
    notify?.error(t('gamesettingsmodal.migoto.saveFailed'), `${e}`);
  }
};

watch(
  currentGameMigotoSupported,
  () => {
    loadMigotoConfig();
  },
);

watch(
  tabs,
  (value) => {
    if (value.some((tab) => tab.id === activeTab.value)) {
      return;
    }
    activeTab.value = 'info';
  },
  { immediate: true, deep: true },
);

const runtimeAttention = ref(false);
const runtimeAttentionMessage = ref('');
const runtimeFocusTarget = ref<RuntimeFocusTarget>('all');
let runtimeAttentionTimer: ReturnType<typeof setTimeout> | null = null;
const runtimeWineVersionRef = ref<HTMLElement | null>(null);
const runtimeDxvkRef = ref<HTMLElement | null>(null);
const runtimeVkd3dRef = ref<HTMLElement | null>(null);
const pendingOpenRequest = ref<GameSettingsOpenRequest | null>(null);

const clearRuntimeAttention = () => {
  runtimeAttention.value = false;
  runtimeAttentionMessage.value = '';
  if (runtimeAttentionTimer) {
    clearTimeout(runtimeAttentionTimer);
    runtimeAttentionTimer = null;
  }
};

const focusRuntimeSetup = (message?: string, focusTarget: RuntimeFocusTarget = 'all') => {
  activeTab.value = 'runtime';
  runtimeFocusTarget.value = focusTarget;
  runtimeAttentionMessage.value =
    message?.trim() || tr('gamesettingsmodal.messages.runtimeAttentionDefault', '请先在此完成运行环境配置（Proton / DXVK / VKD3D）。');
  runtimeAttention.value = false;
  requestAnimationFrame(() => {
    runtimeAttention.value = true;
  });
  if (runtimeAttentionTimer) {
    clearTimeout(runtimeAttentionTimer);
  }
  runtimeAttentionTimer = setTimeout(() => {
    runtimeAttention.value = false;
    runtimeAttentionMessage.value = '';
    runtimeFocusTarget.value = 'all';
    runtimeAttentionTimer = null;
  }, 3600);

  requestAnimationFrame(() => {
    const targetEl =
      focusTarget === 'wine_version'
        ? runtimeWineVersionRef.value
        : focusTarget === 'dxvk'
          ? runtimeDxvkRef.value
          : focusTarget === 'vkd3d'
            ? runtimeVkd3dRef.value
          : null;
    targetEl?.scrollIntoView({ behavior: 'smooth', block: 'center' });
  });
};

const applyOpenRequest = async (request: GameSettingsOpenRequest | null) => {
  if (!request) return;

  if (request.tab === 'runtime') {
    activeTab.value = 'runtime';
    await nextTick();
    focusRuntimeSetup(request.reason, request.runtimeFocus || 'all');
  } else {
    clearRuntimeAttention();
    activeTab.value = request.tab;
  }

  pendingOpenRequest.value = null;
};

const prepareOpen = (request: GameSettingsOpenRequest) => {
  pendingOpenRequest.value = request;
  if (props.modelValue) {
    void applyOpenRequest(request);
  }
};

const syncInfoConfigToLegacyState = () => {
  config.basic.gamePreset = infoConfig.value.meta.gamePreset || config.basic.gamePreset;
  config.basic.runtimeEnv = infoConfig.value.runtime.runtimeEnv;
  config.basic.backgroundType = infoConfig.value.assets.backgroundType;
  config.other.displayName = infoConfig.value.meta.displayName;
};

const saveLegacyConfigPreservingMigoto = async (
  gameName: string,
  nextConfig: GameConfig,
) => {
  const latestRaw = await apiLoadGameConfig(gameName);
  const latestConfig = normalizeLoadedConfig(latestRaw);
  const latestMigoto =
    isRecord(latestConfig.other?.migoto) ? latestConfig.other.migoto : undefined;

  const merged: GameConfig = {
    ...nextConfig,
    basic: { ...nextConfig.basic },
    other: { ...(nextConfig.other || {}) },
  };

  if (latestMigoto) {
    merged.other.migoto = { ...latestMigoto };
  }

  await apiSaveGameConfig(gameName, merged);
};

const loadInfoConfig = async (gameName: string, sessionId: number) => {
  if (!isEditableGameName(gameName)) return;
  try {
    await loadGameInfoState(gameName, () => isActiveLoadSession(sessionId));
    if (!isActiveLoadSession(sessionId)) return;
    syncInfoConfigToLegacyState();
  } catch (e) {
    if (!isActiveLoadSession(sessionId)) return;
    console.error('[GameInfoV2] load failed:', e);
  }
};

const validateConfigNameNow = async () => {
  if (!configName.value.trim()) return;
  try {
    await validateInfoName(configName.value, props.gameName);
  } catch (e) {
    console.error('[GameInfoV2] validate name failed:', e);
  }
};

const onConfigNameInput = (value: string) => {
  configName.value = value;
  void validateConfigNameNow();
};

const onDisplayNameInput = (value: string) => {
  infoConfig.value.meta.displayName = value;
  markInfoDirty('meta');
};

const saveInfoMetaSection = async () => {
  if (!props.gameName) return;
  try {
    await saveInfoMetaRaw(props.gameName);
    syncInfoConfigToLegacyState();
    await loadJadeiteState(props.gameName);
    notify?.success(tr('gamesettingsmodal.messages.successTitle', '保存成功'), tr('gamesettingsmodal.messages.infoSaved', '游戏信息已保存'));
  } catch (e) {
    console.error('[GameInfoV2] save meta failed:', e);
    notify?.error(tr('gamesettingsmodal.message.error.title', '保存失败'), tr('gamesettingsmodal.messages.infoSaveFailed', `游戏信息保存失败: ${e}`).replace('{error}', String(e)));
  }
};

const {
  persistManagedState,
  persistRuntimeSection,
  persistSystemSection,
} = useGameSettingsPersistence({
  normalizeGameName,
  isEditableGameName,
  currentGameName: () => props.gameName,
  isSaveBlocked: () => isLoading.value,
  config,
  protonSettings,
  selectedWineVersionId: () => selectedWineVersionId.value,
  syncSystemOptionsIntoConfig,
  syncInfoConfigToLegacyState,
  saveGameConfig: saveLegacyConfigPreservingMigoto,
  saveWineConfig: setGameWineConfig,
  saveInfoRuntime: saveInfoRuntimeRaw,
});

const saveRuntimeTabSettings = async () => {
  try {
    await persistRuntimeSection();
    notify?.success(tr('gamesettingsmodal.messages.successTitle', '保存成功'), tr('gamesettingsmodal.messages.runtimeSaved', '运行环境配置已保存'));
  } catch (e) {
    notify?.error(tr('gamesettingsmodal.message.error.title', '保存失败'), tr('gamesettingsmodal.messages.runtimeSaveConfigFailed', `运行环境配置保存失败: ${e}`).replace('{error}', String(e)));
  }
};

const saveSystemOptions = async () => {
  try {
    await persistSystemSection();
    notify?.success(tr('gamesettingsmodal.messages.successTitle', '保存成功'), tr('gamesettingsmodal.messages.systemOptionsSaved', '系统选项已保存'));
  } catch (e) {
    notify?.error(tr('gamesettingsmodal.message.error.title', '保存失败'), tr('gamesettingsmodal.messages.systemOptionsSaveFailed', `系统选项保存失败: ${e}`).replace('{error}', String(e)));
  }
};

const refreshPrefixInfoNow = async (gameName: string) => {
  try {
    prefixInfo.value = await getPrefixInfo(gameName);
  } catch {
    prefixInfo.value = {
      game_id: gameName,
      exists: false,
      path: '',
      size_bytes: 0,
      config: null,
    };
  }
};

const pickPrefixDir = async () => {
  try {
    const selected = await openFileDialog({
      directory: true,
      multiple: false,
      title: tr('gamesettingsmodal.gameTab.selectPrefixDirTitle', '选择容器目录'),
    });
    if (selected && typeof selected === 'string') {
      config.other.prefixPath = selected;
    }
  } catch (e) {
    console.error(e);
  }
};

const resetPrefixDirToDefault = () => {
  config.other.prefixPath = '';
};

const saveGameTabSettings = async () => {
  const gameName = normalizeGameName(props.gameName);
  if (!isEditableGameName(gameName)) return;

  try {
    syncInfoConfigToLegacyState();
    const prefixPath = asString(config.other?.prefixPath).trim();
    const previousPrefixPath = prefixInfo.value?.path || null;

    const nextConfig: GameConfig = {
      basic: { ...config.basic },
      other: {
        ...(config.other || {}),
        gamePath: asString(config.other?.gamePath).trim(),
        launchArgs: asString(config.other?.launchArgs).trim(),
        workingDir: asString(config.other?.workingDir).trim(),
      },
    };
    if (prefixPath) {
      nextConfig.other.prefixPath = prefixPath;
    } else {
      delete nextConfig.other.prefixPath;
    }

    if (prefixPath) {
      await setGamePrefixPath(gameName, prefixPath, previousPrefixPath);
    }

    await saveLegacyConfigPreservingMigoto(gameName, nextConfig);

    if (!prefixPath) {
      await setGamePrefixPath(gameName, null, previousPrefixPath);
    }

    config.basic = nextConfig.basic;
    config.other = nextConfig.other;

    await Promise.all([
      refreshGameVersion(),
      refreshPrefixInfoNow(gameName),
      loadGames(),
    ]);

    notify?.success(
      tr('gamesettingsmodal.messages.successTitle', '保存成功'),
      tr('gamesettingsmodal.messages.gameConfigSaved', '游戏选项已保存'),
    );
  } catch (e) {
    notify?.error(
      tr('gamesettingsmodal.message.error.title', '保存失败'),
      tr('gamesettingsmodal.messages.gameConfigSaveFailed', `游戏选项保存失败: ${e}`).replace('{error}', String(e)),
    );
  }
};

const onInfoPresetChange = (presetId: string) => {
  setInfoPreset(presetId);
  config.basic.gamePreset = presetId;
};

const onInfoRuntimeEnvChange = (runtimeEnv: RuntimeEnv) => {
  setInfoRuntimeEnv(runtimeEnv);
  config.basic.runtimeEnv = runtimeEnv;
};

const infoReadonlyWarning = computed(() => {
  if (!infoConfig.value.warningCode) return '';
  return t('gamesettingsmodal.info.readonlyWarning', {
    code: infoConfig.value.warningCode,
  });
});
const infoPageReadOnly = false;
const infoAssetsEditable = true;

const localGameVersion = ref('');
const versionLoading = ref(false);
const versionError = ref('');

const resolveGameFolderFromExePath = (exePath: string): string => {
  const trimmed = exePath.trim();
  if (!trimmed) return '';
  const lastSlash = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
  if (lastSlash <= 0) return '';
  return trimmed.slice(0, lastSlash);
};

const parentFolder = (path: string): string => {
  const trimmed = path.trim().replace(/[\\/]+$/, '');
  if (!trimmed) return '';
  const lastSlash = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
  if (lastSlash <= 0) return '';
  return trimmed.slice(0, lastSlash);
};

const collectVersionProbeFolders = (configuredFolder: string, gamePath: string): string[] => {
  const probes: string[] = [];
  const pushUnique = (value: string) => {
    const normalized = value.trim().replace(/[\\/]+$/, '');
    if (!normalized) return;
    if (!probes.includes(normalized)) {
      probes.push(normalized);
    }
  };

  pushUnique(configuredFolder);
  const exeDir = resolveGameFolderFromExePath(gamePath);
  pushUnique(exeDir);

  let current = exeDir;
  for (let i = 0; i < 3; i += 1) {
    current = parentFolder(current);
    if (!current) break;
    pushUnique(current);
  }

  return probes;
};

const refreshGameVersion = async () => {
  const gameFolderFromConfig = typeof config.other?.gameFolder === 'string' ? config.other.gameFolder.trim() : '';
  const gamePath = typeof config.other?.gamePath === 'string' ? config.other.gamePath : '';
  const probeFolders = collectVersionProbeFolders(gameFolderFromConfig, gamePath);
  if (probeFolders.length === 0) {
    localGameVersion.value = '';
    versionError.value = '';
    return;
  }
  try {
    versionLoading.value = true;
    versionError.value = '';
    localGameVersion.value = '';
    const results = await Promise.allSettled(
      probeFolders.map(folder => getLocalVersion(folder))
    );
    for (const r of results) {
      if (r.status === 'fulfilled' && r.value) {
        localGameVersion.value = r.value;
        break;
      }
    }
  } catch (e) {
    versionError.value = String(e);
    localGameVersion.value = '';
  } finally {
    versionLoading.value = false;
  }
};

const isBusy = computed(() => isLoading.value || infoLoading.value);

const loadConfig = async (gameName: string, sessionId: number) => {
  if (!isEditableGameName(gameName)) return;
  isLoading.value = true;
  hasLoadedConfig.value = false;
  try {
    const data = await apiLoadGameConfig(gameName);
    if (!isActiveLoadSession(sessionId)) return;
    const normalized = normalizeLoadedConfig(data);
    config.basic = normalized.basic;
    config.other = normalized.other || {};
    selectedGpuIndex.value = typeof config.other.gpuIndex === 'number' ? config.other.gpuIndex : -1;
    gameLang.value = typeof config.other.gameLang === 'string' ? config.other.gameLang : '';
    await refreshGameVersion();
    hasLoadedConfig.value = true;
    loadMigotoConfig();
  } catch (e) {
    if (!isActiveLoadSession(sessionId)) return;
    console.error(t('gamesettingsmodal.error.failloadconfig'), e);
  } finally {
    if (isActiveLoadSession(sessionId)) {
      isLoading.value = false;
    }
  }
};

const resetToDefault = async () => {
  const yes = await askConfirm(
    t('gamesettingsmodal.resetconfirm_msg'),
    { title: t('gamesettingsmodal.resetconfirm_title') }
  );
  if (!yes) return;

  try {
    isLoading.value = true;
    const gameName = normalizeGameName(props.gameName);
    const sessionId = startLoadSession();
    await Promise.all([
      apiResetGameIcon(props.gameName),
      apiResetGameBackground(props.gameName),
    ]);
    await apiSaveGameConfig(props.gameName, createDefaultGameConfig(props.gameName));
    await Promise.all([
      loadConfig(gameName, sessionId),
      loadInfoConfig(gameName, sessionId),
      loadGames(),
    ]);
    notify?.success(tr('gamesettingsmodal.messages.resetSuccessTitle', '重置成功'), tr('gamesettingsmodal.messages.resetSuccess', '游戏配置已恢复默认'));
  } catch (e) {
    console.error('Reset failed:', e);
    notify?.error(tr('gamesettingsmodal.messages.resetFailedTitle', '重置失败'), `${e}`);
  } finally {
    isLoading.value = false;
  }
};

const selectIcon = async () => {
  try {
    const file = await openFileDialog({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png'] }]
    });

    if (file) {
      const gameName = normalizeGameName(props.gameName);
      const sessionId = startLoadSession();
      await apiSetGameIcon(props.gameName, file);
      await Promise.all([loadInfoConfig(gameName, sessionId), loadGames()]);
      notify?.success(tr('gamesettingsmodal.messages.iconUpdatedTitle', '图标已更新'), tr('gamesettingsmodal.messages.iconUpdated', '游戏图标设置成功'));
    }
  } catch (e) {
    console.error(e);
    notify?.error(tr('gamesettingsmodal.messages.iconUpdateFailed', '图标设置失败'), `${e}`);
  }
};

const selectBackground = async () => {
  try {
    const filters = [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'svg', 'bmp', 'ico', 'avif'] }];
    const file = await openFileDialog({ multiple: false, filters });
    if (file) {
      const gameName = normalizeGameName(props.gameName);
      const sessionId = startLoadSession();
      await apiSetGameBackground(props.gameName, file, infoConfig.value.assets.backgroundType);
      await Promise.all([loadInfoConfig(gameName, sessionId), loadGames()]);
      notify?.success(tr('gamesettingsmodal.messages.backgroundUpdatedTitle', '背景已更新'), tr('gamesettingsmodal.messages.backgroundUpdated', '背景图片设置成功'));
    }
  } catch (e) {
    console.error(e);
    notify?.error(tr('gamesettingsmodal.messages.backgroundUpdateFailed', '背景设置失败'), `${e}`);
  }
};

const resetBackgroundToDefault = async () => {
  if (!props.gameName) return;
  try {
    const gameName = normalizeGameName(props.gameName);
    const sessionId = startLoadSession();
    await Promise.all([
      apiResetGameIcon(props.gameName),
      apiResetGameBackground(props.gameName),
    ]);
    await Promise.all([
      loadInfoConfig(gameName, sessionId),
      loadGames(),
    ]);
    notify?.success(tr('gamesettingsmodal.messages.restoreSuccessTitle', '恢复成功'), tr('gamesettingsmodal.messages.restoreSuccess', '图标和背景已恢复默认'));
  } catch (e) {
    console.error('[GameInfoV2] reset background failed:', e);
    notify?.error(tr('gamesettingsmodal.messages.restoreFailedTitle', '恢复失败'), `${e}`);
  }
};

const pickGameExe = async () => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: tr('gamesettingsmodal.filePicker.filterName', '可执行文件'), extensions: ['exe', 'sh', 'AppImage', 'desktop', '*'] }],
      title: tr('gamesettingsmodal.filePicker.title', '选择游戏可执行文件')
    });
    if (selected && typeof selected === 'string') {
      config.other.gamePath = selected;
      await refreshGameVersion();
    }
  } catch (e) { console.error(e); }
};

const createNewConfig = async () => {
  if (!configName.value) return;
  const validation = await validateInfoName(configName.value, props.gameName);
  if (!validation.valid) {
    await showMessage(validation.message, {
      title: t('gamesettingsmodal.message.error.title'),
      kind: 'error',
    });
    return;
  }

  const yes = await askConfirm(
    t('gamesettingsmodal.confirm.createConfig.message', { 
      configName: configName.value 
    }),
    {
      title: t('gamesettingsmodal.confirm.createConfig.title'),
      kind: 'info',
    }
  );
  if (!yes) return;

  try {
    isLoading.value = true;
    await apiCreateNewConfig(configName.value, config);

    await loadGames();

    const newGame = gamesList.find(g => g.name === canonicalPreset(configName.value));
    if (newGame) {
      switchToGame(newGame);
    }

    notify?.success(tr('gamesettingsmodal.messages.createSuccessTitle', '创建成功'), tr('gamesettingsmodal.messages.configCreated', `配置 "${configName.value}" 已创建`).replace('{name}', configName.value));
    await close();
  } catch (e) {
    console.error(t('gamesettingsmodal.log.configCreateFailed', { error: e }));
    notify?.error(tr('gamesettingsmodal.messages.createFailedTitle', '创建失败'), `${e}`);
  } finally {
    isLoading.value = false;
  }
};

const deleteCurrentConfig = async () => {
  if (!props.gameName) return;

  const yes = await askConfirm(tr('gamesettingsmodal.messages.deleteConfirm', `确定要删除配置 "${props.gameName}" 吗？此操作不可逆。`).replace('{name}', props.gameName), {
    title: tr('gamesettingsmodal.messages.deleteConfirmTitle', '删除确认'),
    kind: 'warning',
  });
  if (!yes) return;

  try {
    isLoading.value = true;
    await apiDeleteGameConfigFolder(props.gameName);

    await loadGames();
    notify?.success(tr('gamesettingsmodal.messages.deleteSuccessTitle', '删除成功'), tr('gamesettingsmodal.messages.configDeleted', `配置 "${props.gameName}" 已删除`).replace('{name}', props.gameName));
    await close();
  } catch (e) {
    console.error('Failed to delete config:', e);
    notify?.error(tr('gamesettingsmodal.messages.deleteFailedTitle', '删除失败'), `${e}`);
  } finally {
    isLoading.value = false;
  }
};

const loadAllSections = () => {
  return loadManagedSectionGroups(
    props.gameName,
    [
      [loadConfig, loadInfoConfig],
      [
        loadWineState,
        loadJadeiteState,
        loadPrefixState,
        loadDxvkState,
        loadVkd3dState,
      ],
    ],
    {
      beforeLoad: (gameName) => {
        configName.value = gameName;
      },
    },
  );
};

const persistManagedSections = async (targetGameName = props.gameName) => {
  await saveManagedSections(targetGameName, async (gameName) => {
    await persistManagedState(gameName);
  });
};

watch(() => props.modelValue, async (val) => {
  if (val) {
    activeTab.value = 'info';
    await loadAllSections();
    await nextTick();
    await applyOpenRequest(pendingOpenRequest.value);
  } else {
    startLoadSession();
    clearRuntimeAttention();
    await persistManagedSections();
  }
}, { immediate: true });

watch(() => props.gameName, async (newGame, oldGame) => {
  await handleGameNameChange({
    newGame,
    oldGame,
    isModalOpen: props.modelValue,
    hasLoadedConfig: hasLoadedConfig.value,
    hasUnsavedInfoChanges: hasUnsavedInfoChanges.value,
    saveManagedSections: persistManagedSections,
    loadAllSections,
    revertToGame: (gameName) => {
      const previousGame = gamesList.find((game) => game.name === gameName);
      if (previousGame) {
        switchToGame(previousGame);
      }
    },
  });
});

const close = async () => {
  await requestClose({
    hasUnsavedInfoChanges: hasUnsavedInfoChanges.value,
    onClose: () => {
      clearRuntimeAttention();
      emit('update:modelValue', false);
    },
  });
};

defineExpose({
  prepareOpen,
  switchTab: (tabId: GameSettingsTab) => {
    pendingOpenRequest.value = null;
    if (tabId !== 'runtime') {
      clearRuntimeAttention();
    }
    activeTab.value = tabId;
  },
  focusRuntimeSetup,
});
</script>

<template>
  <transition name="modal-fade">
    <div v-if="modelValue" class="settings-overlay">
      <div class="settings-window glass-panel" data-onboarding="game-settings-modal-root">
        <div v-if="isBusy" class="loading-overlay">
          <div class="spinner"></div>
          <div class="loading-text">{{ t('gamesettingsmodal.processing') }}</div>
        </div>

        <div class="settings-sidebar" data-onboarding="game-settings-sidebar">
          <div class="sidebar-title">{{ t('gamesettingsmodal.title') }}</div>

          <div
            v-for="tab in tabs"
            :key="tab.id"
            class="sidebar-item"
            :class="{ active: activeTab === tab.id, 'runtime-attention': tab.id === 'runtime' && runtimeAttention }"
            :data-onboarding="`game-settings-tab-${tab.id}`"
            @click="activeTab = tab.id">
            {{ tab.label }}
          </div>
        </div>

        <div class="settings-content">
          <div class="content-header">
            <span class="header-title">{{tabs.find(t => t.id === activeTab)?.label}}</span>
            <div class="close-btn" @click="close">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </div>
          </div>

          <div class="scroll-content">
            <div v-show="activeTab === 'info'" class="tab-pane" data-onboarding="game-settings-info-tab">
              <div v-if="infoConfig.readOnly || infoPageReadOnly" class="info-readonly-banner">
                <template v-if="infoPageReadOnly">
                  {{ tr('gamesettingsmodal.info.readonlyBanner', '游戏信息页当前为只读展示模式（仅背景资源可修改）。') }}
                </template>
                <template v-else>
                {{ infoReadonlyWarning || t('gamesettingsmodal.info.readonlyGeneric') }}
                </template>
              </div>

              <div data-onboarding="game-settings-info-profile">
                <GameInfoProfileSection
                  :config-name="configName"
                  :display-name="infoConfig.meta.displayName"
                  :name-validation="nameValidation"
                  :read-only="infoPageReadOnly"
                  :can-save="!infoConfig.readOnly && !infoPageReadOnly"
                  :dirty="infoDirty.meta"
                  :saving="infoSaving.meta"
                  :error="infoSectionErrors.meta"
                  @update:config-name="onConfigNameInput"
                  @update:display-name="onDisplayNameInput"
                  @validate-name="validateConfigNameNow"
                  @create="createNewConfig"
                  @delete="deleteCurrentConfig"
                  @reset="resetToDefault"
                  @save="saveInfoMetaSection"
                />
              </div>

              <GameInfoVersionSection
                :version="localGameVersion"
                :game-path="config.other.gamePath || ''"
                :loading="versionLoading"
                :error="versionError"
                @refresh="refreshGameVersion"
              />

              <GameInfoPresetSection
                :model-value="infoConfig.meta.gamePreset"
                :presets="infoPresets"
                :read-only="infoPageReadOnly"
                :can-save="!infoConfig.readOnly && !infoPageReadOnly"
                :dirty="infoDirty.meta"
                :saving="infoSaving.meta"
                :error="infoSectionErrors.meta"
                @update:model-value="onInfoPresetChange"
                @save="saveInfoMetaSection"
              />

              <GameInfoAssetsSection
                :icon-file="infoConfig.assets.iconFile"
                :background-file="infoConfig.assets.backgroundFile"
                :read-only="infoConfig.readOnly || !infoAssetsEditable"
                :saving="infoSaving.assets"
                :error="infoSectionErrors.assets"
                @select-icon="selectIcon"
                @select-background="selectBackground"
                @reset-background="resetBackgroundToDefault"
              />
            </div>

            <div v-show="activeTab === 'game'" class="tab-pane" data-onboarding="game-settings-game-tab">
              <div class="setting-group" data-onboarding="game-settings-game-exe">
                <div class="setting-label">{{ tr('gamesettingsmodal.gameTab.mainExe', '主程序') }}</div>
                <input v-model="config.other.gamePath" type="text" class="custom-input" :placeholder="tr('gamesettingsmodal.gameTab.mainExePlaceholder', '选择游戏可执行文件（如 StarRail.exe）...')" />
                <div class="button-row">
                  <button class="action-btn" @click="pickGameExe">{{ tr('gamesettingsmodal.gameTab.selectFile', '选择文件') }}</button>
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.gameTab.launchArgs', '启动参数') }}</div>
                <input v-model="config.other.launchArgs" type="text" class="custom-input" :placeholder="tr('gamesettingsmodal.gameTab.launchArgsPlaceholder', '可选，如 -screen-fullscreen 0 -popupwindow')" />
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.gameTab.workingDir', '工作目录') }}</div>
                <input v-model="config.other.workingDir" type="text" class="custom-input" :placeholder="tr('gamesettingsmodal.gameTab.workingDirPlaceholder', '留空则使用主程序所在目录')" />
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.gameTab.prefixDir', '容器目录（Wine Prefix）') }}</div>
                <input
                  v-model="config.other.prefixPath"
                  type="text"
                  class="custom-input"
                  :placeholder="tr('gamesettingsmodal.gameTab.prefixPathPlaceholder', '留空则自动使用默认容器目录')"
                />
                <div class="button-row">
                  <button class="action-btn" @click="pickPrefixDir">{{ tr('gamesettingsmodal.gameTab.selectFolder', '选择目录') }}</button>
                  <button class="action-btn" @click="resetPrefixDirToDefault">{{ tr('gamesettingsmodal.gameTab.useDefaultPrefix', '使用默认容器目录') }}</button>
                </div>
                <div class="info-sub" style="margin-top: 8px;">
                  {{ tr('gamesettingsmodal.gameTab.prefixPathHint', '下方显示当前生效的容器目录；留空时将继续使用软件自动推导并创建默认容器。') }}
                </div>
                <div class="info-text glass-card" v-if="prefixInfo">
                  <span :class="prefixInfo.exists ? 'text-ok' : 'text-err'">
                    {{ prefixInfo.exists ? tr('gamesettingsmodal.gameTab.prefixCreated', '✓ 已创建') : tr('gamesettingsmodal.gameTab.prefixNotCreated', '✗ 未创建（首次启动时自动创建）') }}
                  </span>
                  <div class="wine-path">{{ prefixInfo.path }}</div>
                  <div v-if="prefixInfo.exists && prefixInfo.size_bytes > 0" class="info-sub">
                    {{ tr('gamesettingsmodal.gameTab.sizeLabel', '大小') }}：{{ (prefixInfo.size_bytes / 1024 / 1024).toFixed(1) }} MB
                  </div>
                </div>
                <div v-else class="info-text text-muted">{{ tr('gamesettingsmodal.gameTab.loading', '加载中...') }}</div>
              </div>

              <div v-if="isHoyoverse" class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.gameTab.jadeiteTitle', 'Jadeite 反作弊补丁') }}</div>
                <div class="info-text glass-card" v-if="jadeiteStatus">
                  <span :class="jadeiteStatus.installed ? 'text-ok' : 'text-err'">
                    {{ jadeiteStatus.installed ? tr('gamesettingsmodal.gameTab.jadeiteInstalled', `✓ 已安装 (v${jadeiteStatus.localVersion})`).replace('{version}', jadeiteStatus.localVersion || '') : tr('gamesettingsmodal.gameTab.jadeiteNotInstalled', '✗ 未安装（HoYoverse 游戏必需）') }}
                  </span>
                  <div class="wine-path">{{ jadeiteStatus.patchDir }}</div>
                </div>
                <div class="button-row">
                  <button class="action-btn highlight" @click="doInstallJadeite" :disabled="isJadeiteInstalling">
                    {{ isJadeiteInstalling ? tr('gamesettingsmodal.gameTab.installing', '安装中...') : (jadeiteStatus?.installed ? tr('gamesettingsmodal.gameTab.updateJadeite', '更新 Jadeite') : tr('gamesettingsmodal.gameTab.installJadeite', '安装 Jadeite')) }}
                  </button>
                </div>
                <div class="info-sub" style="margin-top:6px;">
                  {{ tr('gamesettingsmodal.gameTab.jadeiteHint', 'Jadeite 用于在 Linux 上绕过 HoYoverse 反作弊，启动时自动通过 jadeite.exe 包装游戏。') }}
                </div>
              </div>

              <div class="button-row">
                <button class="action-btn highlight" @click="saveGameTabSettings">
                  {{ tr('gamesettingsmodal.gameTab.save', '保存游戏选项') }}
                </button>
              </div>
            </div>

            <div v-show="activeTab === 'migoto'" class="tab-pane" data-onboarding="game-settings-migoto-tab">

              <div class="setting-group">
                <div class="setting-checkbox-row glass-card">
                  <label class="checkbox-label" style="font-size: 15px; font-weight: 600;">
                    <input
                      type="checkbox"
                      v-model="migotoEnabled"
                      :disabled="!currentGameMigotoSupported"
                    />
                    {{ t('gamesettingsmodal.migoto.enabled') }}
                  </label>
                  <div class="info-sub" style="margin-top:8px;">
                    {{
                      currentGameMigotoSupported
                        ? (migotoEnabled
                          ? t('gamesettingsmodal.migoto.enabledHint')
                          : t('gamesettingsmodal.migoto.disabledHint'))
                        : tr(
                          'gamesettingsmodal.migoto.unsupportedHint',
                          '当前游戏暂不支持 3DMigoto / Mod 加载，因此无法开启。',
                        )
                    }}
                  </div>
                </div>
              </div>

              <div class="setting-group" style="margin-top: 16px;">
                <div class="button-row">
                  <button
                    class="action-btn highlight"
                    @click="saveMigotoEnabled"
                    :disabled="!currentGameMigotoSupported"
                  >
                    {{ t('gamesettingsmodal.migoto.save') }}
                  </button>
                </div>
                <div class="info-sub" style="margin-top: 12px;">
                  <span v-html="tr('gamesettingsmodal.migoto.gotoSettingsHint', '如需配置 3DMigoto 路径、注入方式、Mod 文件夹等详细设置，请前往<strong>「设置 → 3DMIGOTO 管理」</strong>页面。')"></span>
                </div>
              </div>
            </div>

            <div
              v-show="activeTab === 'runtime'"
              class="tab-pane"
              data-onboarding="game-settings-runtime-tab"
              :class="{ 'runtime-pane-attention': runtimeAttention && runtimeFocusTarget === 'all' }"
            >
              <div v-if="runtimeAttentionMessage" class="runtime-guide-banner">
                {{ runtimeAttentionMessage }}
              </div>
              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.info.runtimeEnv') }}</div>
                <el-select
                  :model-value="infoConfig.runtime.runtimeEnv"
                  :placeholder="t('gamesettingsmodal.info.runtimeEnvPlaceholder')"
                  class="custom-select"
                  @update:model-value="(value: RuntimeEnv) => onInfoRuntimeEnvChange(value)"
                >
                  <el-option :label="t('gamesettingsmodal.info.runtimeWine')" value="wine" />
                  <el-option :label="t('gamesettingsmodal.info.runtimeSteam')" value="steam" />
                  <el-option :label="t('gamesettingsmodal.info.runtimeLinux')" value="linux" />
                </el-select>
                <div class="info-sub" style="margin-top:6px;">
                  <template v-if="infoConfig.runtime.runtimeEnv === 'wine'">{{ t('gamesettingsmodal.info.runtimeWineHint') }}</template>
                  <template v-else-if="infoConfig.runtime.runtimeEnv === 'steam'">{{ t('gamesettingsmodal.info.runtimeSteamHint') }}</template>
                  <template v-else>{{ t('gamesettingsmodal.info.runtimeLinuxHint') }}</template>
                </div>
              </div>

              <div
                ref="runtimeWineVersionRef"
                class="setting-group"
                data-onboarding="game-settings-runtime-wine"
                :class="{ 'runtime-section-attention': runtimeAttention && runtimeFocusTarget === 'wine_version' }"
              >
                <div class="setting-label">{{ tr('gamesettingsmodal.runtimeTab.wineVersionTitle', 'Wine / Proton 版本（本地已安装）') }}</div>
                <el-select v-model="selectedWineVersionId" :placeholder="tr('gamesettingsmodal.runtimeTab.wineVersionPlaceholder', '选择 Wine/Proton 版本...')" class="custom-select" filterable style="width: 100%">
                  <el-option-group
                    v-for="group in [
                      { label: 'GE-Proton', items: wineVersions.filter(v => v.variant === 'geproton') },
                      { label: 'DW-Proton', items: wineVersions.filter(v => v.variant === 'dwproton') },
                      { label: 'Proton Official', items: wineVersions.filter(v => v.variant === 'official') },
                      { label: 'Proton Experimental', items: wineVersions.filter(v => v.variant === 'experimental') },
                      { label: 'Proton-TKG', items: wineVersions.filter(v => v.variant === 'protontkg') },
                      { label: 'Lutris Wine', items: wineVersions.filter(v => v.variant === 'lutris') },
                      { label: 'System Wine', items: wineVersions.filter(v => v.variant === 'systemwine') },
                      { label: 'Custom', items: wineVersions.filter(v => v.variant === 'custom') },
                    ].filter(g => g.items.length > 0)"
                    :key="group.label"
                    :label="group.label"
                  >
                    <el-option v-for="ver in group.items" :key="ver.id" :label="`${ver.name} (${ver.version})`" :value="ver.id" />
                  </el-option-group>
                </el-select>
                <div v-if="selectedWineVersion" class="wine-detail glass-card">
                  <span class="badge">{{ variantLabel(selectedWineVersion.variant) }}</span>
                  <span class="wine-path">{{ selectedWineVersion.path }}</span>
                </div>
                <div class="info-sub" style="margin-top:6px;">
                  {{ tr('gamesettingsmodal.runtimeTab.wineVersionCount', '共检测到 {count} 个本地 Wine/Proton 版本。').replace('{count}', String(wineVersions.length)) }}
                </div>
              </div>

              <div class="setting-group">
                <div class="info-sub" style="margin-top:4px;">
                  {{ tr('gamesettingsmodal.runtimeTab.protonHint', '如需下载更多 Proton 版本，请前往「设置 → Proton 管理」页面。') }}
                </div>
              </div>

              <div
                ref="runtimeDxvkRef"
                class="setting-group"
                data-onboarding="game-settings-runtime-dxvk"
                :class="{ 'runtime-section-attention': runtimeAttention && runtimeFocusTarget === 'dxvk' }"
              >
                <div class="setting-label">{{ tr('gamesettingsmodal.runtimeTab.dxvkTitle', 'DXVK (DirectX → Vulkan)') }}</div>

                <div class="info-card glass-card" style="margin-bottom: 10px;">
                  <div v-if="dxvkInstalledStatus" class="info-grid" style="grid-template-columns: 100px 1fr;">
                    <span class="info-key">{{ tr('gamesettingsmodal.runtimeTab.installStatus', '安装状态') }}</span>
                    <span :class="dxvkInstalledStatus.installed ? 'text-ok' : 'text-err'">
                      {{ dxvkInstalledStatus.installed ? tr('gamesettingsmodal.runtimeTab.installed', '✓ 已安装') : tr('gamesettingsmodal.runtimeTab.notInstalled', '✗ 未安装') }}
                    </span>
                    <template v-if="dxvkInstalledStatus.installed">
                      <span class="info-key">{{ tr('gamesettingsmodal.runtimeTab.currentVersion', '当前版本') }}</span>
                      <span class="info-val">{{ dxvkInstalledStatus.version || tr('gamesettingsmodal.runtimeTab.unknown', '未知') }}</span>
                      <span class="info-key">{{ tr('gamesettingsmodal.runtimeTab.dllFiles', 'DLL 文件') }}</span>
                      <span class="info-val">{{ dxvkInstalledStatus.dlls_found.join(', ') }}</span>
                    </template>
                  </div>
                  <div v-else class="text-muted" style="font-size:13px">{{ tr('gamesettingsmodal.runtimeTab.loading', '加载中...') }}</div>
                </div>

                <div v-if="dxvkLocalVersions.length > 0">
                  <div class="flex-row" style="align-items:flex-end; gap:8px; margin-top:8px;">
                    <div style="flex:1">
                      <select v-model="dxvkSelectedKey" class="custom-input" style="width:100%">
                        <option value="" disabled>{{ tr('gamesettingsmodal.runtimeTab.selectLocalDxvk', '选择本地已缓存的 DXVK 版本...') }}</option>
                        <optgroup
                          v-for="group in dxvkGroupedLocalVersions"
                          :key="group.variant"
                          :label="group.label"
                        >
                          <option
                            v-for="lv in group.items"
                            :key="`${lv.version}|${lv.variant}`"
                            :value="`${lv.version}|${lv.variant}`"
                          >
                            {{ lv.version }}
                          </option>
                        </optgroup>
                      </select>
                    </div>
                  </div>
                  <div class="button-row" style="margin-top:8px;">
                    <button class="action-btn highlight" @click="doInstallDxvk" :disabled="isDxvkBusy || !dxvkSelectedKey">
                      {{ isDxvkBusy ? tr('gamesettingsmodal.runtimeTab.applying', '应用中...') : tr('gamesettingsmodal.runtimeTab.applyOrSwitch', '应用 / 切换版本') }}
                    </button>
                    <button class="action-btn delete" @click="doUninstallDxvk" :disabled="isDxvkBusy || !dxvkInstalledStatus?.installed">
                      {{ tr('gamesettingsmodal.runtimeTab.uninstallDxvk', '卸载 DXVK') }}
                    </button>
                  </div>
                </div>

                <div class="info-sub" style="margin-top:8px;">
                  {{ tr('gamesettingsmodal.runtimeTab.dxvkHint', '如需下载更多 DXVK 版本，请前往「设置 → DXVK 管理」页面。') }}
                </div>
              </div>

              <div
                ref="runtimeVkd3dRef"
                class="setting-group"
                data-onboarding="game-settings-runtime-vkd3d"
                :class="{ 'runtime-section-attention': runtimeAttention && runtimeFocusTarget === 'vkd3d' }"
              >
                <div class="setting-label">{{ tr('gamesettingsmodal.runtimeTab.vkd3dTitle', 'VKD3D-Proton (D3D12 → Vulkan)') }}</div>

                <div class="info-card glass-card" style="margin-bottom: 10px;">
                  <div v-if="vkd3dInstalledStatus" class="info-grid" style="grid-template-columns: 100px 1fr;">
                    <span class="info-key">{{ tr('gamesettingsmodal.runtimeTab.installStatus', '安装状态') }}</span>
                    <span :class="vkd3dInstalledStatus.installed ? 'text-ok' : 'text-err'">
                      {{ vkd3dInstalledStatus.installed ? tr('gamesettingsmodal.runtimeTab.installed', '✓ 已安装') : tr('gamesettingsmodal.runtimeTab.notInstalled', '✗ 未安装') }}
                    </span>
                    <template v-if="vkd3dInstalledStatus.installed">
                      <span class="info-key">{{ tr('gamesettingsmodal.runtimeTab.currentVersion', '当前版本') }}</span>
                      <span class="info-val">{{ vkd3dInstalledStatus.version || tr('gamesettingsmodal.runtimeTab.unknown', '未知') }}</span>
                      <span class="info-key">{{ tr('gamesettingsmodal.runtimeTab.dllFiles', 'DLL 文件') }}</span>
                      <span class="info-val">{{ vkd3dInstalledStatus.dlls_found.join(', ') }}</span>
                    </template>
                  </div>
                  <div v-else class="text-muted" style="font-size:13px">{{ tr('gamesettingsmodal.runtimeTab.loading', '加载中...') }}</div>
                </div>

                <div v-if="vkd3dLocalVersions.length > 0">
                  <div class="flex-row" style="align-items:flex-end; gap:8px; margin-top:8px;">
                    <div style="flex:1">
                      <select v-model="vkd3dSelectedVersion" class="custom-input" style="width:100%">
                        <option value="" disabled>{{ tr('gamesettingsmodal.runtimeTab.selectLocalVkd3d', '选择本地已缓存的 VKD3D 版本...') }}</option>
                        <option
                          v-for="lv in vkd3dLocalVersions"
                          :key="lv.version"
                          :value="lv.version"
                        >
                          {{ lv.version }}
                        </option>
                      </select>
                    </div>
                  </div>
                  <div class="button-row" style="margin-top:8px;">
                    <button class="action-btn highlight" @click="doInstallVkd3d" :disabled="isVkd3dBusy || !vkd3dSelectedVersion">
                      {{ isVkd3dBusy ? tr('gamesettingsmodal.runtimeTab.applying', '应用中...') : tr('gamesettingsmodal.runtimeTab.applyOrSwitch', '应用 / 切换版本') }}
                    </button>
                    <button class="action-btn delete" @click="doUninstallVkd3d" :disabled="isVkd3dBusy || !vkd3dInstalledStatus?.installed">
                      {{ tr('gamesettingsmodal.runtimeTab.uninstallVkd3d', '卸载 VKD3D') }}
                    </button>
                  </div>
                </div>
                <div v-else class="info-sub" style="margin-top:8px;">
                  {{ tr('gamesettingsmodal.runtimeTab.noLocalVkd3d', '本地暂无缓存 VKD3D 版本，请前往「设置 → VKD3D 管理」下载后再应用。') }}
                </div>

                <div class="info-sub" style="margin-top:8px;">
                  {{ tr('gamesettingsmodal.runtimeTab.vkd3dOptionalHint', 'VKD3D 默认不强制安装，仅在需要 D3D12 转译时建议启用。') }}
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.runtimeTab.protonSettings', 'Proton 设置') }}</div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.use_umu_run" /> {{ tr('gamesettingsmodal.runtimeTab.useUmuRun', '使用 umu-run 启动（鸣潮默认开启）') }}</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.use_pressure_vessel" /> {{ tr('gamesettingsmodal.runtimeTab.usePressureVessel', '使用 Pressure Vessel 容器') }}</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.proton_enable_wayland" /> {{ tr('gamesettingsmodal.runtimeTab.enableWayland', '启用 Wayland') }}</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.proton_no_d3d12" /> {{ tr('gamesettingsmodal.runtimeTab.disableD3d12', '禁用 D3D12') }}</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.proton_media_use_gst" /> {{ tr('gamesettingsmodal.runtimeTab.useGstreamer', '使用 GStreamer 媒体') }}</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.mangohud" /> {{ tr('gamesettingsmodal.runtimeTab.enableMangoHud', 'MangoHud 性能覆盖') }}</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.steam_deck_compat" /> {{ tr('gamesettingsmodal.runtimeTab.steamDeckCompat', 'Steam Deck 兼容模式') }}</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.steamos_compat" /> {{ tr('gamesettingsmodal.runtimeTab.steamosCompat', 'SteamOS 兼容模式') }}</label>
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.runtimeTab.dxvkGraphics', 'DXVK / 图形设置') }}</div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.dxvk_async" /> {{ tr('gamesettingsmodal.runtimeTab.dxvkAsync', 'DXVK 异步着色器编译') }}</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.disable_gpu_filter" /> {{ tr('gamesettingsmodal.runtimeTab.disableGpuFilter', '禁用 GPU 自动过滤') }}</label>
                </div>
                <div class="setting-inline-row">
                  <span class="setting-inline-label">DXVK HUD</span>
                  <select v-model="protonSettings.dxvk_hud" class="custom-input" style="flex: 1;">
                    <option value="">{{ tr('gamesettingsmodal.runtimeTab.hudOff', '关闭') }}</option>
                    <option value="version">{{ tr('gamesettingsmodal.runtimeTab.hudVersion', '版本号') }}</option>
                    <option value="fps">{{ tr('gamesettingsmodal.runtimeTab.hudFps', '帧率') }}</option>
                    <option value="version,fps">{{ tr('gamesettingsmodal.runtimeTab.hudVersionFps', '版本 + 帧率') }}</option>
                    <option value="full">{{ tr('gamesettingsmodal.runtimeTab.hudFull', '完整信息') }}</option>
                  </select>
                </div>
                <div class="setting-inline-row">
                  <span class="setting-inline-label">{{ tr('gamesettingsmodal.runtimeTab.frameRateLimit', '帧率限制') }}</span>
                  <input v-model.number="protonSettings.dxvk_frame_rate" type="number" class="custom-input" style="flex: 1;" :placeholder="tr('gamesettingsmodal.runtimeTab.frameRateLimitPlaceholder', '0 = 不限制')" min="0" />
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">Steam App ID</div>
                <input v-model="protonSettings.steam_app_id" type="text" class="custom-input" placeholder="0 = N/A" />
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.runtimeTab.customEnv', '自定义环境变量') }}</div>
                <div v-for="(val, key) in protonSettings.custom_env" :key="key" class="env-row glass-card">
                  <span class="env-key">{{ key }}</span>
                  <span class="env-val">{{ val }}</span>
                  <button class="env-remove" @click="removeCustomEnv(key as string)">✕</button>
                </div>
                <div class="env-add-row">
                  <input v-model="newEnvKey" type="text" class="custom-input env-input" placeholder="KEY" />
                  <input v-model="newEnvValue" type="text" class="custom-input env-input" placeholder="VALUE" />
                  <button class="action-btn" style="flex: 0 0 auto;" @click="addCustomEnv">{{ tr('gamesettingsmodal.runtimeTab.add', '添加') }}</button>
                </div>
              </div>

              <div class="button-row">
                <button class="action-btn highlight" @click="saveRuntimeTabSettings">{{ tr('gamesettingsmodal.runtimeTab.save', '保存运行环境配置') }}</button>
              </div>
            </div>

            <div v-show="activeTab === 'system'" class="tab-pane" data-onboarding="game-settings-system-tab">

              <div v-if="displayInfo" class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.systemTab.systemInfo', '系统信息') }}</div>
                <div class="info-grid glass-card" style="padding: 16px;">
                  <span class="info-key">{{ tr('gamesettingsmodal.systemTab.displayServer', '显示服务器') }}</span>
                  <span class="info-val">{{ displayInfo.server }}{{ displayInfo.wayland_compositor ? ` (${displayInfo.wayland_compositor})` : '' }}</span>
                  <span class="info-key">{{ tr('gamesettingsmodal.systemTab.gpuDriver', 'GPU 驱动') }}</span>
                  <span class="info-val">{{ displayInfo.gpu_driver || tr('gamesettingsmodal.systemTab.unknown', '未知') }}</span>
                  <span class="info-key">Vulkan</span>
                  <span class="info-val" :class="{ 'text-ok': vulkanInfo?.available, 'text-err': !vulkanInfo?.available }">
                    {{ vulkanInfo?.available ? `✓ ${vulkanInfo.version || ''}` : tr('gamesettingsmodal.systemTab.notDetected', '✗ 未检测到') }}
                  </span>
                  <span class="info-key">{{ tr('gamesettingsmodal.systemTab.gamepad', '游戏手柄') }}</span>
                  <span class="info-val">{{ displayInfo.gamepad_detected ? tr('gamesettingsmodal.systemTab.detected', '✓ 已检测') : tr('gamesettingsmodal.systemTab.notDetectedDash', '— 未检测到') }}</span>
                </div>
              </div>

              <div class="setting-group" data-onboarding="game-settings-system-gpu">
                <div class="setting-label">{{ tr('gamesettingsmodal.systemTab.gpuSelect', '指定显卡') }}</div>
                <div v-if="displayInfo && displayInfo.gpus.length > 0">
                  <select v-model="selectedGpuIndex" class="custom-input" style="width:100%">
                    <option value="-1">{{ tr('gamesettingsmodal.systemTab.gpuAuto', '自动（系统默认）') }}</option>
                    <option v-for="gpu in displayInfo.gpus" :key="gpu.index" :value="gpu.index">
                      GPU {{ gpu.index }}: {{ gpu.name }} ({{ gpu.driver }})
                    </option>
                  </select>
                  <div class="info-sub" style="margin-top:6px;">
                    <template v-if="selectedGpuIndex === -1">{{ tr('gamesettingsmodal.systemTab.gpuAutoHint', '使用系统默认 GPU。') }}</template>
                    <template v-else>
                      {{ tr('gamesettingsmodal.systemTab.gpuEnvPrefix', '将通过环境变量') }}
                      <template v-if="displayInfo.gpus[selectedGpuIndex]?.driver === 'nvidia'">
                        <code>__NV_PRIME_RENDER_OFFLOAD=1</code> + <code>__GLX_VENDOR_LIBRARY_NAME=nvidia</code>
                      </template>
                      <template v-else>
                        <code>DRI_PRIME={{ selectedGpuIndex }}</code>
                      </template>
                      {{ tr('gamesettingsmodal.systemTab.gpuEnvSuffix', '指定使用此显卡启动游戏。') }}
                    </template>
                  </div>
                </div>
                <div v-else class="info-sub">{{ tr('gamesettingsmodal.systemTab.gpuNoNeed', '未检测到多个 GPU，无需手动指定。') }}</div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.systemTab.gameLanguage', '游戏语言') }}</div>
                <select v-model="gameLang" class="custom-input" style="width:100%">
                  <option value="">{{ tr('gamesettingsmodal.systemTab.followSystem', '跟随系统') }}</option>
                  <option value="zh_CN">简体中文 (zh_CN)</option>
                  <option value="zh_TW">繁体中文 (zh_TW)</option>
                  <option value="en_US">English (en_US)</option>
                  <option value="ja_JP">日本語 (ja_JP)</option>
                  <option value="ko_KR">한국어 (ko_KR)</option>
                  <option value="de_DE">Deutsch (de_DE)</option>
                  <option value="fr_FR">Français (fr_FR)</option>
                  <option value="es_ES">Español (es_ES)</option>
                  <option value="pt_BR">Português (pt_BR)</option>
                  <option value="ru_RU">Русский (ru_RU)</option>
                  <option value="th_TH">ไทย (th_TH)</option>
                  <option value="vi_VN">Tiếng Việt (vi_VN)</option>
                  <option value="id_ID">Bahasa Indonesia (id_ID)</option>
                </select>
                <div class="info-sub" style="margin-top:6px;">
                  {{ tr('gamesettingsmodal.systemTab.langHint', '设置 LANG 环境变量，部分游戏会根据此值自动切换语言。') }}
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ tr('gamesettingsmodal.systemTab.sandbox', '沙盒设置') }}</div>

                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.sandbox_enabled" />
                    {{ tr('gamesettingsmodal.systemTab.enableSandbox', '启用 bwrap 沙盒') }}
                  </label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.sandbox_isolate_home" :disabled="!protonSettings.sandbox_enabled" />
                    {{ tr('gamesettingsmodal.systemTab.isolateHome', '隔离 HOME（更严格，兼容性更低）') }}
                  </label>
                </div>
              </div>

              <div class="button-row">
                <button class="action-btn highlight" @click="saveSystemOptions">{{ tr('gamesettingsmodal.systemTab.save', '保存系统选项') }}</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
/* =========== 核心：毛玻璃玻璃态容器 =========== */
.glass-panel {
  background-color: rgba(20, 25, 30, 0.75) !important;
  backdrop-filter: blur(24px) saturate(120%);
  -webkit-backdrop-filter: blur(24px) saturate(120%);
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
}

.glass-card {
  background-color: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 8px;
}

.settings-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(0, 0, 0, 0.6);
  z-index: 2000;
  will-change: transform;
  display: flex;
  align-items: center;
  justify-content: center;
}

.settings-window {
  width: 100%;
  max-width: 900px;
  height: 80vh;
  max-height: 700px;
  border-radius: 12px;
  display: flex;
  overflow: hidden;
  animation: slideUp 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  will-change: transform;
  contain: layout style;
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(20px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.setting-checkbox-row {
  margin-bottom: 12px;
}

.setting-inline-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}

.setting-inline-label {
  color: rgba(255,255,255,0.7);
  font-size: 13px;
  white-space: nowrap;
  min-width: 80px;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 8px;
  color: white;
  cursor: pointer;
  user-select: none;
}

.flex-row {
  display: flex;
  gap: 16px;
}

.half-width {
  flex: 1;
}

/* Sidebar */
.settings-sidebar {
  width: 200px;
  background: rgba(0, 0, 0, 0.25);
  border-right: 1px solid rgba(255, 255, 255, 0.05);
  display: flex;
  flex-direction: column;
  padding: 20px 0;
}

.sidebar-title {
  font-size: 16px;
  font-weight: 600;
  color: #fff;
  padding: 0 20px 20px 20px;
  margin-bottom: 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.sidebar-item {
  padding: 12px 20px;
  color: rgba(255, 255, 255, 0.65);
  cursor: pointer;
  transition: all 0.2s;
  font-size: 14px;
  margin: 2px 10px;
  border-radius: 6px;
}

.sidebar-item:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}

.sidebar-item.active {
  background: rgba(var(--el-color-primary-rgb), 0.15);
  color: var(--el-color-primary);
  font-weight: 600;
}

.sidebar-item.runtime-attention {
  animation: runtimeTabPulse 1.5s ease-in-out infinite;
}

@keyframes runtimeTabPulse {
  0%, 100% { background: rgba(var(--el-color-primary-rgb), 0.1); }
  50% { background: rgba(var(--el-color-primary-rgb), 0.3); }
}

.runtime-pane-attention {
  position: relative;
  isolation: isolate;
}

.runtime-pane-attention::after {
  content: '';
  position: absolute;
  inset: -10px;
  border-radius: 10px;
  pointer-events: none;
  border: 1px solid rgba(var(--el-color-primary-rgb), 0.4);
  animation: runtimePaneOutlinePulse 1.5s ease-in-out infinite;
}

.runtime-section-attention {
  border-radius: 8px;
  animation: runtimeSectionPulse 1.5s ease-in-out infinite;
}

@keyframes runtimePaneOutlinePulse {
  0%, 100% { opacity: 0.3; transform: scale(1); }
  50% { opacity: 1; transform: scale(1.01); }
}

@keyframes runtimeSectionPulse {
  0%, 100% { background: rgba(var(--el-color-primary-rgb), 0.03); }
  50% { background: rgba(var(--el-color-primary-rgb), 0.1); }
}

.runtime-guide-banner {
  margin-bottom: 16px;
  border-radius: 6px;
  border: 1px solid rgba(var(--el-color-primary-rgb), 0.4);
  background: rgba(var(--el-color-primary-rgb), 0.15);
  color: var(--el-color-primary-light-3);
  padding: 10px 12px;
  font-size: 13px;
}

/* Content */
.settings-content {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.content-header {
  height: 60px;
  padding: 0 30px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.header-title {
  font-size: 18px;
  font-weight: 600;
  color: #fff;
  text-transform: uppercase;
  letter-spacing: 1px;
}

.close-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  cursor: pointer;
  color: rgba(255, 255, 255, 0.6);
  transition: all 0.2s;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.scroll-content {
  flex: 1;
  padding: 30px;
  overflow-y: auto;
}

.scroll-content::-webkit-scrollbar {
  width: 6px;
}
.scroll-content::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 3px;
}

.setting-group {
  margin-bottom: 24px;
}

.info-readonly-banner {
  margin-bottom: 16px;
  border-radius: 6px;
  border: 1px solid rgba(230, 162, 60, 0.45);
  background: rgba(230, 162, 60, 0.15);
  color: #e6a23c;
  padding: 10px 12px;
  font-size: 13px;
}

.setting-label {
  display: block;
  font-size: 14px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.85);
  margin-bottom: 8px;
}

.custom-input {
  width: 100%;
  box-sizing: border-box;
  background: rgba(0, 0, 0, 0.25);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  padding: 10px 12px;
  color: #fff;
  font-size: 14px;
  outline: none;
  transition: all 0.2s ease;
}

.custom-input:focus {
  border-color: var(--el-color-primary);
  background: rgba(0, 0, 0, 0.4);
}

.button-row {
  display: flex;
  gap: 12px;
  margin-top: 12px;
}

.action-btn {
  padding: 10px 18px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  color: #fff;
  background: rgba(255, 255, 255, 0.05);
}

.action-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
}

.action-btn.highlight {
  background: rgba(var(--el-color-primary-rgb), 0.15);
  border-color: rgba(var(--el-color-primary-rgb), 0.4);
  color: var(--el-color-primary-light-3);
}

.action-btn.highlight:hover:not(:disabled) {
  background: rgba(var(--el-color-primary-rgb), 0.3);
}

.action-btn.delete {
  background: rgba(245, 108, 108, 0.15);
  border-color: rgba(245, 108, 108, 0.4);
  color: #f56c6c;
}

.action-btn.delete:hover:not(:disabled) {
  background: rgba(245, 108, 108, 0.3);
}

.empty-state {
  color: rgba(255, 255, 255, 0.4);
  text-align: center;
  margin-top: 40px;
}

/* Wine Tab Styles */
.info-card {
  padding: 16px;
}

.info-grid {
  display: grid;
  grid-template-columns: 100px 1fr;
  gap: 6px 12px;
  font-size: 13px;
}

.info-key {
  color: rgba(255, 255, 255, 0.5);
}

.info-val {
  color: rgba(255, 255, 255, 0.85);
}

.text-ok { color: var(--el-color-success); }
.text-err { color: var(--el-color-danger); }
.text-muted { color: rgba(255, 255, 255, 0.4); }

.info-text {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.8);
  line-height: 1.6;
}

.info-sub {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
  margin-top: 4px;
}

.action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.wine-detail {
  margin-top: 8px;
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 12px;
  padding: 8px 12px;
}

.badge {
  background: rgba(var(--el-color-primary-rgb), 0.15);
  color: var(--el-color-primary-light-3);
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
  white-space: nowrap;
}

.wine-path {
  color: rgba(255, 255, 255, 0.5);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: monospace;
}

.env-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  margin-bottom: 6px;
  font-size: 13px;
}

.env-key {
  color: var(--el-color-primary-light-3);
  font-family: monospace;
  min-width: 120px;
}

.env-val {
  color: rgba(255, 255, 255, 0.8);
  font-family: monospace;
  flex: 1;
}

.env-remove {
  background: none;
  border: none;
  color: rgba(255, 255, 255, 0.4);
  cursor: pointer;
  font-size: 14px;
  padding: 2px 6px;
}

.env-remove:hover {
  color: var(--el-color-danger);
}

.env-add-row {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}

.env-input {
  flex: 1;
  font-family: monospace;
}

/* Transitions */
.modal-fade-enter-active,
.modal-fade-leave-active {
  transition: opacity 0.2s ease;
}

.modal-fade-enter-from,
.modal-fade-leave-to {
  opacity: 0;
}

.loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  z-index: 100;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
}

.spinner {
  width: 40px;
  height: 40px;
  border: 3px solid rgba(255, 255, 255, 0.1);
  border-top-color: var(--el-color-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin-bottom: 12px;
}

.loading-text {
  color: var(--el-color-primary-light-3);
  font-size: 14px;
  font-weight: 500;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 自定义下拉框和选中的覆盖 */
:deep(.el-select .el-input__wrapper) {
  background-color: rgba(0, 0, 0, 0.25) !important;
  border: 1px solid rgba(255, 255, 255, 0.1) !important;
  box-shadow: none !important;
}
:deep(.el-select .el-input__wrapper.is-focus) {
  border-color: var(--el-color-primary) !important;
}
</style>
