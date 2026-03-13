import { onMounted, onUnmounted, ref, watch } from 'vue';
import { listenAppEvent } from '../api';
import type { ComponentDownloadProgressEvent } from '../types/events';
import {
  applyComponentDownloadProgressEvent,
  applyGameLifecycleEvent,
  createHomeRuntimeSnapshot,
} from '../utils/homeRuntimeState';

interface HomeRuntimeControllerOptions {
  currentGameName: () => string;
  checkGameExe: (force?: boolean) => Promise<void> | void;
  refreshGameUpdateState: (gameName: string) => Promise<unknown>;
  closeMenu: () => void;
  onOnboardingActionEvent: (event: Event) => void;
  showAnticheatWarning: (message: string) => Promise<void> | void;
}

export function useHomeRuntimeController(
  options: HomeRuntimeControllerOptions,
) {
  const runtimeState = ref(createHomeRuntimeSnapshot());

  const isLaunching = ref(false);
  const isGameRunning = ref(false);
  const runningGameName = ref('');
  const componentDlProgress = ref<ComponentDownloadProgressEvent | null>(null);

  const syncRefsFromState = () => {
    isLaunching.value = runtimeState.value.isLaunching;
    isGameRunning.value = runtimeState.value.isGameRunning;
    runningGameName.value = runtimeState.value.runningGameName;
    componentDlProgress.value = runtimeState.value.componentDlProgress;
  };

  const markLaunchRequested = (): boolean => {
    if (runtimeState.value.isLaunching || runtimeState.value.isGameRunning) {
      return false;
    }
    runtimeState.value = {
      ...runtimeState.value,
      isLaunching: true,
    };
    syncRefsFromState();
    return true;
  };

  const resetLaunchState = () => {
    runtimeState.value = {
      ...runtimeState.value,
      isLaunching: false,
    };
    syncRefsFromState();
  };

  watch(
    () => options.currentGameName(),
    () => {
      void options.checkGameExe(false);
      const gameName = String(options.currentGameName() || '').trim();
      if (!gameName || gameName === 'Default') return;
      void options.refreshGameUpdateState(gameName);
    },
  );

  let unlistenLifecycle: (() => void) | null = null;
  let unlistenComponentDl: (() => void) | null = null;
  let unlistenAnticheat: (() => void) | null = null;

  onMounted(async () => {
    document.addEventListener('click', options.closeMenu);
    window.addEventListener(
      'ssmt4-onboarding-action',
      options.onOnboardingActionEvent,
    );
    void options.checkGameExe();

    unlistenLifecycle = await listenAppEvent('game-lifecycle', (event) => {
      runtimeState.value = applyGameLifecycleEvent(runtimeState.value, event.payload);
      syncRefsFromState();
    });

    unlistenComponentDl = await listenAppEvent(
      'component-download-progress',
      (event) => {
        runtimeState.value = applyComponentDownloadProgressEvent(
          runtimeState.value,
          event.payload,
        );
        syncRefsFromState();
      },
    );

    unlistenAnticheat = await listenAppEvent('game-anticheat-warning', (event) => {
      void options.showAnticheatWarning(event.payload.message);
    });
  });

  onUnmounted(() => {
    document.removeEventListener('click', options.closeMenu);
    window.removeEventListener(
      'ssmt4-onboarding-action',
      options.onOnboardingActionEvent,
    );
    unlistenLifecycle?.();
    unlistenComponentDl?.();
    unlistenAnticheat?.();
  });

  return {
    isLaunching,
    isGameRunning,
    runningGameName,
    componentDlProgress,
    markLaunchRequested,
    resetLaunchState,
  };
}
