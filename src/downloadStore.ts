/**
 * Download Task Store
 *
 * 前端只负责维护任务注册表和 UI 状态。
 * 后端进度事件现在显式携带 task id，前端按任务维度更新状态，
 * 不再把所有进度硬塞给“当前活动任务”。
 */

import { computed, reactive } from 'vue';
import {
  downloadGame as apiDownloadGame,
  downloadLauncherInstaller as apiDownloadLauncherInstaller,
  updateGame as apiUpdateGame,
  updateLauncherInstaller as apiUpdateLauncherInstaller,
  verifyGameFiles as apiVerifyGameFiles,
  repairGameFiles as apiRepairGameFiles,
  cancelDownload as apiCancelDownload,
  resolveDownloadedGameExecutable,
  listenAppEvent,
  showMessage,
  saveGameConfig,
  loadGameConfig,
  type DownloadProgress,
} from './api';
import { loadGames, refreshGameUpdateState } from './store';
import type { GameConfig } from './types/ipc';
import {
  buildFailedFilesPreviewSection,
  buildTaskKey,
  buildVerifyFailureKey,
  isCancellationLikeError,
  normalizeErrorText,
  phaseFromProgressOperation,
  phaseFromTaskOperation,
  taskFailureMessage,
  taskFailureTitle,
  type DlPhase,
  type DownloadOperation,
} from './utils/downloadTaskState';

export interface DownloadTaskModel {
  operation: DownloadOperation;
  gameName: string;
  displayName: string;
  launcherApi: string;
  gameFolder: string;
  languages?: string[];
  bizPrefix?: string;
  gamePreset?: string;
  repairFiles?: string[];
}

export interface DownloadTaskState {
  key: string;
  task: DownloadTaskModel;
  gameName: string;
  displayName: string;
  gameFolder: string;
  launcherApi: string;
  phase: DlPhase;
  progress: DownloadProgress | null;
  error: string;
  updatedAt: number;
}

interface DownloadStoreState {
  bootstrapped: boolean;
  listenersReady: boolean;
  tasks: Record<string, DownloadTaskState>;
  lastVerifyFailures: Record<string, string[]>;
}

export const downloadStoreState = reactive<DownloadStoreState>({
  bootstrapped: false,
  listenersReady: false,
  tasks: {},
  lastVerifyFailures: {},
});

export const activeDownloadTask = computed<DownloadTaskState | null>(() => {
  return Object.values(downloadStoreState.tasks)
    .filter((entry) => entry.phase === 'downloading' || entry.phase === 'verifying')
    .sort((a, b) => b.updatedAt - a.updatedAt)[0] ?? null;
});

let pauseRequestedTaskKey: string | null = null;
let bootstrapPromise: Promise<void> | null = null;

function nowTs(): number {
  return Date.now();
}

function cloneTask(task: DownloadTaskModel): DownloadTaskModel {
  return {
    ...task,
    languages: task.languages ? [...task.languages] : undefined,
    repairFiles: task.repairFiles ? [...task.repairFiles] : undefined,
  };
}

function taskKey(task: DownloadTaskModel): string {
  return buildTaskKey(task);
}

function verifyFailureKey(
  gameName: string,
  gameFolder: string,
  launcherApi: string,
  bizPrefix?: string,
): string {
  return buildVerifyFailureKey(gameName, gameFolder, launcherApi, bizPrefix);
}

function ensureTaskEntry(task: DownloadTaskModel): DownloadTaskState {
  const normalizedTask = cloneTask(task);
  const key = taskKey(normalizedTask);
  const existing = downloadStoreState.tasks[key];
  if (existing) {
    existing.task = normalizedTask;
    existing.gameName = normalizedTask.gameName;
    existing.displayName = normalizedTask.displayName;
    existing.gameFolder = normalizedTask.gameFolder;
    existing.launcherApi = normalizedTask.launcherApi;
    existing.updatedAt = nowTs();
    return existing;
  }

  const entry: DownloadTaskState = {
    key,
    task: normalizedTask,
    gameName: normalizedTask.gameName,
    displayName: normalizedTask.displayName,
    gameFolder: normalizedTask.gameFolder,
    launcherApi: normalizedTask.launcherApi,
    phase: 'idle',
    progress: null,
    error: '',
    updatedAt: nowTs(),
  };
  downloadStoreState.tasks[key] = entry;
  return entry;
}

function getTaskEntry(key: string | null | undefined): DownloadTaskState | null {
  if (!key) return null;
  return downloadStoreState.tasks[key] ?? null;
}

function updateTaskState(
  key: string,
  patch: Partial<Pick<DownloadTaskState, 'phase' | 'progress' | 'error'>>,
): DownloadTaskState | null {
  const entry = getTaskEntry(key);
  if (!entry) return null;
  Object.assign(entry, patch, { updatedAt: nowTs() });
  return entry;
}

function markTaskRunning(task: DownloadTaskModel): DownloadTaskState {
  const entry = ensureTaskEntry(task);

  updateTaskState(entry.key, {
    phase: phaseFromTaskOperation(task.operation),
    progress: null,
    error: '',
  });
  return entry;
}

function markTaskDone(key: string) {
  updateTaskState(key, {
    phase: 'done',
    error: '',
  });
}

function markTaskError(key: string, error: string) {
  updateTaskState(key, {
    phase: 'error',
    error,
  });
}

function markTaskPaused(key: string) {
  updateTaskState(key, {
    phase: 'paused',
    error: '',
  });
}

function forgetTask(key: string) {
  delete downloadStoreState.tasks[key];
}

function rememberVerifyFailures(task: DownloadTaskModel, failed: string[]) {
  downloadStoreState.lastVerifyFailures[
    verifyFailureKey(task.gameName, task.gameFolder, task.launcherApi, task.bizPrefix)
  ] = [...failed];
}

async function attachProgressListeners() {
  const onProgress = async (payload: DownloadProgress) => {
    const entry = updateTaskState(payload.task_id, {
      phase: phaseFromProgressOperation(payload.operation),
      progress: payload,
      error: '',
    });
    if (!entry) return;
  };

  await listenAppEvent('game-download-progress', ({ payload }) => {
    if (payload) void onProgress(payload);
  });
}

export async function bootstrapDownloadStore() {
  if (downloadStoreState.listenersReady) {
    downloadStoreState.bootstrapped = true;
    return;
  }
  if (bootstrapPromise) return bootstrapPromise;

  bootstrapPromise = (async () => {
    await attachProgressListeners();
    downloadStoreState.listenersReady = true;
    downloadStoreState.bootstrapped = true;
  })().finally(() => {
    bootstrapPromise = null;
  });

  return bootstrapPromise;
}

function findTaskForGame(gameName: string, phase?: DlPhase): DownloadTaskState | null {
  const tasks = Object.values(downloadStoreState.tasks)
    .filter((entry) => entry.gameName === gameName && (!phase || entry.phase === phase))
    .sort((a, b) => b.updatedAt - a.updatedAt);
  return tasks[0] ?? null;
}

function findBlockingTaskForGame(gameName: string): DownloadTaskState | null {
  return (
    findTaskForGame(gameName, 'downloading')
    || findTaskForGame(gameName, 'verifying')
    || findTaskForGame(gameName, 'paused')
    || null
  );
}

export function getTaskForGame(gameName: string): DownloadTaskState | null {
  return (
    findTaskForGame(gameName, 'downloading')
    || findTaskForGame(gameName, 'verifying')
    || findTaskForGame(gameName, 'paused')
    || findTaskForGame(gameName, 'error')
    || findTaskForGame(gameName, 'done')
    || null
  );
}

async function showTaskFailure(task: DownloadTaskModel, message: string) {
  await showMessage(taskFailureMessage(task.operation, message), {
    title: taskFailureTitle(task.operation),
    kind: 'error',
  });
}

async function bestEffortSaveGameConfig(
  gameName: string,
  mutate: (config: GameConfig) => void | Promise<void>,
) {
  try {
    const config = await loadGameConfig(gameName);
    await mutate(config);
    await saveGameConfig(gameName, config);
  } catch {
    /* best-effort */
  }
}

async function ensureGameConfigAfterContentDownload(task: DownloadTaskModel) {
  await bestEffortSaveGameConfig(task.gameName, async (config) => {
    config.other = config.other || {};
    config.other.launcherApi = task.launcherApi;
    config.other.gameFolder = task.gameFolder;
    if (config.other.gamePath) return;

    const detectedExe = await resolveDownloadedGameExecutable(
      task.gameName,
      task.gameFolder,
      task.launcherApi,
    ).catch(() => null);

    if (detectedExe) {
      config.other.gamePath = detectedExe;
      return;
    }

    const exeName = resolveGameExeName(task.gameName);
    if (exeName) {
      config.other.gamePath = `${task.gameFolder}/${exeName}`;
    }
  });
}

async function ensureGameConfigAfterInstallerDownload(
  task: DownloadTaskModel,
  result: { installerPath: string; version: string },
) {
  await bestEffortSaveGameConfig(task.gameName, (config) => {
    config.other = config.other || {};
    config.other.launcherApi = task.launcherApi;
    config.other.gameFolder = task.gameFolder;
    config.other.gamePath = result.installerPath;
    config.other.launcherInstallerVersion = result.version;
    config.other.launcherInstallerPath = result.installerPath;
  });
}

async function refreshGameStateAfterSuccessfulTask(task: DownloadTaskModel) {
  await Promise.allSettled([
    loadGames(),
    refreshGameUpdateState(task.gameName),
  ]);
}

async function runTask(task: DownloadTaskModel) {
  ensureTaskEntry(task);
  const key = taskKey(task);

  try {
    switch (task.operation) {
      case 'download_game':
      case 'update_game': {
        const langs = task.languages && task.languages.length > 0 ? task.languages : undefined;
        const biz = task.bizPrefix || undefined;

        if (task.operation === 'update_game') {
          await apiUpdateGame(task.launcherApi, task.gameFolder, key, langs, biz);
        } else {
          await apiDownloadGame(task.launcherApi, task.gameFolder, key, langs, biz);
        }

        await ensureGameConfigAfterContentDownload(task);

        updateTaskState(key, {
          phase: 'verifying',
          progress: null,
        });

        const verifyResult = await apiVerifyGameFiles(task.launcherApi, task.gameFolder, key, biz)
          .catch(async (error) => {
            const msg = normalizeErrorText(error);
            markTaskError(key, `自动校验失败: ${msg}`);
            await showMessage(
              `下载已完成，但自动校验失败：${msg}\n请稍后手动执行“校验文件”。`,
              { title: '校验失败', kind: 'warning' },
            ).catch(() => {});
            return null;
          });

        if (!verifyResult) {
          break;
        }

        rememberVerifyFailures(task, verifyResult.failed);

        if (verifyResult.failed.length > 0) {
          await showMessage(
            `校验完成，但有 ${verifyResult.failed.length} 个文件异常。`,
            { title: '校验结果', kind: 'warning' },
          );
        } else {
          await showMessage(
            `下载并校验完成！共 ${verifyResult.total_files} 个文件全部正常。`,
            { title: '成功', kind: 'info' },
          );
        }

        markTaskDone(key);
        await refreshGameStateAfterSuccessfulTask(task);
        break;
      }

      case 'verify_game': {
        const biz = task.bizPrefix || undefined;
        const result = await apiVerifyGameFiles(task.launcherApi, task.gameFolder, key, biz);

        rememberVerifyFailures(task, result.failed);

        if (result.failed.length > 0) {
          updateTaskState(key, {
            error: `校验完成，但有 ${result.failed.length} 个文件仍然异常`,
          });
          const details = buildFailedFilesPreviewSection(result.failed, '异常文件');
          await showMessage(
            `校验完成：共 ${result.total_files} 个文件，${result.verified_ok} 个正常，${result.failed.length} 个异常。${details}\n\n可点击“修复异常文件”仅重下异常条目。`,
            { title: '校验结果', kind: 'warning' },
          );
        } else if (result.redownloaded > 0) {
          await showMessage(
            `校验完成！共 ${result.total_files} 个文件，${result.verified_ok} 个正常，${result.redownloaded} 个已重新下载。`,
            { title: '校验结果', kind: 'info' },
          );
        } else {
          await showMessage(
            `校验完成！共 ${result.total_files} 个文件，${result.verified_ok} 个正常。`,
            { title: '校验结果', kind: 'info' },
          );
        }

        markTaskDone(key);
        await refreshGameStateAfterSuccessfulTask(task);
        break;
      }

      case 'repair_game': {
        const biz = task.bizPrefix || undefined;
        const files = task.repairFiles ? [...task.repairFiles] : [];
        if (files.length === 0) {
          throw new Error('没有可修复的异常文件列表');
        }

        const result = await apiRepairGameFiles(task.launcherApi, task.gameFolder, key, files, biz);
        rememberVerifyFailures(task, result.failed);

        if (result.failed.length > 0) {
          updateTaskState(key, {
            error: `修复完成，但仍有 ${result.failed.length} 个文件异常`,
          });
          const details = buildFailedFilesPreviewSection(result.failed, '仍异常文件');
          await showMessage(
            `修复完成：请求修复 ${result.requested_files} 个文件，成功 ${result.repaired_ok} 个，失败 ${result.failed.length} 个。${details}`,
            { title: '修复结果', kind: 'warning' },
          );
        } else {
          await showMessage(
            `修复完成！请求修复 ${result.requested_files} 个文件，成功 ${result.repaired_ok} 个。`,
            { title: '修复结果', kind: 'info' },
          );
        }

        markTaskDone(key);
        await refreshGameStateAfterSuccessfulTask(task);
        break;
      }

      case 'download_launcher_installer':
      case 'update_launcher_installer': {
        const preset = task.gamePreset || task.gameName;
        const result = task.operation === 'update_launcher_installer'
          ? await apiUpdateLauncherInstaller(task.launcherApi, task.gameFolder, preset, key)
          : await apiDownloadLauncherInstaller(task.launcherApi, task.gameFolder, preset, key);

        await ensureGameConfigAfterInstallerDownload(task, result);

        await showMessage(
          `官方启动器安装器下载完成：${result.version}\n已自动将 gamePath 指向安装器，可在游戏设置中改为实际游戏主程序。`,
          { title: '下载完成', kind: 'info' },
        );

        markTaskDone(key);
        await refreshGameStateAfterSuccessfulTask(task);
        break;
      }
    }
  } catch (error: unknown) {
    const cancelled = isCancellationLikeError(error);
    const pauseRequested = pauseRequestedTaskKey === key;
    pauseRequestedTaskKey = null;

    if (cancelled && pauseRequested) {
      markTaskPaused(key);
      return;
    }

    if (cancelled) {
      forgetTask(key);
      return;
    }

    const message = error instanceof Error ? (error.message || String(error)) : String(error ?? '');
    markTaskError(key, message);

    await showTaskFailure(task, message).catch(() => {});
  } finally {
    if (pauseRequestedTaskKey === key) {
      pauseRequestedTaskKey = null;
    }
  }
}

function startTask(task: DownloadTaskModel): DownloadTaskState {
  const entry = ensureTaskEntry(task);
  const blockingTask = findBlockingTaskForGame(task.gameName);
  if (blockingTask && blockingTask.key !== entry.key) {
    throw new Error(`download task already running for game: ${blockingTask.key}`);
  }
  if (blockingTask?.key === entry.key && blockingTask.phase !== 'paused') {
    throw new Error(`download task already running: ${blockingTask.key}`);
  }
  void bootstrapDownloadStore().catch((error) => {
    console.warn('[dlStore] bootstrap failed:', error);
  });
  pauseRequestedTaskKey = null;
  markTaskRunning(task);
  runTask(cloneTask(task)).catch((error) => {
    console.error('[dlStore] uncaught task:', error);
  });
  return entry;
}

export function isActiveFor(gameName: string): boolean {
  return findTaskForGame(gameName, 'downloading') != null || findTaskForGame(gameName, 'verifying') != null;
}

export function isPausedFor(gameName: string): boolean {
  return findTaskForGame(gameName, 'paused') != null;
}

export interface StartDlOpts {
  gameName: string;
  displayName: string;
  launcherApi: string;
  gameFolder: string;
  languages?: string[];
  bizPrefix?: string;
  isUpdate: boolean;
}

export interface VerifyOpts {
  gameName: string;
  displayName: string;
  launcherApi: string;
  gameFolder: string;
  bizPrefix?: string;
}

export interface RepairOpts {
  gameName: string;
  displayName: string;
  launcherApi: string;
  gameFolder: string;
  bizPrefix?: string;
  files: string[];
}

export interface StartLauncherInstallerDlOpts {
  gameName: string;
  gamePreset: string;
  displayName: string;
  launcherApi: string;
  gameFolder: string;
  isUpdate: boolean;
}

function taskFromStartDl(opts: StartDlOpts): DownloadTaskModel {
  return {
    operation: opts.isUpdate ? 'update_game' : 'download_game',
    gameName: opts.gameName,
    displayName: opts.displayName,
    launcherApi: opts.launcherApi,
    gameFolder: opts.gameFolder,
    languages: opts.languages ? [...opts.languages] : undefined,
    bizPrefix: opts.bizPrefix || undefined,
  };
}

function taskFromVerify(opts: VerifyOpts): DownloadTaskModel {
  return {
    operation: 'verify_game',
    gameName: opts.gameName,
    displayName: opts.displayName,
    launcherApi: opts.launcherApi,
    gameFolder: opts.gameFolder,
    bizPrefix: opts.bizPrefix || undefined,
  };
}

function taskFromRepair(opts: RepairOpts): DownloadTaskModel {
  return {
    operation: 'repair_game',
    gameName: opts.gameName,
    displayName: opts.displayName,
    launcherApi: opts.launcherApi,
    gameFolder: opts.gameFolder,
    bizPrefix: opts.bizPrefix || undefined,
    repairFiles: [...opts.files],
  };
}

function taskFromInstaller(opts: StartLauncherInstallerDlOpts): DownloadTaskModel {
  return {
    operation: opts.isUpdate ? 'update_launcher_installer' : 'download_launcher_installer',
    gameName: opts.gameName,
    displayName: opts.displayName,
    launcherApi: opts.launcherApi,
    gameFolder: opts.gameFolder,
    gamePreset: opts.gamePreset,
  };
}

export async function fireDownload(opts: StartDlOpts): Promise<DownloadTaskState> {
  return startTask(taskFromStartDl(opts));
}

export async function fireVerify(opts: VerifyOpts): Promise<DownloadTaskState> {
  return startTask(taskFromVerify(opts));
}

export async function fireRepair(opts: RepairOpts): Promise<DownloadTaskState> {
  return startTask(taskFromRepair(opts));
}

export async function fireLauncherInstallerDownload(
  opts: StartLauncherInstallerDlOpts,
): Promise<DownloadTaskState> {
  return startTask(taskFromInstaller(opts));
}

export function getRepairableFailuresFor(
  gameName: string,
  gameFolder: string,
  launcherApi: string,
  bizPrefix?: string,
): string[] {
  const key = verifyFailureKey(gameName, gameFolder, launcherApi, bizPrefix);
  return [...(downloadStoreState.lastVerifyFailures[key] ?? [])];
}

export async function pauseActive(gameName?: string) {
  const task = gameName
    ? findTaskForGame(gameName, 'downloading') || findTaskForGame(gameName, 'verifying')
    : activeDownloadTask.value;
  if (!task) return;
  if (gameName && task.gameName !== gameName) return;

  pauseRequestedTaskKey = task.key;
  try {
    await apiCancelDownload(task.key);
  } catch (error) {
    pauseRequestedTaskKey = null;
    console.error('[dlStore] pause failed:', error);
  }
}

export async function resumePaused(gameName?: string): Promise<DownloadTaskState> {
  const task = gameName
    ? findTaskForGame(gameName, 'paused')
    : Object.values(downloadStoreState.tasks)
      .filter((entry) => entry.phase === 'paused')
      .sort((a, b) => b.updatedAt - a.updatedAt)[0] ?? null;
  if (!task) {
    throw new Error('no paused download task');
  }
  return startTask(task.task);
}

export async function cancelActive(gameName?: string) {
  const task = gameName
    ? findTaskForGame(gameName, 'downloading') || findTaskForGame(gameName, 'verifying')
    : activeDownloadTask.value;
  if (!task) {
    if (gameName) {
      const pausedTask = findTaskForGame(gameName, 'paused');
      if (pausedTask) {
        forgetTask(pausedTask.key);
      }
      return;
    }

    Object.values(downloadStoreState.tasks)
      .filter((entry) => entry.phase === 'paused')
      .forEach((entry) => forgetTask(entry.key));
    return;
  }

  if (gameName && task.gameName !== gameName) return;

  pauseRequestedTaskKey = null;
  try {
    await apiCancelDownload(task.key);
  } catch (error) {
    console.error('[dlStore] cancel failed:', error);
  }
}

function resolveGameExeName(gameName: string): string | null {
  switch (gameName) {
    case 'HonkaiStarRail':
      return 'StarRail.exe';
    case 'ZenlessZoneZero':
      return 'ZenlessZoneZero.exe';
    default:
      return null;
  }
}
