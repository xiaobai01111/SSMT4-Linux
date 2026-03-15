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
import { sharedBridgeMigotoDefaults } from '../shared/bridgeMigotoDefaults';
import type { BridgeMigotoFormModel } from '../shared/generated/bridgeMigotoContract';

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

export type MigotoGameConfig = BridgeMigotoFormModel;

const LOCKED_IMPORTER_REVISION_KEY = '__ssmt4_locked_defaults_revision';
const LOCKED_IMPORTER_REVISION = 1;

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
  use_hook: sharedBridgeMigotoDefaults.migoto.useHook,
  enforce_rendering: sharedBridgeMigotoDefaults.migoto.enforceRendering,
  enable_hunting: sharedBridgeMigotoDefaults.migoto.enableHunting,
  dump_shaders: sharedBridgeMigotoDefaults.migoto.dumpShaders,
  mute_warnings: sharedBridgeMigotoDefaults.migoto.muteWarnings,
  calls_logging: sharedBridgeMigotoDefaults.migoto.callsLogging,
  debug_logging: sharedBridgeMigotoDefaults.migoto.debugLogging,
  unsafe_mode: sharedBridgeMigotoDefaults.migoto.unsafeMode,
  process_timeout: sharedBridgeMigotoDefaults.game.processTimeout,
  migoto_path: '',
  importer_folder: '',
  mod_folder: '',
  shader_fixes_folder: '',
  d3dx_ini_path: '',
  bridge_exe_path: '',
  start_args: '',
  process_start_method: sharedBridgeMigotoDefaults.game.processStartMethod,
  process_priority: sharedBridgeMigotoDefaults.game.processPriority,
  xxmi_dll_init_delay: sharedBridgeMigotoDefaults.migoto.xxmiDllInitDelay,
  extra_libraries_enabled: sharedBridgeMigotoDefaults.extraLibraries.enabled,
  extra_libraries_paths: '',
  custom_launch_enabled: sharedBridgeMigotoDefaults.customLaunch.enabled,
  custom_launch_cmd: '',
  custom_launch_inject_mode: sharedBridgeMigotoDefaults.customLaunch.injectMode,
  pre_launch_enabled: sharedBridgeMigotoDefaults.shellCommand.enabled,
  pre_launch_cmd: '',
  pre_launch_wait: sharedBridgeMigotoDefaults.shellCommand.wait,
  post_load_enabled: sharedBridgeMigotoDefaults.shellCommand.enabled,
  post_load_cmd: '',
  post_load_wait: sharedBridgeMigotoDefaults.shellCommand.wait,
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

  const stripRequiredMigotoStartArgs = (
    value: string | null | undefined,
    requiredArgs: string[],
  ) => {
    const normalizedArgs = splitMigotoStartArgs(value);
    if (!requiredArgs.length) {
      return normalizedArgs.join(' ');
    }

    return normalizedArgs
      .filter(
        (arg) =>
          !requiredArgs.some(
            (requiredArg) =>
              arg.toLowerCase() === requiredArg.toLowerCase(),
          ),
      )
      .join(' ');
  };

  const normalizeMigotoConfig = (
    config: Partial<MigotoGameConfig> & Record<string, unknown>,
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

    const lockedImporterRevision =
      typeof config[LOCKED_IMPORTER_REVISION_KEY] === 'number'
        ? Number(config[LOCKED_IMPORTER_REVISION_KEY])
        : 0;

    const shouldHealLockedImporterDefaults = Boolean(
      requiredImporter &&
        normalized.importer === requiredImporter &&
        importerBehavior.injectionLocked &&
        lockedImporterRevision < LOCKED_IMPORTER_REVISION,
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

  const migotoRequiredStartArgs = computed(() =>
    migotoImporterBehavior.value.requiredStartArgs.join(' '),
  );

  const migotoEditableStartArgs = computed({
    get: () =>
      stripRequiredMigotoStartArgs(
        migotoConfig.start_args,
        migotoImporterBehavior.value.requiredStartArgs,
      ),
    set: (value: string) => {
      migotoConfig.start_args = normalizeMigotoStartArgs(
        value,
        migotoImporterBehavior.value.requiredStartArgs,
      );
    },
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
    const supportedGames = games.filter((game) => game.migotoSupported);
    migotoGamesList.value = supportedGames;
    if (!supportedGames.some((game) => game.name === migotoSelectedGame.value)) {
      migotoSelectedGame.value = supportedGames[0]?.name || '';
    }
  };

  const loadMigotoGameConfig = async (): Promise<boolean> => {
    const gameName = migotoSelectedGame.value;
    if (!gameName) return false;
    try {
      const data = await loadGameConfig(gameName);
      const saved = data?.other?.migoto;
      if (saved && typeof saved === 'object') {
        const normalized = normalizeMigotoConfig(
          saved as Partial<MigotoGameConfig> & Record<string, unknown>,
          gameName,
        );
        await refreshMigotoAutoPathDetection(normalized);
        Object.assign(migotoConfig, normalized);
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
      const normalized = normalizeMigotoConfig(
        {
          ...(migotoConfig as Record<string, unknown>),
          [LOCKED_IMPORTER_REVISION_KEY]: LOCKED_IMPORTER_REVISION,
        },
        gameName,
      );
      await refreshMigotoAutoPathDetection(normalized);
      Object.assign(migotoConfig, normalized);

      const existingMigoto =
        data.other.migoto &&
        typeof data.other.migoto === 'object' &&
        !Array.isArray(data.other.migoto)
          ? (data.other.migoto as Record<string, unknown>)
          : {};
      data.other.migoto = {
        ...existingMigoto,
        ...normalized,
        [LOCKED_IMPORTER_REVISION_KEY]: LOCKED_IMPORTER_REVISION,
      };
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
    migotoEditableStartArgs,
    migotoRiskStatement,
    migotoRequiredStartArgs,
    migotoSelectedGame,
    migotoStartArgsHint,
    restoreMigotoPathAuto,
    saveMigotoGameConfig,
    selectMigotoPath,
    showMigotoRiskRestatement,
  };
}
