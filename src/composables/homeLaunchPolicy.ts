import {
  checkGameProtectionStatus,
  detectDxvkStatus,
  getGameWineConfig,
  resolveDownloadedGameExecutable,
  scanLocalDxvk,
  scanWineVersions,
  type GameConfig,
} from '../api';
import type {
  ExecutablePathProbeResult,
  ProtectionRequirementResult,
  RuntimeReadinessResult,
} from '../types/homeLaunch';

const SHOULD_CHECK_EXECUTABLE_PRESETS = new Set([
  'wutheringwaves',
  'honkaistarrail',
  'zenlesszonezero',
]);

const normalizePathForCompare = (value: string) =>
  String(value || '')
    .trim()
    .replace(/\\/g, '/')
    .replace(/\/+/g, '/')
    .replace(/\/$/, '');

const parentDir = (path: string): string => {
  const normalized = normalizePathForCompare(path);
  if (!normalized) return '';
  const idx = normalized.lastIndexOf('/');
  if (idx <= 0) return '';
  return normalized.slice(0, idx);
};

const pushUniquePath = (arr: string[], value: string) => {
  const normalized = normalizePathForCompare(value);
  if (!normalized || arr.includes(normalized)) return;
  arr.push(normalized);
};

const buildExecutableCheckRoots = (
  preset: string,
  gamePath: string,
  configuredGameFolder?: string,
): string[] => {
  const roots: string[] = [];
  pushUniquePath(roots, configuredGameFolder || '');

  const normalizedPath = normalizePathForCompare(gamePath);
  if (!normalizedPath) return roots;

  const exeDir = parentDir(normalizedPath);
  pushUniquePath(roots, exeDir);

  let current = exeDir;
  for (let i = 0; i < 4; i += 1) {
    current = parentDir(current);
    if (!current) break;
    pushUniquePath(roots, current);
  }

  if (preset === 'wutheringwaves') {
    const lower = normalizedPath.toLowerCase();
    for (const marker of [
      '/wuthering waves game/client/binaries/win64/',
      '/client/binaries/win64/',
      '/wuthering waves game/',
    ]) {
      const idx = lower.indexOf(marker);
      if (idx > 0) {
        pushUniquePath(roots, normalizedPath.slice(0, idx));
      }
    }
  }

  return roots;
};

const resolveGameRoot = (gameConfig: GameConfig): string | undefined => {
  const folder = String(gameConfig?.other?.gameFolder || '').trim();
  if (folder) return folder;

  const exePath = String(gameConfig?.other?.gamePath || '').trim();
  if (!exePath) return undefined;

  const normalized = exePath.replace(/\\/g, '/');
  const idx = normalized.lastIndexOf('/');
  return idx > 0 ? normalized.slice(0, idx) : undefined;
};

export async function resolveWineVersionIdForLaunch(
  gameName: string,
  gameConfig: GameConfig,
): Promise<string> {
  const configured = String(gameConfig?.other?.wineVersionId || '').trim();
  if (configured) return configured;

  try {
    const wineConfig = await getGameWineConfig(gameName);
    return String(wineConfig?.wine_version_id || '').trim();
  } catch {
    return '';
  }
}

export async function probeExecutablePathMismatch(
  gameName: string,
  gameConfig: GameConfig,
): Promise<ExecutablePathProbeResult> {
  const presetRaw = String(gameConfig?.basic?.gamePreset || gameName).trim();
  const preset = presetRaw.toLowerCase();
  if (!SHOULD_CHECK_EXECUTABLE_PRESETS.has(preset)) {
    return { kind: 'ok' };
  }

  const configuredPath = normalizePathForCompare(
    String(gameConfig?.other?.gamePath || ''),
  );
  if (!configuredPath) {
    return { kind: 'ok' };
  }

  const launcherApi = String(gameConfig?.other?.launcherApi || '').trim();
  const configuredGameFolder = String(gameConfig?.other?.gameFolder || '').trim();
  const roots = buildExecutableCheckRoots(
    preset,
    configuredPath,
    configuredGameFolder,
  );
  if (roots.length === 0) {
    return { kind: 'ok' };
  }

  const probeResults = await Promise.allSettled(
    roots.map((root) =>
      resolveDownloadedGameExecutable(
        presetRaw,
        root,
        launcherApi || undefined,
      ),
    ),
  );

  let detectedPath: string | null = null;
  for (const result of probeResults) {
    if (result.status === 'fulfilled' && result.value) {
      detectedPath = result.value;
      break;
    }
  }

  const detected = normalizePathForCompare(String(detectedPath || ''));
  if (!detected || detected === configuredPath) {
    return { kind: 'ok' };
  }

  return {
    kind: 'mismatch',
    configuredPath,
    detectedPath: detected,
  };
}

export async function checkRuntimeReadiness(
  gameName: string,
  gameConfig: GameConfig,
  wineVersionId: string,
): Promise<RuntimeReadinessResult> {
  const runtimeEnv = String(gameConfig?.basic?.runtimeEnv || 'wine').toLowerCase();
  if (runtimeEnv !== 'wine') {
    return { kind: 'ready' };
  }

  let localDxvk: Awaited<ReturnType<typeof scanLocalDxvk>> = [];
  let dxvkInstalledInPrefix = false;
  let dxvkStatusCached: Awaited<ReturnType<typeof detectDxvkStatus>> | null = null;

  const [wineResult, dxvkLocalResult, dxvkStatusResult] = await Promise.allSettled(
    [scanWineVersions(), scanLocalDxvk(), detectDxvkStatus(gameName)],
  );

  if (wineResult.status === 'rejected') {
    return {
      kind: 'runtime_scan_failed',
      reason: String(wineResult.reason),
    };
  }
  const versions = wineResult.value;

  if (dxvkLocalResult.status === 'fulfilled') {
    localDxvk = dxvkLocalResult.value;
  }

  if (dxvkStatusResult.status === 'fulfilled') {
    dxvkStatusCached = dxvkStatusResult.value;
    dxvkInstalledInPrefix = !!dxvkStatusResult.value.installed;
  }

  if (versions.length === 0) {
    return { kind: 'no_proton' };
  }

  if (localDxvk.length === 0 && !dxvkInstalledInPrefix) {
    return { kind: 'no_dxvk' };
  }

  if (!wineVersionId.trim()) {
    return { kind: 'proton_not_configured' };
  }

  const selected = versions.find((version) => version.id === wineVersionId);
  if (!selected) {
    return {
      kind: 'invalid_proton_version',
      wineVersionId,
    };
  }

  try {
    const dxvkStatus = dxvkStatusCached || (await detectDxvkStatus(gameName));
    if (!dxvkStatus.installed) {
      return { kind: 'dxvk_not_installed' };
    }
  } catch {
    // DXVK detection is best-effort. Keep launch available when the probe itself fails.
  }

  return { kind: 'ready' };
}

export async function checkProtectionRequirement(
  gameName: string,
  gameConfig: GameConfig,
): Promise<ProtectionRequirementResult> {
  try {
    const preset = gameConfig?.basic?.gamePreset || gameName;
    const gameRoot = resolveGameRoot(gameConfig);

    let status = await checkGameProtectionStatus(preset, gameRoot);
    if (status?.enforceAtLaunch !== true && preset !== gameName) {
      const fallback = await checkGameProtectionStatus(gameName, gameRoot);
      if (fallback?.enforceAtLaunch === true) {
        status = fallback;
      }
    }

    if (status?.enforceAtLaunch !== true) {
      const missing = Array.isArray(status?.missing) ? status.missing : [];
      if (missing.length > 0) {
        return {
          kind: 'warning_missing_optional',
          missing,
        };
      }
      return { kind: 'ready' };
    }

    if (status?.enabled) {
      return { kind: 'ready' };
    }

    return {
      kind: 'required_missing',
      missing: Array.isArray(status?.missing) ? status.missing : [],
    };
  } catch (error) {
    return {
      kind: 'check_failed',
      error: String(error),
    };
  }
}
