import { watch, type Ref } from 'vue';
import {
  buildCompletedTaskMarker,
  shouldHandleCompletedTaskMarker,
  type CompletedTaskLike,
} from '../utils/downloadTaskUi';

interface SyncableServerOption {
  launcherApi: string;
}

interface GameDownloadModalLifecycleOptions<
  TTask extends CompletedTaskLike | null | undefined,
  TServer extends SyncableServerOption,
> {
  modelValue: () => boolean;
  gameName: () => string;
  launcherApi: Ref<string>;
  availableServers: Ref<TServer[]>;
  selectedServer: Ref<TServer | null>;
  currentTask: () => TTask;
  onOpen: () => Promise<void> | void;
  onGameChange: () => Promise<void> | void;
  onCompletedTask: () => Promise<void> | void;
}

export function useGameDownloadModalLifecycle<
  TTask extends CompletedTaskLike | null | undefined,
  TServer extends SyncableServerOption,
>(options: GameDownloadModalLifecycleOptions<TTask, TServer>) {
  watch(
    () => options.modelValue(),
    (value) => {
      if (!value) return;
      void options.onOpen();
    },
    { immediate: true },
  );

  watch(
    () => options.gameName(),
    () => {
      if (!options.modelValue()) return;
      void options.onGameChange();
    },
  );

  watch(options.launcherApi, (api) => {
    const normalized = api.trim();
    if (!normalized || options.availableServers.value.length === 0) return;
    const matched =
      options.availableServers.value.find(
        (server) => server.launcherApi.trim() === normalized,
      ) || null;
    if (matched) {
      options.selectedServer.value = matched;
    }
  });

  let lastHandledDoneTaskMarker = '';

  watch(
    () => buildCompletedTaskMarker(options.currentTask()),
    (marker) => {
      if (
        !shouldHandleCompletedTaskMarker(
          marker,
          lastHandledDoneTaskMarker,
          options.modelValue(),
        )
      ) {
        return;
      }
      lastHandledDoneTaskMarker = marker;
      void options.onCompletedTask();
    },
  );
}
