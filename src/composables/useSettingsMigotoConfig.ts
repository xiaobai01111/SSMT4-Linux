import { computed, reactive, ref, watch } from 'vue';
import { appSettings } from '../store';
import {
  askConfirm,
  joinPath,
  loadGameConfig,
  openFileDialog,
  pathExists,
  saveGameConfig,
  scanGames,
  showMessage,
  type GameInfo,
} from '../api';
import {
  MIGOTO_DEFAULT_IMPORTER,
  MIGOTO_IMPORTER_ORDER,
  MIGOTO_LOCKED_IMPORTER_LEGACY_DEFAULTS,
  buildMigotoResolvedPaths,
  getMigotoImporterBehavior,
  getRequiredMigotoImporter,
  joinMigotoPath,
  resolveMigotoImporter,
  resolveMigotoImporterFolder,
  splitMigotoStartArgs,
  trimMigotoPathValue,
} from '../utils/migotoLayout';

type TranslateFn = (key: string, params?: Record<string, unknown>) => string;
type FallbackTranslateFn = (
  key: string,
  fallback: string,
  params?: Record<string, unknown>,
) => string;
type ToastFn = (
  kind: 'success' | 'warning' | 'info' | 'error',
  title: string,
  message: string,
) => Promise<void>;

interface UseSettingsMigotoConfigOptions {
  t: TranslateFn;
  tr: FallbackTranslateFn;
  toast: ToastFn;
}

export interface MigotoGameConfig {
  enabled: boolean;
  importer: string;
  use_hook: boolean;
  enforce_rendering: boolean;
  enable_hunting: boolean;
  dump_shaders: boolean;
  mute_warnings: boolean;
  calls_logging: boolean;
  debug_logging: boolean;
  unsafe_mode: boolean;
  process_timeout: number;
  migoto_path: string;
  importer_folder: string;
  mod_folder: string;
  shader_fixes_folder: string;
  d3dx_ini_path: string;
  bridge_exe_path: string;
  start_args: string;
  process_start_method: string;
  process_priority: string;
  xxmi_dll_init_delay: number;
  extra_libraries_enabled: boolean;
  extra_libraries_paths: string;
  custom_launch_enabled: boolean;
  custom_launch_cmd: string;
  custom_launch_inject_mode: string;
  pre_launch_enabled: boolean;
  pre_launch_cmd: string;
  pre_launch_wait: boolean;
  post_load_enabled: boolean;
  post_load_cmd: string;
  post_load_wait: boolean;
  wwmi_configure_game: boolean;
  wwmi_unlock_fps: boolean;
  wwmi_perf_tweaks: boolean;
  wwmi_disable_wounded_fx: boolean;
}

export type MigotoPathOverrideField =
  | 'importer_folder'
  | 'mod_folder'
  | 'shader_fixes_folder'
  | 'd3dx_ini_path';

type MigotoSelectablePathField =
  | 'migoto_path'
  | 'importer_folder'
  | 'mod_folder'
  | 'shader_fixes_folder'
  | 'd3dx_ini_path'
  | 'bridge_exe_path';

const defaultMigotoConfig: MigotoGameConfig = {
  enabled: false,
  importer: MIGOTO_DEFAULT_IMPORTER,
  use_hook: true,
  enforce_rendering: true,
  enable_hunting: false,
  dump_shaders: false,
  mute_warnings: true,
  calls_logging: false,
  debug_logging: false,
  unsafe_mode: false,
  process_timeout: 30,
  migoto_path: '',
  importer_folder: '',
  mod_folder: '',
  shader_fixes_folder: '',
  d3dx_ini_path: '',
  bridge_exe_path: '',
  start_args: '',
  process_start_method: 'Native',
  process_priority: 'Normal',
  xxmi_dll_init_delay: 500,
  extra_libraries_enabled: false,
  extra_libraries_paths: '',
  custom_launch_enabled: false,
  custom_launch_cmd: '',
  custom_launch_inject_mode: 'Hook',
  pre_launch_enabled: false,
  pre_launch_cmd: '',
  pre_launch_wait: true,
  post_load_enabled: false,
  post_load_cmd: '',
  post_load_wait: true,
  wwmi_configure_game: true,
  wwmi_unlock_fps: true,
  wwmi_perf_tweaks: true,
  wwmi_disable_wounded_fx: false,
};

export function useSettingsMigotoConfig({
  t,
  tr,
  toast,
}: UseSettingsMigotoConfigOptions) {
  const migotoGamesList = ref<GameInfo[]>([]);
  const migotoSelectedGame = ref('');
  const migotoLoaded = ref(false);
  const isMigotoSaving = ref(false);
  const isMigotoTogglePending = ref(false);
  const detectedMigotoImporterFolder = ref({
    basePath: '',
    importer: '',
    folder: '',
  });
  const migotoConfig = reactive<MigotoGameConfig>({ ...defaultMigotoConfig });

  const globalMigotoEnabled = computed(() => !!appSettings.migotoEnabled);
  const migotoRiskStatement = tr(
    'settings.migoto.riskDescription',
    '3DMigoto 目前属于实验性功能。启用后可能导致注入失败、画面异常、游戏崩溃、性能问题、兼容性故障，且不排除带来账号、数据或系统层面的风险。继续使用即表示你已知悉相关风险并自行承担全部后果，开发者不对由此造成的任何损失负责。',
  );

  const hasOwnMigotoConfigKey = <K extends keyof MigotoGameConfig>(
    config: Partial<MigotoGameConfig>,
    key: K,
  ) => Object.prototype.hasOwnProperty.call(config, key);

  const normalizeMigotoStartArgs = (
    value: string | null | undefined,
    requiredArgs: string[],
  ) => {
    const normalizedArgs = splitMigotoStartArgs(value);
    for (const requiredArg of requiredArgs) {
      if (
        normalizedArgs.some(
          (arg) => arg.toLowerCase() === requiredArg.toLowerCase(),
        )
      ) {
        continue;
      }
      normalizedArgs.push(requiredArg);
    }
    return normalizedArgs.join(' ');
  };

  const normalizeMigotoConfig = (
    config: Partial<MigotoGameConfig>,
    gameName = migotoSelectedGame.value,
  ): MigotoGameConfig => {
    const normalized: MigotoGameConfig = { ...defaultMigotoConfig, ...config };
    const requestedImporter = trimMigotoPathValue(config.importer);
    const requiredImporter = getRequiredMigotoImporter(gameName);

    normalized.importer = resolveMigotoImporter(gameName, normalized.importer);
    const importerBehavior = getMigotoImporterBehavior(
      gameName,
      normalized.importer,
    );
    normalized.migoto_path = trimMigotoPathValue(normalized.migoto_path);
    normalized.importer_folder = trimMigotoPathValue(
      normalized.importer_folder,
    );
    normalized.mod_folder = trimMigotoPathValue(normalized.mod_folder);
    normalized.shader_fixes_folder = trimMigotoPathValue(
      normalized.shader_fixes_folder,
    );
    normalized.d3dx_ini_path = trimMigotoPathValue(normalized.d3dx_ini_path);
    normalized.bridge_exe_path = trimMigotoPathValue(
      normalized.bridge_exe_path,
    );
    normalized.start_args = normalizeMigotoStartArgs(
      normalized.start_args,
      importerBehavior.requiredStartArgs,
    );

    if (importerBehavior.injectionLocked || !hasOwnMigotoConfigKey(config, 'use_hook')) {
      normalized.use_hook = importerBehavior.defaultUseHook;
    }

    const shouldHealLockedImporterDefaults = Boolean(
      requiredImporter &&
        normalized.importer === requiredImporter &&
        importerBehavior.injectionLocked,
    );

    if (
      shouldHealLockedImporterDefaults &&
      normalized.enforce_rendering === MIGOTO_LOCKED_IMPORTER_LEGACY_DEFAULTS.enforceRendering &&
      importerBehavior.defaultEnforceRendering !==
        MIGOTO_LOCKED_IMPORTER_LEGACY_DEFAULTS.enforceRendering
    ) {
      normalized.enforce_rendering = importerBehavior.defaultEnforceRendering;
    } else if (!hasOwnMigotoConfigKey(config, 'enforce_rendering')) {
      normalized.enforce_rendering = importerBehavior.defaultEnforceRendering;
    }

    if (
      shouldHealLockedImporterDefaults &&
      normalized.process_timeout === MIGOTO_LOCKED_IMPORTER_LEGACY_DEFAULTS.processTimeout &&
      importerBehavior.defaultProcessTimeout !==
        MIGOTO_LOCKED_IMPORTER_LEGACY_DEFAULTS.processTimeout
    ) {
      normalized.process_timeout = importerBehavior.defaultProcessTimeout;
    } else if (!hasOwnMigotoConfigKey(config, 'process_timeout')) {
      normalized.process_timeout = importerBehavior.defaultProcessTimeout;
    }

    if (
      shouldHealLockedImporterDefaults &&
      normalized.xxmi_dll_init_delay ===
        MIGOTO_LOCKED_IMPORTER_LEGACY_DEFAULTS.xxmiDllInitDelay &&
      importerBehavior.defaultXxmiDllInitDelay !==
        MIGOTO_LOCKED_IMPORTER_LEGACY_DEFAULTS.xxmiDllInitDelay
    ) {
      normalized.xxmi_dll_init_delay =
        importerBehavior.defaultXxmiDllInitDelay;
    } else if (!hasOwnMigotoConfigKey(config, 'xxmi_dll_init_delay')) {
      normalized.xxmi_dll_init_delay =
        importerBehavior.defaultXxmiDllInitDelay;
    }

    if (
      requiredImporter &&
      requestedImporter &&
      requestedImporter !== requiredImporter &&
      normalized.migoto_path
    ) {
      const legacyImporterFolder = joinMigotoPath(
        normalized.migoto_path,
        requestedImporter,
      );
      if (normalized.importer_folder === legacyImporterFolder) {
        normalized.importer_folder = '';
      }

      const legacyModFolder = joinMigotoPath(legacyImporterFolder, 'Mods');
      if (normalized.mod_folder === legacyModFolder) {
        normalized.mod_folder = '';
      }

      const legacyShaderFixesFolder = joinMigotoPath(
        legacyImporterFolder,
        'ShaderFixes',
      );
      if (normalized.shader_fixes_folder === legacyShaderFixesFolder) {
        normalized.shader_fixes_folder = '';
      }

      const legacyD3dxIniPath = joinMigotoPath(legacyImporterFolder, 'd3dx.ini');
      if (normalized.d3dx_ini_path === legacyD3dxIniPath) {
        normalized.d3dx_ini_path = '';
      }
    }

    return normalized;
  };

  const migotoImporterOptions = computed(() => [
    ...MIGOTO_IMPORTER_ORDER.map((value) => ({
      value,
      label: t(`settings.migoto.importer${value}`),
    })),
  ]);

  const migotoAvailableImporterOptions = computed(() => {
    const requiredImporter = getRequiredMigotoImporter(migotoSelectedGame.value);
    if (!requiredImporter) return migotoImporterOptions.value;
    return migotoImporterOptions.value.filter(
      (option) => option.value === requiredImporter,
    );
  });

  const migotoImporterBehavior = computed(() =>
    getMigotoImporterBehavior(migotoSelectedGame.value, migotoConfig.importer),
  );
  const isMigotoWwmi = computed(() => migotoConfig.importer === 'WWMI');
  const isMigotoImporterLocked = computed(() =>
    Boolean(getRequiredMigotoImporter(migotoSelectedGame.value)),
  );
  const isMigotoInjectionLocked = computed(() =>
    Boolean(migotoImporterBehavior.value.injectionLocked),
  );

  const migotoImporterHint = computed(() => {
    const requiredImporter = getRequiredMigotoImporter(migotoSelectedGame.value);
    if (requiredImporter) {
      return t('settings.migoto.importerLockedHint', {
        importer: requiredImporter,
      });
    }
    return t('settings.migoto.importerSelectHint');
  });

  const migotoInjectionHint = computed(() => {
    if (!isMigotoInjectionLocked.value) {
      return t('settings.migoto.injectionHint');
    }

    const modeKey = migotoImporterBehavior.value.defaultUseHook
      ? 'settings.migoto.injectionHook'
      : 'settings.migoto.injectionDirect';
    return t('settings.migoto.injectionLockedHint', {
      importer: resolveMigotoImporter(
        migotoSelectedGame.value,
        migotoConfig.importer,
      ),
      mode: t(modeKey),
    });
  });

  const migotoStartArgsHint = computed(() => {
    const requiredArgs = migotoImporterBehavior.value.requiredStartArgs.join(' ');
    if (!requiredArgs) {
      return t('settings.migoto.startArgsHint');
    }

    return t('settings.migoto.startArgsRequiredHint', { args: requiredArgs });
  });

  let migotoPathDetectSeq = 0;

  const getDetectedMigotoImporterFolder = (
    config: Partial<MigotoGameConfig>,
  ) => {
    const normalized = normalizeMigotoConfig(config);
    const detected = detectedMigotoImporterFolder.value;
    if (
      detected.basePath === normalized.migoto_path &&
      detected.importer === normalized.importer
    ) {
      return trimMigotoPathValue(detected.folder);
    }
    return '';
  };

  const buildEffectiveMigotoPaths = (
    config: Partial<MigotoGameConfig>,
    autoImporterFolder = getDetectedMigotoImporterFolder(config),
  ) => {
    const normalized = normalizeMigotoConfig(config);
    const resolved = buildMigotoResolvedPaths({
      gameName: migotoSelectedGame.value,
      config: normalized,
      detectedImporterFolder: autoImporterFolder,
    });

    return {
      importer_folder: resolved.importerFolder,
      mod_folder: resolved.modFolder,
      shader_fixes_folder: resolved.shaderFixesFolder,
      d3dx_ini_path: resolved.d3dxIniPath,
    };
  };

  const collapseMigotoAutoOverrides = (
    config: Partial<MigotoGameConfig>,
    autoImporterFolder = getDetectedMigotoImporterFolder(config),
  ) => {
    const normalized = normalizeMigotoConfig(config);
    const resolvedAutoImporterFolder =
      trimMigotoPathValue(autoImporterFolder) || normalized.migoto_path;

    if (
      normalized.importer_folder &&
      resolvedAutoImporterFolder &&
      normalized.importer_folder === resolvedAutoImporterFolder
    ) {
      normalized.importer_folder = '';
    }

    const effectiveImporterFolder =
      normalized.importer_folder || resolvedAutoImporterFolder;
    const autoModFolder = effectiveImporterFolder
      ? joinMigotoPath(effectiveImporterFolder, 'Mods')
      : '';
    if (normalized.mod_folder && autoModFolder && normalized.mod_folder === autoModFolder) {
      normalized.mod_folder = '';
    }

    const autoShaderFixesFolder = effectiveImporterFolder
      ? joinMigotoPath(effectiveImporterFolder, 'ShaderFixes')
      : '';
    if (
      normalized.shader_fixes_folder &&
      autoShaderFixesFolder &&
      normalized.shader_fixes_folder === autoShaderFixesFolder
    ) {
      normalized.shader_fixes_folder = '';
    }

    const autoD3dxIniPath = effectiveImporterFolder
      ? joinMigotoPath(effectiveImporterFolder, 'd3dx.ini')
      : '';
    if (
      normalized.d3dx_ini_path &&
      autoD3dxIniPath &&
      normalized.d3dx_ini_path === autoD3dxIniPath
    ) {
      normalized.d3dx_ini_path = '';
    }

    return normalized;
  };

  const refreshMigotoAutoPathDetection = async (
    config: Partial<MigotoGameConfig> = migotoConfig,
  ) => {
    const normalized = normalizeMigotoConfig(config);
    const seq = ++migotoPathDetectSeq;

    if (!normalized.migoto_path) {
      if (seq === migotoPathDetectSeq) {
        detectedMigotoImporterFolder.value = {
          basePath: '',
          importer: normalized.importer,
          folder: '',
        };
      }
      return '';
    }

    const folder = await resolveMigotoImporterFolder({
      basePath: normalized.migoto_path,
      importer: normalized.importer,
      pathExistsAt: pathExists,
      joinPath,
    });
    if (seq === migotoPathDetectSeq) {
      detectedMigotoImporterFolder.value = {
        basePath: normalized.migoto_path,
        importer: normalized.importer,
        folder,
      };
    }
    return folder;
  };

  const effectiveMigotoPaths = computed(() =>
    buildEffectiveMigotoPaths(migotoConfig),
  );

  const isMigotoPathOverridden = (field: MigotoPathOverrideField) =>
    trimMigotoPathValue(migotoConfig[field]).length > 0;

  const getMigotoPathDisplayValue = (field: MigotoPathOverrideField) =>
    isMigotoPathOverridden(field)
      ? trimMigotoPathValue(migotoConfig[field])
      : effectiveMigotoPaths.value[field];

  const restoreMigotoPathAuto = (field: MigotoPathOverrideField) => {
    migotoConfig[field] = '';
  };

  const getMigotoAutoPathDescription = (field: MigotoPathOverrideField) => {
    const path = effectiveMigotoPaths.value[field];
    return path
      ? t('settings.migoto.autoDerivedHint', { path })
      : t('settings.migoto.autoDerivedPending');
  };

  const refreshMigotoGamesList = async () => {
    const games = await scanGames();
    migotoGamesList.value = games;
    if (!migotoSelectedGame.value && games.length > 0) {
      migotoSelectedGame.value = games[0].name;
    }
  };

  const loadMigotoGameConfig = async (): Promise<boolean> => {
    const gameName = migotoSelectedGame.value;
    if (!gameName) return false;
    try {
      const data = await loadGameConfig(gameName);
      const saved = data?.other?.migoto;
      if (saved && typeof saved === 'object') {
        const normalized = normalizeMigotoConfig(saved, gameName);
        Object.assign(migotoConfig, normalized);
        const autoImporterFolder = await refreshMigotoAutoPathDetection(normalized);
        Object.assign(
          migotoConfig,
          collapseMigotoAutoOverrides(normalized, autoImporterFolder),
        );
        return true;
      }

      const normalized = normalizeMigotoConfig({}, gameName);
      Object.assign(migotoConfig, normalized);
      await refreshMigotoAutoPathDetection(normalized);
      return false;
    } catch {
      const normalized = normalizeMigotoConfig({}, gameName);
      Object.assign(migotoConfig, normalized);
      await refreshMigotoAutoPathDetection(normalized);
      return false;
    }
  };

  const saveMigotoGameConfig = async () => {
    const gameName = migotoSelectedGame.value;
    if (!gameName || isMigotoSaving.value) return;
    try {
      isMigotoSaving.value = true;
      const data = await loadGameConfig(gameName);
      data.other = data.other || {};
      const normalized = normalizeMigotoConfig({ ...migotoConfig }, gameName);
      const autoImporterFolder = await refreshMigotoAutoPathDetection(normalized);
      const collapsed = collapseMigotoAutoOverrides(
        normalized,
        autoImporterFolder,
      );
      Object.assign(migotoConfig, collapsed);
      data.other.migoto = { ...collapsed };
      await saveGameConfig(gameName, data);
      await toast(
        'success',
        t('gamesettingsmodal.message.success.title'),
        t('settings.migoto.saveSuccess'),
      );
    } catch (error) {
      await toast(
        'error',
        t('gamesettingsmodal.message.error.title'),
        `${t('settings.migoto.saveFailed')}: ${error}`,
      );
    } finally {
      isMigotoSaving.value = false;
    }
  };

  const selectMigotoPath = async (field: MigotoSelectablePathField) => {
    const fileFields: MigotoSelectablePathField[] = [
      'd3dx_ini_path',
      'bridge_exe_path',
    ];
    const isDir = !fileFields.includes(field);
    const filterMap: Partial<
      Record<MigotoSelectablePathField, { name: string; extensions: string[] }[]>
    > = {
      d3dx_ini_path: [{ name: 'INI', extensions: ['ini'] }],
      bridge_exe_path: [{ name: 'EXE', extensions: ['exe'] }],
    };
    const selected = await openFileDialog({
      directory: isDir,
      multiple: false,
      title: isDir
        ? t('gamesettingsmodal.selectfolder')
        : t('gamesettingsmodal.selectfile'),
      filters: filterMap[field],
    });
    if (selected && typeof selected === 'string') {
      migotoConfig[field] = selected;
    }
  };

  const showMigotoRiskRestatement = async () => {
    await showMessage(
      [
        tr(
          'settings.migoto.riskDialogLead',
          '请在启用或继续使用前再次确认以下事项：',
        ),
        '',
        migotoRiskStatement,
        '',
        tr(
          'settings.migoto.riskDialogTermsHint',
          '建议先阅读文档中心中的《服务条款》与风险声明。',
        ),
      ].join('\n'),
      {
        title: tr(
          'settings.migoto.riskDialogTitle',
          '《服务条款》与风险声明',
        ),
        kind: 'warning',
      },
    );
  };

  const handleMigotoGlobalToggle = async (
    nextValue: string | number | boolean,
  ) => {
    const enabled = Boolean(nextValue);
    if (
      isMigotoTogglePending.value ||
      enabled === appSettings.migotoEnabled
    ) {
      return;
    }

    if (!enabled) {
      appSettings.migotoEnabled = false;
      return;
    }

    try {
      isMigotoTogglePending.value = true;
      const confirmed = await askConfirm(
        tr(
          'settings.migoto.enableRiskConfirm',
          '严重警告：3DMigoto 属于实验性功能，可能导致注入失败、画面异常、闪退、卡顿、配置损坏，甚至带来账号或环境风险。继续启用即表示你已理解并愿意自行承担全部后果，开发者不承担任何责任。是否继续启用？',
        ),
        {
          title: tr('settings.migoto.enableRiskTitle', '3DMigoto 风险确认'),
          kind: 'warning',
          okLabel: tr(
            'settings.migoto.enableRiskAccept',
            '我已理解并承担风险',
          ),
          cancelLabel: tr('settings.migoto.enableRiskCancel', '取消启用'),
        },
      );
      appSettings.migotoEnabled = confirmed;
    } finally {
      isMigotoTogglePending.value = false;
    }
  };

  const ensureMigotoLoaded = async () => {
    if (migotoLoaded.value) return;
    await refreshMigotoGamesList();
    migotoLoaded.value = true;
  };

  watch(
    [() => migotoConfig.migoto_path, () => migotoConfig.importer],
    () => {
      void refreshMigotoAutoPathDetection();
    },
    { immediate: true },
  );

  watch(
    () => migotoSelectedGame.value,
    async () => {
      await loadMigotoGameConfig();
    },
  );

  return {
    ensureMigotoLoaded,
    globalMigotoEnabled,
    handleMigotoGlobalToggle,
    isMigotoImporterLocked,
    isMigotoInjectionLocked,
    isMigotoPathOverridden,
    isMigotoSaving,
    isMigotoTogglePending,
    isMigotoWwmi,
    getMigotoAutoPathDescription,
    getMigotoPathDisplayValue,
    loadMigotoGameConfig,
    migotoAvailableImporterOptions,
    migotoConfig,
    migotoGamesList,
    migotoImporterHint,
    migotoInjectionHint,
    migotoRiskStatement,
    migotoSelectedGame,
    migotoStartArgsHint,
    restoreMigotoPathAuto,
    saveMigotoGameConfig,
    selectMigotoPath,
    showMigotoRiskRestatement,
  };
}
