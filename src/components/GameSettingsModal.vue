<script setup lang="ts">
import { ref, watch, reactive, computed } from 'vue';
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
import { loadGames, gamesList, switchToGame } from '../store';
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
  other: any;
}

const config = reactive<GameConfig>({
  basic: { gamePreset: 'GenshinImpact', runtimeEnv: 'wine' },
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
    'GenshinImpact';

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

// Jadeite 状态
const jadeiteStatus = ref<JadeiteStatus | null>(null);
const isJadeiteInstalling = ref(false);
const prefixInfo = ref<PrefixInfo | null>(null);

const isHoyoverse = computed(() =>
  ['GenshinImpact', 'HonkaiStarRail', 'ZenlessZoneZero'].includes(
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

// DXVK 版本管理（本地安装/卸载，无远程下载）
const dxvkLocalVersions = ref<DxvkLocalVersion[]>([]);
const dxvkInstalledStatus = ref<DxvkInstalledStatus | null>(null);
const dxvkSelectedKey = ref('');  // "version|variant" 格式
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

const loadDxvkState = async () => {
  if (!props.gameName) return;
  try {
    const [local, status] = await Promise.all([
      scanLocalDxvk(),
      detectDxvkStatus(props.gameName),
    ]);
    dxvkLocalVersions.value = local;
    dxvkInstalledStatus.value = status;

    if (status.installed && status.version) {
      // 自动匹配已安装版本对应的本地缓存
      const match = local.find(lv => lv.version === status.version);
      dxvkSelectedKey.value = match
        ? `${match.version}|${match.variant}`
        : `${status.version}|dxvk`;
    } else if (local.length > 0 && !dxvkSelectedKey.value) {
      dxvkSelectedKey.value = `${local[0].version}|${local[0].variant}`;
    }
  } catch (e) {
    console.warn('[dxvk] 加载状态失败:', e);
  }
};

const doInstallDxvk = async () => {
  if (isDxvkBusy.value || !dxvkSelectedKey.value) return;
  const [version, variant] = dxvkSelectedKey.value.split('|');
  if (!version || !variant) return;
  try {
    isDxvkBusy.value = true;
    const label = dxvkVariantLabel(variant);
    notify?.info(label, `正在应用 ${label} ${version}...`);
    const result = await installDxvk(props.gameName, version, variant);
    notify?.success(`${label} 应用完成`, result);
    await loadDxvkState();
  } catch (e) {
    notify?.error('DXVK 应用失败', `${e}`);
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

// VKD3D 版本管理（本地安装/卸载，无远程下载）
const vkd3dLocalVersions = ref<Vkd3dLocalVersion[]>([]);
const vkd3dInstalledStatus = ref<Vkd3dInstalledStatus | null>(null);
const vkd3dSelectedVersion = ref('');
const isVkd3dBusy = ref(false);

const loadVkd3dState = async () => {
  if (!props.gameName) return;
  try {
    const [local, status] = await Promise.all([
      scanLocalVkd3d(),
      detectVkd3dStatus(props.gameName),
    ]);
    vkd3dLocalVersions.value = local;
    vkd3dInstalledStatus.value = status;

    if (status.installed && status.version) {
      vkd3dSelectedVersion.value = status.version;
    } else if (local.length > 0 && !vkd3dSelectedVersion.value) {
      vkd3dSelectedVersion.value = local[0].version;
    }
  } catch (e) {
    console.warn('[vkd3d] 加载状态失败:', e);
  }
};

const doInstallVkd3d = async () => {
  if (isVkd3dBusy.value || !vkd3dSelectedVersion.value) return;
  try {
    isVkd3dBusy.value = true;
    notify?.info('VKD3D-Proton', `正在应用 VKD3D-Proton ${vkd3dSelectedVersion.value}...`);
    const result = await installVkd3d(props.gameName, vkd3dSelectedVersion.value);
    notify?.success('VKD3D 应用完成', result);
    await loadVkd3dState();
  } catch (e) {
    notify?.error('VKD3D 应用失败', `${e}`);
  } finally {
    isVkd3dBusy.value = false;
  }
};

const doUninstallVkd3d = async () => {
  if (isVkd3dBusy.value) return;
  const confirmed = await askConfirm('确定要从当前 Prefix 中卸载 VKD3D 吗？', { title: 'VKD3D', kind: 'warning' });
  if (!confirmed) return;
  try {
    isVkd3dBusy.value = true;
    const result = await uninstallVkd3d(props.gameName);
    notify?.success('VKD3D 卸载完成', result);
    await loadVkd3dState();
  } catch (e) {
    notify?.error('VKD3D 卸载失败', `${e}`);
  } finally {
    isVkd3dBusy.value = false;
  }
};

// Tabs（参考 Lutris 风格：5个标签页）
const activeTab = ref('info');
const tabs = computed(() => [
  { id: 'info', label: '游戏信息' },
  { id: 'game', label: '游戏选项' },
  { id: 'runtime', label: '运行环境' },
  { id: 'system', label: '系统选项' },
]);
type RuntimeFocusTarget = 'all' | 'wine_version' | 'dxvk' | 'vkd3d';
const runtimeAttention = ref(false);
const runtimeAttentionMessage = ref('');
const runtimeFocusTarget = ref<RuntimeFocusTarget>('all');
let runtimeAttentionTimer: ReturnType<typeof setTimeout> | null = null;
const runtimeWineVersionRef = ref<HTMLElement | null>(null);
const runtimeDxvkRef = ref<HTMLElement | null>(null);
const runtimeVkd3dRef = ref<HTMLElement | null>(null);

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
    message?.trim() || '请先在此完成运行环境配置（Proton / DXVK / VKD3D）。';
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
    loadVkd3dState();
  } else {
    clearRuntimeAttention();
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
    loadVkd3dState();
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
  clearRuntimeAttention();
  emit('update:modelValue', false);
};

// Expose methods to parent
defineExpose({
  switchTab: (tabId: string) => {
    activeTab.value = tabId;
  },
  focusRuntimeSetup,
});
</script>

<template>
  <transition name="modal-fade">
    <div v-if="modelValue" class="settings-overlay">
      <div class="settings-window" data-onboarding="game-settings-modal-root">
        <!-- Loading Overlay -->
        <div v-if="isBusy" class="loading-overlay">
          <div class="spinner"></div>
          <div class="loading-text">{{ t('gamesettingsmodal.processing') }}</div>
        </div>

        <!-- Sidebar -->
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
            <div v-if="activeTab === 'info'" class="tab-pane" data-onboarding="game-settings-info-tab">
              <div v-if="infoConfig.readOnly || infoPageReadOnly" class="info-readonly-banner">
                <template v-if="infoPageReadOnly">
                  游戏信息页当前为只读展示模式（仅背景资源可修改）。
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

            <!-- ==================== Tab 2: 游戏选项 ==================== -->
            <div v-if="activeTab === 'game'" class="tab-pane" data-onboarding="game-settings-game-tab">
              <div class="setting-group" data-onboarding="game-settings-game-exe">
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
            <div
              v-if="activeTab === 'runtime'"
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

              <!-- Wine/Proton 本地已安装版本 -->
              <div
                ref="runtimeWineVersionRef"
                class="setting-group"
                data-onboarding="game-settings-runtime-wine"
                :class="{ 'runtime-section-attention': runtimeAttention && runtimeFocusTarget === 'wine_version' }"
              >
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

              <!-- Proton 版本提示 -->
              <div class="setting-group">
                <div class="info-sub" style="margin-top:4px;">
                  如需下载更多 Proton 版本，请前往「设置 → Proton 管理」页面。
                </div>
              </div>

              <!-- DXVK 版本管理 -->
              <div
                ref="runtimeDxvkRef"
                class="setting-group"
                data-onboarding="game-settings-runtime-dxvk"
                :class="{ 'runtime-section-attention': runtimeAttention && runtimeFocusTarget === 'dxvk' }"
              >
                <div class="setting-label">DXVK (DirectX → Vulkan)</div>

                <!-- 当前安装状态 -->
                <div class="info-card" style="margin-bottom: 10px;">
                  <div v-if="dxvkInstalledStatus" class="info-grid" style="grid-template-columns: 100px 1fr;">
                    <span class="info-key">安装状态</span>
                    <span :class="dxvkInstalledStatus.installed ? 'text-ok' : 'text-err'">
                      {{ dxvkInstalledStatus.installed ? '✓ 已安装' : '✗ 未安装' }}
                    </span>
                    <template v-if="dxvkInstalledStatus.installed">
                      <span class="info-key">当前版本</span>
                      <span class="info-val">{{ dxvkInstalledStatus.version || '未知' }}</span>
                      <span class="info-key">DLL 文件</span>
                      <span class="info-val">{{ dxvkInstalledStatus.dlls_found.join(', ') }}</span>
                    </template>
                  </div>
                  <div v-else class="text-muted" style="font-size:13px">加载中...</div>
                </div>

                <!-- 本地版本安装/卸载 -->
                <div v-if="dxvkLocalVersions.length > 0">
                  <div class="flex-row" style="align-items:flex-end; gap:8px; margin-top:8px;">
                    <div style="flex:1">
                      <select v-model="dxvkSelectedKey" class="custom-input" style="width:100%">
                        <option value="" disabled>选择本地已缓存的 DXVK 版本...</option>
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
                      {{ isDxvkBusy ? '应用中...' : '应用 / 切换版本' }}
                    </button>
                    <button class="action-btn delete" @click="doUninstallDxvk" :disabled="isDxvkBusy || !dxvkInstalledStatus?.installed">
                      卸载 DXVK
                    </button>
                  </div>
                </div>

                <div class="info-sub" style="margin-top:8px;">
                  如需下载更多 DXVK 版本，请前往「设置 → DXVK 管理」页面。
                </div>
              </div>

              <!-- VKD3D 版本管理 -->
              <div
                ref="runtimeVkd3dRef"
                class="setting-group"
                data-onboarding="game-settings-runtime-vkd3d"
                :class="{ 'runtime-section-attention': runtimeAttention && runtimeFocusTarget === 'vkd3d' }"
              >
                <div class="setting-label">VKD3D-Proton (D3D12 → Vulkan)</div>

                <!-- 当前安装状态 -->
                <div class="info-card" style="margin-bottom: 10px;">
                  <div v-if="vkd3dInstalledStatus" class="info-grid" style="grid-template-columns: 100px 1fr;">
                    <span class="info-key">安装状态</span>
                    <span :class="vkd3dInstalledStatus.installed ? 'text-ok' : 'text-err'">
                      {{ vkd3dInstalledStatus.installed ? '✓ 已安装' : '✗ 未安装' }}
                    </span>
                    <template v-if="vkd3dInstalledStatus.installed">
                      <span class="info-key">当前版本</span>
                      <span class="info-val">{{ vkd3dInstalledStatus.version || '未知' }}</span>
                      <span class="info-key">DLL 文件</span>
                      <span class="info-val">{{ vkd3dInstalledStatus.dlls_found.join(', ') }}</span>
                    </template>
                  </div>
                  <div v-else class="text-muted" style="font-size:13px">加载中...</div>
                </div>

                <div v-if="vkd3dLocalVersions.length > 0">
                  <div class="flex-row" style="align-items:flex-end; gap:8px; margin-top:8px;">
                    <div style="flex:1">
                      <select v-model="vkd3dSelectedVersion" class="custom-input" style="width:100%">
                        <option value="" disabled>选择本地已缓存的 VKD3D 版本...</option>
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
                      {{ isVkd3dBusy ? '应用中...' : '应用 / 切换版本' }}
                    </button>
                    <button class="action-btn delete" @click="doUninstallVkd3d" :disabled="isVkd3dBusy || !vkd3dInstalledStatus?.installed">
                      卸载 VKD3D
                    </button>
                  </div>
                </div>
                <div v-else class="info-sub" style="margin-top:8px;">
                  本地暂无缓存 VKD3D 版本，请前往「设置 → VKD3D 管理」下载后再应用。
                </div>

                <div class="info-sub" style="margin-top:8px;">
                  VKD3D 默认不强制安装，仅在需要 D3D12 转译时建议启用。
                </div>
              </div>

              <!-- Proton 设置 -->
              <div class="setting-group">
                <div class="setting-label">Proton 设置</div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.use_umu_run" /> 使用 umu-run 启动（鸣潮默认开启）</label>
                </div>
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
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.steamos_compat" /> SteamOS 兼容模式</label>
                </div>
              </div>

              <!-- DXVK/VKD3D 设置 -->
              <div class="setting-group">
                <div class="setting-label">DXVK / 图形设置</div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.dxvk_async" /> DXVK 异步着色器编译</label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label"><input type="checkbox" v-model="protonSettings.disable_gpu_filter" /> 禁用 GPU 自动过滤</label>
                </div>
                <div class="setting-inline-row">
                  <span class="setting-inline-label">DXVK HUD</span>
                  <select v-model="protonSettings.dxvk_hud" class="custom-input" style="flex: 1;">
                    <option value="">关闭</option>
                    <option value="version">版本号</option>
                    <option value="fps">帧率</option>
                    <option value="version,fps">版本 + 帧率</option>
                    <option value="full">完整信息</option>
                  </select>
                </div>
                <div class="setting-inline-row">
                  <span class="setting-inline-label">帧率限制</span>
                  <input v-model.number="protonSettings.dxvk_frame_rate" type="number" class="custom-input" style="flex: 1;" placeholder="0 = 不限制" min="0" />
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

            <!-- ==================== Tab 5: 系统选项 ==================== -->
            <div v-if="activeTab === 'system'" class="tab-pane" data-onboarding="game-settings-system-tab">

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
              <div class="setting-group" data-onboarding="game-settings-system-gpu">
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
  display: flex;
  align-items: center;
  justify-content: center;
}

.settings-window {
  width: 100%;
  max-width: 900px;
  height: 80vh;
  max-height: 700px;
  background: rgba(10, 15, 20, 0.85);
  backdrop-filter: blur(16px);
  border: 1px solid rgba(0, 240, 255, 0.3);
  box-shadow: 0 0 30px rgba(0, 240, 255, 0.1), inset 0 0 20px rgba(0, 240, 255, 0.05);
  border-radius: 8px;
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
  background: rgba(0, 240, 255, 0.1);
  color: #00f0ff;
  border-left: 3px solid #00f0ff;
  box-shadow: inset 20px 0 20px -20px rgba(0, 240, 255, 0.3);
}

.sidebar-item.runtime-attention {
  animation: runtimeTabPulse 0.8s ease-in-out 0s 4;
  color: #8ffbff;
}

@keyframes runtimeTabPulse {
  0% {
    background: rgba(0, 240, 255, 0.08);
    box-shadow: inset 20px 0 20px -20px rgba(0, 240, 255, 0.2);
  }
  50% {
    background: rgba(0, 240, 255, 0.32);
    box-shadow: 0 0 0 2px rgba(0, 240, 255, 0.45), 0 0 18px rgba(0, 240, 255, 0.5);
  }
  100% {
    background: rgba(0, 240, 255, 0.08);
    box-shadow: inset 20px 0 20px -20px rgba(0, 240, 255, 0.2);
  }
}

.runtime-pane-attention {
  position: relative;
  isolation: isolate;
  animation: runtimePaneGlow 0.9s ease-in-out 0s 3;
}

.runtime-pane-attention::after {
  content: '';
  position: absolute;
  inset: -10px;
  border-radius: 10px;
  pointer-events: none;
  border: 1px solid rgba(0, 240, 255, 0.38);
  box-shadow: 0 0 0 1px rgba(0, 240, 255, 0.22), 0 0 18px rgba(0, 240, 255, 0.24);
  animation: runtimePaneOutlinePulse 0.9s ease-in-out 0s 3;
}

.runtime-pane-attention .setting-group {
  animation: runtimeGroupPulse 0.9s ease-in-out 0s 3;
}

.runtime-section-attention {
  border-radius: 8px;
  animation: runtimeSectionPulse 0.9s ease-in-out 0s 4;
}

@keyframes runtimePaneGlow {
  0% {
    filter: saturate(1);
  }
  50% {
    filter: saturate(1.22) drop-shadow(0 0 14px rgba(0, 240, 255, 0.38));
  }
  100% {
    filter: saturate(1);
  }
}

@keyframes runtimePaneOutlinePulse {
  0% {
    opacity: 0.25;
    transform: scale(1);
  }
  50% {
    opacity: 0.95;
    transform: scale(1.01);
  }
  100% {
    opacity: 0.28;
    transform: scale(1);
  }
}

@keyframes runtimeGroupPulse {
  0% {
    background: rgba(0, 240, 255, 0.02);
    box-shadow: 0 0 0 rgba(0, 240, 255, 0);
  }
  50% {
    background: rgba(0, 240, 255, 0.1);
    box-shadow: 0 0 0 1px rgba(0, 240, 255, 0.26), 0 0 12px rgba(0, 240, 255, 0.22);
  }
  100% {
    background: rgba(0, 240, 255, 0.02);
    box-shadow: 0 0 0 rgba(0, 240, 255, 0);
  }
}

@keyframes runtimeSectionPulse {
  0% {
    background: rgba(0, 240, 255, 0.03);
    box-shadow: 0 0 0 rgba(0, 240, 255, 0);
  }
  50% {
    background: rgba(0, 240, 255, 0.14);
    box-shadow:
      0 0 0 1px rgba(0, 240, 255, 0.4),
      0 0 16px rgba(0, 240, 255, 0.35),
      inset 0 0 18px rgba(0, 240, 255, 0.12);
  }
  100% {
    background: rgba(0, 240, 255, 0.03);
    box-shadow: 0 0 0 rgba(0, 240, 255, 0);
  }
}

.runtime-guide-banner {
  margin-bottom: 16px;
  border-radius: 6px;
  border: 1px solid rgba(0, 240, 255, 0.55);
  background: rgba(0, 240, 255, 0.16);
  color: #b2feff;
  padding: 10px 12px;
  font-size: 13px;
  animation: runtimeBannerBlink 0.95s ease-in-out 0s 3;
}

@keyframes runtimeBannerBlink {
  0% {
    opacity: 0.7;
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0.75;
  }
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
  color: #00f0ff;
  text-transform: uppercase;
  letter-spacing: 1px;
  text-shadow: 0 0 8px rgba(0, 240, 255, 0.4);
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
  border-color: #00f0ff;
  box-shadow: 0 0 8px rgba(0, 240, 255, 0.2);
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
  background: rgba(0, 240, 255, 0.15);
  border: 1px solid rgba(0, 240, 255, 0.4);
  color: #00f0ff;
}

.action-btn.create:hover {
  background: rgba(0, 240, 255, 0.3);
  box-shadow: 0 0 10px rgba(0, 240, 255, 0.3);
}

.action-btn.highlight {
  background: rgba(0, 240, 255, 0.15);
  border: 1px solid rgba(0, 240, 255, 0.4);
  color: #00f0ff;
}

.action-btn.highlight:hover {
  background: rgba(0, 240, 255, 0.3);
  box-shadow: 0 0 10px rgba(0, 240, 255, 0.4);
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
  background: rgba(0, 240, 255, 0.15);
  color: #00f0ff;
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
  border-top-color: #00f0ff;
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin-bottom: 12px;
}

.loading-text {
  color: #00f0ff;
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
  color: #00f0ff;
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
  background: linear-gradient(90deg, #00f0ff, #0099ff);
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
