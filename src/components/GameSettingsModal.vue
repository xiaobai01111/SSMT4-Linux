<script setup lang="ts">
import { ref, watch, reactive, computed } from 'vue';
import {
  loadGameConfig as apiLoadGameConfig,
  saveGameConfig as apiSaveGameConfig,
  createNewConfig as apiCreateNewConfig,
  deleteGameConfigFolder as apiDeleteGameConfigFolder,
  setGameIcon as apiSetGameIcon,
  setGameBackground as apiSetGameBackground,
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
  type WineVersion,
  type ProtonSettings,
  type PrefixInfo,
  type JadeiteStatus,
  type VulkanInfo,
  type DisplayInfo,
} from '../api';
import { loadGames, appSettings, gamesList, switchToGame } from '../store';
import { useI18n } from 'vue-i18n';

const { t } = useI18n();

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
    // configName is kept in UI state but separate from the object sent to backend
    gamePreset: string;
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
  basic: { gamePreset: 'GIMI', backgroundType: 'Image' },
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


const isLoading = ref(false);
const hasLoadedConfig = ref(false);

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const asString = (value: unknown, fallback = ''): string =>
  typeof value === 'string' ? value : fallback;

const asNumber = (value: unknown, fallback: number): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : fallback;

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
    'GIMI';

  const backgroundTypeRaw =
    asString(basicRaw.backgroundType) || asString(root.backgroundType);
  const backgroundType: 'Image' | 'Video' =
    backgroundTypeRaw === 'Video' ? 'Video' : 'Image';

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
      gamePreset,
      backgroundType,
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

const isHoyoverse = computed(() => ['GIMI', 'SRMI', 'ZZMI', 'HIMI'].includes(config.basic.gamePreset));

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

// DXVK 管理
const dxvkVersion = ref('2.5.3');
const isDxvkBusy = ref(false);

const doInstallDxvk = async () => {
  if (isDxvkBusy.value) return;
  try {
    isDxvkBusy.value = true;
    const result = await installDxvk(props.gameName, dxvkVersion.value);
    await showMessage(result, { title: 'DXVK', kind: 'info' });
    await loadPrefixState();
  } catch (e) {
    await showMessage(`DXVK 安装失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isDxvkBusy.value = false;
  }
};

const doUninstallDxvk = async () => {
  if (isDxvkBusy.value) return;
  try {
    isDxvkBusy.value = true;
    const result = await uninstallDxvk(props.gameName);
    await showMessage(result, { title: 'DXVK', kind: 'info' });
    await loadPrefixState();
  } catch (e) {
    await showMessage(`DXVK 卸载失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isDxvkBusy.value = false;
  }
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

const presetOptions = computed(() => [
  { label: `${t('games.GIMI')} (GIMI)`, value: 'GIMI' },
  { label: `${t('games.HIMI')} (HIMI)`, value: 'HIMI' },
  { label: `${t('games.SRMI')} (SRMI)`, value: 'SRMI' },
  { label: `${t('games.ZZMI')} (ZZMI)`, value: 'ZZMI' },
  { label: `${t('games.WWMI')} (WWMI)`, value: 'WWMI' },
  { label: `${t('games.EFMI')} (EFMI)`, value: 'EFMI' },
  { label: `${t('games.GF2')} (GF2)`, value: 'GF2' },
  { label: `${t('games.IdentityV')} NeoX2`, value: 'IdentityVNeoX2' },
  { label: `${t('games.IdentityV')} NeoX3`, value: 'IdentityVNeoX3' },
  { label: 'AI LIMIT', value: 'AILIMIT' },
  { label: `${t('games.DOAV')} (DOAV)`, value: 'DOAV' },
  { label: 'MiSide', value: 'MiSide' },
  { label: `${t('games.SnowBreak')} (SnowBreak)`, value: 'SnowBreak' },
  { label: 'Strinova', value: 'Strinova' },
  { label: `${t('games.Nioh2')} (Nioh2)`, value: 'Nioh2' },
  { label: `${t('games.AEMI')} (AEMI)`, value: 'AEMI' },
]);

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
    if (!config.threeDMigoto.installDir && appSettings.cacheDir) {
      try {
        // "SSMT Cache/3Dmigoto/GameName"
        config.threeDMigoto.installDir = await joinPath(appSettings.cacheDir, '3Dmigoto', props.gameName);
      } catch (err) {
        console.error(t('gamesettingsmodal.error.failconstructdefaultpath'), err);
      }
    }

    config.other = normalized.other || {};
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
    // 删除自定义背景文件
    await apiResetGameBackground(props.gameName);
    // 重置配置为默认值
    await apiSaveGameConfig(props.gameName, {
      basic: { gamePreset: 'GIMI', backgroundType: 'Image' },
      threeDMigoto: {},
      other: {}
    } as any);
    await loadConfig();
    await loadGames();
  } catch (e) {
    console.error('Reset failed:', e);
  } finally {
    isLoading.value = false;
  }
};

const handleBgTypeChange = async () => {
  await saveConfig();
  // Refresh global state if this is the active game
  if (appSettings.currentConfigName === props.gameName) {
    await loadGames();
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
      await loadGames();
    }
  } catch (e) {
    console.error(e);
  }
};

const selectBackground = async () => {
  try {
    const isVideo = config.basic.backgroundType === 'Video';
    const filters = isVideo
      ? [{ name: 'Videos', extensions: ['mp4', 'webm', 'ogg', 'mov'] }]
      : [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'svg', 'bmp', 'ico', 'avif'] }];

    const file = await openFileDialog({
      multiple: false,
      filters
    });

    if (file) {
      await apiSetGameBackground(props.gameName, file, config.basic.backgroundType || 'Image');
      await loadGames(); // Refresh
    }
  } catch (e) {
    console.error(e);
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

const packageSupportedPresets = ['GIMI', 'HIMI', 'SRMI', 'ZZMI', 'WWMI', 'EFMI', 'AEMI'];
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
    const newGame = gamesList.find(g => g.name === configName.value);
    if (newGame) {
      switchToGame(newGame);
    }

    close();
  } catch (e) {
    console.error(t('gamesettingsmodal.log.configCreateFailed', { error: e }));
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
    close();
  } catch (e) {
    console.error('Failed to delete config:', e);
  } finally {
    isLoading.value = false;
  }
};

// Open/Close
watch(() => props.modelValue, (val) => {
  if (val) {
    activeTab.value = 'info'; // Reset to first tab
    configName.value = props.gameName;
    hasLoadedConfig.value = false;
    loadConfig();
    loadWineState();
    loadJadeiteState();
    loadPrefixState();
  } else {
    // Only save when current modal session loaded successfully.
    if (hasLoadedConfig.value) {
      saveConfig();
      saveWineConfig();
    }
  }
});

const close = () => {
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
        <div v-if="isLoading" class="loading-overlay">
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
              <div class="setting-group">
                <div class="setting-label">配置名称</div>
                <input v-model="configName" type="text" class="custom-input" placeholder="输入配置名称..." />
                <div class="button-row">
                  <button class="action-btn create" @click="createNewConfig">新建配置</button>
                  <button class="action-btn delete" @click="deleteCurrentConfig">删除配置</button>
                  <button class="action-btn" @click="resetToDefault">恢复默认</button>
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">游戏预设</div>
                <el-select v-model="config.basic.gamePreset" placeholder="Select" class="custom-select" @change="saveConfig">
                  <el-option v-for="item in presetOptions" :key="item.value" :label="item.label" :value="item.value" />
                </el-select>
              </div>

              <div class="setting-group">
                <div class="setting-label">游戏图标</div>
                <button class="action-btn" @click="selectIcon">选择图标</button>
              </div>

              <div class="setting-group">
                <div class="setting-label">背景设置</div>
                <div style="margin-bottom: 10px;">
                  <el-radio-group v-model="config.basic.backgroundType" @change="handleBgTypeChange">
                    <el-radio value="Image" label="Image">图片</el-radio>
                    <el-radio value="Video" label="Video">视频</el-radio>
                  </el-radio-group>
                </div>
                <div class="button-row">
                  <button class="action-btn" @click="selectBackground">
                    {{ config.basic.backgroundType === 'Video' ? '选择背景视频' : '选择背景图片' }}
                  </button>
                </div>
              </div>
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
              <!-- Wine/Proton 版本选择 -->
              <div class="setting-group">
                <div class="setting-label">Wine 版本</div>
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
              </div>

              <!-- DXVK -->
              <div class="setting-group">
                <div class="setting-label">DXVK</div>
                <div class="info-text" v-if="prefixInfo?.config?.dxvk">
                  <span :class="prefixInfo.config.dxvk.enabled ? 'text-ok' : 'text-muted'">
                    {{ prefixInfo.config.dxvk.enabled ? `✓ 已启用 (${prefixInfo.config.dxvk.version || '未知版本'})` : '未启用' }}
                  </span>
                </div>
                <div class="flex-row" style="align-items:flex-end; gap:8px; margin-top:6px;">
                  <input v-model="dxvkVersion" type="text" class="custom-input" placeholder="版本号，如 2.5.3" style="flex:1" />
                  <button class="action-btn highlight" @click="doInstallDxvk" :disabled="isDxvkBusy" style="flex:0 0 auto">安装</button>
                  <button class="action-btn" @click="doUninstallDxvk" :disabled="isDxvkBusy" style="flex:0 0 auto">卸载</button>
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
                <button class="action-btn highlight" @click="saveWineConfig">保存运行环境配置</button>
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
                <button class="action-btn highlight" @click="saveWineConfig">保存系统选项</button>
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
</style>
