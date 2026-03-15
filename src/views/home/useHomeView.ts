import { computed, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import {
  appSettings,
  gamesList,
  gamesLoading,
  getGameUpdateState,
  loadGames,
  refreshGameUpdateState,
  switchToGame,
} from '../../store';
import {
  askConfirm,
  loadGameConfig,
  openGameLogWindow,
  setGameVisibility,
  showMessage,
  startGame as apiStartGame,
} from '../../api';
import { activeDownloadTask } from '../../downloadStore';
import { useHomeLaunchGuards } from '../../composables/useHomeLaunchGuards';
import { useHomeLaunchGuardUi } from '../../composables/useHomeLaunchGuardUi';
import { useHomeRuntimeController } from '../../composables/useHomeRuntimeController';
import {
  resolveHomePrimaryAction,
  resolveHomePrimaryLabelKey,
  shouldOpenDownloadForPrimaryAction,
} from '../../utils/gameUpdateUi';
import type { GameInfo } from '../../types/ipc';
import type {
  GameSettingsOpenRequest,
  GameSettingsTab,
  GlobalSettingsMenu,
  OnboardingHomeAction,
  RuntimeFocusTarget,
} from '../../types/gameSettings';

export const useHomeView = () => {
  const { t, te } = useI18n();
  const router = useRouter();
  const getGameName = (game: GameInfo) =>
    te(`games.${game.name}`) ? t(`games.${game.name}`) : game.displayName || game.name;

  const hasCurrentGame = computed(() => {
    const gameName = appSettings.currentConfigName;
    if (!gameName || gameName === 'Default') return false;
    return gamesList.some((game) => game.name === gameName);
  });

  const sidebarGames = computed(() => gamesList.filter((game) => game.showSidebar).reverse());
  const isGameActive = (gameName: string) => appSettings.currentConfigName === gameName;
  const handleGameClick = (game: GameInfo) => {
    switchToGame(game);
  };

  const showMenu = ref(false);
  const menuX = ref(0);
  const menuY = ref(0);
  const targetGame = ref<GameInfo | null>(null);

  const handleContextMenu = (event: MouseEvent, game: GameInfo) => {
    event.preventDefault();
    targetGame.value = game;
    menuX.value = event.clientX;
    menuY.value = event.clientY;
    showMenu.value = true;
  };

  const closeMenu = () => {
    showMenu.value = false;
  };

  const hideGame = async () => {
    if (!targetGame.value) return;

    const gameName = targetGame.value.name;
    const wasActive = isGameActive(gameName);

    try {
      await setGameVisibility(gameName, true);
      await loadGames();
      if (wasActive && sidebarGames.value.length > 0) {
        switchToGame(sidebarGames.value[0]);
      }
    } catch (error) {
      console.error(t('home.hidegame.fail'), error);
    }

    closeMenu();
  };

  const showSettings = ref(false);
  const showDownload = ref(false);

  const currentDisplayName = computed(() => {
    const game = gamesList.find((item) => item.name === appSettings.currentConfigName);
    return game?.displayName || appSettings.currentConfigName;
  });
  const currentGameUpdateCheck = computed(() => {
    const gameName = String(appSettings.currentConfigName || '').trim();
    if (!gameName || gameName === 'Default') return null;
    return getGameUpdateState(gameName);
  });
  const currentGameNeedsUpdate = computed(() => currentGameUpdateCheck.value?.state?.state === 'needupdate');
  const settingsModalRef = ref<{
    prepareOpen?: (request: GameSettingsOpenRequest) => void;
    switchTab?: (tabId: GameSettingsTab) => void;
    focusRuntimeSetup?: (message?: string, focusTarget?: RuntimeFocusTarget) => void;
  } | null>(null);

  const ensureSettingsGameSelected = async (): Promise<boolean> => {
    const current = String(appSettings.currentConfigName || '').trim();
    if (current && current !== 'Default' && gamesList.some((game) => game.name === current)) {
      return true;
    }

    const fallback = sidebarGames.value[0] || gamesList[0];
    if (fallback) {
      switchToGame(fallback);
      return true;
    }

    await showMessage(t('home.messages.needGameInLibrary'), { title: t('home.messages.title.info'), kind: 'info' });
    return false;
  };

  const openGlobalSettingsMenu = async (menu: GlobalSettingsMenu, reason?: string) => {
    showSettings.value = false;
    await router.push({
      path: '/settings',
      query: {
        menu,
        guide: '1',
        reason: reason || '',
        t: String(Date.now()),
      },
    });
  };

  const openGameSettings = async (request: GameSettingsOpenRequest) => {
    if (!(await ensureSettingsGameSelected())) return false;
    showDownload.value = false;
    settingsModalRef.value?.prepareOpen?.(request);
    showSettings.value = true;
    return true;
  };

  const openRuntimeSettings = async (reason?: string, focusTarget: RuntimeFocusTarget = 'all') => {
    await openGameSettings({
      tab: 'runtime',
      reason,
      runtimeFocus: focusTarget,
    });
  };

  const openGameSettingsGameTab = async () => {
    await openGameSettings({ tab: 'game' });
  };

  const openGameSettingsTab = async (tab: GameSettingsTab, runtimeFocus: RuntimeFocusTarget = 'all') => {
    await openGameSettings({
      tab,
      runtimeFocus: tab === 'runtime' ? runtimeFocus : undefined,
    });
  };

  const applyOnboardingHomeAction = async (detail?: OnboardingHomeAction) => {
    if (!detail) return;
    if (detail.type === 'close_modals') {
      showSettings.value = false;
      showDownload.value = false;
      return;
    }
    if (detail.type === 'open_download_modal') {
      if (!(await ensureSettingsGameSelected())) return;
      showSettings.value = false;
      showDownload.value = true;
      return;
    }
    if (detail.type === 'open_game_settings') {
      await openGameSettingsTab(detail.tab, detail.runtimeFocus || 'all');
    }
  };

  const onOnboardingActionEvent = (event: Event) => {
    const detail = (event as CustomEvent<OnboardingHomeAction>).detail;
    void applyOnboardingHomeAction(detail);
  };

  const errorText = (error: unknown): string => {
    if (error instanceof Error) return error.message;
    return String(error ?? '');
  };

  const gameHasExe = ref(false);
  const gameExeCache = new Map<string, boolean>();
  let checkGameExeToken = 0;

  const checkGameExe = async (force = false) => {
    const gameName = appSettings.currentConfigName;
    if (!gameName || gameName === 'Default') {
      gameHasExe.value = false;
      return;
    }
    if (!force && gameExeCache.has(gameName)) {
      gameHasExe.value = !!gameExeCache.get(gameName);
      return;
    }
    const token = ++checkGameExeToken;
    try {
      const data = await loadGameConfig(gameName);
      if (token !== checkGameExeToken) return;
      const hasExe = !!data.other?.gamePath;
      gameExeCache.set(gameName, hasExe);
      gameHasExe.value = hasExe;
    } catch {
      if (token !== checkGameExeToken) return;
      gameExeCache.set(gameName, false);
      gameHasExe.value = false;
    }
  };

  const {
    isLaunching,
    isGameRunning,
    componentDlProgress,
    markLaunchRequested,
    resetLaunchState,
  } = useHomeRuntimeController({
    currentGameName: () => String(appSettings.currentConfigName || ''),
    checkGameExe,
    refreshGameUpdateState,
    closeMenu,
    onOnboardingActionEvent,
    showAnticheatWarning: (message) =>
      showMessage(message, {
        title: t('home.messages.title.anticheatWarning'),
        kind: 'warning',
      }),
  });

  const currentPrimaryAction = computed(() =>
    resolveHomePrimaryAction({
      isGameRunning: isGameRunning.value,
      needsUpdate: !!currentGameNeedsUpdate.value,
      hasExecutable: gameHasExe.value,
    }),
  );
  const startButtonLabel = computed(() => t(resolveHomePrimaryLabelKey(currentPrimaryAction.value)));

  const { planLaunchGuards } = useHomeLaunchGuards();
  const { ensureRiskAcknowledged, executeGuardPlan } = useHomeLaunchGuardUi({
    openGlobalSettingsMenu,
    openRuntimeSettings,
    openGameSettingsGameTab,
    openDownloadModal: () => {
      showDownload.value = true;
    },
  });

  const launchGame = async () => {
    if (!markLaunchRequested()) {
      return;
    }

    const gameName = appSettings.currentConfigName;
    if (!gameName || gameName === 'Default') {
      await showMessage(t('home.messages.needSelectGame'), { title: t('home.messages.title.info'), kind: 'info' });
      resetLaunchState();
      return;
    }

    if (!(await ensureRiskAcknowledged(appSettings.tosRiskAcknowledged, () => {
      appSettings.tosRiskAcknowledged = true;
    }))) {
      resetLaunchState();
      return;
    }

    try {
      const data = await loadGameConfig(gameName);
      const gameExePath = data.other?.gamePath || '';
      const guardPlan = await planLaunchGuards(gameName, data, !!gameExePath);
      if (!(await executeGuardPlan(guardPlan))) {
        resetLaunchState();
        return;
      }

      if (!gameExePath) {
        await showMessage(t('home.messages.needGameExePath'), { title: t('home.messages.title.info'), kind: 'info' });
        resetLaunchState();
        return;
      }

      await apiStartGame(gameName, gameExePath, guardPlan.wineVersionId);
    } catch (error: unknown) {
      resetLaunchState();
      const errText = errorText(error);
      if (
        errText.includes('Wine/Proton') ||
        errText.includes('运行环境') ||
        errText.includes('启动配置错误')
      ) {
        const openNow = await askConfirm(
          t('home.messages.runtimeLaunchFailedConfirm', { error: errText }),
          {
            title: t('home.messages.title.runtimeError'),
            kind: 'error',
            okLabel: t('home.messages.ok.openRuntime'),
            cancelLabel: t('home.messages.cancel.later'),
          },
        );
        if (openNow) {
          await openRuntimeSettings(t('home.messages.reason.fixRuntime'), 'wine_version');
          return;
        }
      }
      await showMessage(t('home.messages.launchFailed', { error: errText }), { title: t('home.messages.title.error'), kind: 'error' });
    }
  };

  const openCurrentGameLog = async () => {
    const gameName = appSettings.currentConfigName;
    if (!gameName || gameName === 'Default') {
      await showMessage(t('home.messages.needSelectGame'), { title: t('home.messages.title.info'), kind: 'info' });
      return;
    }
    try {
      await openGameLogWindow(gameName);
    } catch (error: unknown) {
      await showMessage(t('home.messages.openGameLogFailed', { error: errorText(error) }), { title: t('home.messages.title.error'), kind: 'error' });
    }
  };

  const handlePrimaryAction = () => {
    if (isGameRunning.value || isLaunching.value) return;
    if (shouldOpenDownloadForPrimaryAction(currentPrimaryAction.value)) {
      showDownload.value = true;
      return;
    }
    void launchGame();
  };

  return {
    t,
    router,
    appSettings,
    gamesLoading,
    sidebarGames,
    hasCurrentGame,
    targetGame,
    showMenu,
    menuX,
    menuY,
    showSettings,
    showDownload,
    currentDisplayName,
    currentGameNeedsUpdate,
    startButtonLabel,
    settingsModalRef,
    gameHasExe,
    isLaunching,
    isGameRunning,
    componentDlProgress,
    getGameName,
    isGameActive,
    handleGameClick,
    handleContextMenu,
    hideGame,
    openGameSettingsTab,
    openCurrentGameLog,
    handlePrimaryAction,
    checkGameExe,
    activeDownloadTask,
  };
};
