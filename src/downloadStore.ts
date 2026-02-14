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
  updateGame as apiUpdateGame,
  verifyGameFiles as apiVerifyGameFiles,
  cancelDownload as apiCancelDownload,
  listenEvent,
  showMessage,
  saveGameConfig,
  loadGameConfig,
  type DownloadProgress,
} from './api';

export type DlPhase = 'idle' | 'downloading' | 'verifying' | 'done' | 'error';

export interface DlState {
  active: boolean;
  gameName: string;
  displayName: string;
  phase: DlPhase;
  progress: DownloadProgress | null;
  error: string;
}

export const dlState = reactive<DlState>({
  active: false,
  gameName: '',
  displayName: '',
  phase: 'idle',
  progress: null,
  error: '',
});

// ---- Global event listeners (registered once) ----

let _listenersReady = false;

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

export function isActiveFor(gameName: string): boolean {
  return dlState.active && dlState.gameName === gameName;
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

/**
 * Fire-and-forget: starts the download in the background.
 * Returns immediately; progress is tracked via `dlState`.
 */
export function fireDownload(opts: StartDlOpts) {
  if (dlState.active) return;

  dlState.active = true;
  dlState.gameName = opts.gameName;
  dlState.displayName = opts.displayName;
  dlState.phase = 'downloading';
  dlState.progress = null;
  dlState.error = '';

  _execDownload(opts).catch((e) => console.error('[dlStore] uncaught:', e));
}

async function _execDownload(opts: StartDlOpts) {
  try {
    const langs = opts.languages && opts.languages.length > 0 ? opts.languages : undefined;
    const biz = opts.bizPrefix || undefined;

    if (opts.isUpdate) {
      await apiUpdateGame(opts.launcherApi, opts.gameFolder, langs, biz);
    } else {
      await apiDownloadGame(opts.launcherApi, opts.gameFolder, langs, biz);
    }

    // Persist config on success
    try {
      const config = await loadGameConfig(opts.gameName);
      config.other = config.other || {};
      config.other.launcherApi = opts.launcherApi;
      config.other.gameFolder = opts.gameFolder;
      // 自动设置游戏可执行文件路径（首次安装时）
      if (!config.other.gamePath) {
        const exeName = resolveGameExeName(opts.gameName, opts.launcherApi);
        if (exeName) {
          config.other.gamePath = opts.gameFolder + '/' + exeName;
        }
      }
      await saveGameConfig(opts.gameName, config);
    } catch { /* best-effort */ }

    // Auto-verify
    dlState.phase = 'verifying';
    dlState.progress = null;

    try {
      const biz = opts.bizPrefix || undefined;
      const result = await apiVerifyGameFiles(opts.launcherApi, opts.gameFolder, biz);
      if (result.failed.length > 0) {
        await showMessage(
          `校验完成，但有 ${result.failed.length} 个文件异常。`,
          { title: '校验结果', kind: 'warning' },
        );
      } else {
        await showMessage(
          `下载并校验完成！共 ${result.total_files} 个文件全部正常。`,
          { title: '成功', kind: 'info' },
        );
      }
    } catch (e) {
      console.warn('[dlStore] verify error:', e);
    }

    dlState.phase = 'done';
    dlState.active = false;
  } catch (e: any) {
    if (String(e).includes('cancelled')) {
      dlState.phase = 'idle';
    } else {
      dlState.phase = 'error';
      dlState.error = String(e);
      // 弹窗通知用户下载失败
      await showMessage(
        `下载失败: ${String(e)}`,
        { title: '下载错误', kind: 'error' },
      ).catch(() => {});
    }
    dlState.active = false;
  }
}

// ---- Standalone verify ----

export interface VerifyOpts {
  gameName: string;
  displayName: string;
  launcherApi: string;
  gameFolder: string;
  bizPrefix?: string;
}

export function fireVerify(opts: VerifyOpts) {
  if (dlState.active) return;

  dlState.active = true;
  dlState.gameName = opts.gameName;
  dlState.displayName = opts.displayName;
  dlState.phase = 'verifying';
  dlState.progress = null;
  dlState.error = '';

  _execVerify(opts).catch((e) => console.error('[dlStore] uncaught:', e));
}

async function _execVerify(opts: VerifyOpts) {
  try {
    const biz = opts.bizPrefix || undefined;
    const result = await apiVerifyGameFiles(opts.launcherApi, opts.gameFolder, biz);

    dlState.phase = 'done';
    dlState.active = false;

    if (result.failed.length > 0) {
      dlState.error = `校验完成，但有 ${result.failed.length} 个文件仍然异常`;
    } else {
      await showMessage(
        `校验完成！共 ${result.total_files} 个文件，${result.verified_ok} 个正常，${result.redownloaded} 个已重新下载。`,
        { title: '校验结果', kind: 'info' },
      );
    }
  } catch (e: any) {
    if (String(e).includes('cancelled')) {
      dlState.phase = 'idle';
    } else {
      dlState.phase = 'error';
      dlState.error = String(e);
    }
    dlState.active = false;
  }
}

// ---- Cancel ----

export async function cancelActive() {
  try {
    await apiCancelDownload();
  } catch (e) {
    console.error('[dlStore] cancel failed:', e);
  }
}

// ---- 已知游戏可执行文件名映射 ----

function resolveGameExeName(gameName: string, launcherApi: string): string | null {
  const isCN = launcherApi.includes('mihoyo.com');
  switch (gameName) {
    case 'SRMI': return 'StarRail.exe';
    case 'GIMI': return isCN ? 'YuanShen.exe' : 'GenshinImpact.exe';
    case 'ZZMI': return 'ZenlessZoneZero.exe';
    case 'HIMI': return 'BH3.exe';
    default: return null;
  }
}
