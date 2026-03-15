<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  getGameState,
  getLauncherInstallerState,
  getDefaultGameFolder,
  getGameLauncherApi,
  listGamePresetsForInfo,
  getGameProtectionInfo,
  applyGameProtection,
  checkGameProtectionStatus,
  getChannelProtectionStatus,
  setChannelProtectionMode,
  restoreTelemetry,
  askConfirm,
  openFileDialog,
  showMessage,
  loadGameConfig,
  saveGameConfig,
  type GameState,
  type GameProtectionInfo,
  type PresetCatalogItem,
  type ChannelProtectionStatus,
} from '../api';
import { appSettings, loadGames, refreshGameUpdateState } from '../store';
import {
  getTaskForGame,
  isActiveFor,
  isPausedFor,
  fireDownload,
  fireLauncherInstallerDownload,
  fireVerify,
  fireRepair,
  pauseActive,
  resumePaused,
  cancelActive,
  getRepairableFailuresFor,
} from '../downloadStore';
import { useGameDownloadModalLifecycle } from '../composables/useGameDownloadModalLifecycle';

const { t, te } = useI18n();
const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);

const props = defineProps<{
  modelValue: boolean;
  gameName: string;
  displayName: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
  (e: 'gameConfigured'): void;
}>();

const gameState = ref<GameState | null>(null);
const isChecking = ref(false);
const launcherApi = ref('');
const gameFolder = ref('');
const error = ref('');
const gamePreset = ref('');
const isSupported = ref(false);
const downloadMode = ref<'full_game' | 'launcher_installer'>('full_game');
const statusMsg = ref('');
const installerOfficialUrl = ref('');
const protectionInfo = ref<GameProtectionInfo | null>(null);
const protectionApplied = ref(false);
const protectionEnforceAtLaunch = ref(false);
const isProtectionBusy = ref(false);
const channelProtection = ref<ChannelProtectionStatus | null>(null);
const isChannelModeBusy = ref(false);

// 语言包选择
interface AudioLangOption {
  code: string;
  label: string;
}
const availableLanguages = ref<AudioLangOption[]>([]);
const selectedLanguages = ref<string[]>([]);

// 服务器选择
interface ServerOption {
  id: string;
  label: string;
  launcherApi: string;
  bizPrefix: string;
}
const availableServers = ref<ServerOption[]>([]);
const selectedServer = ref<ServerOption | null>(null);
const isLauncherInstallerMode = computed(() => downloadMode.value === 'launcher_installer');
const loadSeq = ref(0);
type RuntimeChannelMode = 'init' | 'protected' | 'unknown';

const toFiniteNumber = (value: unknown): number | null => {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
};

const channelCurrentValue = computed(() => toFiniteNumber(channelProtection.value?.channel?.currentValue));
const channelInitValue = computed(() => toFiniteNumber(channelProtection.value?.channel?.initValue) ?? 19);
const channelProtectedValue = computed(() => toFiniteNumber(channelProtection.value?.channel?.protectedValue) ?? 205);
const currentRuntimeChannelMode = computed<RuntimeChannelMode>(() => {
  const current = channelCurrentValue.value;
  if (current === null) return 'unknown';
  if (current === channelProtectedValue.value) return 'protected';
  if (current === channelInitValue.value) return 'init';
  return 'unknown';
});
const runtimeChannelModeLabel = computed(() => {
  switch (currentRuntimeChannelMode.value) {
    case 'protected':
      return tr('gamedownload.channel.modeProtected', `游戏(${channelProtectedValue.value})`).replace('{value}', String(channelProtectedValue.value));
    case 'init':
      return tr('gamedownload.channel.modeInit', `初始化(${channelInitValue.value})`).replace('{value}', String(channelInitValue.value));
    default:
      return tr('gamedownload.common.unknown', '未知');
  }
});

const currentTask = computed(() => getTaskForGame(props.gameName));
const hasCurrentGameActiveTask = computed(() => isActiveFor(props.gameName));
const runtimeChannelPillLabel = computed(() => {
  switch (currentRuntimeChannelMode.value) {
    case 'protected':
      return tr('gamedownload.channel.pillProtected', '游戏模式');
    case 'init':
      return tr('gamedownload.channel.pillInit', '初始化模式');
    default:
      return tr('gamedownload.channel.pillUnknown', '未知模式');
  }
});

const protectionStatusLabel = computed(() => {
  if (!protectionInfo.value?.hasProtections) return tr('gamedownload.protection.none', '该游戏暂无可用防护');
  if (protectionEnforceAtLaunch.value) {
    return protectionApplied.value ? tr('gamedownload.protection.enabledEnforced', '防护状态：已启用（强制）') : tr('gamedownload.protection.disabledEnforced', '防护状态：未启用（将阻止启动）');
  }
  return protectionApplied.value ? tr('gamedownload.protection.enabled', '防护状态：已启用') : tr('gamedownload.protection.disabled', '防护状态：未启用');
});

const protectionStatusClass = computed(() => {
  if (!protectionInfo.value?.hasProtections) return 'neutral';
  return protectionApplied.value ? 'enabled' : 'disabled';
});

const normalizePresetKey = (value: string): string => value.trim().toLowerCase().replace(/[_\s-]+/g, '');

const WUTHERING_PRESET_KEYS = new Set(['wutheringwaves', 'wwmi', 'wuwa']);

const isWutheringPreset = (value: string): boolean => WUTHERING_PRESET_KEYS.has(normalizePresetKey(value));

const hideDisableProtectionButton = computed(() => isWutheringPreset(getProtectionPreset()));

const canonicalPreset = (value: string): string => {
  const trimmed = value.trim();
  if (!trimmed) return '';
  if (isWutheringPreset(trimmed)) return 'WutheringWaves';
  return trimmed;
};

const getProtectionPreset = () => canonicalPreset(gamePreset.value || props.gameName);

const normalizePathForCompare = (value: string): string =>
  value.replace(/\\/g, '/').replace(/\/+/g, '/').replace(/\/$/, '');

const trimFolderPart = (value: string): string =>
  value.trim().replace(/^[\\/]+|[\\/]+$/g, '');

const parentDir = (value: string): string => {
  const normalized = normalizePathForCompare(value);
  const idx = normalized.lastIndexOf('/');
  if (idx <= 0) return '';
  return normalized.slice(0, idx);
};

const findPresetCatalog = (catalog: PresetCatalogItem[], key: string): PresetCatalogItem | null => {
  const target = key.trim().toLowerCase();
  if (!target) return null;
  return (
    catalog.find(
      (item) =>
        item.id.trim().toLowerCase() === target ||
        item.legacyIds.some((alias) => alias.trim().toLowerCase() === target),
    ) || null
  );
};

interface LauncherApiConfig {
  defaultFolder: string;
  downloadMode: 'full_game' | 'launcher_installer';
  servers: ServerOption[];
  audioLanguages?: AudioLangOption[];
}

const isHoyoverseApi = (api: string): boolean =>
  api.includes('mihoyo.com') || api.includes('hoyoverse.com');
const isSnowbreakApi = (api: string): boolean =>
  api.includes('amazingseasuncdn.com') || api.includes('snowbreak');

const currentBizPrefix = (): string => (selectedServer.value?.bizPrefix || '').trim();
const resolvedBizPrefix = (): string => {
  const api = launcherApi.value.trim();
  if (api && availableServers.value.length > 0) {
    const matched = availableServers.value.find((s) => s.launcherApi.trim() === api);
    if (matched?.bizPrefix) return matched.bizPrefix.trim();
  }
  return currentBizPrefix();
};

const ensureBizPrefixReady = (): boolean => {
  if (!isHoyoverseApi(launcherApi.value)) return true;
  if (resolvedBizPrefix()) return true;
  error.value = tr('gamedownload.messages.bizPrefixMissing', '当前 HoYoverse 服务器缺少 biz_prefix，请检查游戏预设配置');
  return false;
};

const close = () => {
  emit('update:modelValue', false);
};

const ensureRiskAcknowledged = async () => {
  if (appSettings.tosRiskAcknowledged) return true;

  const accepted = await askConfirm(
    tr('gamedownload.messages.tosPrimary', '本启动器为非官方工具，与游戏厂商无关。\n\n在 Linux/Wine/Proton 环境运行游戏，可能被反作弊误判，存在账号处罚（包括封禁）风险。\n\n是否确认你已理解并愿意自行承担风险？'),
    {
      title: tr('gamedownload.messages.title.tosRisk', 'ToS / 封禁风险提示'),
      kind: 'warning',
      okLabel: tr('gamedownload.messages.ok.riskUnderstood', '我已理解风险'),
      cancelLabel: tr('gamedownload.messages.cancel.cancel', '取消'),
    }
  );
  if (!accepted) return false;

  const second = await askConfirm(
    tr('gamedownload.messages.tosSecondary', '请再次确认：继续使用即表示你了解这是非官方方案，且可能导致账号风险。'),
    {
      title: tr('gamedownload.messages.title.secondConfirm', '二次确认'),
      kind: 'warning',
      okLabel: tr('gamedownload.messages.ok.confirmContinue', '确认继续'),
      cancelLabel: tr('gamedownload.messages.cancel.back', '返回'),
    }
  );
  if (!second) return false;

  appSettings.tosRiskAcknowledged = true;
  return true;
};

const refreshProtectionStatus = async () => {
  try {
    const preset = getProtectionPreset();
    const info = await getGameProtectionInfo(preset);
    protectionInfo.value = info;

    if (!info?.hasProtections) {
      protectionApplied.value = true;
      channelProtection.value = null;
      return;
    }

    const [status, channel] = await Promise.all([
      checkGameProtectionStatus(preset, gameFolder.value || undefined),
      getChannelProtectionStatus(preset, gameFolder.value || undefined),
    ]);
    protectionApplied.value = !!status?.enabled;
    protectionEnforceAtLaunch.value = status?.enforceAtLaunch !== false;
    channelProtection.value = channel;
  } catch (e) {
    protectionInfo.value = null;
    protectionApplied.value = false;
    protectionEnforceAtLaunch.value = false;
    channelProtection.value = null;
  }
};

const setChannelMode = async (mode: 'init' | 'protected') => {
  if (isChannelModeBusy.value) return;
  if (!gameFolder.value) {
    await showMessage(tr('gamedownload.messages.needGameFolder', '请先选择游戏安装目录'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
    return;
  }
  try {
    isChannelModeBusy.value = true;
    const preset = getProtectionPreset();
    channelProtection.value = await setChannelProtectionMode(preset, mode, gameFolder.value);
    await refreshProtectionStatus();
    await showMessage(
      mode === 'init' ? tr('gamedownload.messages.switchInitDone', '已切换到初始化模式 (KR_ChannelId=19)') : tr('gamedownload.messages.switchProtectedDone', '已切换到联机模式 (KR_ChannelId=205)'),
      { title: tr('gamedownload.channel.title', '渠道模式'), kind: 'info' },
    );
  } catch (e) {
    await showMessage(tr('gamedownload.messages.switchModeFailed', `切换渠道模式失败: ${e}`).replace('{error}', String(e)), { title: tr('gamedownload.messages.title.error', '错误'), kind: 'error' });
  } finally {
    isChannelModeBusy.value = false;
  }
};

// 打开时自动加载：检测是否支持自动下载 + 读取配置
const loadState = async () => {
  if (!props.gameName) return;
  const seq = ++loadSeq.value;
  const isStale = () => seq !== loadSeq.value || !props.modelValue;
  error.value = '';
  statusMsg.value = tr('gamedownload.status.loadingConfig', '正在加载配置...');
  selectedServer.value = null;
  availableServers.value = [];
  availableLanguages.value = [];
  selectedLanguages.value = [];

  // 1. 先尝试从 Config.json 读取 GamePreset，失败则直接用 gameName
  let preset = canonicalPreset(props.gameName);
  let savedFolder = '';
  try {
    const config = await loadGameConfig(props.gameName);
    const legacyPreset =
      typeof (config as { GamePreset?: unknown }).GamePreset === 'string'
        ? (config as { GamePreset?: string }).GamePreset
        : '';
    preset = canonicalPreset(legacyPreset || config.basic?.gamePreset || props.gameName);
    // 恢复之前保存的下载配置（保存在 config.other 下）
    const other = config.other || {};
    launcherApi.value = other.launcherApi || '';
    savedFolder = other.gameFolder || '';
  } catch (e) {
  }
  if (isStale()) return;
  gamePreset.value = preset;

  // 2. 并行：刷新防护状态 + 检测是否支持自动下载
  const [, launcherInfo] = await Promise.all([
    refreshProtectionStatus(),
    getGameLauncherApi(preset),
  ]);
  if (isStale()) return;
  const knownApi: LauncherApiConfig | null = launcherInfo.supported
    ? {
        defaultFolder: launcherInfo.defaultFolder || '',
        downloadMode: launcherInfo.downloadMode || 'full_game',
        servers: (launcherInfo.servers || []).map((s) => ({
          id: s.id,
          label: s.label,
          launcherApi: s.launcherApi,
          bizPrefix: s.bizPrefix || '',
        })),
        audioLanguages: launcherInfo.audioLanguages,
      }
    : null;
  isSupported.value = !!knownApi && knownApi.servers.length > 0;
  downloadMode.value = knownApi?.downloadMode || 'full_game';
  installerOfficialUrl.value = '';

  // 设置可用服务器列表
  availableServers.value = knownApi?.servers || [];
  if (availableServers.value.length > 0) {
    const savedApi = launcherApi.value.trim();
    const matched = savedApi
      ? availableServers.value.find((s) => s.launcherApi.trim() === savedApi) || null
      : null;
    selectedServer.value = matched || availableServers.value[0];
    if (selectedServer.value) {
      launcherApi.value = selectedServer.value.launcherApi;
    }
  }

  // 设置可用语言包
  availableLanguages.value = knownApi?.audioLanguages || [];
  if (availableLanguages.value.length > 0 && selectedLanguages.value.length === 0) {
    selectedLanguages.value = ['zh-cn'];
  }

  if (!knownApi || knownApi.servers.length === 0) {
    statusMsg.value = '';
    return;
  }

  // 3. 自动填充 launcher API（从当前选中的服务器获取）
  if (!launcherApi.value && selectedServer.value) {
    launcherApi.value = selectedServer.value.launcherApi;
  }

  // 4. 始终从后端获取最新默认目录（跟随 dataDir 变化）
  try {
    const baseDir = await getDefaultGameFolder(props.gameName);
    if (isStale()) return;
    const defaultFolderPart = trimFolderPart(knownApi.defaultFolder);
    const defaultFolder = defaultFolderPart ? `${baseDir}/${defaultFolderPart}` : baseDir;
    const defaultNorm = normalizePathForCompare(defaultFolder);

    let legacyDefaults = new Set<string>();
    try {
      const catalog = await listGamePresetsForInfo();
      const presetCatalog = findPresetCatalog(catalog, preset);
      if (presetCatalog?.legacyIds?.length) {
        const gamesRoot = parentDir(baseDir);
        for (const alias of presetCatalog.legacyIds) {
          const aliasKey = alias.trim();
          if (!aliasKey || !gamesRoot) continue;
          const legacyBase = `${gamesRoot}/${aliasKey}`;
          const legacyFolder = defaultFolderPart ? `${legacyBase}/${defaultFolderPart}` : legacyBase;
          legacyDefaults.add(normalizePathForCompare(legacyFolder));
        }
      }
    } catch (e) {
      void e;
    }

    const savedNorm = normalizePathForCompare(savedFolder);
    const looksLikeOldDefault =
      !!savedNorm && (savedNorm.includes('/.local/share/ssmt4/') || legacyDefaults.has(savedNorm));

    // 仅当用户没有手动设置过自定义目录时，使用默认路径
    // 判断依据：savedFolder 为空，或 savedFolder 是旧的默认路径格式
    if (!savedFolder || looksLikeOldDefault || savedNorm === defaultNorm) {
      gameFolder.value = defaultFolder;
      if (savedNorm && savedNorm !== defaultNorm) {
        await saveDownloadConfig();
      }
    } else {
      gameFolder.value = savedFolder;
    }
  } catch (e) {
    gameFolder.value = savedFolder || '';
  }
  if (isStale()) return;

  statusMsg.value = '';

  // 5. 自动检查游戏状态
  try {
    await checkState();
  } catch {}

};

const onServerChange = async (server: ServerOption) => {
  selectedServer.value = server;
  launcherApi.value = server.launcherApi;
  gameState.value = null;
  await checkState();
};

const checkState = async () => {
  if (!launcherApi.value || !gameFolder.value) {
    error.value = tr('gamedownload.messages.needLauncherAndFolder', '请先配置启动器 API 和游戏安装目录');
    return;
  }
  if (!isLauncherInstallerMode.value && !ensureBizPrefixReady()) return;
  isChecking.value = true;
  error.value = '';
  statusMsg.value = tr('gamedownload.status.checkingState', '正在检查游戏状态...');
  try {
    if (isLauncherInstallerMode.value) {
      const state = await getLauncherInstallerState(
        launcherApi.value,
        gameFolder.value,
        gamePreset.value || props.gameName,
      );
      installerOfficialUrl.value = state.installer_url || '';
      gameState.value = {
        state: state.state,
        local_version: state.local_version,
        remote_version: state.remote_version,
        supports_incremental: false,
      };
    } else {
      installerOfficialUrl.value = '';
      const biz = resolvedBizPrefix() || undefined;
      gameState.value = await getGameState(launcherApi.value, gameFolder.value, biz);
    }
    await refreshProtectionStatus();
    statusMsg.value = '';
  } catch (e: unknown) {
    error.value = tr('gamedownload.messages.checkStateFailed', `检查状态失败: ${e}`).replace('{error}', String(e));
    statusMsg.value = '';
  } finally {
    isChecking.value = false;
  }
};

const startDownload = async () => {
  if (isPaused.value) {
    await resumeDownload();
    return;
  }
  if (!launcherApi.value || !gameFolder.value) {
    await showMessage(tr('gamedownload.messages.needSourceAndFolderForDownload', '请先配置下载源和安装目录，再开始下载。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
    return;
  }
  if (hasCurrentGameActiveTask.value) {
    await showMessage(tr('gamedownload.messages.taskRunning', '当前该游戏已有下载/校验任务正在进行，请稍候。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
    return;
  }
  if (!isLauncherInstallerMode.value && !ensureBizPrefixReady()) return;
  if (!(await ensureRiskAcknowledged())) return;
  error.value = '';

  if (isLauncherInstallerMode.value) {
    try {
      await fireLauncherInstallerDownload({
        gameName: props.gameName,
        gamePreset: gamePreset.value || props.gameName,
        displayName: props.displayName,
        launcherApi: launcherApi.value,
        gameFolder: gameFolder.value,
        isUpdate: gameState.value?.state === 'needupdate',
      });
    } catch (e) {
      await showMessage(
        tr('gamedownload.messages.taskRunning', '当前该游戏已有下载/校验任务正在进行，请稍候。'),
        { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' },
      );
    }
    return;
  }

  try {
    await fireDownload({
      gameName: props.gameName,
      displayName: props.displayName,
      launcherApi: launcherApi.value,
      gameFolder: gameFolder.value,
      languages: selectedLanguages.value.length > 0 ? [...selectedLanguages.value] : undefined,
      bizPrefix: resolvedBizPrefix() || undefined,
      isUpdate: gameState.value?.state === 'needupdate',
    });
  } catch (e) {
    await showMessage(
      tr('gamedownload.messages.taskRunning', '当前该游戏已有下载/校验任务正在进行，请稍候。'),
      { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' },
    );
  }
};

const startVerify = async () => {
  if (isPaused.value) {
    await resumeDownload();
    return;
  }
  if (isLauncherInstallerMode.value) return;
  if (!launcherApi.value || !gameFolder.value) {
    await showMessage(tr('gamedownload.messages.needSourceAndFolderForVerify', '请先配置下载源和安装目录，再执行校验。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
    return;
  }
  if (hasCurrentGameActiveTask.value) {
    await showMessage(tr('gamedownload.messages.taskRunning', '当前该游戏已有下载/校验任务正在进行，请稍候。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
    return;
  }
  if (!gameState.value?.local_version) {
    await showMessage('未识别到本地版本号，将按远程清单尝试校验当前目录文件。', {
      title: tr('gamedownload.messages.title.info', '提示'),
      kind: 'info',
    });
  }
  if (!ensureBizPrefixReady()) return;

  try {
    await fireVerify({
      gameName: props.gameName,
      displayName: props.displayName,
      launcherApi: launcherApi.value,
      gameFolder: gameFolder.value,
      bizPrefix: resolvedBizPrefix() || undefined,
    });
  } catch (e) {
    await showMessage(
      tr('gamedownload.messages.taskRunning', '当前该游戏已有下载/校验任务正在进行，请稍候。'),
      { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' },
    );
  }
};

const startRepair = async () => {
  if (isPaused.value) {
    await resumeDownload();
    return;
  }
  if (isLauncherInstallerMode.value) return;
  if (isHoyoverseApi(launcherApi.value) || isSnowbreakApi(launcherApi.value)) {
    await showMessage(tr('gamedownload.messages.repairUnsupported', '当前游戏暂不支持按异常列表单文件修复。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
    return;
  }
  if (!launcherApi.value || !gameFolder.value) {
    await showMessage(tr('gamedownload.messages.needSourceAndFolderForRepair', '请先配置下载源和安装目录，再执行修复。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
    return;
  }
  if (hasCurrentGameActiveTask.value) {
    await showMessage(tr('gamedownload.messages.taskRunningAny', '当前该游戏已有下载/校验/修复任务正在进行，请稍候。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
    return;
  }
  const files = repairableFailures.value;
  if (files.length === 0) {
    await showMessage(tr('gamedownload.messages.noRepairableFiles', '当前没有可修复的异常文件，请先执行一次校验。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'info' });
    return;
  }
  if (!ensureBizPrefixReady()) return;
  try {
    await fireRepair({
      gameName: props.gameName,
      displayName: props.displayName,
      launcherApi: launcherApi.value,
      gameFolder: gameFolder.value,
      bizPrefix: resolvedBizPrefix() || undefined,
      files,
    });
  } catch (e) {
    await showMessage(
      tr('gamedownload.messages.taskRunningAny', '当前该游戏已有下载/校验/修复任务正在进行，请稍候。'),
      { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' },
    );
  }
};

// 手动应用游戏防护（渠道参数切换/遥测处理）
const applyProtectionAfterDownload = async () => {
  if (isProtectionBusy.value) return;
  try {
    isProtectionBusy.value = true;
    if (!(await ensureRiskAcknowledged())) return;
    const preset = getProtectionPreset();
    const info = await getGameProtectionInfo(preset);
    protectionInfo.value = info;
    if (!info?.hasProtections) return;

    const protNames = info.protections.map((p) => p.name).join('、');
    const confirmed = await askConfirm(
      tr('gamedownload.messages.protectionDetected', `检测到该游戏支持以下安全防护：\n\n${protNames}\n\n是否立即应用？`).replace('{protections}', protNames),
      { title: tr('gamedownload.protection.title', '游戏安全防护'), kind: 'warning', okLabel: tr('gamedownload.protection.apply', '应用防护'), cancelLabel: tr('gamedownload.common.skip', '跳过') }
    );
    if (confirmed) {
      await applyGameProtection(preset, gameFolder.value);
      await refreshProtectionStatus();
      if (protectionApplied.value) {
        await showMessage(tr('gamedownload.messages.protectionApplied', '游戏安全防护已成功应用！'), { title: tr('gamedownload.protection.doneTitle', '防护完成'), kind: 'info' });
      } else {
        const status = await checkGameProtectionStatus(preset, gameFolder.value || undefined);
        const missing = Array.isArray(status?.missing) && status.missing.length > 0
          ? status.missing.join('\n')
          : tr('gamedownload.common.unknown', '未知');
        await showMessage(
          tr('gamedownload.messages.protectionPartial', `防护未完全生效，仍有以下问题：\n${missing}`).replace('{missing}', missing),
          { title: tr('gamedownload.protection.partialTitle', '防护部分完成'), kind: 'warning' }
        );
      }
    }
  } catch (e) {
    await showMessage(tr('gamedownload.messages.protectionApplyFailed', `应用防护失败: ${e}`).replace('{error}', String(e)), { title: tr('gamedownload.messages.title.error', '错误'), kind: 'error' });
  } finally {
    isProtectionBusy.value = false;
  }
};

const disableProtection = async () => {
  if (isProtectionBusy.value) return;
  try {
    isProtectionBusy.value = true;
    const preset = getProtectionPreset();
    const yes = await askConfirm(
      tr('gamedownload.messages.disableProtectionConfirm', '禁用防护会恢复渠道参数为原始值并可能增加账号风险，确认继续？'),
      { title: tr('gamedownload.protection.disableTitle', '禁用防护'), kind: 'warning', okLabel: tr('gamedownload.protection.confirmDisable', '确认禁用'), cancelLabel: tr('gamedownload.messages.cancel.cancel', '取消') }
    );
    if (!yes) return;

    const result = await restoreTelemetry(preset, gameFolder.value || undefined);
    await refreshProtectionStatus();
    const channelTip = result?.channel?.message;
    await showMessage(
      channelTip ? tr('gamedownload.messages.disableProtectionWithTip', `已禁用域名/文件防护。\n${channelTip}`).replace('{tip}', channelTip) : tr('gamedownload.messages.disableProtectionDone', '已禁用防护'),
      { title: tr('gamedownload.protection.disabledTitle', '已禁用'), kind: 'info' },
    );
  } catch (e) {
    await showMessage(tr('gamedownload.messages.disableProtectionFailed', `禁用防护失败: ${e}`).replace('{error}', String(e)), { title: tr('gamedownload.messages.title.error', '错误'), kind: 'error' });
  } finally {
    isProtectionBusy.value = false;
  }
};

const cancelDownload = async () => {
  await cancelActive(props.gameName);
};

const pauseDownload = async () => {
  await pauseActive(props.gameName);
};

const resumeDownload = async () => {
  try {
    await resumePaused(props.gameName);
  } catch (e) {
    const message = String(e || '');
    if (message.includes('already running')) {
      await showMessage(
        tr('gamedownload.messages.taskRunningAny', '当前该游戏已有下载/校验/修复任务正在进行，请稍候。'),
        { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' },
      );
      return;
    }
    await showMessage(tr('gamedownload.messages.noTaskToResume', '当前没有可恢复的下载任务。'), { title: tr('gamedownload.messages.title.info', '提示'), kind: 'warning' });
  }
};

const selectGameExe = async () => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: tr('gamedownload.filePicker.executables', '可执行文件'), extensions: ['exe', 'sh', 'AppImage', 'desktop', '*'] }],
      title: tr('gamedownload.filePicker.selectExe', '选择游戏可执行文件')
    });
    if (selected && typeof selected === 'string') {
      const config = await loadGameConfig(props.gameName);
      config.other = config.other || {};
      config.other.gamePath = selected;
      await saveGameConfig(props.gameName, config);
      await showMessage(tr('gamedownload.messages.gamePathSet', '已设置游戏路径'), { title: tr('gamedownload.messages.title.success', '成功'), kind: 'info' });
      emit('gameConfigured');
      close();
    }
  } catch (e: unknown) {
    error.value = tr('gamedownload.messages.selectFileFailed', `选择文件失败: ${e}`).replace('{error}', String(e));
  }
};

const selectGameFolder = async () => {
  try {
    const selected = await openFileDialog({ directory: true, title: tr('gamedownload.filePicker.selectGameFolder', '选择游戏安装目录') });
    if (selected && typeof selected === 'string') {
      gameFolder.value = selected;
      // 自动保存用户选择的目录
      await saveDownloadConfig();
      // 用新目录重新检查游戏状态
      await checkState();
    }
  } catch (e) { console.error(e); }
};

const saveDownloadConfig = async () => {
  try {
    const config = await loadGameConfig(props.gameName);
    config.other = config.other || {};
    config.other.launcherApi = launcherApi.value;
    config.other.gameFolder = gameFolder.value;
    await saveGameConfig(props.gameName, config);
  } catch (e) { console.error(e); }
};

const copyOfficialLink = async () => {
  if (!installerOfficialUrl.value) return;
  try {
    await navigator.clipboard.writeText(installerOfficialUrl.value);
    await showMessage(tr('gamedownload.messages.installerLinkCopied', '官方启动器链接已复制'), { title: tr('gamedownload.messages.title.success', '成功'), kind: 'info' });
  } catch (e) {
    await showMessage(tr('gamedownload.messages.copyFailed', `复制失败: ${e}`).replace('{error}', String(e)), { title: tr('gamedownload.messages.title.error', '错误'), kind: 'error' });
  }
};

// === 格式化辅助 ===
const formatSize = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
};
const formatEta = (seconds: number) => {
  if (seconds <= 0) return tr('gamedownload.common.calculating', '计算中...');
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  return `${m}:${s.toString().padStart(2, '0')}`;
};

const progress = computed(() => {
  if (isActiveFor(props.gameName)) return currentTask.value?.progress || null;
  return null;
});

const isPaused = computed(() => isPausedFor(props.gameName));

const progressPercent = computed(() => {
  if (!progress.value || progress.value.total_size === 0) return 0;
  return Math.round((progress.value.finished_size / progress.value.total_size) * 100);
});

const stateLabel = computed(() => {
  if (!gameState.value) return '';
  switch (gameState.value.state) {
    case 'startgame': return isLauncherInstallerMode.value ? tr('gamedownload.state.latestInstaller', '✓ 已是最新启动器版本') : tr('gamedownload.state.latestGame', '✓ 已是最新版本');
    case 'needinstall': return isLauncherInstallerMode.value ? tr('gamedownload.state.needInstaller', '需要下载官方启动器安装器') : tr('gamedownload.state.needInstall', '需要下载安装');
    case 'needupdate':
      return isLauncherInstallerMode.value
        ? tr('gamedownload.state.installerNeedUpdate', `启动器需要更新 (${gameState.value.local_version} → ${gameState.value.remote_version})`).replace('{local}', gameState.value.local_version || '').replace('{remote}', gameState.value.remote_version || '')
        : tr('gamedownload.state.needUpdate', `需要更新 (${gameState.value.local_version} → ${gameState.value.remote_version})`).replace('{local}', gameState.value.local_version || '').replace('{remote}', gameState.value.remote_version || '');
    case 'networkerror': return tr('gamedownload.state.networkError', '⚠ 网络错误，请检查网络连接');
    default: return String(gameState.value.state);
  }
});
const stateClass = computed(() => {
  if (!gameState.value) return '';
  switch (gameState.value.state) {
    case 'startgame': return 'state-ok';
    case 'needinstall': return 'state-install';
    case 'needupdate': return 'state-update';
    case 'networkerror': return 'state-error';
    default: return '';
  }
});

const canDownload = computed(() => {
  if (!gameState.value) return false;
  return gameState.value.state === 'needinstall' || gameState.value.state === 'needupdate';
});

const canShowPrimaryDownload = computed(() => {
  // 全量/增量游戏下载模式：始终保留下载入口（已安装后也支持“重新下载”）
  if (!isLauncherInstallerMode.value) return true;
  // 启动器安装器模式维持原有状态判断
  return canDownload.value;
});

const primaryDownloadLabel = computed(() => {
  if (isLauncherInstallerMode.value) {
    return gameState.value?.state === 'needupdate' ? tr('gamedownload.actions.updateInstaller', '更新官方启动器') : tr('gamedownload.actions.downloadInstaller', '下载官方启动器');
  }
  if (gameState.value?.state === 'needupdate') return tr('gamedownload.actions.startUpdate', '开始更新');
  if (gameState.value?.state === 'startgame') return tr('gamedownload.actions.redownload', '重新下载');
  return tr('gamedownload.actions.startDownload', '开始下载');
});

const canVerifyNow = computed(() => {
  if (isLauncherInstallerMode.value) return false;
  // 后端校验能力不强依赖 local_version，版本识别失败时也允许发起校验。
  return !!launcherApi.value && !!gameFolder.value;
});

const repairableFailures = computed(() =>
  getRepairableFailuresFor(
    props.gameName,
    gameFolder.value,
    launcherApi.value,
    resolvedBizPrefix() || undefined,
  ),
);

const canRepairNow = computed(() => {
  if (isLauncherInstallerMode.value) return false;
  if (isHoyoverseApi(launcherApi.value) || isSnowbreakApi(launcherApi.value)) return false;
  return repairableFailures.value.length > 0 && !!launcherApi.value && !!gameFolder.value;
});

const isWorking = computed(() => {
  const phase = currentTask.value?.phase;
  return isActiveFor(props.gameName) && (phase === 'downloading' || phase === 'verifying');
});
const isVerifyPhase = computed(() => currentTask.value?.phase === 'verifying' || progress.value?.phase === 'verify');
const workingPhase = computed(() => {
  if (!isActiveFor(props.gameName)) return '';
  if (currentTask.value?.phase === 'verifying') return tr('gamedownload.phase.verifying', '校验中');
  if (currentTask.value?.phase === 'downloading') {
    if (progress.value?.phase === 'install') return tr('gamedownload.phase.installing', '安装中');
    return tr('gamedownload.phase.downloading', '下载中');
  }
  return '';
});

const handleModalOpen = async () => {
  error.value = '';
  statusMsg.value = '';
  if (!isActiveFor(props.gameName)) {
    gameState.value = null;
  }
  await loadState();
};

const handleGameChange = async () => {
  error.value = '';
  statusMsg.value = '';
  gameState.value = null;
  await loadState();
};

useGameDownloadModalLifecycle({
  modelValue: () => props.modelValue,
  gameName: () => props.gameName,
  launcherApi,
  availableServers,
  selectedServer,
  currentTask: () => currentTask.value,
  onOpen: handleModalOpen,
  onGameChange: handleGameChange,
  onCompletedTask: async () => {
    await Promise.allSettled([
      checkState(),
      loadGames(),
      refreshGameUpdateState(props.gameName),
    ]);
  },
});
</script>

<template>
  <transition name="modal-fade">
    <div v-if="modelValue" class="dl-overlay" @click.self="close">
      <div class="dl-window glass-panel" data-onboarding="download-modal-root">
        <div class="dl-header">
          <span class="dl-title">{{ tr('gamedownload.title', '下载 / 安装游戏') }}</span>
          <div class="dl-close" @click="close">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
              fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </div>
        </div>

        <div class="dl-body">
          <div class="game-info">
            <span class="game-name">{{ displayName || gameName }}</span>
            <span v-if="isSupported" class="badge supported">{{ tr('gamedownload.badge.supported', '支持自动下载') }}</span>
            <span v-else class="badge manual">{{ tr('gamedownload.badge.manual', '手动安装') }}</span>
          </div>

          <div v-if="statusMsg" class="status-msg">{{ statusMsg }}</div>

          <template v-if="isSupported">
            <div v-if="availableServers.length > 1 && !isWorking" class="server-section">
              <label class="server-label">{{ tr('gamedownload.server.label', '服务器') }}</label>
              <div class="server-options">
                <button
                  v-for="srv in availableServers"
                  :key="srv.id"
                  class="server-btn"
                  :class="{ active: selectedServer?.id === srv.id }"
                  @click="onServerChange(srv)"
                >
                  {{ srv.label }}
                </button>
              </div>
            </div>

            <div v-if="gameState" class="state-card glass-card" :class="stateClass" data-onboarding="download-state-card">
              <div class="state-label">{{ stateLabel }}</div>
              <div class="state-versions" v-if="gameState.remote_version">
                <span v-if="gameState.local_version">{{ tr('gamedownload.state.local', '本地') }}: {{ gameState.local_version }}</span>
                <span>{{ tr('gamedownload.state.latest', '最新') }}: {{ gameState.remote_version }}</span>
              </div>
            </div>

            <div
              v-if="!isLauncherInstallerMode && channelProtection?.supported && channelProtection.channel?.required && !isWorking"
              class="channel-mode-card glass-card"
            >
              <div class="channel-mode-head">
                <span class="channel-mode-title">{{ tr('gamedownload.channel.title', '渠道模式') }}</span>
                <span
                  class="channel-mode-pill"
                  :class="{ ok: currentRuntimeChannelMode === 'protected', warn: currentRuntimeChannelMode !== 'protected' }"
                >
                  {{ runtimeChannelPillLabel }}
                </span>
              </div>
              <div class="channel-mode-meta">
                <span>{{ tr('gamedownload.channel.currentValue', '当前值') }}: {{ channelCurrentValue ?? tr('gamedownload.common.unknown', '未知') }}</span>
                <span>{{ tr('gamedownload.channel.targetValue', '目标值') }}: {{ channelProtection.channel?.expectedValue ?? tr('gamedownload.common.unknown', '未知') }}</span>
                <span>{{ tr('gamedownload.channel.currentMode', '当前模式') }}: {{ runtimeChannelModeLabel }}</span>
              </div>
              <div class="channel-mode-actions">
                <button class="action-btn sm" @click="setChannelMode('init')" :disabled="isChannelModeBusy || !gameFolder">
                  {{ tr('gamedownload.channel.switchInit', '切换初始化模式(19)') }}
                </button>
                <button class="action-btn sm highlight" @click="setChannelMode('protected')" :disabled="isChannelModeBusy || !gameFolder">
                  {{ tr('gamedownload.channel.switchProtected', '切换联机模式(205)') }}
                </button>
                <button class="action-btn sm" @click="setChannelMode('init')" :disabled="isChannelModeBusy || !gameFolder">
                  {{ tr('gamedownload.channel.restoreDefault', '恢复默认(19)') }}
                </button>
              </div>
              <p v-if="currentRuntimeChannelMode === 'init'" class="channel-mode-hint">
                {{ tr('gamedownload.channel.initHint', '当前处于初始化模式。完成首次初始化后，请手动切换到联机模式(205)。') }}
              </p>
              <p
                v-else-if="currentRuntimeChannelMode === 'unknown'"
                class="channel-mode-hint warning"
              >
                {{ tr('gamedownload.channel.unknownHint', '当前渠道值无法识别（既不是 19 也不是 205），建议重新应用当前模式。') }}
              </p>
            </div>

            <div v-if="!isWorking" class="install-dir-section" data-onboarding="download-install-dir">
              <label class="install-dir-label">{{ tr('gamedownload.installDir.label', '安装目录') }}</label>
              <div class="install-dir-row">
                <input v-model="gameFolder" type="text" class="dl-input" :placeholder="tr('gamedownload.installDir.placeholder', '选择游戏安装目录...')" />
                <button class="dir-btn" @click="selectGameFolder" :title="tr('gamedownload.installDir.selectTitle', '选择目录')">
                  <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
                    fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                  </svg>
                </button>
              </div>
              <p v-if="isLauncherInstallerMode" class="install-dir-hint">
                {{ tr('gamedownload.installDir.installerHint', '官方启动器安装器将下载到此目录，并自动写入游戏路径（可后续修改）。') }}
              </p>
              <p v-else class="install-dir-hint">{{ tr('gamedownload.installDir.gameHint', '游戏文件将下载到此目录，请确保有足够磁盘空间（约 130GB+）') }}</p>
            </div>

            <div v-if="!isLauncherInstallerMode && availableLanguages.length > 0 && !isWorking" class="lang-section">
              <label class="lang-label">{{ tr('gamedownload.language.label', '语音包（可多选）') }}</label>
              <div class="lang-options">
                <label
                  v-for="lang in availableLanguages"
                  :key="lang.code"
                  class="lang-checkbox"
                >
                  <input
                    type="checkbox"
                    :value="lang.code"
                    v-model="selectedLanguages"
                  />
                  <span class="lang-name">{{ lang.label }}</span>
                </label>
              </div>
              <p class="lang-hint" v-if="selectedLanguages.length === 0">
                <i class="el-icon-warning-outline mr-1"></i> {{ tr('gamedownload.language.noneSelectedHint', '未选择任何语音包，游戏将没有角色语音') }}
              </p>
            </div>

            <div v-if="!isWorking" class="main-actions" data-onboarding="download-main-actions">
              <div
                v-if="!isLauncherInstallerMode && protectionInfo?.hasProtections"
                class="protection-status glass-card"
                :class="protectionStatusClass"
              >
                {{ protectionStatusLabel }}
              </div>
              <button
                v-if="isPaused"
                class="action-btn highlight large"
                @click="resumeDownload"
                :disabled="!gameFolder"
              >
                {{ tr('gamedownload.actions.resume', '继续下载') }}
              </button>
              <button
                v-else-if="canShowPrimaryDownload"
                class="action-btn highlight large"
                @click="startDownload"
                :disabled="!gameFolder"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
                  fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                  <polyline points="7 10 12 15 17 10"/>
                  <line x1="12" y1="15" x2="12" y2="3"/>
                </svg>
                {{ primaryDownloadLabel }}
              </button>
              <button class="action-btn" @click="startVerify" v-if="!isPaused && canVerifyNow">
                {{ tr('gamedownload.actions.verify', '校验游戏文件') }}
              </button>
              <button class="action-btn warning-soft" @click="startRepair" v-if="!isPaused && canRepairNow">
                {{ tr('gamedownload.actions.repair', '修复异常文件') }} ({{ repairableFailures.length }})
              </button>
              <button
                class="action-btn"
                v-if="!isPaused && !isLauncherInstallerMode && protectionInfo?.hasProtections && !protectionApplied"
                @click="applyProtectionAfterDownload"
                :disabled="isProtectionBusy"
              >
                {{ isProtectionBusy ? tr('gamedownload.common.processing', '处理中...') : tr('gamedownload.protection.apply', '应用安全防护') }}
              </button>
              <button
                class="action-btn delete"
                v-if="!isPaused && !isLauncherInstallerMode && protectionInfo?.hasProtections && protectionApplied && !hideDisableProtectionButton"
                @click="disableProtection"
                :disabled="isProtectionBusy"
              >
                {{ isProtectionBusy ? tr('gamedownload.common.processing', '处理中...') : tr('gamedownload.protection.disable', '删除/禁用防护') }}
              </button>
              <button class="action-btn" @click="checkState" :disabled="isChecking">
                {{ isChecking ? tr('gamedownload.common.checking', '检查中...') : tr('gamedownload.actions.refreshState', '刷新状态') }}
              </button>
              <button v-if="isPaused" class="action-btn delete" @click="cancelDownload">
                {{ tr('gamedownload.actions.cancelTask', '取消任务') }}
              </button>
            </div>

            <div v-if="isWorking" class="progress-section glass-card">
              <div class="progress-phase">{{ workingPhase }}</div>
              <template v-if="progress">
                <div class="progress-bar-track">
                  <div class="progress-bar-fill"
                    :class="{ 'verify-fill': isVerifyPhase }"
                    :style="{ width: progressPercent + '%' }"></div>
                </div>
                <div class="progress-info">
                  <span>{{ progressPercent }}%</span>
                  <span v-if="progress.phase === 'install'">
                    {{ progress.finished_size }} / {{ progress.total_size }} {{ tr('gamedownload.progress.entries', '条目') }}
                  </span>
                  <span v-else>
                    {{ formatSize(progress.finished_size) }} / {{ formatSize(progress.total_size) }}
                  </span>
                </div>
                <div class="progress-detail">
                  <span class="progress-file">{{ progress.current_file }}</span>
                  <span v-if="isVerifyPhase">
                    {{ tr('gamedownload.progress.verifySpeed', '校验速度') }} {{ formatSize(progress.speed_bps) }}/s · {{ tr('gamedownload.progress.remaining', '剩余') }} {{ formatEta(progress.eta_seconds) }}
                  </span>
                  <span v-else-if="progress.phase !== 'install'">
                    {{ formatSize(progress.speed_bps) }}/s · {{ tr('gamedownload.progress.remaining', '剩余') }} {{ formatEta(progress.eta_seconds) }}
                  </span>
                </div>
                <div class="progress-counts">
                  {{ isVerifyPhase ? tr('gamedownload.progress.files', '文件') : tr('gamedownload.progress.packages', '包') }}: {{ progress.finished_count }} / {{ progress.total_count }}
                </div>
              </template>
              <div v-else class="progress-waiting">{{ tr('gamedownload.progress.waiting', '任务已启动，正在等待进度数据...') }}</div>
              <div class="progress-actions">
                <button class="action-btn" @click="pauseDownload">{{ tr('gamedownload.actions.pause', '暂停') }}</button>
                <button class="action-btn delete" @click="cancelDownload">{{ tr('gamedownload.actions.cancel', '取消') }}</button>
              </div>
            </div>

            <details class="config-details">
              <summary>{{ tr('gamedownload.advanced.title', '高级配置') }}</summary>
              <div class="config-content">
                <div class="field">
                  <label>{{ tr('gamedownload.advanced.launcherApi', '启动器 API') }}</label>
                  <input v-model="launcherApi" type="text" class="dl-input" />
                </div>
                <div v-if="isLauncherInstallerMode && installerOfficialUrl" class="field">
                  <label>{{ tr('gamedownload.advanced.installerUrl', '官方启动器下载链接') }}</label>
                  <div class="install-dir-row">
                    <input :value="installerOfficialUrl" type="text" class="dl-input" readonly />
                    <button class="action-btn sm" @click="copyOfficialLink">{{ tr('gamedownload.actions.copy', '复制') }}</button>
                  </div>
                </div>
                <button class="action-btn sm" @click="saveDownloadConfig">{{ tr('gamedownload.actions.saveConfig', '保存配置') }}</button>
              </div>
            </details>

            <div class="divider"><span>{{ tr('gamedownload.common.or', '或者') }}</span></div>
          </template>

          <div class="section">
            <p v-if="!isSupported" class="hint">{{ tr('gamedownload.manual.notSupported', '此游戏暂不支持自动下载。') }}</p>
            <p class="hint">{{ tr('gamedownload.manual.hint', '如果游戏已安装（通过 Steam、Lutris 或手动安装），选择可执行文件即可。') }}</p>
            <button class="action-btn highlight large" @click="selectGameExe" style="max-width: 320px;">
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
                fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
              {{ tr('gamedownload.manual.selectExe', '选择游戏可执行文件') }}
            </button>
          </div>

          <div v-if="error" class="error-msg">{{ error }}</div>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
/* =========== 核心：毛玻璃玻璃态容器 =========== */
.glass-panel {
  background-color: rgba(20, 25, 30, 0.75) !important;
  backdrop-filter: blur(24px) saturate(120%);
  -webkit-backdrop-filter: blur(24px) saturate(120%);
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
}

.glass-card {
  background-color: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 8px;
}

.dl-overlay {
  position: fixed; inset: 0;
  background: rgba(0, 0, 0, 0.6); will-change: transform;
  z-index: 2000; display: flex; align-items: center; justify-content: center;
}
.dl-window {
  width: 100%; max-width: 900px;
  height: 80vh; max-height: 700px;
  border-radius: 12px; display: flex; flex-direction: column; 
  animation: slideUp 0.15s cubic-bezier(0.25, 0.8, 0.25, 1);
  contain: layout style;
}
@keyframes slideUp {
  from { opacity: 0; transform: translateY(20px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}
.dl-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 0 30px; height: 60px; border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}
.dl-title { font-size: 18px; font-weight: 600; color: var(--el-color-primary); text-transform: uppercase; letter-spacing: 1px; }
.dl-close {
  width: 32px; height: 32px; display: flex; align-items: center; justify-content: center;
  border-radius: 6px; cursor: pointer; color: rgba(255, 255, 255, 0.6); transition: all 0.2s;
}
.dl-close:hover { background: rgba(255, 255, 255, 0.1); color: #fff; }
.dl-body { padding: 30px; overflow-y: auto; flex: 1; }

.dl-body::-webkit-scrollbar { width: 6px; }
.dl-body::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.2); border-radius: 3px; }

/* 游戏信息 */
.game-info { margin-bottom: 20px; display: flex; align-items: center; gap: 10px; }
.game-name { font-size: 18px; font-weight: 600; color: #fff; }
.badge {
  font-size: 11px; padding: 2px 8px; border-radius: 4px; white-space: nowrap;
}
.badge.supported { background: rgba(var(--el-color-success-rgb), 0.15); color: var(--el-color-success); border: 1px solid rgba(var(--el-color-success-rgb), 0.3); }
.badge.manual { background: rgba(255, 255, 255, 0.06); color: rgba(255, 255, 255, 0.4); }

.status-msg {
  text-align: center; color: rgba(255, 255, 255, 0.5); font-size: 13px;
  padding: 16px 0; animation: pulse 1.5s ease-in-out infinite;
}
@keyframes pulse { 0%, 100% { opacity: 0.5; } 50% { opacity: 1; } }

/* 服务器选择 */
.server-section { margin-bottom: 16px; }
.server-label {
  display: block; font-size: 14px; font-weight: 500;
  color: rgba(255, 255, 255, 0.85); margin-bottom: 8px;
}
.server-options { display: flex; gap: 8px; }
.server-btn {
  padding: 6px 16px; border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 6px;
  background: rgba(255, 255, 255, 0.05); color: rgba(255, 255, 255, 0.7);
  font-size: 13px; font-weight: 500; cursor: pointer; transition: all 0.2s;
}
.server-btn:hover { background: rgba(255, 255, 255, 0.15); }
.server-btn.active {
  background: rgba(var(--el-color-primary-rgb), 0.15); color: var(--el-color-primary-light-3);
  border-color: rgba(var(--el-color-primary-rgb), 0.4);
}

/* 状态卡片 */
.state-card {
  padding: 18px 20px; margin-bottom: 24px;
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}
.state-card:hover { transform: translateY(-2px); }
.state-card.state-ok { border-color: rgba(var(--el-color-success-rgb), 0.4); background: rgba(var(--el-color-success-rgb), 0.05); }
.state-card.state-install { border-color: rgba(var(--el-color-warning-rgb), 0.4); background: rgba(var(--el-color-warning-rgb), 0.05); }
.state-card.state-update { border-color: rgba(var(--el-color-primary-rgb), 0.4); background: rgba(var(--el-color-primary-rgb), 0.05); }
.state-card.state-error { border-color: rgba(var(--el-color-danger-rgb), 0.4); background: rgba(var(--el-color-danger-rgb), 0.05); }
.state-label { font-size: 16px; font-weight: 600; color: rgba(255, 255, 255, 0.95); }
.state-versions {
  margin-top: 8px; font-size: 13px; color: rgba(255, 255, 255, 0.6);
  display: flex; gap: 16px;
}

.channel-mode-card {
  padding: 16px 20px; margin-bottom: 24px;
  transition: all 0.3s ease;
}
.channel-mode-card:hover { border-color: rgba(255, 255, 255, 0.15); }
.channel-mode-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.channel-mode-title { color: rgba(255, 255, 255, 0.85); font-size: 13px; font-weight: 600; }
.channel-mode-pill { font-size: 11px; padding: 2px 8px; border-radius: 12px; border: 1px solid rgba(255, 255, 255, 0.2); color: rgba(255, 255, 255, 0.75); }
.channel-mode-pill.ok { color: var(--el-color-success); border-color: rgba(var(--el-color-success-rgb), 0.4); background: rgba(var(--el-color-success-rgb), 0.1); }
.channel-mode-pill.warn { color: var(--el-color-warning); border-color: rgba(var(--el-color-warning-rgb), 0.4); background: rgba(var(--el-color-warning-rgb), 0.1); }
.channel-mode-meta { display: flex; flex-wrap: wrap; gap: 8px 14px; color: rgba(255, 255, 255, 0.55); font-size: 12px; margin-bottom: 10px; }
.channel-mode-actions { display: flex; gap: 8px; flex-wrap: wrap; }
.channel-mode-hint { margin: 8px 0 0 0; color: rgba(255, 255, 255, 0.5); font-size: 12px; }
.channel-mode-hint.warning { color: var(--el-color-warning); }

/* 安装目录 */
.install-dir-section { margin-bottom: 24px; }
.install-dir-label {
  display: block; font-size: 14px; font-weight: 500;
  color: rgba(255, 255, 255, 0.85); margin-bottom: 10px;
}
.install-dir-row { display: flex; gap: 12px; }
.install-dir-row .dl-input { 
  flex: 1; 
  background: rgba(0, 0, 0, 0.25);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  padding: 10px 12px;
  color: #fff;
  font-size: 14px;
  outline: none;
  transition: all 0.2s ease;
}
.install-dir-row .dl-input:focus {
  background: rgba(0, 0, 0, 0.4);
  border-color: var(--el-color-primary);
}
.install-dir-row .dir-btn {
  display: flex; align-items: center; justify-content: center;
  color: #fff; padding: 0 16px; height: 42px;
  border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 6px;
  background: rgba(255, 255, 255, 0.05); cursor: pointer; transition: all 0.2s;
}
.install-dir-row .dir-btn:hover { 
  background: rgba(255, 255, 255, 0.15); border-color: rgba(255, 255, 255, 0.25); transform: translateY(-1px);
}
.install-dir-row .dir-btn:active { transform: translateY(0); }
.install-dir-hint { font-size: 12px; color: rgba(255, 255, 255, 0.5); margin-top: 8px; line-height: 1.5; }

/* 语言包选择 */
.lang-section { margin-bottom: 24px; }
.lang-label { display: block; font-size: 14px; font-weight: 500; color: rgba(255, 255, 255, 0.85); margin-bottom: 8px; }
.lang-options { display: flex; flex-wrap: wrap; gap: 12px; }
.lang-checkbox {
  display: flex; align-items: center; gap: 8px; padding: 8px 16px; border-radius: 6px;
  background: rgba(0, 0, 0, 0.2); border: 1px solid rgba(255, 255, 255, 0.05);
  cursor: pointer; transition: all 0.2s; user-select: none;
}
.lang-checkbox:hover { background: rgba(255, 255, 255, 0.05); border-color: rgba(255, 255, 255, 0.2); }
.lang-checkbox input[type="checkbox"] { accent-color: var(--el-color-primary); width: 16px; height: 16px; cursor: pointer; }
.lang-name { font-size: 14px; color: rgba(255, 255, 255, 0.9); }
.lang-hint { font-size: 12px; color: var(--el-color-warning); margin-top: 8px; line-height: 1.4; display: flex; align-items: center; }

/* 主要操作按钮 */
.main-actions { display: flex; gap: 12px; margin-bottom: 24px; flex-wrap: wrap; }
.protection-status {
  width: 100%; font-size: 13px; border-radius: 8px; padding: 12px 16px;
  display: flex; align-items: center; gap: 8px;
}
.protection-status::before { content: ''; display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: currentColor; }
.protection-status.enabled { color: var(--el-color-success); border-color: rgba(var(--el-color-success-rgb), 0.3); background: rgba(var(--el-color-success-rgb), 0.1); }
.protection-status.disabled { color: var(--el-color-warning); border-color: rgba(var(--el-color-warning-rgb), 0.3); background: rgba(var(--el-color-warning-rgb), 0.1); }
.protection-status.neutral { color: rgba(255, 255, 255, 0.5); }

/* 统一动作按钮 (与 GameSettingsModal 一致) */
.action-btn {
  padding: 10px 18px; border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 6px;
  font-size: 13px; font-weight: 500; cursor: pointer; transition: all 0.2s;
  color: #fff; background: rgba(255, 255, 255, 0.05);
  display: inline-flex; align-items: center; justify-content: center; gap: 8px;
  flex: 1; min-width: 120px; height: 40px;
}
.action-btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.15); transform: translateY(-1px); }
.action-btn:active:not(:disabled) { transform: translateY(0); }
.action-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.action-btn.large { padding: 0 24px; font-size: 15px; font-weight: 600; flex: 2; height: 44px; }

.action-btn.highlight, .action-btn.primary {
  background: rgba(var(--el-color-primary-rgb), 0.15);
  border-color: rgba(var(--el-color-primary-rgb), 0.4);
  color: var(--el-color-primary-light-3);
}
.action-btn.highlight:hover:not(:disabled), .action-btn.primary:hover:not(:disabled) {
  background: rgba(var(--el-color-primary-rgb), 0.3);
}

.action-btn.warning-soft {
  background: rgba(var(--el-color-warning-rgb), 0.15);
  color: var(--el-color-warning-light-3);
  border-color: rgba(var(--el-color-warning-rgb), 0.4);
}
.action-btn.warning-soft:hover:not(:disabled) { background: rgba(var(--el-color-warning-rgb), 0.3); }

.action-btn.delete, .action-btn.danger {
  background: rgba(var(--el-color-danger-rgb), 0.15);
  border-color: rgba(var(--el-color-danger-rgb), 0.4);
  color: var(--el-color-danger-light-3);
}
.action-btn.delete:hover:not(:disabled), .action-btn.danger:hover:not(:disabled) {
  background: rgba(var(--el-color-danger-rgb), 0.3);
}
.action-btn.sm { padding: 0 12px; font-size: 13px; flex: none; height: 32px; min-width: auto; }

/* 进度条 */
.progress-section { padding: 20px; margin-bottom: 16px; }
.progress-phase { font-size: 14px; font-weight: 600; color: rgba(255, 255, 255, 0.9); margin-bottom: 12px; }
.progress-bar-track { width: 100%; height: 8px; background: rgba(255, 255, 255, 0.1); border-radius: 4px; overflow: hidden; }
.progress-bar-fill {
  height: 100%; background: linear-gradient(90deg, var(--el-color-primary), var(--el-color-primary-light-3));
  border-radius: 4px; transition: width 0.3s ease;
}
.progress-bar-fill.verify-fill { background: linear-gradient(90deg, var(--el-color-success), var(--el-color-success-light-3)); }
.progress-info { display: flex; justify-content: space-between; font-size: 13px; color: rgba(255, 255, 255, 0.9); margin-top: 8px; font-weight: 500; }
.progress-detail { display: flex; justify-content: space-between; font-size: 11px; color: rgba(255, 255, 255, 0.6); margin-top: 4px; }
.progress-file { max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.progress-counts { font-size: 11px; color: rgba(255, 255, 255, 0.5); margin-top: 4px; }
.progress-waiting { margin-top: 10px; font-size: 12px; color: rgba(255, 255, 255, 0.6); }
.progress-actions { display: flex; gap: 8px; margin-top: 12px; }

/* 折叠配置 */
.config-details {
  margin: 20px 0; background: rgba(0, 0, 0, 0.15); border: 1px solid rgba(255, 255, 255, 0.05); border-radius: 8px; overflow: hidden;
}
.config-details summary {
  font-size: 13px; color: rgba(255, 255, 255, 0.6); cursor: pointer; padding: 10px 14px; user-select: none;
  background: rgba(255, 255, 255, 0.02); transition: background 0.2s;
}
.config-details summary:hover { color: rgba(255, 255, 255, 0.9); background: rgba(255, 255, 255, 0.05); }
.config-content { padding: 16px 14px; border-top: 1px solid rgba(255, 255, 255, 0.04); }
.field { margin-bottom: 12px; }
.field label { display: block; font-size: 13px; color: rgba(255, 255, 255, 0.6); margin-bottom: 6px; }
.dl-input {
  width: 100%; box-sizing: border-box; background: rgba(0, 0, 0, 0.25);
  border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 6px;
  padding: 8px 12px; color: #fff; font-size: 13px; outline: none; transition: border-color 0.2s;
}
.dl-input:focus { border-color: var(--el-color-primary); }

/* 分隔线 */
.divider {
  display: flex; align-items: center; gap: 16px; margin: 28px 0;
  color: rgba(255, 255, 255, 0.4); font-size: 12px; text-transform: uppercase; letter-spacing: 1px;
}
.divider::before, .divider::after { content: ''; flex: 1; height: 1px; background: rgba(255, 255, 255, 0.08); }

/* 手动安装区 */
.section { margin-bottom: 24px; }
.hint { font-size: 13px; color: rgba(255, 255, 255, 0.5); margin-bottom: 16px; line-height: 1.5; }

/* 错误 */
.error-msg {
  background: rgba(var(--el-color-danger-rgb), 0.15); border: 1px solid rgba(var(--el-color-danger-rgb), 0.3);
  border-radius: 6px; padding: 10px 14px; color: var(--el-color-danger); font-size: 13px; margin-top: 12px;
}

/* 过渡 */
.modal-fade-enter-active, .modal-fade-leave-active { transition: opacity 0.15s; }
.modal-fade-enter-from, .modal-fade-leave-to { opacity: 0; }
</style>