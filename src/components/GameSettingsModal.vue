<script setup lang="ts">
import { ref, watch, reactive, computed, onMounted, onUnmounted } from 'vue';
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
  get3dmigotoLatestRelease,
  install3dmigotoUpdate as apiInstall3dmigotoUpdate,
  ensureDirectory,
  openInExplorer,
  openFileDialog,
  showMessage,
  askConfirm,
  joinPath,
  scanWineVersions,
  getGameWineConfig,
  setGameWineConfig,
  checkVulkan,
  getDisplayInfo,
  getPrefixInfo,
  getJadeiteStatus,
  installJadeite,
  installDxvk,
  uninstallDxvk,
  scanLocalDxvk,
  detectDxvkStatus,
  fetchDxvkVersions,
  fetchRemoteProton,
  downloadProton,
  getLocalVersion,
  listenEvent,
  type WineVersion,
  type ProtonSettings,
  type PrefixInfo,
  type JadeiteStatus,
  type VulkanInfo,
  type DisplayInfo,
  type DxvkLocalVersion,
  type DxvkRemoteVersion,
  type DxvkInstalledStatus,
  type RemoteWineVersion,
  type RuntimeEnv,
} from '../api';
import { loadGames, appSettings, gamesList, switchToGame } from '../store';
import { useI18n } from 'vue-i18n';
import { inject } from 'vue';
import { useGameInfoEditor } from '../composables/useGameInfoEditor';
import GameInfoProfileSection from './game-info/GameInfoProfileSection.vue';
import GameInfoVersionSection from './game-info/GameInfoVersionSection.vue';
import GameInfoPresetSection from './game-info/GameInfoPresetSection.vue';
import GameInfoAssetsSection from './game-info/GameInfoAssetsSection.vue';

const { t } = useI18n();
const notify = inject<any>('notify');

const props = defineProps<{
  modelValue: boolean;
  gameName: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

// Config State
interface GameConfig {
  basic: {
    gamePreset: string;
    runtimeEnv: 'wine' | 'steam' | 'linux';
    backgroundType?: 'Image' | 'Video';
  };
  threeDMigoto: {
    installDir: string;
    targetExePath: string;
    launcherExePath: string;
    launchArgs: string;
    showErrorPopup: boolean;
    autoSetAnalyseOptions: boolean;
    useShell: boolean;
    useUpx: boolean;
    delay: number;
    autoExitSeconds: number;
    extraDll: string;
  };
  other: any;
}

const config = reactive<GameConfig>({
  basic: { gamePreset: 'GenshinImpact', runtimeEnv: 'wine' },
  threeDMigoto: {
    installDir: '',
    targetExePath: '',
    launcherExePath: '',
    launchArgs: '',
    showErrorPopup: true,
    autoSetAnalyseOptions: true,
    useShell: false,
    useUpx: false,
    delay: 0,
    autoExitSeconds: 0,
    extraDll: ''
  },
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

const isLoading = ref(false);
const hasLoadedConfig = ref(false);

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const asString = (value: unknown, fallback = ''): string =>
  typeof value === 'string' ? value : fallback;

const asNumber = (value: unknown, fallback: number): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : fallback;

const canonicalPreset = (value: string): string => {
  return value.trim();
};

const normalizeLoadedConfig = (raw: unknown): GameConfig => {
  const root = isRecord(raw) ? raw : {};
  const basicRaw = isRecord(root.basic) ? root.basic : {};
  const migotoRaw = isRecord(root.threeDMigoto) ? root.threeDMigoto : {};
  const otherRaw = isRecord(root.other) ? root.other : {};

  const gamePreset =
    asString(basicRaw.gamePreset) ||
    asString(basicRaw.GamePreset) ||
    asString(root.GamePreset) ||
    asString(root.LogicName) ||
    'GenshinImpact';

  const runtimeEnvRaw =
    asString(basicRaw.runtimeEnv) || asString(root.runtimeEnv);
  const runtimeEnv: 'wine' | 'steam' | 'linux' =
    runtimeEnvRaw === 'steam' ? 'steam' : runtimeEnvRaw === 'linux' ? 'linux' : 'wine';

  let autoSetAnalyseOptions = true;
  if (typeof migotoRaw.autoSetAnalyseOptions === 'boolean') {
    autoSetAnalyseOptions = migotoRaw.autoSetAnalyseOptions;
  } else if (typeof root.AutoSetAnalyseOptions === 'boolean') {
    autoSetAnalyseOptions = root.AutoSetAnalyseOptions;
  } else if (typeof root.AutoSetAnalyseOptionsSelectedIndex === 'number') {
    autoSetAnalyseOptions = root.AutoSetAnalyseOptionsSelectedIndex === 0;
  }

  const showErrorPopup =
    typeof migotoRaw.showErrorPopup === 'boolean'
      ? migotoRaw.showErrorPopup
      : typeof root.AutoRunIgnoreErrorGIPlugin === 'boolean'
        ? !root.AutoRunIgnoreErrorGIPlugin
        : true;

  const mergedOther: Record<string, unknown> = { ...otherRaw };
  for (const [key, value] of Object.entries(root)) {
    if (key === 'basic' || key === 'threeDMigoto' || key === 'other') continue;
    if (mergedOther[key] === undefined) {
      mergedOther[key] = value;
    }
  }

  const legacyGamePath =
    asString(mergedOther.gamePath) ||
    asString(mergedOther.game_path) ||
    asString(root.gamePath) ||
    asString(root.game_path) ||
    asString(root.TargetPath);
  if (legacyGamePath) {
    mergedOther.gamePath = legacyGamePath;
  }

  return {
    basic: {
      gamePreset: canonicalPreset(gamePreset),
      runtimeEnv,
    },
    threeDMigoto: {
      installDir:
        asString(migotoRaw.installDir) || asString(root['3DmigotoPath']),
      targetExePath:
        asString(migotoRaw.targetExePath) || asString(root.TargetPath),
      launcherExePath:
        asString(migotoRaw.launcherExePath) || asString(root.LaunchPath),
      launchArgs: asString(migotoRaw.launchArgs) || asString(root.LaunchArgs),
      showErrorPopup,
      autoSetAnalyseOptions,
      useShell:
        typeof migotoRaw.useShell === 'boolean'
          ? migotoRaw.useShell
          : typeof root.RunWithShell === 'boolean'
            ? root.RunWithShell
            : false,
      useUpx:
        typeof migotoRaw.useUpx === 'boolean'
          ? migotoRaw.useUpx
          : typeof root.DllReplaceSelectedIndex === 'number'
            ? root.DllReplaceSelectedIndex > 0
            : false,
      delay:
        typeof migotoRaw.delay === 'number'
          ? migotoRaw.delay
          : asNumber(root.DllInitializationDelay, 100),
      autoExitSeconds:
        typeof migotoRaw.autoExitSeconds === 'number'
          ? migotoRaw.autoExitSeconds
          : asNumber(root.Delay, 5),
      extraDll: asString(migotoRaw.extraDll),
    },
    other: mergedOther,
  };
};

// Wine/Proton State
const wineVersions = ref<WineVersion[]>([]);
const selectedWineVersionId = ref('');
const protonSettings = reactive<ProtonSettings>({
  steam_app_id: '0',
  use_pressure_vessel: true,
  proton_media_use_gst: false,
  proton_enable_wayland: false,
  proton_no_d3d12: false,
  mangohud: false,
  steam_deck_compat: false,
  sandbox_enabled: false,
  sandbox_isolate_home: false,
  custom_env: {},
});
const vulkanInfo = ref<VulkanInfo | null>(null);
const displayInfo = ref<DisplayInfo | null>(null);
const newEnvKey = ref('');
const newEnvValue = ref('');

// 系统选项状态
const selectedGpuIndex = ref(-1); // -1 = 自动
const gameLang = ref(''); // '' = 跟随系统

// 远程 Proton 版本管理
const remoteProtonVersions = ref<RemoteWineVersion[]>([]);
const isProtonFetching = ref(false);
const isProtonDownloading = ref(false);
const protonDownloadTag = ref('');

// 组件下载进度（Proton/DXVK）
interface ComponentDlProgress {
  component: string;
  phase: string;
  downloaded: number;
  total: number;
}
const componentDlProgress = ref<ComponentDlProgress | null>(null);
let unlistenComponentDl: (() => void) | null = null;

const doFetchRemoteProton = async () => {
  if (isProtonFetching.value) return;
  try {
    isProtonFetching.value = true;
    remoteProtonVersions.value = await fetchRemoteProton();
  } catch (e) {
    await showMessage(`获取远程 Proton 版本失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isProtonFetching.value = false;
  }
};

const doDownloadProton = async (rv: RemoteWineVersion) => {
  if (isProtonDownloading.value) return;
  try {
    isProtonDownloading.value = true;
    protonDownloadTag.value = rv.tag;
    notify?.info('Proton 下载', `开始下载 ${rv.tag}...`);
    const result = await downloadProton(rv.download_url, rv.tag, rv.variant);
    notify?.success('Proton 下载完成', result);
    // 刷新本地版本列表
    wineVersions.value = await scanWineVersions();
    // 刷新远程列表标记
    await doFetchRemoteProton();
  } catch (e) {
    notify?.error('Proton 下载失败', `${e}`);
  } finally {
    isProtonDownloading.value = false;
    protonDownloadTag.value = '';
  }
};

const loadWineState = async () => {
  try {
    wineVersions.value = await scanWineVersions();
    const wineConfig = await getGameWineConfig(props.gameName);
    if (wineConfig.wine_version_id) {
      selectedWineVersionId.value = wineConfig.wine_version_id;
    }
    Object.assign(protonSettings, wineConfig.proton_settings);
    vulkanInfo.value = await checkVulkan();
    displayInfo.value = await getDisplayInfo();
  } catch (e) {
    console.error('Failed to load wine state:', e);
  }
};

const saveWineConfig = async () => {
  if (!props.gameName || !selectedWineVersionId.value) return;
  try {
    await setGameWineConfig(props.gameName, selectedWineVersionId.value, protonSettings);
    // Also save wineVersionId into game config other section
    config.other.wineVersionId = selectedWineVersionId.value;
    await saveConfig();
  } catch (e) {
    console.error('Failed to save wine config:', e);
    notify?.error('保存失败', `Wine 配置保存失败: ${e}`);
  }
};

const saveSystemOptions = async () => {
  try {
    // 保存 GPU 和语言到 config.other
    config.other.gpuIndex = selectedGpuIndex.value;
    config.other.gameLang = gameLang.value;
    await saveConfig();
    // 同步保存 Proton 设置（沙盒等）
    await saveWineConfig();
    notify?.success('保存成功', '系统选项已保存');
  } catch (e) {
    notify?.error('保存失败', `系统选项保存失败: ${e}`);
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

const variantBadgeClass = (variant: string) => {
  const v = variant.toLowerCase().replace(/[- ]/g, '');
  if (v.includes('geproton') || v.includes('ge')) return 'ge';
  if (v.includes('dwproton') || v.includes('dw')) return 'dw';
  if (v.includes('wine') || v.includes('winege') || v.includes('winebuilds')) return 'wine';
  if (v.includes('tkg')) return 'tkg';
  return 'default';
};

// Jadeite 状态
const jadeiteStatus = ref<JadeiteStatus | null>(null);
const isJadeiteInstalling = ref(false);
const prefixInfo = ref<PrefixInfo | null>(null);

const isHoyoverse = computed(() =>
  ['GenshinImpact', 'HonkaiStarRail', 'ZenlessZoneZero', 'HonkaiImpact3rd'].includes(
    config.basic.gamePreset,
  ),
);

const loadJadeiteState = async () => {
  if (!props.gameName) return;
  try {
    jadeiteStatus.value = await getJadeiteStatus(props.gameName);
  } catch (e) {
    console.warn('[jadeite] 获取状态失败:', e);
    jadeiteStatus.value = null;
  }
};

const doInstallJadeite = async () => {
  if (isJadeiteInstalling.value) return;
  try {
    isJadeiteInstalling.value = true;
    const result = await installJadeite(props.gameName);
    await showMessage(result, { title: 'Jadeite', kind: 'info' });
    await loadJadeiteState();
  } catch (e) {
    await showMessage(`安装 jadeite 失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isJadeiteInstalling.value = false;
  }
};

const loadPrefixState = async () => {
  if (!props.gameName) return;
  try {
    prefixInfo.value = await getPrefixInfo(props.gameName);
  } catch (e) {
    console.warn('[prefix] 获取状态失败:', e);
    prefixInfo.value = null;
  }
};

// DXVK 版本管理
const dxvkLocalVersions = ref<DxvkLocalVersion[]>([]);
const dxvkRemoteVersions = ref<DxvkRemoteVersion[]>([]);
const dxvkInstalledStatus = ref<DxvkInstalledStatus | null>(null);
const dxvkSelectedVersion = ref('');
const isDxvkBusy = ref(false);
const isDxvkFetching = ref(false);

// 合并的版本列表（远程 + 本地）
const dxvkVersionList = computed(() => {
  const map = new Map<string, { version: string; isLocal: boolean; isRemote: boolean; fileSize: number; publishedAt: string }>();

  // 先添加远程版本
  for (const rv of dxvkRemoteVersions.value) {
    map.set(rv.version, {
      version: rv.version,
      isLocal: rv.is_local,
      isRemote: true,
      fileSize: rv.file_size,
      publishedAt: rv.published_at,
    });
  }

  // 补充仅在本地的版本
  for (const lv of dxvkLocalVersions.value) {
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

  // 按版本号降序排列
  return Array.from(map.values()).sort((a, b) => b.version.localeCompare(a.version));
});

const loadDxvkState = async () => {
  if (!props.gameName) return;
  try {
    const [local, status] = await Promise.all([
      scanLocalDxvk(),
      detectDxvkStatus(props.gameName),
    ]);
    dxvkLocalVersions.value = local;
    dxvkInstalledStatus.value = status;

    // 如果已安装且检测到版本号，自动选中
    if (status.installed && status.version) {
      dxvkSelectedVersion.value = status.version;
    } else if (local.length > 0 && !dxvkSelectedVersion.value) {
      dxvkSelectedVersion.value = local[0].version;
    }
  } catch (e) {
    console.warn('[dxvk] 加载状态失败:', e);
  }
};

const doFetchDxvkVersions = async () => {
  if (isDxvkFetching.value) return;
  try {
    isDxvkFetching.value = true;
    dxvkRemoteVersions.value = await fetchDxvkVersions();
    // 如果当前没选中版本且有远程版本，选最新的
    if (!dxvkSelectedVersion.value && dxvkRemoteVersions.value.length > 0) {
      dxvkSelectedVersion.value = dxvkRemoteVersions.value[0].version;
    }
  } catch (e) {
    await showMessage(`获取 DXVK 版本列表失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isDxvkFetching.value = false;
  }
};

const doInstallDxvk = async () => {
  if (isDxvkBusy.value || !dxvkSelectedVersion.value) return;
  try {
    isDxvkBusy.value = true;
    notify?.info('DXVK', `正在安装 DXVK ${dxvkSelectedVersion.value}...`);
    const result = await installDxvk(props.gameName, dxvkSelectedVersion.value);
    notify?.success('DXVK 安装完成', result);
    await loadDxvkState();
  } catch (e) {
    notify?.error('DXVK 安装失败', `${e}`);
  } finally {
    isDxvkBusy.value = false;
  }
};

const doUninstallDxvk = async () => {
  if (isDxvkBusy.value) return;
  const confirmed = await askConfirm('确定要从当前 Prefix 中卸载 DXVK 吗？', { title: 'DXVK', kind: 'warning' });
  if (!confirmed) return;
  try {
    isDxvkBusy.value = true;
    const result = await uninstallDxvk(props.gameName);
    notify?.success('DXVK 卸载完成', result);
    await loadDxvkState();
  } catch (e) {
    notify?.error('DXVK 卸载失败', `${e}`);
  } finally {
    isDxvkBusy.value = false;
  }
};

const formatFileSize = (bytes: number): string => {
  if (bytes <= 0) return '';
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
};

// Tabs（参考 Lutris 风格：5个标签页）
const activeTab = ref('info');
const tabs = computed(() => [
  { id: 'info', label: '游戏信息' },
  { id: 'game', label: '游戏选项' },
  { id: 'runtime', label: '运行环境' },
  { id: '3dmigoto', label: '3Dmigoto' },
  { id: 'system', label: '系统选项' },
]);

const syncInfoConfigToLegacyState = () => {
  config.basic.gamePreset = infoConfig.value.meta.gamePreset || config.basic.gamePreset;
  config.basic.runtimeEnv = infoConfig.value.runtime.runtimeEnv;
  config.basic.backgroundType = infoConfig.value.assets.backgroundType;
  config.other.displayName = infoConfig.value.meta.displayName;
};

const loadInfoConfig = async () => {
  if (!props.gameName) return;
  try {
    await loadGameInfoState(props.gameName);
    syncInfoConfigToLegacyState();
  } catch (e) {
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
    await loadJadeiteState();
    notify?.success('保存成功', '游戏信息已保存');
  } catch (e) {
    console.error('[GameInfoV2] save meta failed:', e);
    notify?.error('保存失败', `游戏信息保存失败: ${e}`);
  }
};

const saveInfoRuntimeSection = async () => {
  if (!props.gameName) return;
  try {
    await saveInfoRuntimeRaw(props.gameName);
    syncInfoConfigToLegacyState();
  } catch (e) {
    console.error('[GameInfoV2] save runtime failed:', e);
    notify?.error('保存失败', `运行环境保存失败: ${e}`);
  }
};

const saveRuntimeTabSettings = async () => {
  try {
    await saveInfoRuntimeSection();
    await saveWineConfig();
    notify?.success('保存成功', '运行环境配置已保存');
  } catch (e) {
    notify?.error('保存失败', `运行环境配置保存失败: ${e}`);
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
const infoPageReadOnly = true;
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
    for (const folder of probeFolders) {
      const version = await getLocalVersion(folder);
      if (version) {
        localGameVersion.value = version;
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

// Load/Save Logic
const loadConfig = async () => {
  if (!props.gameName) return;
  isLoading.value = true;
  hasLoadedConfig.value = false;
  try {
    const data = await apiLoadGameConfig(props.gameName);
    const normalized = normalizeLoadedConfig(data);
    config.basic = normalized.basic;
    config.threeDMigoto = normalized.threeDMigoto;

    // Default Logic for installDir if empty on first load (user requirement)
    if (!config.threeDMigoto.installDir && appSettings.dataDir) {
      try {
        // "SSMT Data/3Dmigoto/GameName"
        config.threeDMigoto.installDir = await joinPath(appSettings.dataDir, '3Dmigoto', props.gameName);
      } catch (err) {
        console.error(t('gamesettingsmodal.error.failconstructdefaultpath'), err);
      }
    }

    config.other = normalized.other || {};
    // 恢复系统选项
    selectedGpuIndex.value = typeof config.other.gpuIndex === 'number' ? config.other.gpuIndex : -1;
    gameLang.value = typeof config.other.gameLang === 'string' ? config.other.gameLang : '';
    await refreshGameVersion();
    hasLoadedConfig.value = true;
    // Note: configName is NOT set from file, but from props
  } catch (e) {
    console.error(t('gamesettingsmodal.error.failloadconfig'), e);
  } finally {
    isLoading.value = false;
  }
};

const saveConfig = async () => {
  if (!props.gameName || isLoading.value) return; // Prevent saving if loading isn't complete
  console.log(t('gamesettingsmodal.log.saving', { gameName: props.gameName }));
  console.log(t('gamesettingsmodal.log.currentstate', { currentState: JSON.parse(JSON.stringify(config)) }));

  try {
    await apiSaveGameConfig(props.gameName, config as any);
    console.log(t('gamesettingsmodal.log.configsaved'));
  } catch (e) {
    console.error(t('gamesettingsmodal.error.configfailedsaving'), { e: e });
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
    await apiResetGameIcon(props.gameName);
    // 删除自定义背景文件
    await apiResetGameBackground(props.gameName);
    // 重置配置为默认值
    await apiSaveGameConfig(props.gameName, {
      basic: { gamePreset: 'GenshinImpact', runtimeEnv: 'wine' },
      threeDMigoto: {},
      other: {}
    } as any);
    await loadConfig();
    await loadInfoConfig();
    await loadGames();
    notify?.success('重置成功', '游戏配置已恢复默认');
  } catch (e) {
    console.error('Reset failed:', e);
    notify?.error('重置失败', `${e}`);
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
      await apiSetGameIcon(props.gameName, file);
      await loadInfoConfig();
      await loadGames();
      notify?.success('图标已更新', '游戏图标设置成功');
    }
  } catch (e) {
    console.error(e);
    notify?.error('图标设置失败', `${e}`);
  }
};

const selectBackground = async () => {
  try {
    const filters = [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'svg', 'bmp', 'ico', 'avif'] }];
    const file = await openFileDialog({ multiple: false, filters });
    if (file) {
      await apiSetGameBackground(props.gameName, file, infoConfig.value.assets.backgroundType);
      await loadInfoConfig();
      await loadGames();
      notify?.success('背景已更新', '背景图片设置成功');
    }
  } catch (e) {
    console.error(e);
    notify?.error('背景设置失败', `${e}`);
  }
};

const resetBackgroundToDefault = async () => {
  if (!props.gameName) return;
  try {
    await apiResetGameIcon(props.gameName);
    await apiResetGameBackground(props.gameName);
    await loadInfoConfig();
    await loadGames();
    notify?.success('恢复成功', '图标和背景已恢复默认');
  } catch (e) {
    console.error('[GameInfoV2] reset background failed:', e);
    notify?.error('恢复失败', `${e}`);
  }
};

// 自动更新背景暂不可用（资源目录中无默认背景文件）
// const canAutoUpdate = computed(() => false);
// const autoUpdateBackground — 功能保留但暂时注释，待资源目录准备好后启用

// 3Dmigoto Helper Functions
const pick3dmigotoDir = async () => {
  try {
    const selected = await openFileDialog({
      directory: true,
      multiple: false,
      title: '选择3Dmigoto所在目录'
    });
    if (selected && typeof selected === 'string') {
      config.threeDMigoto.installDir = selected;
    }
  } catch (e) { console.error(e); }
};

const open3dmigotoDir = async () => {
  if (!config.threeDMigoto.installDir) {
    await showMessage('请先设置 3Dmigoto 目录', { title: '提示', kind: 'info' });
    return;
  }
  try {
    await ensureDirectory(config.threeDMigoto.installDir);
    await openInExplorer(config.threeDMigoto.installDir);
  } catch (e) {
    console.error(t('gamesettingsmodal.error.failedopendir'), { e: e });
    await showMessage(`打开目录失败: ${e}`, { title: '错误', kind: 'error' });
  }
};

const packageSupportedPresets = [
  'GenshinImpact',
  'HonkaiImpact3rd',
  'HonkaiStarRail',
  'ZenlessZoneZero',
  'WutheringWaves',
  'SnowbreakContainmentZone',
  'AEMI',
];
const canUpdatePackage = computed(() => packageSupportedPresets.includes(config.basic.gamePreset));

const check3DMigotoPackageUpdate = async () => {
  // 1. Initial Confirmation
  const checkConfirm = await askConfirm(
    t('gamesettingsmodal.confirm.checkUpdate.message', {
      gamePreset: config.basic.gamePreset
    }),
    {
      title: t('gamesettingsmodal.confirm.checkUpdate.title'),
      kind: 'info'
    }
  );
  if (!checkConfirm) return;

  try {
    isLoading.value = true;

    // 2. Fetch Info
    const info = await get3dmigotoLatestRelease(config.basic.gamePreset);

    isLoading.value = false;

    // 3. Show info and ask for second confirmation
    const updateConfirm = await askConfirm(
      t('gamesettingsmodal.confirm.versionUpdate.message', {
        version: info.version,
        description: info.description
      }),
      {
        title: t('gamesettingsmodal.confirm.versionUpdate.title'),
        kind: 'info'
      }
    );

    if (!updateConfirm) return;

    // 4. Perform Update
    isLoading.value = true;
    await apiInstall3dmigotoUpdate(props.gameName, info.downloadUrl);

    await showMessage(
      t('gamesettingsmodal.message.success.updatedToVersion', {
        version: info.version
      }),
      {
        title: t('gamesettingsmodal.message.success.title'),
        kind: 'info'
      }
    );

  } catch (e) {
    console.error(e);
    await showMessage(
      t('gamesettingsmodal.message.error.operationFailed', { error: e }),
      {
        title: t('gamesettingsmodal.message.error.title'),
        kind: 'error'
      }
    );
  } finally {
    isLoading.value = false;
  }
};

const pickExe = async (field: 'targetExePath' | 'launcherExePath') => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ 
        name: '可执行文件', 
        extensions: ['exe', 'sh', 'AppImage', 'desktop', '*'] 
      }],
      title: '选择可执行文件'
    });
    if (selected && typeof selected === 'string') {
      config.threeDMigoto[field] = selected;
    }
  } catch (e) { console.error(e); }
};

const openExeDir = async (field: 'targetExePath' | 'launcherExePath') => {
  const path = config.threeDMigoto[field];
  if (!path) {
    await showMessage('请先选择文件路径', { title: '提示', kind: 'info' });
    return;
  }
  try {
    const lastSlash = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    if (lastSlash > -1) {
      const dir = path.substring(0, lastSlash);
      await openInExplorer(dir);
    } else {
      await openInExplorer(path);
    }
  } catch (e) {
    console.error(e);
    await showMessage(`打开目录失败: ${e}`, { title: '错误', kind: 'error' });
  }
};

const pickDll = async () => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ 
        name: t('gamesettingsmodal.filePicker.dllFilterName'), 
        extensions: ['dll'] 
      }],
      title: t('gamesettingsmodal.filePicker.dllTitle')
    });
    if (selected && typeof selected === 'string') {
      config.threeDMigoto.extraDll = selected;
    }
  } catch (e) { console.error(e); }
};

const pickGameExe = async () => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: '可执行文件', extensions: ['exe', 'sh', 'AppImage', 'desktop', '*'] }],
      title: '选择游戏可执行文件'
    });
    if (selected && typeof selected === 'string') {
      config.other.gamePath = selected;
      await refreshGameVersion();
    }
  } catch (e) { console.error(e); }
};

const setDefaultDll = async () => {
  if (config.threeDMigoto.installDir) {
    try {
      const dllPath = await joinPath(config.threeDMigoto.installDir, 'd3d11.dll');
      config.threeDMigoto.extraDll = dllPath;
    } catch (e) {
      console.error(t('gamesettingsmodal.log.joinPathFailed', { error: e }));
    }
  }
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
    await apiCreateNewConfig(configName.value, config as any);
    console.log(t('gamesettingsmodal.log.configCreated', { 
      configName: configName.value 
    }));

    // Refresh games list and close
    await loadGames();

    // Switch to the newly created game
    const newGame = gamesList.find(g => g.name === canonicalPreset(configName.value));
    if (newGame) {
      switchToGame(newGame);
    }

    notify?.success('创建成功', `配置 "${configName.value}" 已创建`);
    await close();
  } catch (e) {
    console.error(t('gamesettingsmodal.log.configCreateFailed', { error: e }));
    notify?.error('创建失败', `${e}`);
  } finally {
    isLoading.value = false;
  }
};

const deleteCurrentConfig = async () => {
  if (!props.gameName) return;

  const yes = await askConfirm(`确定要删除配置 "${props.gameName}" 吗？此操作不可逆。`, {
    title: '删除确认',
    kind: 'warning',
  });
  if (!yes) return;

  try {
    isLoading.value = true;
    await apiDeleteGameConfigFolder(props.gameName);
    console.log('Deleted config:', props.gameName);

    // Refresh games list and close
    await loadGames();
    notify?.success('删除成功', `配置 "${props.gameName}" 已删除`);
    await close();
  } catch (e) {
    console.error('Failed to delete config:', e);
    notify?.error('删除失败', `${e}`);
  } finally {
    isLoading.value = false;
  }
};

// Open/Close
watch(() => props.modelValue, async (val) => {
  if (val) {
    activeTab.value = 'info'; // Reset to first tab
    configName.value = props.gameName;
    hasLoadedConfig.value = false;
    loadConfig();
    loadInfoConfig();
    loadWineState();
    loadJadeiteState();
    loadPrefixState();
    loadDxvkState();
  } else {
    // Only save when current modal session loaded successfully.
    if (hasLoadedConfig.value) {
      await saveConfig();
      await saveWineConfig();
    }
  }
});

// 切换游戏时重新加载配置，确保每个游戏的设置独立
watch(() => props.gameName, async (newGame, oldGame) => {
  if (!newGame || newGame === oldGame) return;
  // 先保存旧游戏的配置
  if (oldGame && hasLoadedConfig.value && props.modelValue) {
    await saveConfig();
    await saveWineConfig();
  }
  // 加载新游戏的配置
  if (props.modelValue) {
    configName.value = newGame;
    hasLoadedConfig.value = false;
    loadConfig();
    loadInfoConfig();
    loadWineState();
    loadJadeiteState();
    loadPrefixState();
    loadDxvkState();
  }
});

// 组件下载进度事件监听
onMounted(async () => {
  unlistenComponentDl = await listenEvent('component-download-progress', (event: any) => {
    const data = event.payload as ComponentDlProgress;
    if (data.phase === 'done') {
      componentDlProgress.value = null;
    } else {
      componentDlProgress.value = data;
    }
  });
});

onUnmounted(() => {
  if (unlistenComponentDl) {
    unlistenComponentDl();
    unlistenComponentDl = null;
  }
});

const close = async () => {
  if (hasUnsavedInfoChanges.value) {
    const discard = await askConfirm(t('gamesettingsmodal.info.unsavedPrompt'), {
      title: t('gamesettingsmodal.info.unsavedTitle'),
      kind: 'warning',
    });
    if (!discard) return;
  }
  emit('update:modelValue', false);
};

// Expose methods to parent
defineExpose({
  switchTab: (tabId: string) => {
    activeTab.value = tabId;
  },
  runPackageUpdate: () => {
    // Ensure we are on the right tab visually
    activeTab.value = '3dmigoto';
    // Run the update check
    if (canUpdatePackage.value) {
      check3DMigotoPackageUpdate();
    } else {
      showMessage(t('gamesettingsmodal.no_auto_update'), { title: 'Info', kind: 'info' });
    }
  }
});
</script>

<template>
  <transition name="modal-fade">
    <div v-if="modelValue" class="settings-overlay">
      <div class="settings-window">
        <!-- Loading Overlay -->
        <div v-if="isBusy" class="loading-overlay">
          <div class="spinner"></div>
          <div class="loading-text">{{ t('gamesettingsmodal.processing') }}</div>
        </div>

        <!-- Sidebar -->
        <div class="settings-sidebar">
          <div class="sidebar-title">{{ t('gamesettingsmodal.title') }}</div>

          <div v-for="tab in tabs" :key="tab.id" class="sidebar-item" :class="{ active: activeTab === tab.id }"
            @click="activeTab = tab.id">
            {{ tab.label }}
          </div>
        </div>

        <!-- Content Area -->
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
            <!-- ==================== Tab 1: 游戏信息 ==================== -->
            <div v-if="activeTab === 'info'" class="tab-pane">
              <div v-if="infoConfig.readOnly || infoPageReadOnly" class="info-readonly-banner">
                <template v-if="infoPageReadOnly">
                  游戏信息页当前为只读展示模式（仅背景资源可修改）。
                </template>
                <template v-else>
                {{ infoReadonlyWarning || t('gamesettingsmodal.info.readonlyGeneric') }}
                </template>
              </div>

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

            <!-- ==================== Tab 2: 游戏选项 ==================== -->
            <div v-if="activeTab === 'game'" class="tab-pane">
              <div class="setting-group">
                <div class="setting-label">主程序</div>
                <input v-model="config.other.gamePath" type="text" class="custom-input" placeholder="选择游戏可执行文件（如 StarRail.exe）..." />
                <div class="button-row">
                  <button class="action-btn" @click="pickGameExe">选择文件</button>
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">启动参数</div>
                <input v-model="config.other.launchArgs" type="text" class="custom-input" placeholder="可选，如 -screen-fullscreen 0 -popupwindow" />
              </div>

              <div class="setting-group">
                <div class="setting-label">工作目录</div>
                <input v-model="config.other.workingDir" type="text" class="custom-input" placeholder="留空则使用主程序所在目录" />
              </div>

              <div class="setting-group">
                <div class="setting-label">容器目录（Wine Prefix）</div>
                <div class="info-text" v-if="prefixInfo">
                  <span :class="prefixInfo.exists ? 'text-ok' : 'text-err'">
                    {{ prefixInfo.exists ? '✓ 已创建' : '✗ 未创建（首次启动时自动创建）' }}
                  </span>
                  <div class="wine-path">{{ prefixInfo.path }}</div>
                  <div v-if="prefixInfo.exists && prefixInfo.size_bytes > 0" class="info-sub">
                    大小：{{ (prefixInfo.size_bytes / 1024 / 1024).toFixed(1) }} MB
                  </div>
                </div>
                <div v-else class="info-text text-muted">加载中...</div>
              </div>

              <!-- Jadeite 反作弊补丁（仅 HoYoverse 游戏） -->
              <div v-if="isHoyoverse" class="setting-group">
                <div class="setting-label">Jadeite 反作弊补丁</div>
                <div class="info-text" v-if="jadeiteStatus">
                  <span :class="jadeiteStatus.installed ? 'text-ok' : 'text-err'">
                    {{ jadeiteStatus.installed ? `✓ 已安装 (v${jadeiteStatus.localVersion})` : '✗ 未安装（HoYoverse 游戏必需）' }}
                  </span>
                  <div class="wine-path">{{ jadeiteStatus.patchDir }}</div>
                </div>
                <div class="button-row">
                  <button class="action-btn highlight" @click="doInstallJadeite" :disabled="isJadeiteInstalling">
                    {{ isJadeiteInstalling ? '安装中...' : (jadeiteStatus?.installed ? '更新 Jadeite' : '安装 Jadeite') }}
                  </button>
                </div>
                <div class="info-sub" style="margin-top:6px;">
                  Jadeite 用于在 Linux 上绕过 HoYoverse 反作弊，启动时自动通过 jadeite.exe 包装游戏。
                </div>
              </div>
            </div>

            <!-- ==================== Tab 3: 运行环境 ==================== -->
            <div v-if="activeTab === 'runtime'" class="tab-pane">
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

              <!-- Wine/Proton 本地已安装版本 -->
              <div class="setting-group">
                <div class="setting-label">Wine / Proton 版本（本地已安装）</div>
                <el-select v-model="selectedWineVersionId" placeholder="选择 Wine/Proton 版本..." class="custom-select" filterable style="width: 100%">
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
                <div v-if="selectedWineVersion" class="wine-detail">
                  <span class="badge">{{ variantLabel(selectedWineVersion.variant) }}</span>
                  <span class="wine-path">{{ selectedWineVersion.path }}</span>
                </div>
                <div class="info-sub" style="margin-top:6px;">
                  共检测到 {{ wineVersions.length }} 个本地 Wine/Proton 版本。
                </div>
              </div>

              <!-- 远程 Proton 版本下载 -->
              <div class="setting-group">
                <div class="setting-label">下载 Proton 版本</div>
                <div class="button-row" style="margin-bottom:10px;">
                  <button class="action-btn" @click="doFetchRemoteProton" :disabled="isProtonFetching">
                    {{ isProtonFetching ? '获取中...' : '获取可用版本' }}
                  </button>
                </div>

                <div v-if="remoteProtonVersions.length > 0" class="version-list">
                  <div v-for="rv in remoteProtonVersions" :key="rv.tag" class="version-item">
                    <div class="version-info">
                      <span class="version-tag">{{ rv.tag }}</span>
                      <span class="badge" :class="'badge-' + variantBadgeClass(rv.variant)" style="margin-left:6px;">{{ rv.variant }}</span>
                      <span v-if="rv.file_size > 0" class="text-muted" style="margin-left:8px;">{{ formatFileSize(rv.file_size) }}</span>
                    </div>
                    <div class="version-action">
                      <span v-if="rv.installed" class="text-ok" style="font-size:13px;">✓ 已安装</span>
                      <button v-else class="action-btn highlight" style="padding:4px 12px; font-size:12px;"
                        @click="doDownloadProton(rv)"
                        :disabled="isProtonDownloading">
                        {{ isProtonDownloading && protonDownloadTag === rv.tag ? '下载中...' : '下载' }}
                      </button>
                    </div>
                  </div>
                </div>
                <div v-else class="info-sub">
                  点击"获取可用版本"从 GitHub 获取 GE-Proton 等版本列表。
                </div>

                <!-- 组件下载进度条 -->
                <div v-if="componentDlProgress" class="component-dl-progress">
                  <div class="component-dl-header">
                    <span class="component-dl-name">{{ componentDlProgress.component }}</span>
                    <span class="component-dl-phase">
                      {{ componentDlProgress.phase === 'downloading' ? '下载中' : componentDlProgress.phase === 'extracting' ? '解压中...' : componentDlProgress.phase }}
                    </span>
                    <span v-if="componentDlProgress.total > 0 && componentDlProgress.phase === 'downloading'" class="component-dl-pct">
                      {{ Math.round(componentDlProgress.downloaded / componentDlProgress.total * 100) }}%
                    </span>
                  </div>
                  <div class="component-dl-track">
                    <div class="component-dl-fill" :class="{ 'component-dl-extracting': componentDlProgress.phase === 'extracting' }"
                      :style="{ width: componentDlProgress.total > 0 ? Math.round(componentDlProgress.downloaded / componentDlProgress.total * 100) + '%' : '100%' }">
                    </div>
                  </div>
                  <div v-if="componentDlProgress.total > 0 && componentDlProgress.phase === 'downloading'" class="component-dl-size">
                    {{ formatFileSize(componentDlProgress.downloaded) }} / {{ formatFileSize(componentDlProgress.total) }}
                  </div>
                </div>
              </div>

              <!-- DXVK 版本管理 -->
              <div class="setting-group">
                <div class="setting-label">DXVK (DirectX → Vulkan)</div>

                <!-- DXVK 未安装警告 -->
                <div v-if="dxvkInstalledStatus && !dxvkInstalledStatus.installed" class="dxvk-warning">
                  ⚠ DXVK 未安装。DirectX 9/10/11 游戏需要 DXVK 才能正常渲染，建议在下方选择版本并安装。
                </div>

                <!-- 当前安装状态 -->
                <div class="info-card" style="margin-bottom: 10px;">
                  <div v-if="dxvkInstalledStatus" class="info-grid" style="grid-template-columns: 100px 1fr;">
                    <span class="info-key">安装状态</span>
                    <span :class="dxvkInstalledStatus.installed ? 'text-ok' : 'text-err'">
                      {{ dxvkInstalledStatus.installed ? '✓ 已安装' : '✗ 未安装' }}
                    </span>
                    <template v-if="dxvkInstalledStatus.installed">
                      <span class="info-key">检测版本</span>
                      <span class="info-val">{{ dxvkInstalledStatus.version || '未知' }}</span>
                      <span class="info-key">DLL 文件</span>
                      <span class="info-val">{{ dxvkInstalledStatus.dlls_found.join(', ') }}</span>
                    </template>
                  </div>
                  <div v-else class="text-muted" style="font-size:13px">加载中...</div>
                </div>

                <!-- 版本选择 -->
                <div class="flex-row" style="align-items:flex-end; gap:8px; margin-top:8px;">
                  <div style="flex:1">
                    <select v-model="dxvkSelectedVersion" class="custom-input" style="width:100%">
                      <option value="" disabled>选择 DXVK 版本...</option>
                      <option v-for="v in dxvkVersionList" :key="v.version" :value="v.version">
                        {{ v.version }}
                        {{ v.isLocal ? ' [本地]' : '' }}
                        {{ v.fileSize > 0 ? ` (${formatFileSize(v.fileSize)})` : '' }}
                      </option>
                    </select>
                  </div>
                  <button class="action-btn" @click="doFetchDxvkVersions" :disabled="isDxvkFetching" style="flex:0 0 auto; white-space:nowrap;">
                    {{ isDxvkFetching ? '获取中...' : '刷新列表' }}
                  </button>
                </div>

                <!-- 操作按钮 -->
                <div class="button-row" style="margin-top:8px;">
                  <button class="action-btn highlight" @click="doInstallDxvk" :disabled="isDxvkBusy || !dxvkSelectedVersion">
                    {{ isDxvkBusy ? '安装中...' : '安装 / 切换版本' }}
                  </button>
                  <button class="action-btn delete" @click="doUninstallDxvk" :disabled="isDxvkBusy || !dxvkInstalledStatus?.installed">
                    卸载 DXVK
                  </button>
                </div>

                <!-- 本地缓存信息 -->
                <div v-if="dxvkLocalVersions.length > 0" class="info-sub" style="margin-top:8px;">
                  本地已缓存 {{ dxvkLocalVersions.length }} 个版本：{{ dxvkLocalVersions.map(v => v.version).join(', ') }}
                </div>
              </div>

              <!-- Proton 设置 -->
              <div class="setting-group">
                <div class="setting-label">Proton 设置</div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.use_pressure_vessel" /> 使用 Pressure Vessel 容器</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.proton_enable_wayland" /> 启用 Wayland</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.proton_no_d3d12" /> 禁用 D3D12</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.proton_media_use_gst" /> 使用 GStreamer 媒体</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.mangohud" /> MangoHud 性能覆盖</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.steam_deck_compat" /> Steam Deck 兼容模式</label>
                </div>
              </div>

              <!-- Steam App ID -->
              <div class="setting-group">
                <div class="setting-label">Steam App ID</div>
                <input v-model="protonSettings.steam_app_id" type="text" class="custom-input" placeholder="0 = N/A" />
              </div>

              <!-- 自定义环境变量 -->
              <div class="setting-group">
                <div class="setting-label">自定义环境变量</div>
                <div v-for="(val, key) in protonSettings.custom_env" :key="key" class="env-row">
                  <span class="env-key">{{ key }}</span>
                  <span class="env-val">{{ val }}</span>
                  <button class="env-remove" @click="removeCustomEnv(key as string)">✕</button>
                </div>
                <div class="env-add-row">
                  <input v-model="newEnvKey" type="text" class="custom-input env-input" placeholder="KEY" />
                  <input v-model="newEnvValue" type="text" class="custom-input env-input" placeholder="VALUE" />
                  <button class="action-btn" style="flex: 0 0 auto;" @click="addCustomEnv">添加</button>
                </div>
              </div>

              <!-- 保存 -->
              <div class="button-row">
                <button class="action-btn highlight" @click="saveRuntimeTabSettings">保存运行环境配置</button>
              </div>
            </div>

            <!-- ==================== Tab 4: 3Dmigoto ==================== -->
            <div v-if="activeTab === '3dmigoto'" class="tab-pane">

              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.migotodir') }}</div>
                <input v-model="config.threeDMigoto.installDir" type="text" class="custom-input"
                  :placeholder="t('gamesettingsmodal.migotodir_placeholder')" />
                <div class="button-row">
                  <button class="action-btn" @click="pick3dmigotoDir">{{ t('gamesettingsmodal.selectfolder') }}</button>
                  <button class="action-btn" @click="open3dmigotoDir">{{ t('gamesettingsmodal.openfolder') }}</button>
                  <button v-if="canUpdatePackage" class="action-btn highlight"
                    @click="check3DMigotoPackageUpdate">{{ t('gamesettingsmodal.updatepackage') }}</button>
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.targetexe') }}</div>
                <input v-model="config.threeDMigoto.targetExePath" type="text" class="custom-input"
                  :placeholder="t('gamesettingsmodal.targetexe_placeholder')" />
                <div class="button-row">
                  <button class="action-btn" @click="pickExe('targetExePath')">{{ t('gamesettingsmodal.selectfile') }}</button>
                  <button class="action-btn" @click="openExeDir('targetExePath')">{{ t('gamesettingsmodal.openlocation') }}</button>
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.launcherexe') }}</div>
                <input v-model="config.threeDMigoto.launcherExePath" type="text" class="custom-input"
                  :placeholder="t('gamesettingsmodal.launcherexe_placeholder')" />
                <div class="button-row">
                  <button class="action-btn" @click="pickExe('launcherExePath')">{{ t('gamesettingsmodal.selectfile') }}</button>
                  <button class="action-btn" @click="openExeDir('launcherExePath')">{{ t('gamesettingsmodal.openlocation') }}</button>
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.launchargs') }}</div>
                <input v-model="config.threeDMigoto.launchArgs" type="text" class="custom-input"
                  :placeholder="t('gamesettingsmodal.launchargs_placeholder')" />
              </div>

              <div class="setting-checkbox-row">
                <label class="checkbox-label">
                  <input type="checkbox" v-model="config.threeDMigoto.showErrorPopup" />
                  {{ t('gamesettingsmodal.show_warnings') }}
                </label>
              </div>

              <div class="setting-checkbox-row">
                <label class="checkbox-label">
                  <input type="checkbox" v-model="config.threeDMigoto.autoSetAnalyseOptions" />
                  {{ t('gamesettingsmodal.auto_analyse') }}
                </label>
              </div>

              <div class="setting-checkbox-row">
                <label class="checkbox-label">
                  <input type="checkbox" v-model="config.threeDMigoto.useShell" />
                  {{ t('gamesettingsmodal.use_shell') }}
                </label>
              </div>

              <div class="setting-checkbox-row">
                <label class="checkbox-label">
                  <input type="checkbox" v-model="config.threeDMigoto.useUpx" />
                  {{ t('gamesettingsmodal.use_upx') }}
                </label>
              </div>

              <div class="flex-row">
                <div class="setting-group half-width">
                  <div class="setting-label">{{ t('gamesettingsmodal.dll_delay') }}</div>
                  <input v-model.number="config.threeDMigoto.delay" type="number" class="custom-input" />
                </div>
                <div class="setting-group half-width">
                  <div class="setting-label">{{ t('gamesettingsmodal.auto_exit') }}</div>
                  <input v-model.number="config.threeDMigoto.autoExitSeconds" type="number" class="custom-input" />
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.extra_dll') }}</div>
                <input v-model="config.threeDMigoto.extraDll" type="text" class="custom-input" :placeholder="t('gamesettingsmodal.extra_dll_placeholder')" />
                <div class="button-row">
                  <button class="action-btn" @click="pickDll">{{ t('gamesettingsmodal.selectfile') }}</button>
                  <button class="action-btn highlight" @click="setDefaultDll">{{ t('gamesettingsmodal.setdefaultdll') }}</button>
                </div>
              </div>

            </div>

            <!-- ==================== Tab 5: 系统选项 ==================== -->
            <div v-if="activeTab === 'system'" class="tab-pane">

              <!-- 系统信息 -->
              <div v-if="displayInfo" class="setting-group info-card">
                <div class="setting-label">系统信息</div>
                <div class="info-grid">
                  <span class="info-key">显示服务器</span>
                  <span class="info-val">{{ displayInfo.server }}{{ displayInfo.wayland_compositor ? ` (${displayInfo.wayland_compositor})` : '' }}</span>
                  <span class="info-key">GPU 驱动</span>
                  <span class="info-val">{{ displayInfo.gpu_driver || '未知' }}</span>
                  <span class="info-key">Vulkan</span>
                  <span class="info-val" :class="{ 'text-ok': vulkanInfo?.available, 'text-err': !vulkanInfo?.available }">
                    {{ vulkanInfo?.available ? `✓ ${vulkanInfo.version || ''}` : '✗ 未检测到' }}
                  </span>
                  <span class="info-key">游戏手柄</span>
                  <span class="info-val">{{ displayInfo.gamepad_detected ? '✓ 已检测' : '— 未检测到' }}</span>
                </div>
              </div>

              <!-- GPU 选择（多显卡切换） -->
              <div class="setting-group">
                <div class="setting-label">指定显卡</div>
                <div v-if="displayInfo && displayInfo.gpus.length > 0">
                  <select v-model="selectedGpuIndex" class="custom-input" style="width:100%">
                    <option value="-1">自动（系统默认）</option>
                    <option v-for="gpu in displayInfo.gpus" :key="gpu.index" :value="gpu.index">
                      GPU {{ gpu.index }}: {{ gpu.name }} ({{ gpu.driver }})
                    </option>
                  </select>
                  <div class="info-sub" style="margin-top:6px;">
                    <template v-if="selectedGpuIndex === -1">使用系统默认 GPU。</template>
                    <template v-else>
                      将通过环境变量
                      <template v-if="displayInfo.gpus[selectedGpuIndex]?.driver === 'nvidia'">
                        <code>__NV_PRIME_RENDER_OFFLOAD=1</code> + <code>__GLX_VENDOR_LIBRARY_NAME=nvidia</code>
                      </template>
                      <template v-else>
                        <code>DRI_PRIME={{ selectedGpuIndex }}</code>
                      </template>
                      指定使用此显卡启动游戏。
                    </template>
                  </div>
                </div>
                <div v-else class="info-sub">未检测到多个 GPU，无需手动指定。</div>
              </div>

              <!-- 语言设置 -->
              <div class="setting-group">
                <div class="setting-label">游戏语言</div>
                <select v-model="gameLang" class="custom-input" style="width:100%">
                  <option value="">跟随系统</option>
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
                  设置 <code>LANG</code> 环境变量，部分游戏会根据此值自动切换语言。
                </div>
              </div>

              <!-- 沙盒设置 -->
              <div class="setting-group">
                <div class="setting-label">沙盒设置</div>

                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.sandbox_enabled" />
                    启用 bwrap 沙盒
                  </label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.sandbox_isolate_home" :disabled="!protonSettings.sandbox_enabled" />
                    隔离 HOME（更严格，兼容性更低）
                  </label>
                </div>
              </div>

              <!-- 保存 -->
              <div class="button-row">
                <button class="action-btn highlight" @click="saveSystemOptions">保存系统选项</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.settings-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
  z-index: 2000;
  /* High z-index */
}

.settings-window {
  position: absolute;
  top: 50px;
  bottom: 60px;
  left: 100px;
  right: 100px;
  background: rgba(30, 30, 30, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  border-radius: 12px;
  display: flex;
  overflow: hidden;
  animation: slideUp 0.3s ease-out;
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }

  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.setting-checkbox-row {
  margin-bottom: 12px;
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
  background: rgba(0, 0, 0, 0.2);
  border-right: 1px solid rgba(255, 255, 255, 0.05);
  display: flex;
  flex-direction: column;
  padding: 20px 0;
}

.sidebar-title {
  font-size: 16px;
  font-weight: bold;
  color: rgba(255, 255, 255, 0.9);
  padding: 0 20px 20px 20px;
  margin-bottom: 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.sidebar-item {
  padding: 12px 20px;
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  transition: all 0.2s;
  font-size: 14px;
}

.sidebar-item:hover {
  background: rgba(255, 255, 255, 0.05);
  color: #fff;
}

.sidebar-item.active {
  background: rgba(247, 206, 70, 0.1);
  /* Yellow tint */
  color: #F7CE46;
  border-left: 3px solid #F7CE46;
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
}

.close-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
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
  color: rgba(255, 255, 255, 0.8);
  margin-bottom: 8px;
}

.custom-input {
  width: 100%;
  box-sizing: border-box;
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  padding: 8px 12px;
  color: #fff;
  font-size: 14px;
  outline: none;
  transition: border-color 0.2s;
}

.custom-input:focus {
  border-color: #F7CE46;
}

.button-row {
  display: flex;
  gap: 12px;
  margin-top: 12px;
}

.action-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 4px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
  flex: 1;
  color: #fff;
  background: rgba(255, 255, 255, 0.1);
}

.action-btn:hover {
  background: rgba(255, 255, 255, 0.2);
}

.action-btn.create {
  background: rgba(247, 206, 70, 0.2);
  border: 1px solid rgba(247, 206, 70, 0.4);
  color: #F7CE46;
}

.action-btn.create:hover {
  background: rgba(247, 206, 70, 0.3);
}

.action-btn.highlight {
  background: rgba(0, 122, 204, 0.3);
  border: 1px solid rgba(0, 122, 204, 0.5);
  color: #61afef;
}

.action-btn.highlight:hover {
  background: rgba(0, 122, 204, 0.5);
}

.action-btn.delete {
  background: rgba(232, 17, 35, 0.2);
  border: 1px solid rgba(232, 17, 35, 0.4);
  color: #ff6b6b;
}

.action-btn.delete:hover {
  background: rgba(232, 17, 35, 0.3);
}

.empty-state {
  color: rgba(255, 255, 255, 0.3);
  text-align: center;
  margin-top: 40px;
}

/* Wine Tab Styles */
.info-card {
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
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

.text-ok { color: #67c23a; }
.text-err { color: #f56c6c; }
.text-muted { color: rgba(255, 255, 255, 0.4); }
.dxvk-warning {
  background: rgba(245, 158, 11, 0.15);
  border: 1px solid rgba(245, 158, 11, 0.4);
  color: #fbbf24;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 13px;
  margin-bottom: 10px;
  line-height: 1.5;
}

.info-text {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.75);
  line-height: 1.6;
}

.info-sub {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
  margin-top: 2px;
}

.action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.wine-detail {
  margin-top: 8px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.badge {
  background: rgba(247, 206, 70, 0.15);
  color: #F7CE46;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
  white-space: nowrap;
}
.badge-ge { background: rgba(168, 85, 247, 0.2); color: #c084fc; }
.badge-dw { background: rgba(59, 130, 246, 0.2); color: #60a5fa; }
.badge-wine { background: rgba(239, 68, 68, 0.2); color: #f87171; }
.badge-tkg { background: rgba(34, 197, 94, 0.2); color: #4ade80; }
.badge-default { background: rgba(156, 163, 175, 0.2); color: #9ca3af; }

.wine-path {
  color: rgba(255, 255, 255, 0.4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.env-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  background: rgba(0, 0, 0, 0.2);
  border-radius: 4px;
  margin-bottom: 4px;
  font-size: 13px;
}

.env-key {
  color: #61afef;
  font-family: monospace;
  min-width: 120px;
}

.env-val {
  color: rgba(255, 255, 255, 0.7);
  font-family: monospace;
  flex: 1;
}

.env-remove {
  background: none;
  border: none;
  color: rgba(255, 255, 255, 0.3);
  cursor: pointer;
  font-size: 14px;
  padding: 2px 6px;
}

.env-remove:hover {
  color: #f56c6c;
}

.env-add-row {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}

.env-input {
  flex: 1;
  font-family: monospace;
}

/* 版本列表（Proton 下载等） */
.version-list {
  max-height: 300px;
  overflow-y: auto;
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.15);
}

.version-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}

.version-item:last-child {
  border-bottom: none;
}

.version-item:hover {
  background: rgba(255, 255, 255, 0.03);
}

.version-info {
  display: flex;
  align-items: center;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.85);
}

.version-tag {
  font-weight: 500;
}

.version-action {
  flex-shrink: 0;
}

/* Transitions */
.modal-fade-enter-active,
.modal-fade-leave-active {
  transition: opacity 0.3s ease;
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
  background: rgba(0, 0, 0, 0.7);
  z-index: 100;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  backdrop-filter: blur(2px);
  border-radius: 12px;
}

.spinner {
  width: 40px;
  height: 40px;
  border: 4px solid rgba(255, 255, 255, 0.1);
  border-top-color: #F7CE46;
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin-bottom: 12px;
}

.loading-text {
  color: #F7CE46;
  font-size: 14px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* 组件下载进度条 */
.component-dl-progress {
  margin-top: 10px;
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  padding: 10px 12px;
}
.component-dl-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
  font-size: 13px;
}
.component-dl-name {
  color: rgba(255, 255, 255, 0.85);
  font-weight: 500;
}
.component-dl-phase {
  color: rgba(255, 255, 255, 0.5);
}
.component-dl-pct {
  margin-left: auto;
  color: #F7CE46;
  font-weight: 600;
}
.component-dl-track {
  height: 6px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 3px;
  overflow: hidden;
}
.component-dl-fill {
  height: 100%;
  background: linear-gradient(90deg, #F7CE46, #f59e0b);
  border-radius: 3px;
  transition: width 0.3s ease;
}
.component-dl-fill.component-dl-extracting {
  background: linear-gradient(90deg, #60a5fa, #3b82f6);
  animation: pulse-extract 1.5s ease-in-out infinite;
}
@keyframes pulse-extract {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
.component-dl-size {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  margin-top: 4px;
  text-align: right;
}
</style>
