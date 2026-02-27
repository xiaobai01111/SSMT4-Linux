import { reactive, watch, ref } from 'vue'
import {
  loadSettings as apiLoadSettings,
  saveSettings as apiSaveSettings,
  getResourcePath,
  scanGames as apiScanGames,
  convertFileSrc,
  showMessage,
} from './api'

// Global UI State
export const isDrawerOpen = ref(false);

export enum BGType {
  Image = 'Image',
  Video = 'Video'
}

export type Locale = 'en' | 'zhs' | 'zht';

// Define the shape of our settings
export interface AppSettings {
  bgType: BGType;
  bgImage: string;
  bgVideo: string;
  contentOpacity: number;
  contentBlur: number;
  cacheDir: string;
  currentConfigName: string;
  githubToken: string;
  showWebsites: boolean;
  showDocuments: boolean;
  locale: Locale;  // 我新增
  dataDir: string;
  initialized: boolean;
  tosRiskAcknowledged: boolean;
  onboardingCompleted: boolean;
  onboardingVersion: number;
  snowbreakSourcePolicy: 'official_first' | 'community_first';
}

export interface GameInfo {
  name: string;
  displayName: string;
  iconPath: string;
  bgPath: string;
  bgVideoPath?: string;
  bgVideoRawPath?: string;
  bgType: BGType;
  showSidebar: boolean;
}

const defaultSettings: AppSettings = {
  bgType: BGType.Image,
  bgImage: '',
  bgVideo: '',
  contentOpacity: 0,
  contentBlur: 0,
  cacheDir: '',
  currentConfigName: 'Default',
  githubToken: '',
  showWebsites: false,
  showDocuments: false,
  locale: 'zhs', // 新增
  dataDir: '',
  initialized: false,
  tosRiskAcknowledged: false,
  onboardingCompleted: false,
  onboardingVersion: 0,
  snowbreakSourcePolicy: 'official_first',
}

export const appSettings = reactive<AppSettings>({ ...defaultSettings })
export const gamesList = reactive<GameInfo[]>([])
export const FEATURE_ONBOARDING_VERSION = 1
export const onboardingVisible = ref(false)
export const onboardingStepIndex = ref(0)

export const startFeatureOnboarding = (startStep = 0) => {
  onboardingStepIndex.value = Math.max(0, Math.floor(startStep))
  onboardingVisible.value = true
}

const markFeatureOnboardingDone = () => {
  appSettings.onboardingCompleted = true
  appSettings.onboardingVersion = FEATURE_ONBOARDING_VERSION
}

export const finishFeatureOnboarding = () => {
  markFeatureOnboardingDone()
  onboardingVisible.value = false
}

export const skipFeatureOnboarding = () => {
  markFeatureOnboardingDone()
  onboardingVisible.value = false
}
let onboardingAutoStarted = false

const shouldAutoStartFeatureOnboarding = () => {
  return (
    appSettings.initialized &&
    appSettings.tosRiskAcknowledged &&
    Number(appSettings.onboardingVersion || 0) < FEATURE_ONBOARDING_VERSION &&
    !appSettings.onboardingCompleted
  )
}

const tryAutoStartFeatureOnboarding = () => {
  if (onboardingAutoStarted) return
  if (!shouldAutoStartFeatureOnboarding()) return
  onboardingAutoStarted = true
  setTimeout(() => startFeatureOnboarding(0), 160)
}

const canonicalGameKey = (value: string): string => {
  return value.trim();
}

// Initial load
let isInitialized = false;
let _settingsLoadedResolve: () => void;
export const settingsLoaded = new Promise<void>((resolve) => {
  _settingsLoadedResolve = resolve;
});

async function loadSettings() {
  try {
    const loaded = await apiLoadSettings()
    Object.assign(appSettings, loaded)
    appSettings.currentConfigName = canonicalGameKey(appSettings.currentConfigName)
    lastSavedSettingsJson = JSON.stringify({ ...appSettings })
    lastSavedSettingsSnapshot = { ...appSettings }
    setTimeout(() => {
      isInitialized = true;
    }, 100);
  } catch (e) {
    console.error('Failed to load settings:', e)
    await showMessage(`加载设置失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    _settingsLoadedResolve();
  }
}


// Default background path
let defaultBgPath = '';

async function initDefaultBackground() {
    try {
        const path = await getResourcePath('Background.png');
        defaultBgPath = convertFileSrc(path);
        if (appSettings.bgType === BGType.Image) {
            const currentBg = appSettings.bgImage || '';
            // 默认背景迁移由后端 settings 归一化负责，前端仅在为空时补默认值。
            if (!currentBg) {
                appSettings.bgImage = defaultBgPath;
            }
        }
    } catch (e) {
        console.warn('Failed to get default background:', e);
    }
}

export async function loadGames() {
  try {
    const games = await apiScanGames();

    // 简化转换逻辑，直接使用后端返回的字段
    const processed = games.map((g: any) => {
      const timestamp = Date.now();

      return {
        name: g.name,
        displayName: g.displayName || g.name,
        iconPath: g.iconPath ? convertFileSrc(g.iconPath) + `?t=${timestamp}` : '',
        bgPath: g.bgPath ? convertFileSrc(g.bgPath) + `?t=${timestamp}` : '',
        bgVideoPath: undefined,  // 视频不再通过 asset 协议，由 switchToGame 按需加载 Blob URL
        bgVideoRawPath: g.bgVideoPath || undefined,
        bgType: g.bgType || BGType.Image,
        showSidebar: g.showSidebar,
      } as GameInfo;
    });

    gamesList.splice(0, gamesList.length, ...processed);
    
    // Ensure default background is loaded
    await initDefaultBackground();

    // Refresh current game background if it exists
    if (appSettings.currentConfigName) {
      const current = gamesList.find(g => g.name === appSettings.currentConfigName);
      if (current) {
        switchToGame(current);
      }
    } else {
        // If no game selected, ensure default background is shown
         if (!appSettings.bgImage && appSettings.bgType === BGType.Image) {
            appSettings.bgImage = defaultBgPath;
        }
    }
  } catch (e) {
    console.error('Failed to scan games:', e);
  }
}

export function switchToGame(game: GameInfo) {
  appSettings.currentConfigName = game.name;
  appSettings.bgType = BGType.Image;
  appSettings.bgImage = game.bgPath || defaultBgPath;
}

// Initial load
async function initStore() {
  await loadSettings();
  await loadGames();
  tryAutoStartFeatureOnboarding();
}
initStore();

// Initialize global download event listeners
import { initDlListeners } from './downloadStore';
initDlListeners();

// Auto-save behavior
let saveSettingsTimer: ReturnType<typeof setTimeout> | null = null;
let isSavingSettings = false;
let hasPendingSave = false;
let lastSavedSettingsJson = '';
let lastSavedSettingsSnapshot: AppSettings | null = null;
const NON_CRITICAL_SAVE_KEYS = new Set<keyof AppSettings>(['currentConfigName', 'bgImage', 'bgType']);

const saveSettingsNow = async () => {
  if (!isInitialized) return;

  const payload = { ...appSettings };
  const currentJson = JSON.stringify(payload);
  if (!hasPendingSave && currentJson === lastSavedSettingsJson) {
    return;
  }

  if (isSavingSettings) {
    hasPendingSave = true;
    return;
  }

  isSavingSettings = true;
  hasPendingSave = false;
  try {
    await apiSaveSettings(payload);
    lastSavedSettingsJson = currentJson;
    lastSavedSettingsSnapshot = payload;
  } catch (e) {
    console.error('Failed to save settings:', e);
  } finally {
    isSavingSettings = false;
    if (hasPendingSave) {
      hasPendingSave = false;
      void scheduleSettingsSave();
    }
  }
};

const scheduleSettingsSave = async () => {
  if (!isInitialized) return;
  const current = { ...appSettings };
  const baseline = lastSavedSettingsSnapshot;
  const changedKeys: (keyof AppSettings)[] = baseline
    ? (Object.keys(current) as (keyof AppSettings)[]).filter((key) => current[key] !== baseline[key])
    : (Object.keys(current) as (keyof AppSettings)[]);

  if (changedKeys.length === 0) return;
  const onlyNonCriticalChanges = changedKeys.every((key) => NON_CRITICAL_SAVE_KEYS.has(key));
  const debounceMs = onlyNonCriticalChanges ? 2400 : 900;

  if (saveSettingsTimer) {
    clearTimeout(saveSettingsTimer);
  }
  saveSettingsTimer = setTimeout(() => {
    saveSettingsTimer = null;
    void saveSettingsNow();
  }, debounceMs);
};

watch(
  appSettings,
  () => {
    if (!isInitialized) return;
    void scheduleSettingsSave();
  },
  { deep: true },
)

watch(
  () => [
    appSettings.initialized,
    appSettings.tosRiskAcknowledged,
    appSettings.onboardingCompleted,
    appSettings.onboardingVersion,
  ],
  () => {
    tryAutoStartFeatureOnboarding()
  },
)
