import { reactive } from 'vue';

export type TaskNotificationStatus = 'running' | 'success' | 'error' | 'info' | 'warning';

export interface TaskNotificationItem {
  id: string;
  title: string;
  message: string;
  status: TaskNotificationStatus;
  progress: number | null;
  createdAt: number;
}

export const taskNotifications = reactive<TaskNotificationItem[]>([]);

const dismissTimers = new Map<string, ReturnType<typeof setTimeout>>();

function clearDismissTimer(id: string) {
  const timer = dismissTimers.get(id);
  if (timer) {
    clearTimeout(timer);
    dismissTimers.delete(id);
  }
}

function ensureTask(id: string): TaskNotificationItem {
  let existing = taskNotifications.find((item) => item.id === id);
  if (!existing) {
    existing = {
      id,
      title: '',
      message: '',
      status: 'info',
      progress: null,
      createdAt: Date.now(),
    };
    taskNotifications.unshift(existing);
  }
  return existing;
}

function scheduleDismiss(id: string, delayMs = 2600) {
  clearDismissTimer(id);
  dismissTimers.set(
    id,
    setTimeout(() => {
      dismissTask(id);
    }, delayMs),
  );
}

export function upsertTaskNotification(
  id: string,
  patch: Partial<Omit<TaskNotificationItem, 'id' | 'createdAt'>>,
) {
  const task = ensureTask(id);
  clearDismissTimer(id);

  if (patch.title !== undefined) task.title = patch.title;
  if (patch.message !== undefined) task.message = patch.message;
  if (patch.status !== undefined) task.status = patch.status;
  if (patch.progress !== undefined) {
    task.progress = patch.progress == null ? null : Math.max(0, Math.min(100, patch.progress));
  }
}

export function startTaskNotification(id: string, title: string, message: string) {
  upsertTaskNotification(id, {
    title,
    message,
    status: 'running',
    progress: null,
  });
}

export function updateTaskNotification(
  id: string,
  patch: Partial<Omit<TaskNotificationItem, 'id' | 'createdAt'>>,
) {
  upsertTaskNotification(id, patch);
}

export function finishTaskNotification(id: string, title: string, message: string, autoCloseMs = 2400) {
  upsertTaskNotification(id, {
    title,
    message,
    status: 'success',
    progress: 100,
  });
  scheduleDismiss(id, autoCloseMs);
}

export function failTaskNotification(id: string, title: string, message: string) {
  upsertTaskNotification(id, {
    title,
    message,
    status: 'error',
    progress: null,
  });
}

export function infoTaskNotification(id: string, title: string, message: string, autoCloseMs = 2400) {
  upsertTaskNotification(id, {
    title,
    message,
    status: 'info',
    progress: null,
  });
  scheduleDismiss(id, autoCloseMs);
}

export function dismissTask(id: string) {
  clearDismissTimer(id);
  const index = taskNotifications.findIndex((item) => item.id === id);
  if (index >= 0) {
    taskNotifications.splice(index, 1);
  }
}
