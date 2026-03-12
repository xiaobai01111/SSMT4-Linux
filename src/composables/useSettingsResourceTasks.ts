import { onMounted, onUnmounted, reactive } from 'vue';
import { listenAppEvent } from '../api';
import {
  failTaskNotification,
  finishTaskNotification,
  startTaskNotification,
  updateTaskNotification,
} from '../taskNotifications';

type Translator = (key: string, fallback: string) => string;

type TaskMessage<T> = string | ((value: T) => string);

interface RunTaskOptions<T> {
  taskId: string;
  title: string;
  pendingMessage: string;
  componentKey?: string;
  run: () => Promise<T>;
  successMessage: TaskMessage<T>;
  errorMessage: TaskMessage<unknown>;
  refresh?: () => Promise<void>;
}

export function useSettingsResourceTasks(tr: Translator) {
  const activeComponentTasks = reactive<Record<string, string>>({});

  const rememberComponentTask = (component: string, taskId: string) => {
    activeComponentTasks[component] = taskId;
  };

  const forgetComponentTask = (component: string) => {
    delete activeComponentTasks[component];
  };

  const taskDownloadingMessage = () =>
    tr('settings.messages.taskDownloading', '正在下载，请稍候...');

  const taskExtractingMessage = () =>
    tr('settings.messages.taskExtracting', '正在解压，请稍候...');

  const taskDeletingMessage = () =>
    tr('settings.messages.taskDeleting', '正在删除，请稍候...');

  let unlistenComponentProgress: null | (() => void) = null;

  onMounted(async () => {
    unlistenComponentProgress = await listenAppEvent(
      'component-download-progress',
      (event) => {
        const payload = event.payload;
        const componentKey = payload.componentId.trim();
        const taskId = activeComponentTasks[componentKey];
        if (!taskId) return;

        const phase = payload.phase;
        const downloaded = payload.downloaded;
        const total = payload.total;

        if (phase === 'extracting') {
          updateTaskNotification(taskId, {
            message: taskExtractingMessage(),
            progress: null,
          });
          return;
        }

        if (phase === 'done') {
          updateTaskNotification(taskId, { progress: 100 });
          return;
        }

        updateTaskNotification(taskId, {
          message: taskDownloadingMessage(),
          progress: total > 0 ? (downloaded / total) * 100 : null,
        });
      },
    );
  });

  onUnmounted(() => {
    unlistenComponentProgress?.();
    unlistenComponentProgress = null;
  });

  const resolveTaskMessage = <T>(message: TaskMessage<T>, value: T) =>
    typeof message === 'function' ? message(value) : message;

  const runTaskAction = async <T>({
    taskId,
    title,
    pendingMessage,
    componentKey,
    run,
    successMessage,
    errorMessage,
    refresh,
  }: RunTaskOptions<T>) => {
    if (componentKey) {
      rememberComponentTask(componentKey, taskId);
    }
    try {
      startTaskNotification(taskId, title, pendingMessage);
      const result = await run();
      finishTaskNotification(
        taskId,
        title,
        resolveTaskMessage(successMessage, result),
      );
      if (refresh) {
        await refresh();
      }
      return result;
    } catch (error) {
      failTaskNotification(
        taskId,
        title,
        resolveTaskMessage(errorMessage, error),
      );
      throw error;
    } finally {
      if (componentKey) {
        forgetComponentTask(componentKey);
      }
    }
  };

  const runDownloadTask = async <T>(
    options: Omit<RunTaskOptions<T>, 'pendingMessage'> & {
      pendingMessage?: string;
    },
  ) =>
    runTaskAction({
      ...options,
      pendingMessage: options.pendingMessage ?? taskDownloadingMessage(),
    });

  const runDeleteTask = async <T>(
    options: Omit<RunTaskOptions<T>, 'title' | 'pendingMessage'> & {
      title?: string;
    },
  ) =>
    runTaskAction({
      ...options,
      title: options.title ?? tr('settings.actions.delete', '删除'),
      pendingMessage: taskDeletingMessage(),
    });

  return {
    runDownloadTask,
    runDeleteTask,
  };
}
