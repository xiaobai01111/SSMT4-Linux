/**
 * Global Download Store
 *
 * Manages download/verify lifecycle independently of any component.
 * Progress events are listened to globally so closing the modal
 * does not interrupt an active download.
 */

import { reactive } from 'vue';
import {
  downloadGame as apiDownloadGame,
  downloadLauncherInstaller as apiDownloadLauncherInstaller,
  updateGame as apiUpdateGame,
  updateLauncherInstaller as apiUpdateLauncherInstaller,
  verifyGameFiles as apiVerifyGameFiles,
  repairGameFiles as apiRepairGameFiles,
  cancelDownload as apiCancelDownload,
  resolveDownloadedGameExecutable,
  listenEvent,
  showMessage,
  saveGameConfig,
  loadGameConfig,
  type DownloadProgress,
} from './api';

export type DlPhase = 'idle' | 'downloading' | 'verifying' | 'paused' | 'done' | 'error';

export type DownloadOperation =
  | 'download_game'
  | 'update_game'
  | 'verify_game'
  | 'repair_game'
  | 'download_launcher_installer'
  | 'update_launcher_installer';

// Unified task model to avoid duplicated option models across download flows.
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

export interface DlState {
  active: boolean;
  gameName: string;
  gameFolder: string;
  displayName: string;
  phase: DlPhase;
  progress: DownloadProgress | null;
  error: string;
  task: DownloadTaskModel | null;
  pausedTask: DownloadTaskModel | null;
  lastVerifyGameName: string;
  lastVerifyGameFolder: string;
  lastVerifyLauncherApi: string;
  lastVerifyBizPrefix: string;
  lastVerifyFailed: string[];
}

export const dlState = reactive<DlState>({
  active: false,
  gameName: '',
  gameFolder: '',
  displayName: '',
  phase: 'idle',
  progress: null,
  error: '',
  task: null,
  pausedTask: null,
  lastVerifyGameName: '',
  lastVerifyGameFolder: '',
  lastVerifyLauncherApi: '',
  lastVerifyBizPrefix: '',
  lastVerifyFailed: [],
});

// ---- Global event listeners (registered once) ----

let _listenersReady = false;
let _pauseRequestedTaskKey: string | null = null;

export async function initDlListeners() {
  if (_listenersReady) return;
  _listenersReady = true;

  const onProgress = (event: any) => {
    if (dlState.active) {
      dlState.progress = event.payload;
    }
  };

  await listenEvent('game-download-progress', onProgress);
  await listenEvent('game-update-progress', onProgress);
  await listenEvent('game-install-progress', onProgress);
  await listenEvent('game-verify-progress', onProgress);
}

// ---- Helpers ----

function cloneTask(task: DownloadTaskModel): DownloadTaskModel {
  return {
    ...task,
    languages: task.languages ? [...task.languages] : undefined,
    repairFiles: task.repairFiles ? [...task.repairFiles] : undefined,
  };
}

function normalizeErrorText(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message || String(error);
  if (error && typeof error === 'object') {
    const maybeMessage = (error as { message?: unknown }).message;
    if (typeof maybeMessage === 'string') return maybeMessage;
  }
  return String(error ?? '');
}

function isCancellationLikeError(error: unknown): boolean {
  const raw = normalizeErrorText(error);
  if (!raw) return false;
  const lower = raw.toLowerCase();

  const englishMarkers = ['cancel', 'cancelled', 'canceled', 'abort', 'aborted'];
  if (englishMarkers.some((marker) => lower.includes(marker))) return true;

  const chineseMarkers = ['取消', '已取消', '中止', '终止'];
  return chineseMarkers.some((marker) => raw.includes(marker));
}

function taskKey(task: DownloadTaskModel): string {
  const langs = task.languages ? task.languages.join(',') : '';
  const repairFiles = task.repairFiles ? task.repairFiles.join(',') : '';
  return [
    task.operation,
    task.gameName,
    task.gameFolder,
    task.launcherApi,
    task.bizPrefix || '',
    task.gamePreset || '',
    langs,
    repairFiles,
  ].join('::');
}

function resetStateToIdle() {
  dlState.active = false;
  dlState.phase = 'idle';
  dlState.error = '';
  dlState.progress = null;
  dlState.task = null;
}

function markTaskRunning(task: DownloadTaskModel) {
  const phase: DlPhase =
    task.operation === 'verify_game' || task.operation === 'repair_game'
      ? 'verifying'
      : 'downloading';
  const normalized = cloneTask(task);
  dlState.active = true;
  dlState.gameName = normalized.gameName;
  dlState.gameFolder = normalized.gameFolder;
  dlState.displayName = normalized.displayName;
  dlState.phase = phase;
  dlState.progress = null;
  dlState.error = '';
  dlState.task = normalized;
}

async function runTask(task: DownloadTaskModel) {
  const key = taskKey(task);
  try {
    switch (task.operation) {
      case 'download_game':
      case 'update_game': {
        const langs = task.languages && task.languages.length > 0 ? task.languages : undefined;
        const biz = task.bizPrefix || undefined;

        if (task.operation === 'update_game') {
          await apiUpdateGame(task.launcherApi, task.gameFolder, langs, biz);
        } else {
          await apiDownloadGame(task.launcherApi, task.gameFolder, langs, biz);
        }

        // Persist config on success
        try {
          const config = await loadGameConfig(task.gameName);
          config.other = config.other || {};
          config.other.launcherApi = task.launcherApi;
          config.other.gameFolder = task.gameFolder;
          // 自动设置游戏可执行文件路径（首次安装时）
          if (!config.other.gamePath) {
            const detectedExe = await resolveDownloadedGameExecutable(
              task.gameName,
              task.gameFolder,
              task.launcherApi,
            ).catch(() => null);
            if (detectedExe) {
              config.other.gamePath = detectedExe;
            } else {
              const exeName = resolveGameExeName(task.gameName);
              if (exeName) {
                config.other.gamePath = task.gameFolder + '/' + exeName;
              }
            }
          }
          await saveGameConfig(task.gameName, config);
        } catch {
          /* best-effort */
        }

        // Auto-verify
        dlState.phase = 'verifying';
        dlState.progress = null;

        const verifyResult = await apiVerifyGameFiles(task.launcherApi, task.gameFolder, biz)
          .catch(async (e) => {
            const msg = normalizeErrorText(e);
            console.warn('[dlStore] verify error:', e);
            dlState.phase = 'error';
            dlState.error = `自动校验失败: ${msg}`;
            dlState.active = false;
            dlState.task = null;
            dlState.pausedTask = null;
            await showMessage(
              `下载已完成，但自动校验失败：${msg}\n请稍后手动执行“校验文件”。`,
              { title: '校验失败', kind: 'warning' },
            ).catch(() => {});
            return null;
          });

        if (!verifyResult) {
          break;
        }
        dlState.lastVerifyGameName = task.gameName;
        dlState.lastVerifyGameFolder = task.gameFolder;
        dlState.lastVerifyLauncherApi = task.launcherApi;
        dlState.lastVerifyBizPrefix = task.bizPrefix || '';
        dlState.lastVerifyFailed = [...verifyResult.failed];

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

        dlState.phase = 'done';
        dlState.active = false;
        dlState.task = null;
        dlState.pausedTask = null;
        break;
      }

      case 'verify_game': {
        const biz = task.bizPrefix || undefined;
        const result = await apiVerifyGameFiles(task.launcherApi, task.gameFolder, biz);

        dlState.phase = 'done';
        dlState.active = false;
        dlState.task = null;
        dlState.pausedTask = null;
        dlState.lastVerifyGameName = task.gameName;
        dlState.lastVerifyGameFolder = task.gameFolder;
        dlState.lastVerifyLauncherApi = task.launcherApi;
        dlState.lastVerifyBizPrefix = task.bizPrefix || '';
        dlState.lastVerifyFailed = [...result.failed];

        if (result.failed.length > 0) {
          dlState.error = `校验完成，但有 ${result.failed.length} 个文件仍然异常`;
          const preview = result.failed.slice(0, 8);
          const suffix =
            result.failed.length > preview.length
              ? `\n... 还有 ${result.failed.length - preview.length} 个文件未展示`
              : '';
          const details = preview.length > 0 ? `\n\n异常文件（部分）：\n${preview.join('\n')}${suffix}` : '';
          await showMessage(
            `校验完成：共 ${result.total_files} 个文件，${result.verified_ok} 个正常，${result.failed.length} 个异常。${details}\n\n可点击“修复异常文件”仅重下异常条目。`,
            { title: '校验结果', kind: 'warning' },
          );
        } else {
          dlState.error = '';
          if (result.redownloaded > 0) {
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
        }
        break;
      }

      case 'repair_game': {
        const biz = task.bizPrefix || undefined;
        const files = task.repairFiles ? [...task.repairFiles] : [];
        if (files.length === 0) {
          throw new Error('没有可修复的异常文件列表');
        }
        const result = await apiRepairGameFiles(task.launcherApi, task.gameFolder, files, biz);

        dlState.phase = 'done';
        dlState.active = false;
        dlState.task = null;
        dlState.pausedTask = null;
        dlState.lastVerifyGameName = task.gameName;
        dlState.lastVerifyGameFolder = task.gameFolder;
        dlState.lastVerifyLauncherApi = task.launcherApi;
        dlState.lastVerifyBizPrefix = task.bizPrefix || '';
        dlState.lastVerifyFailed = [...result.failed];

        if (result.failed.length > 0) {
          dlState.error = `修复完成，但仍有 ${result.failed.length} 个文件异常`;
          const preview = result.failed.slice(0, 8);
          const suffix =
            result.failed.length > preview.length
              ? `\n... 还有 ${result.failed.length - preview.length} 个文件未展示`
              : '';
          const details = preview.length > 0 ? `\n\n仍异常文件（部分）：\n${preview.join('\n')}${suffix}` : '';
          await showMessage(
            `修复完成：请求修复 ${result.requested_files} 个文件，成功 ${result.repaired_ok} 个，失败 ${result.failed.length} 个。${details}`,
            { title: '修复结果', kind: 'warning' },
          );
        } else {
          dlState.error = '';
          await showMessage(
            `修复完成！请求修复 ${result.requested_files} 个文件，成功 ${result.repaired_ok} 个。`,
            { title: '修复结果', kind: 'info' },
          );
        }
        break;
      }

      case 'download_launcher_installer':
      case 'update_launcher_installer': {
        const preset = task.gamePreset || task.gameName;
        const result = task.operation === 'update_launcher_installer'
          ? await apiUpdateLauncherInstaller(task.launcherApi, task.gameFolder, preset)
          : await apiDownloadLauncherInstaller(task.launcherApi, task.gameFolder, preset);

        try {
          const config = await loadGameConfig(task.gameName);
          config.other = config.other || {};
          config.other.launcherApi = task.launcherApi;
          config.other.gameFolder = task.gameFolder;
          config.other.gamePath = result.installerPath;
          config.other.launcherInstallerVersion = result.version;
          config.other.launcherInstallerPath = result.installerPath;
          await saveGameConfig(task.gameName, config);
        } catch {
          /* best-effort */
        }

        await showMessage(
          `官方启动器安装器下载完成：${result.version}\n已自动将 gamePath 指向安装器，可在游戏设置中改为实际游戏主程序。`,
          { title: '下载完成', kind: 'info' },
        );

        dlState.phase = 'done';
        dlState.active = false;
        dlState.task = null;
        dlState.pausedTask = null;
        break;
      }
    }
  } catch (e: any) {
    const cancelled = isCancellationLikeError(e);
    const pauseRequested = _pauseRequestedTaskKey === key;
    _pauseRequestedTaskKey = null;

    if (cancelled && pauseRequested) {
      dlState.phase = 'paused';
      dlState.active = false;
      dlState.error = '';
      dlState.task = null;
      dlState.pausedTask = cloneTask(task);
      return;
    }

    if (cancelled) {
      resetStateToIdle();
      dlState.pausedTask = null;
      return;
    }

    dlState.phase = 'error';
    dlState.error = String(e);
    dlState.active = false;
    dlState.task = null;

    if (task.operation === 'download_launcher_installer' || task.operation === 'update_launcher_installer') {
      await showMessage(
        `下载启动器失败: ${String(e)}`,
        { title: '下载错误', kind: 'error' },
      ).catch(() => {});
    } else if (task.operation === 'download_game' || task.operation === 'update_game') {
      await showMessage(
        `下载失败: ${String(e)}`,
        { title: '下载错误', kind: 'error' },
      ).catch(() => {});
    } else if (task.operation === 'verify_game') {
      await showMessage(
        `校验失败: ${String(e)}`,
        { title: '校验错误', kind: 'error' },
      ).catch(() => {});
    } else if (task.operation === 'repair_game') {
      await showMessage(
        `修复失败: ${String(e)}`,
        { title: '修复错误', kind: 'error' },
      ).catch(() => {});
    }
  } finally {
    if (_pauseRequestedTaskKey === key) {
      _pauseRequestedTaskKey = null;
    }
  }
}

function startTask(task: DownloadTaskModel): boolean {
  if (dlState.active) return false;
  // 兜底初始化事件监听，避免“任务已开始但界面无进度反馈”
  initDlListeners().catch((e) => console.warn('[dlStore] init listeners failed:', e));
  _pauseRequestedTaskKey = null;
  dlState.pausedTask = null;
  markTaskRunning(task);
  runTask(task).catch((e) => console.error('[dlStore] uncaught task:', e));
  return true;
}

export function isActiveFor(gameName: string): boolean {
  return dlState.active && dlState.gameName === gameName;
}

export function isPausedFor(gameName: string): boolean {
  return !dlState.active && dlState.phase === 'paused' && dlState.pausedTask?.gameName === gameName;
}

// ---- Download ----

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

/**
 * Fire-and-forget: starts the download in the background.
 * Returns immediately; progress is tracked via `dlState`.
 */
export function fireDownload(opts: StartDlOpts) {
  startTask(taskFromStartDl(opts));
}

export function fireVerify(opts: VerifyOpts) {
  startTask(taskFromVerify(opts));
}

export function fireRepair(opts: RepairOpts) {
  startTask(taskFromRepair(opts));
}

export function fireLauncherInstallerDownload(opts: StartLauncherInstallerDlOpts) {
  startTask(taskFromInstaller(opts));
}

export function getRepairableFailuresFor(
  gameName: string,
  gameFolder: string,
  launcherApi: string,
  bizPrefix?: string,
): string[] {
  if (dlState.lastVerifyFailed.length === 0) return [];
  if (dlState.lastVerifyGameName !== gameName) return [];
  if (dlState.lastVerifyGameFolder !== gameFolder) return [];
  if (dlState.lastVerifyLauncherApi !== launcherApi) return [];
  if ((dlState.lastVerifyBizPrefix || '') !== (bizPrefix || '')) return [];
  return [...dlState.lastVerifyFailed];
}

export async function pauseActive() {
  if (!dlState.active || !dlState.task) return;
  _pauseRequestedTaskKey = taskKey(dlState.task);
  try {
    await apiCancelDownload(dlState.gameFolder || undefined);
  } catch (e) {
    _pauseRequestedTaskKey = null;
    console.error('[dlStore] pause failed:', e);
  }
}

export async function resumePaused(gameName?: string): Promise<boolean> {
  if (dlState.active) return false;
  const task = dlState.pausedTask;
  if (!task) return false;
  if (gameName && task.gameName !== gameName) return false;
  return startTask(task);
}

// ---- Cancel ----

export async function cancelActive() {
  _pauseRequestedTaskKey = null;
  dlState.pausedTask = null;

  if (!dlState.active) {
    if (dlState.phase === 'paused') {
      resetStateToIdle();
    }
    return;
  }

  try {
    await apiCancelDownload(dlState.gameFolder || undefined);
  } catch (e) {
    console.error('[dlStore] cancel failed:', e);
  }
}

// ---- 已知游戏可执行文件名映射 ----

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
