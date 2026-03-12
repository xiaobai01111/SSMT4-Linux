export interface GameUpdateServerInfo {
  launcherApi: string;
  bizPrefix?: string;
}

export interface ResolvedGameUpdateSource {
  launcherApi: string;
  bizPrefix?: string;
}

export function trimFolderPart(value: string): string {
  return value.trim().replace(/^[\\/]+|[\\/]+$/g, '');
}

export function normalizePathForCompare(value: string): string {
  return value.replace(/\\/g, '/').replace(/\/+/g, '/').replace(/\/$/, '');
}

export function parentDir(value: string): string {
  const normalized = normalizePathForCompare(value);
  const idx = normalized.lastIndexOf('/');
  if (idx <= 0) return '';
  return normalized.slice(0, idx);
}

export function resolveGameUpdateFolder(
  savedGameFolder: string,
  savedGamePath: string,
  baseDir: string,
  defaultFolderPart: string,
): string {
  const normalizedSavedFolder = savedGameFolder.trim();
  if (normalizedSavedFolder) return normalizedSavedFolder;

  const normalizedSavedPath = savedGamePath.trim();
  if (normalizedSavedPath) return parentDir(normalizedSavedPath);

  const normalizedBaseDir = baseDir.trim();
  if (!normalizedBaseDir) return '';

  const trimmedDefaultFolder = trimFolderPart(defaultFolderPart);
  return trimmedDefaultFolder
    ? `${normalizedBaseDir}/${trimmedDefaultFolder}`
    : normalizedBaseDir;
}

export function resolveGameUpdateSource(
  savedLauncherApi: string,
  servers: GameUpdateServerInfo[],
  fallbackLauncherApi: string,
): ResolvedGameUpdateSource | null {
  const normalizedSavedLauncherApi = savedLauncherApi.trim();
  if (normalizedSavedLauncherApi) {
    const matched = servers.find(
      (server) => server.launcherApi.trim() === normalizedSavedLauncherApi,
    );
    return {
      launcherApi: normalizedSavedLauncherApi,
      bizPrefix: matched?.bizPrefix?.trim() || undefined,
    };
  }

  const fallbackServer = servers[0];
  const launcherApi =
    fallbackServer?.launcherApi?.trim()
    || fallbackLauncherApi.trim();
  if (!launcherApi) return null;

  return {
    launcherApi,
    bizPrefix: fallbackServer?.bizPrefix?.trim() || undefined,
  };
}
