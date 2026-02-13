<script setup lang="ts">
import { ref, watch, reactive, computed } from 'vue';
import {
  loadGameConfig as apiLoadGameConfig,
  saveGameConfig as apiSaveGameConfig,
  createNewConfig as apiCreateNewConfig,
  deleteGameConfigFolder as apiDeleteGameConfigFolder,
  setGameIcon as apiSetGameIcon,
  setGameBackground as apiSetGameBackground,
  updateGameBackground as apiUpdateGameBackground,
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
  type WineVersion,
  type ProtonSettings,
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

// Tabs
const activeTab = ref('basic');
const tabs = computed(() => [
  { id: 'basic', label: t('gamesettingsmodal.basicsettings') },
  { id: '3dmigoto', label: t('gamesettingsmodal.migoto') },
  { id: 'wine', label: t('gamesettingsmodal.wine') },
  { id: 'other', label: t('gamesettingsmodal.other') },
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
  try {
    const data = await apiLoadGameConfig(props.gameName) as unknown as GameConfig;
    // Merge
    config.basic = {
      gamePreset: data.basic.gamePreset || 'GIMI',
      backgroundType: (data.basic as any).backgroundType || 'Image'
    };

    const threeDMigotoData = data.threeDMigoto || {};
    config.threeDMigoto = {
      installDir: threeDMigotoData.installDir || '',
      targetExePath: threeDMigotoData.targetExePath || '',
      launcherExePath: threeDMigotoData.launcherExePath || '',
      launchArgs: threeDMigotoData.launchArgs || '',
      showErrorPopup: threeDMigotoData.showErrorPopup !== undefined ? threeDMigotoData.showErrorPopup : true,
      autoSetAnalyseOptions: threeDMigotoData.autoSetAnalyseOptions !== undefined ? threeDMigotoData.autoSetAnalyseOptions : true,
      useShell: threeDMigotoData.useShell || false,
      useUpx: threeDMigotoData.useUpx || false,
      delay: threeDMigotoData.delay !== undefined ? threeDMigotoData.delay : 100,
      autoExitSeconds: threeDMigotoData.autoExitSeconds !== undefined ? threeDMigotoData.autoExitSeconds : 5,
      extraDll: threeDMigotoData.extraDll || ''
    };

    // Default Logic for installDir if empty on first load (user requirement)
    if (!config.threeDMigoto.installDir && appSettings.cacheDir) {
      try {
        // "SSMT Cache/3Dmigoto/GameName"
        config.threeDMigoto.installDir = await joinPath(appSettings.cacheDir, '3Dmigoto', props.gameName);
      } catch (err) {
        console.error(t('gamesettingsmodal.error.failconstructdefaultpath'), err);
      }
    }

    config.other = data.other || {};
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
const canAutoUpdate = computed(() => false);

const autoUpdateBackground = async () => {
  try {
    isLoading.value = true;
    await apiUpdateGameBackground(props.gameName, config.basic.gamePreset, config.basic.backgroundType || 'Image');
    await loadGames();
    await showMessage(
      t('gamesettingsmodal.message.success.backgroundupdated'),
      {
        title: t('gamesettingsmodal.message.success.title'),
        kind: 'info'
      }
    );

    if (appSettings.currentConfigName === props.gameName) {
      // Force refresh UI if active
      const current = gamesList.find(g => g.name === props.gameName);
      if (current) switchToGame(current);
    }

  } catch (e) {
    console.error(e);

    await showMessage(
      t('gamesettingsmodal.message.error.updateFailed', { error: e }),
      {
        title: t('gamesettingsmodal.message.error.title'),
        kind: 'error'
      }
    );
  } finally {
    isLoading.value = false;
  }
};

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
    activeTab.value = 'basic'; // Reset to first tab
    configName.value = props.gameName; // Initialize config name from current game
    loadConfig();
    loadWineState();
  } else {
    // When closing, save
    saveConfig();
    saveWineConfig();
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
            <!-- Basic Settings -->
            <div v-if="activeTab === 'basic'" class="tab-pane">
              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.configname') }}</div>
                <input v-model="configName" type="text" class="custom-input" :placeholder="t('gamesettingsmodal.configname_placeholder')" />

                <div class="button-row">
                  <button class="action-btn create" @click="createNewConfig">
                    {{ t('gamesettingsmodal.createnew') }}
                  </button>
                  <button class="action-btn delete" @click="deleteCurrentConfig">
                    {{ t('gamesettingsmodal.deletecurrent') }}
                  </button>
                  <button class="action-btn" @click="resetToDefault">
                    {{ t('gamesettingsmodal.resetdefault') }}
                  </button>
                </div>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.gamepreset') }}</div>
                <el-select v-model="config.basic.gamePreset" placeholder="Select" class="custom-select"
                  @change="saveConfig">
                  <el-option v-for="item in presetOptions" :key="item.value" :label="item.label" :value="item.value" />
                </el-select>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.gameicon') }}</div>
                <button class="action-btn" @click="selectIcon">{{ t('gamesettingsmodal.selecticon') }}</button>
              </div>

              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.bgsettings') }}</div>
                <div style="margin-bottom: 10px;">
                  <el-radio-group v-model="config.basic.backgroundType" @change="handleBgTypeChange">
                    <el-radio value="Image" label="Image">{{ t('gamesettingsmodal.image') }}</el-radio>
                    <el-radio value="Video" label="Video">{{ t('gamesettingsmodal.video') }}</el-radio>
                  </el-radio-group>
                </div>
                <!-- Separate check: if video, show video file btn, if image, show image file btn -->
                <div class="button-row">
                  <button class="action-btn" @click="selectBackground">
                    {{ config.basic.backgroundType === 'Video' ? t('gamesettingsmodal.selectbgvideo') : t('gamesettingsmodal.selectbgimage') }}
                  </button>
                  <button v-if="canAutoUpdate" class="action-btn" @click="autoUpdateBackground">
                    {{ t('gamesettingsmodal.autoupdatebg') }}
                  </button>
                </div>
              </div>
            </div>

            <!-- 3Dmigoto Settings -->
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

            <!-- Wine / Proton Settings -->
            <div v-if="activeTab === 'wine'" class="tab-pane">

              <!-- System Info -->
              <div v-if="displayInfo" class="setting-group info-card">
                <div class="setting-label">{{ t('gamesettingsmodal.sysinfo') }}</div>
                <div class="info-grid">
                  <span class="info-key">{{ t('gamesettingsmodal.displayserver') }}</span>
                  <span class="info-val">{{ displayInfo.server }}{{ displayInfo.wayland_compositor ? ` (${displayInfo.wayland_compositor})` : '' }}</span>
                  <span class="info-key">{{ t('gamesettingsmodal.gpu_driver') }}</span>
                  <span class="info-val">{{ displayInfo.gpu_driver || t('gamesettingsmodal.unknown') }}</span>
                  <span class="info-key">Vulkan</span>
                  <span class="info-val" :class="{ 'text-ok': vulkanInfo?.available, 'text-err': !vulkanInfo?.available }">
                    {{ vulkanInfo?.available ? `✓ ${vulkanInfo.version || ''}` : `✗ ${t('gamesettingsmodal.notdetected')}` }}
                  </span>
                  <span class="info-key">{{ t('gamesettingsmodal.gamepad') }}</span>
                  <span class="info-val">{{ displayInfo.gamepad_detected ? `✓ ${t('gamesettingsmodal.detected')}` : `— ${t('gamesettingsmodal.notdetected')}` }}</span>
                </div>
              </div>

              <!-- Wine/Proton Version Selector -->
              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.wineversion') }}</div>
                <el-select
                  v-model="selectedWineVersionId"
                  :placeholder="t('gamesettingsmodal.wineversion_placeholder')"
                  class="custom-select"
                  filterable
                  style="width: 100%"
                >
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
                    <el-option
                      v-for="ver in group.items"
                      :key="ver.id"
                      :label="`${ver.name} (${ver.version})`"
                      :value="ver.id"
                    />
                  </el-option-group>
                </el-select>

                <div v-if="selectedWineVersion" class="wine-detail">
                  <span class="badge">{{ variantLabel(selectedWineVersion.variant) }}</span>
                  <span class="wine-path">{{ selectedWineVersion.path }}</span>
                </div>
              </div>

              <!-- Proton Settings -->
              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.protonsettings') }}</div>

                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.use_pressure_vessel" />
                    {{ t('gamesettingsmodal.pressure_vessel') }}
                  </label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.proton_enable_wayland" />
                    {{ t('gamesettingsmodal.wayland') }}
                  </label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.proton_no_d3d12" />
                    {{ t('gamesettingsmodal.no_d3d12') }}
                  </label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.proton_media_use_gst" />
                    {{ t('gamesettingsmodal.gstreamer') }}
                  </label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.mangohud" />
                    {{ t('gamesettingsmodal.mangohud') }}
                  </label>
                </div>
                <div class="setting-checkbox-row">
                  <label class="checkbox-label">
                    <input type="checkbox" v-model="protonSettings.steam_deck_compat" />
                    {{ t('gamesettingsmodal.steamdeck') }}
                  </label>
                </div>
              </div>

              <!-- Steam App ID -->
              <div class="setting-group">
                <div class="setting-label">Steam App ID</div>
                <input v-model="protonSettings.steam_app_id" type="text" class="custom-input" placeholder="0 = N/A" />
              </div>

              <!-- Custom Environment Variables -->
              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.custom_env') }}</div>
                <div v-for="(val, key) in protonSettings.custom_env" :key="key" class="env-row">
                  <span class="env-key">{{ key }}</span>
                  <span class="env-val">{{ val }}</span>
                  <button class="env-remove" @click="removeCustomEnv(key as string)">✕</button>
                </div>
                <div class="env-add-row">
                  <input v-model="newEnvKey" type="text" class="custom-input env-input" placeholder="KEY" />
                  <input v-model="newEnvValue" type="text" class="custom-input env-input" placeholder="VALUE" />
                  <button class="action-btn" style="flex: 0 0 auto;" @click="addCustomEnv">{{ t('gamesettingsmodal.add') }}</button>
                </div>
              </div>

              <!-- Save -->
              <div class="button-row">
                <button class="action-btn highlight" @click="saveWineConfig">{{ t('gamesettingsmodal.save_wine') }}</button>
              </div>

            </div>

            <!-- Other Settings -->
            <div v-if="activeTab === 'other'" class="tab-pane">
              <div class="setting-group">
                <div class="setting-label">{{ t('gamesettingsmodal.gamepath') }}</div>
                <input v-model="config.other.gamePath" type="text" class="custom-input" :placeholder="t('gamesettingsmodal.gamepath_placeholder')" />
                <div class="button-row">
                  <button class="action-btn" @click="pickGameExe">{{ t('gamesettingsmodal.selectfile') }}</button>
                </div>
              </div>
              <div class="empty-state" style="margin-top: 20px;">{{ t('gamesettingsmodal.more_coming') }}</div>
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
