import type { GameDownloadOperation } from '../types/events';

export type DlPhase = 'idle' | 'downloading' | 'verifying' | 'paused' | 'done' | 'error';

export type DownloadOperation =
  | 'download_game'
  | 'update_game'
  | 'verify_game'
  | 'repair_game'
  | 'download_launcher_installer'
  | 'update_launcher_installer';

export interface DownloadTaskIdentity {
  operation: DownloadOperation;
  gameName: string;
  launcherApi: string;
  gameFolder: string;
  languages?: string[];
  bizPrefix?: string;
  gamePreset?: string;
  repairFiles?: string[];
}

export function phaseFromTaskOperation(operation: DownloadOperation): DlPhase {
  return operation === 'verify_game' || operation === 'repair_game'
    ? 'verifying'
    : 'downloading';
}

export function phaseFromProgressOperation(operation: GameDownloadOperation): DlPhase {
  return operation === 'verify-game' || operation === 'repair-game'
    ? 'verifying'
    : 'downloading';
}

export function normalizeErrorText(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message || String(error);
  if (error && typeof error === 'object') {
    const maybeMessage = (error as { message?: unknown }).message;
    if (typeof maybeMessage === 'string') return maybeMessage;
  }
  return String(error ?? '');
}

export function isCancellationLikeError(error: unknown): boolean {
  const raw = normalizeErrorText(error);
  if (!raw) return false;
  const lower = raw.toLowerCase();

  const englishMarkers = ['cancel', 'cancelled', 'canceled', 'abort', 'aborted'];
  if (englishMarkers.some((marker) => lower.includes(marker))) return true;

  const chineseMarkers = ['取消', '已取消', '中止', '终止'];
  return chineseMarkers.some((marker) => raw.includes(marker));
}

export function buildTaskKey(task: DownloadTaskIdentity): string {
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

export function buildVerifyFailureKey(
  gameName: string,
  gameFolder: string,
  launcherApi: string,
  bizPrefix?: string,
): string {
  return [gameName, gameFolder, launcherApi, bizPrefix || ''].join('::');
}

export function isGameDownloadOperation(operation: DownloadOperation): boolean {
  return operation === 'download_game' || operation === 'update_game';
}

export function isLauncherInstallerOperation(operation: DownloadOperation): boolean {
  return operation === 'download_launcher_installer' || operation === 'update_launcher_installer';
}

export function taskFailureMessage(operation: DownloadOperation, message: string): string {
  if (isLauncherInstallerOperation(operation)) return `下载启动器失败: ${message}`;
  if (isGameDownloadOperation(operation)) return `下载失败: ${message}`;
  if (operation === 'verify_game') return `校验失败: ${message}`;
  return `修复失败: ${message}`;
}

export function taskFailureTitle(operation: DownloadOperation): string {
  if (isGameDownloadOperation(operation) || isLauncherInstallerOperation(operation)) return '下载错误';
  if (operation === 'verify_game') return '校验错误';
  return '修复错误';
}

export function buildFailedFilesPreviewSection(
  failedFiles: string[],
  title: string,
  maxPreview: number = 8,
): string {
  const preview = failedFiles.slice(0, maxPreview);
  if (preview.length === 0) return '';

  const remaining = failedFiles.length - preview.length;
  const suffix = remaining > 0 ? `\n... 还有 ${remaining} 个文件未展示` : '';
  return `\n\n${title}（部分）：\n${preview.join('\n')}${suffix}`;
}
