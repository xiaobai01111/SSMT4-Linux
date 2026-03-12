import migotoLayoutManifest from '../shared/migoto-layout.json';

export interface MigotoImporterBehavior {
  defaultUseHook: boolean;
  defaultEnforceRendering: boolean;
  defaultProcessTimeout: number;
  defaultXxmiDllInitDelay: number;
  requiredStartArgs: string[];
  injectionLocked?: boolean;
}

export interface MigotoLockedImporterLegacyDefaults {
  enforceRendering: boolean;
  processTimeout: number;
  xxmiDllInitDelay: number;
}

interface MigotoLayoutManifest {
  defaultImporter: string;
  importerOrder: string[];
  gameImporters: Record<string, string>;
  importerMarkers: string[];
  importerBehaviors: Record<string, MigotoImporterBehavior>;
  defaultImporterBehavior: MigotoImporterBehavior;
  lockedImporterLegacyDefaults: MigotoLockedImporterLegacyDefaults;
}

export interface MigotoPathConfigLike {
  importer?: string | null;
  migoto_path?: string | null;
  importer_folder?: string | null;
  mod_folder?: string | null;
  shader_fixes_folder?: string | null;
  d3dx_ini_path?: string | null;
}

export interface MigotoResolvedPaths {
  importer: string;
  migotoPath: string;
  importerFolder: string;
  modFolder: string;
  shaderFixesFolder: string;
  d3dxIniPath: string;
}

type PathExistsFn = (path: string) => Promise<boolean>;
type JoinPathFn = (base: string, child: string) => string | Promise<string>;

const MIGOTO_LAYOUT = migotoLayoutManifest as MigotoLayoutManifest;

export const trimMigotoPathValue = (value: unknown) =>
  String(value ?? '').trim();

export const normalizeMigotoImporterValue = (value: unknown) =>
  trimMigotoPathValue(value).toUpperCase();

export const splitMigotoStartArgs = (value: unknown) =>
  trimMigotoPathValue(value).split(/\s+/).filter(Boolean);

export const joinMigotoPath = (base: unknown, child: unknown) => {
  const normalizedBase = trimMigotoPathValue(base).replace(/[\\/]+$/g, '');
  const normalizedChild = trimMigotoPathValue(child).replace(/^[/\\]+/g, '');
  if (!normalizedBase) return normalizedChild;
  if (!normalizedChild) return normalizedBase;
  return `${normalizedBase}/${normalizedChild}`;
};

export const getMigotoPathBasename = (path: unknown) => {
  const normalized = trimMigotoPathValue(path).replace(/[\\/]+$/g, '');
  const segments = normalized.split(/[\\/]+/).filter(Boolean);
  return segments[segments.length - 1] || '';
};

export const MIGOTO_DEFAULT_IMPORTER =
  trimMigotoPathValue(MIGOTO_LAYOUT.defaultImporter) || 'WWMI';
export const MIGOTO_IMPORTER_ORDER = [...MIGOTO_LAYOUT.importerOrder];
export const MIGOTO_IMPORTER_MARKERS = [...MIGOTO_LAYOUT.importerMarkers];
export const MIGOTO_DEFAULT_IMPORTER_BEHAVIOR = {
  ...MIGOTO_LAYOUT.defaultImporterBehavior,
};
export const MIGOTO_LOCKED_IMPORTER_LEGACY_DEFAULTS = {
  ...MIGOTO_LAYOUT.lockedImporterLegacyDefaults,
};

export const getRequiredMigotoImporter = (gameName: string) =>
  trimMigotoPathValue(MIGOTO_LAYOUT.gameImporters[gameName] || '');

export const resolveMigotoImporter = (
  gameName: string,
  importerValue: unknown,
) => {
  const requiredImporter = getRequiredMigotoImporter(gameName);
  if (requiredImporter) return requiredImporter;

  const explicitImporter = normalizeMigotoImporterValue(importerValue);
  return explicitImporter || MIGOTO_DEFAULT_IMPORTER;
};

export const getMigotoImporterBehavior = (
  gameName: string,
  importerValue: unknown,
): MigotoImporterBehavior => {
  const importer = resolveMigotoImporter(gameName, importerValue);
  return (
    MIGOTO_LAYOUT.importerBehaviors[normalizeMigotoImporterValue(importer)] ||
    MIGOTO_DEFAULT_IMPORTER_BEHAVIOR
  );
};

const resolveJoinedPath = async (
  base: string,
  child: string,
  joinPath?: JoinPathFn,
) => {
  if (!trimMigotoPathValue(base) || !trimMigotoPathValue(child)) {
    return '';
  }

  if (!joinPath) {
    return joinMigotoPath(base, child);
  }

  return trimMigotoPathValue(await joinPath(base, child));
};

export const pathLooksLikeMigotoImporterFolder = async (
  basePath: string,
  pathExistsAt: PathExistsFn,
  joinPath?: JoinPathFn,
) => {
  const normalizedBase = trimMigotoPathValue(basePath);
  if (!normalizedBase) return false;

  for (const marker of MIGOTO_IMPORTER_MARKERS) {
    try {
      const candidate = await resolveJoinedPath(
        normalizedBase,
        marker,
        joinPath,
      );
      if (candidate && await pathExistsAt(candidate)) {
        return true;
      }
    } catch {
      // ignore invalid candidates and keep probing markers
    }
  }

  return false;
};

export const resolveMigotoImporterFolder = async ({
  basePath,
  importer,
  explicitImporterFolder,
  pathExistsAt,
  joinPath,
}: {
  basePath: string;
  importer: string;
  explicitImporterFolder?: string | null;
  pathExistsAt: PathExistsFn;
  joinPath?: JoinPathFn;
}) => {
  const normalizedExplicit = trimMigotoPathValue(explicitImporterFolder);
  if (normalizedExplicit) {
    return normalizedExplicit;
  }

  const normalizedBase = trimMigotoPathValue(basePath);
  if (!normalizedBase) {
    return '';
  }

  if (
    await pathLooksLikeMigotoImporterFolder(normalizedBase, pathExistsAt, joinPath) ||
    getMigotoPathBasename(normalizedBase).toUpperCase() ===
      normalizeMigotoImporterValue(importer)
  ) {
    return normalizedBase;
  }

  try {
    const nestedImporterFolder = await resolveJoinedPath(
      normalizedBase,
      normalizeMigotoImporterValue(importer),
      joinPath,
    );
    if (nestedImporterFolder && await pathExistsAt(nestedImporterFolder)) {
      return nestedImporterFolder;
    }
  } catch {
    // ignore invalid candidates and fall back to the configured data path
  }

  return normalizedBase;
};

export const buildMigotoResolvedPaths = ({
  gameName,
  config,
  defaultMigotoPath = '',
  detectedImporterFolder = '',
}: {
  gameName: string;
  config: MigotoPathConfigLike;
  defaultMigotoPath?: string;
  detectedImporterFolder?: string;
}): MigotoResolvedPaths => {
  const importer = resolveMigotoImporter(gameName, config.importer);
  const migotoPath =
    trimMigotoPathValue(config.migoto_path) ||
    trimMigotoPathValue(defaultMigotoPath);
  const importerFolder =
    trimMigotoPathValue(config.importer_folder) ||
    trimMigotoPathValue(detectedImporterFolder) ||
    migotoPath;
  const modFolder =
    trimMigotoPathValue(config.mod_folder) ||
    (importerFolder ? joinMigotoPath(importerFolder, 'Mods') : '');
  const shaderFixesFolder =
    trimMigotoPathValue(config.shader_fixes_folder) ||
    (importerFolder ? joinMigotoPath(importerFolder, 'ShaderFixes') : '');
  const d3dxIniPath =
    trimMigotoPathValue(config.d3dx_ini_path) ||
    (importerFolder ? joinMigotoPath(importerFolder, 'd3dx.ini') : '');

  return {
    importer,
    migotoPath,
    importerFolder,
    modFolder,
    shaderFixesFolder,
    d3dxIniPath,
  };
};

export const detectMigotoResolvedPaths = async ({
  gameName,
  config,
  defaultMigotoPath = '',
  pathExistsAt,
  joinPath,
}: {
  gameName: string;
  config: MigotoPathConfigLike;
  defaultMigotoPath?: string;
  pathExistsAt: PathExistsFn;
  joinPath?: JoinPathFn;
}) => {
  const importer = resolveMigotoImporter(gameName, config.importer);
  const migotoPath =
    trimMigotoPathValue(config.migoto_path) ||
    trimMigotoPathValue(defaultMigotoPath);
  const importerFolder = await resolveMigotoImporterFolder({
    basePath: migotoPath,
    importer,
    explicitImporterFolder: config.importer_folder,
    pathExistsAt,
    joinPath,
  });

  return buildMigotoResolvedPaths({
    gameName,
    config,
    defaultMigotoPath,
    detectedImporterFolder: importerFolder,
  });
};
