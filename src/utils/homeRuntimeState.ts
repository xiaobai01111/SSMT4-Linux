import type {
  ComponentDownloadProgressEvent,
  GameLifecycleEvent,
} from '../types/events';

export interface HomeRuntimeSnapshot {
  isLaunching: boolean;
  isGameRunning: boolean;
  runningGameName: string;
  componentDlProgress: ComponentDownloadProgressEvent | null;
}

export const createHomeRuntimeSnapshot = (): HomeRuntimeSnapshot => ({
  isLaunching: false,
  isGameRunning: false,
  runningGameName: '',
  componentDlProgress: null,
});

export const applyGameLifecycleEvent = (
  state: HomeRuntimeSnapshot,
  event: GameLifecycleEvent,
): HomeRuntimeSnapshot => {
  switch (event.event) {
    case 'started':
      return {
        ...state,
        isLaunching: false,
        isGameRunning: true,
        runningGameName: event.game,
      };
    case 'exited':
      return {
        ...state,
        isLaunching: false,
        isGameRunning: false,
        runningGameName: '',
      };
    default:
      return state;
  }
};

export const applyComponentDownloadProgressEvent = (
  state: HomeRuntimeSnapshot,
  event: ComponentDownloadProgressEvent,
): HomeRuntimeSnapshot => ({
  ...state,
  componentDlProgress: event.phase === 'done' ? null : event,
});
