import { reactive, watch, ref } from 'vue'
import {
  loadSettings as apiLoadSettings,
  saveSettings as apiSaveSettings,
  getResourcePath,
  scanGames as apiScanGames,
  getGameLauncherApi,
  getGameState as apiGetGameState,
  getLauncherInstallerState,
  getDefaultGameFolder,
  loadGameConfig,
  convertFileSrc,
  showMessage,
} from './api'
import type { AppSettings, GameConfig, GameInfo } from './types/ipc'
import type { Locale } from './types/ipc'
import {
  resolveGameUpdateFolder,
  resolveGameUpdateSource,
} from './utils/gameUpdateContext'
import {
  beginGameUpdateCheck,
  buildErrorGameUpdateCheckPatch,
  buildIdleGameUpdateCheckPatch,
  buildReadyGameUpdateCheckPatch,
  createGameUpdateCheckState,
  shouldApplyGameUpdateResult,
  type GameUpdateCheckState,
} from './utils/gameUpdateCheckState'

// Global UI State
export const isDrawerOpen = ref(false);

export enum BGType {
  Image = 'Image',
  Video = 'Video'
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
  migotoEnabled: false,
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
export const gamesLoading = ref(false)
export const settingsReady = ref(false)
export type { GameUpdateCheckPhase, GameUpdateCheckState } from './utils/gameUpdateCheckState'

export const gameUpdateStates = reactive<Record<string, GameUpdateCheckState>>({})
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
let onboardingHostReady = false
let onboardingAutoStartPending = false

const shouldAutoStartFeatureOnboarding = () => {
  return (
    appSettings.initialized &&
    appSettings.tosRiskAcknowledged &&
    Number(appSettings.onboardingVersion || 0) < FEATURE_ONBOARDING_VERSION &&
    !appSettings.onboardingCompleted
  )
}

const flushAutoStartFeatureOnboarding = () => {
  if (onboardingAutoStarted) return
  if (!onboardingAutoStartPending) return
  if (!onboardingHostReady) return
  onboardingAutoStarted = true
  onboardingAutoStartPending = false
  startFeatureOnboarding(0)
}

const tryAutoStartFeatureOnboarding = () => {
  if (onboardingAutoStarted) return
  if (!shouldAutoStartFeatureOnboarding()) return
  onboardingAutoStartPending = true
  flushAutoStartFeatureOnboarding()
}

export const markOnboardingHostReady = () => {
  onboardingHostReady = true
  flushAutoStartFeatureOnboarding()
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
  } catch (e) {
    console.error('Failed to load settings:', e)
    await showMessage(`加载设置失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    settingsReady.value = true;
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

let suspendAutoSaveDepth = 0;
let gamesRefreshPromise: Promise<void> | null = null;
let queuedGamesRefresh = false;
let gameUpdateRequestId = 0;

const withAutoSaveSuspended = <T>(fn: () => T): T => {
  suspendAutoSaveDepth += 1;
  try {
    return fn();
  } finally {
    suspendAutoSaveDepth -= 1;
  }
};

const mapScannedGames = (games: GameInfo[]): GameInfo[] => {
  const timestamp = Date.now();
  return games.map((g) => ({
    name: g.name,
    displayName: g.displayName || g.name,
    iconPath: g.iconPath ? convertFileSrc(g.iconPath) + `?t=${timestamp}` : '',
    bgPath: g.bgPath ? convertFileSrc(g.bgPath) + `?t=${timestamp}` : '',
    bgVideoPath: undefined,
    bgVideoRawPath: g.bgVideoPath || undefined,
    bgType: g.bgType || BGType.Image,
    showSidebar: g.showSidebar,
    migotoSupported: Boolean(g.migotoSupported),
  } as GameInfo));
};

const nowTs = () => Date.now();

const ensureGameUpdateState = (gameName: string): GameUpdateCheckState => {
  const existing = gameUpdateStates[gameName];
  if (existing) return existing;

  const created: GameUpdateCheckState = createGameUpdateCheckState();
  gameUpdateStates[gameName] = created;
  return created;
};

const cleanupRemovedGameUpdateStates = (games: GameInfo[]) => {
  const validNames = new Set(games.map((game) => game.name));
  for (const gameName of Object.keys(gameUpdateStates)) {
    if (!validNames.has(gameName)) {
      delete gameUpdateStates[gameName];
    }
  }
};

interface ResolvedGameUpdateContext {
  launcherApi: string;
  gameFolder: string;
  bizPrefix?: string;
  downloadMode: 'full_game' | 'launcher_installer';
  gamePreset: string;
}

const resolveConfiguredGamePreset = (gameName: string, config: GameConfig): string => {
  const preset = String(config.basic?.gamePreset || '').trim();
  return preset || gameName;
};

const resolveSavedLauncherApi = (config: GameConfig): string =>
  typeof config.other?.launcherApi === 'string' ? config.other.launcherApi.trim() : '';

const resolveSavedGameFolder = (config: GameConfig): string =>
  typeof config.other?.gameFolder === 'string' ? config.other.gameFolder.trim() : '';

const resolveSavedGamePath = (config: GameConfig): string =>
  typeof config.other?.gamePath === 'string' ? config.other.gamePath.trim() : '';

async function resolveGameUpdateContext(gameName: string): Promise<ResolvedGameUpdateContext | null> {
  let config: GameConfig;
  try {
    config = await loadGameConfig(gameName);
  } catch {
    return null;
  }

  const savedGamePath = resolveSavedGamePath(config);
  const gamePreset = resolveConfiguredGamePreset(gameName, config);
  let launcherInfo;
  try {
    launcherInfo = await getGameLauncherApi(gamePreset);
  } catch {
    return null;
  }

  if (!launcherInfo.supported) {
    return null;
  }

  let gameFolder = resolveGameUpdateFolder(
    resolveSavedGameFolder(config),
    savedGamePath,
    '',
    '',
  );
  if (!gameFolder) {
    try {
      const baseDir = await getDefaultGameFolder(gameName);
      gameFolder = resolveGameUpdateFolder(
        resolveSavedGameFolder(config),
        savedGamePath,
        baseDir,
        launcherInfo.defaultFolder || '',
      );
    } catch {
      gameFolder = '';
    }
  }

  if (!gameFolder) {
    return null;
  }

  const servers = launcherInfo.servers || [];
  const resolvedSource = resolveGameUpdateSource(
    resolveSavedLauncherApi(config),
    servers,
    launcherInfo.launcherApi || '',
  );
  if (!resolvedSource) {
    return null;
  }

  return {
    launcherApi: resolvedSource.launcherApi,
    gameFolder,
    bizPrefix: resolvedSource.bizPrefix,
    downloadMode: launcherInfo.downloadMode || 'full_game',
    gamePreset,
  };
}

export function getGameUpdateState(gameName: string): GameUpdateCheckState | null {
  return gameUpdateStates[gameName] || null;
}

export async function refreshGameUpdateState(gameName: string): Promise<void> {
  const entry = ensureGameUpdateState(gameName);
  const requestId = ++gameUpdateRequestId;
  Object.assign(entry, beginGameUpdateCheck(requestId, nowTs()));

  try {
    const context = await resolveGameUpdateContext(gameName);
    if (!shouldApplyGameUpdateResult(entry, requestId)) return;

    if (!context) {
      Object.assign(entry, buildIdleGameUpdateCheckPatch(nowTs()));
      return;
    }

    const state =
      context.downloadMode === 'launcher_installer'
        ? await getLauncherInstallerState(
            context.launcherApi,
            context.gameFolder,
            context.gamePreset,
          )
        : await apiGetGameState(
            context.launcherApi,
            context.gameFolder,
            context.bizPrefix,
          );
    if (!shouldApplyGameUpdateResult(entry, requestId)) return;

    Object.assign(entry, buildReadyGameUpdateCheckPatch(state, nowTs()));
  } catch (error) {
    if (!shouldApplyGameUpdateResult(entry, requestId)) return;
    Object.assign(entry, buildErrorGameUpdateCheckPatch(error, nowTs()));
  }
}

export async function refreshAllGameUpdateStates(games: GameInfo[] = gamesList): Promise<void> {
  cleanupRemovedGameUpdateStates(games);
  await Promise.allSettled(games.map((game) => refreshGameUpdateState(game.name)));
}

const applyScannedGames = (games: GameInfo[]) => {
  const processed = mapScannedGames(games);
  gamesList.splice(0, gamesList.length, ...processed);
  cleanupRemovedGameUpdateStates(processed);

  withAutoSaveSuspended(() => {
    if (appSettings.currentConfigName) {
      const current = gamesList.find(g => g.name === appSettings.currentConfigName);
      if (current) {
        switchToGame(current);
      }
    } else if (!appSettings.bgImage && appSettings.bgType === BGType.Image) {
      appSettings.bgImage = defaultBgPath;
    }
  });

  const baseline = lastSavedSettingsSnapshot
    ? { ...lastSavedSettingsSnapshot }
    : { ...appSettings };
  baseline.currentConfigName = appSettings.currentConfigName;
  baseline.bgImage = appSettings.bgImage;
  baseline.bgType = appSettings.bgType;
  lastSavedSettingsSnapshot = baseline;
  lastSavedSettingsJson = JSON.stringify(baseline);
};

const runQueuedGamesRefresh = async () => {
  if (gamesRefreshPromise) {
    return gamesRefreshPromise;
  }

  gamesRefreshPromise = (async () => {
    gamesLoading.value = true;
    try {
      while (queuedGamesRefresh) {
        queuedGamesRefresh = false;
        try {
          const games = await apiScanGames();
          applyScannedGames(games);
          void refreshAllGameUpdateStates(games);
        } catch (e) {
          console.error('Failed to scan games:', e);
        }
      }
    } finally {
      gamesLoading.value = false;
      gamesRefreshPromise = null;
    }
  })();

  return gamesRefreshPromise;
};

const queueGamesRefresh = async () => {
  queuedGamesRefresh = true;
  await runQueuedGamesRefresh();
};

export async function loadGames() {
  await queueGamesRefresh();
}

export function switchToGame(game: GameInfo) {
  appSettings.currentConfigName = game.name;
  appSettings.bgType = BGType.Image;
  appSettings.bgImage = game.bgPath || defaultBgPath;
}

// Initial load
async function initStore() {
  // 启动阶段只等待 settings 和默认背景；游戏扫描由 post-bootstrap 任务显式启动。
  await Promise.all([
    loadSettings(),
    initDefaultBackground(),
  ]);

  if (!appSettings.bgImage && appSettings.bgType === BGType.Image) {
    appSettings.bgImage = defaultBgPath;
  }

  lastSavedSettingsJson = JSON.stringify({ ...appSettings });
  lastSavedSettingsSnapshot = { ...appSettings };
  isInitialized = true;
  tryAutoStartFeatureOnboarding();
}

let storeBootstrapPromise: Promise<void> | null = null;
let postBootstrapTasksStarted = false;

export function bootstrapStore(): Promise<void> {
  if (isInitialized) return Promise.resolve();
  if (storeBootstrapPromise) return storeBootstrapPromise;

  ensureStoreWatchers();
  storeBootstrapPromise = initStore().finally(() => {
    storeBootstrapPromise = null;
  });
  return storeBootstrapPromise;
}

export function startStorePostBootstrapTasks() {
  if (postBootstrapTasksStarted) return;
  postBootstrapTasksStarted = true;
  void loadGames();
}

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

const waitForSettingsSaveIdle = async () => {
  while (isSavingSettings) {
    await new Promise((resolve) => setTimeout(resolve, 40));
  }
};

export const flushSettingsSave = async () => {
  if (saveSettingsTimer) {
    clearTimeout(saveSettingsTimer);
    saveSettingsTimer = null;
  }

  hasPendingSave = true;
  await waitForSettingsSaveIdle();
  await saveSettingsNow();
};

export const updateLocaleAndReload = async (locale: Locale) => {
  if (!locale || appSettings.locale === locale) return;

  withAutoSaveSuspended(() => {
    appSettings.locale = locale;
  });

  await flushSettingsSave();

  if (typeof window !== 'undefined') {
    window.location.reload();
  }
};

let storeWatchersReady = false;

function ensureStoreWatchers() {
  if (storeWatchersReady) return;
  storeWatchersReady = true;

  watch(
    appSettings,
    () => {
      if (!isInitialized) return;
      if (suspendAutoSaveDepth > 0) return;
      void scheduleSettingsSave();
    },
    { deep: true },
  );

  watch(
    () => [
      appSettings.initialized,
      appSettings.tosRiskAcknowledged,
      appSettings.onboardingCompleted,
      appSettings.onboardingVersion,
    ],
    () => {
      tryAutoStartFeatureOnboarding();
    },
  );
}
