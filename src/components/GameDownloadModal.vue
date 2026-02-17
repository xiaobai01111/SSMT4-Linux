<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import {
  getGameState,
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
  type PresetCatalogItem,
  type ChannelProtectionStatus,
} from '../api';
import { appSettings } from '../store';
import { dlState, isActiveFor, fireDownload, fireVerify, cancelActive } from '../downloadStore';

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
const statusMsg = ref('');
const protectionInfo = ref<any>(null);
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

const protectionStatusLabel = computed(() => {
  if (!protectionInfo.value?.hasProtections) return '该游戏暂无可用防护';
  if (protectionEnforceAtLaunch.value) {
    return protectionApplied.value ? '防护状态：已启用（强制）' : '防护状态：未启用（将阻止启动）';
  }
  return protectionApplied.value ? '防护状态：已启用' : '防护状态：未启用';
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
  servers: ServerOption[];
  audioLanguages?: AudioLangOption[];
}

const isHoyoverseApi = (api: string): boolean =>
  api.includes('mihoyo.com') || api.includes('hoyoverse.com');

const currentBizPrefix = (): string => (selectedServer.value?.bizPrefix || '').trim();

const ensureBizPrefixReady = (): boolean => {
  if (!isHoyoverseApi(launcherApi.value)) return true;
  if (currentBizPrefix()) return true;
  error.value = '当前 HoYoverse 服务器缺少 biz_prefix，请检查游戏预设配置';
  return false;
};

const close = () => {
  emit('update:modelValue', false);
};

const ensureRiskAcknowledged = async () => {
  if (appSettings.tosRiskAcknowledged) return true;

  const accepted = await askConfirm(
    '本启动器为非官方工具，与游戏厂商无关。\n\n在 Linux/Wine/Proton 环境运行游戏，可能被反作弊误判，存在账号处罚（包括封禁）风险。\n\n是否确认你已理解并愿意自行承担风险？',
    {
      title: 'ToS / 封禁风险提示',
      kind: 'warning',
      okLabel: '我已理解风险',
      cancelLabel: '取消',
    }
  );
  if (!accepted) return false;

  const second = await askConfirm(
    '请再次确认：继续使用即表示你了解这是非官方方案，且可能导致账号风险。',
    {
      title: '二次确认',
      kind: 'warning',
      okLabel: '确认继续',
      cancelLabel: '返回',
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

    const status = await checkGameProtectionStatus(preset, gameFolder.value || undefined);
    protectionApplied.value = !!status?.enabled;
    protectionEnforceAtLaunch.value = status?.enforceAtLaunch !== false;
    channelProtection.value = await getChannelProtectionStatus(preset, gameFolder.value || undefined);
  } catch (e) {
    console.warn('[防护] 刷新状态失败:', e);
    protectionInfo.value = null;
    protectionApplied.value = false;
    protectionEnforceAtLaunch.value = false;
    channelProtection.value = null;
  }
};

const setChannelMode = async (mode: 'init' | 'protected') => {
  if (isChannelModeBusy.value) return;
  if (!gameFolder.value) {
    await showMessage('请先选择游戏安装目录', { title: '提示', kind: 'warning' });
    return;
  }
  try {
    isChannelModeBusy.value = true;
    const preset = getProtectionPreset();
    channelProtection.value = await setChannelProtectionMode(preset, mode, gameFolder.value);
    await refreshProtectionStatus();
    await showMessage(
      mode === 'init' ? '已切换到初始化模式 (KR_ChannelId=19)' : '已切换到联机模式 (KR_ChannelId=205)',
      { title: '渠道模式', kind: 'info' },
    );
  } catch (e) {
    console.warn('[防护] 切换渠道模式失败:', e);
    await showMessage(`切换渠道模式失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isChannelModeBusy.value = false;
  }
};

// 打开时自动加载：检测是否支持自动下载 + 读取配置
const loadState = async () => {
  if (!props.gameName) return;
  error.value = '';
  statusMsg.value = '正在加载配置...';

  // 1. 先尝试从 Config.json 读取 GamePreset，失败则直接用 gameName
  let preset = canonicalPreset(props.gameName);
  let savedFolder = '';
  try {
    const config = await loadGameConfig(props.gameName);
    preset = canonicalPreset((config as any).GamePreset || config.basic?.gamePreset || props.gameName);
    // 恢复之前保存的下载配置（保存在 config.other 下）
    const other = config.other || {};
    launcherApi.value = other.launcherApi || '';
    savedFolder = other.gameFolder || '';
  } catch (e) {
    console.warn('[GameDownload] loadGameConfig 失败，使用 gameName 作为 preset:', e);
  }
  gamePreset.value = preset;
  console.log('[GameDownload] gameName =', props.gameName, ', preset =', preset);
  await refreshProtectionStatus();

  // 2. 检测是否支持自动下载
  const launcherInfo = await getGameLauncherApi(preset);
  const knownApi: LauncherApiConfig | null = launcherInfo.supported
    ? {
        defaultFolder: launcherInfo.defaultFolder || '',
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

  // 设置可用服务器列表
  availableServers.value = knownApi?.servers || [];
  if (availableServers.value.length > 0 && !selectedServer.value) {
    selectedServer.value = availableServers.value[0]; // 默认选择第一个（国服）
  }

  // 设置可用语言包
  availableLanguages.value = knownApi?.audioLanguages || [];
  if (availableLanguages.value.length > 0 && selectedLanguages.value.length === 0) {
    selectedLanguages.value = ['zh-cn']; // 国服默认中文
  }
  console.log('[GameDownload] isSupported =', isSupported.value, ', servers =', availableServers.value.length);

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
      console.warn('[GameDownload] listGamePresetsForInfo failed:', e);
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
    console.warn('[GameDownload] getDefaultGameFolder failed:', e);
    gameFolder.value = savedFolder || '';
  }

  statusMsg.value = '';
  console.log('[GameDownload] launcherApi =', launcherApi.value);
  console.log('[GameDownload] gameFolder =', gameFolder.value);

  // 5. 自动检查游戏状态
  try {
    await checkState();
  } catch (e) {
    console.warn('[GameDownload] checkState 失败:', e);
  }

};

const onServerChange = async (server: ServerOption) => {
  selectedServer.value = server;
  launcherApi.value = server.launcherApi;
  gameState.value = null;
  await checkState();
};

const checkState = async () => {
  if (!launcherApi.value || !gameFolder.value) {
    error.value = '请先配置启动器 API 和游戏安装目录';
    return;
  }
  if (!ensureBizPrefixReady()) return;
  isChecking.value = true;
  error.value = '';
  statusMsg.value = '正在检查游戏状态...';
  try {
    const biz = currentBizPrefix() || undefined;
    gameState.value = await getGameState(launcherApi.value, gameFolder.value, biz);
    await refreshProtectionStatus();
    statusMsg.value = '';
  } catch (e: any) {
    error.value = `检查状态失败: ${e}`;
    statusMsg.value = '';
  } finally {
    isChecking.value = false;
  }
};

const startDownload = async () => {
  if (!launcherApi.value || !gameFolder.value) return;
  if (dlState.active) return;
  if (!ensureBizPrefixReady()) return;
  if (!(await ensureRiskAcknowledged())) return;
  error.value = '';

  fireDownload({
    gameName: props.gameName,
    displayName: props.displayName,
    launcherApi: launcherApi.value,
    gameFolder: gameFolder.value,
    languages: selectedLanguages.value.length > 0 ? [...selectedLanguages.value] : undefined,
    bizPrefix: currentBizPrefix() || undefined,
    isUpdate: gameState.value?.state === 'needupdate',
  });
};

const startVerify = async () => {
  if (!launcherApi.value || !gameFolder.value) return;
  if (dlState.active) return;
  if (!ensureBizPrefixReady()) return;

  fireVerify({
    gameName: props.gameName,
    displayName: props.displayName,
    launcherApi: launcherApi.value,
    gameFolder: gameFolder.value,
    bizPrefix: currentBizPrefix() || undefined,
  });
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

    const protNames = (info.protections as any[]).map((p: any) => p.name).join('、');
    const confirmed = await askConfirm(
      `检测到该游戏支持以下安全防护：\n\n${protNames}\n\n是否立即应用？`,
      { title: '游戏安全防护', kind: 'warning', okLabel: '应用防护', cancelLabel: '跳过' }
    );
    if (confirmed) {
      const result = await applyGameProtection(preset, gameFolder.value);
      console.log('[防护] 应用结果:', result);
      await refreshProtectionStatus();
      if (protectionApplied.value) {
        await showMessage('游戏安全防护已成功应用！', { title: '防护完成', kind: 'info' });
      } else {
        const status = await checkGameProtectionStatus(preset, gameFolder.value || undefined);
        const missing = Array.isArray(status?.missing) && status.missing.length > 0
          ? status.missing.join('\n')
          : '未知';
        await showMessage(
          `防护未完全生效，仍有以下问题：\n${missing}`,
          { title: '防护部分完成', kind: 'warning' }
        );
      }
    }
  } catch (e) {
    console.warn('[防护] 应用失败:', e);
    await showMessage(`应用防护失败: ${e}`, { title: '错误', kind: 'error' });
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
      '禁用防护会恢复渠道参数为原始值并可能增加账号风险，确认继续？',
      { title: '禁用防护', kind: 'warning', okLabel: '确认禁用', cancelLabel: '取消' }
    );
    if (!yes) return;

    const result = await restoreTelemetry(preset, gameFolder.value || undefined);
    await refreshProtectionStatus();
    const channelTip = result?.channel?.message;
    await showMessage(
      channelTip ? `已禁用域名/文件防护。\n${channelTip}` : '已禁用防护',
      { title: '已禁用', kind: 'info' },
    );
  } catch (e) {
    console.warn('[防护] 禁用失败:', e);
    await showMessage(`禁用防护失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    isProtectionBusy.value = false;
  }
};

const cancelDownload = async () => {
  await cancelActive();
};

const selectGameExe = async () => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: '可执行文件', extensions: ['exe', 'sh', 'AppImage', 'desktop', '*'] }],
      title: '选择游戏可执行文件'
    });
    if (selected && typeof selected === 'string') {
      const config = await loadGameConfig(props.gameName);
      config.other = config.other || {};
      config.other.gamePath = selected;
      await saveGameConfig(props.gameName, config);
      await showMessage('已设置游戏路径', { title: '成功', kind: 'info' });
      emit('gameConfigured');
      close();
    }
  } catch (e: any) {
    error.value = `选择文件失败: ${e}`;
  }
};

const selectGameFolder = async () => {
  try {
    const selected = await openFileDialog({ directory: true, title: '选择游戏安装目录' });
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

// === 格式化辅助 ===
const formatSize = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
};
const formatEta = (seconds: number) => {
  if (seconds <= 0) return '计算中...';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  return `${m}:${s.toString().padStart(2, '0')}`;
};

const progress = computed(() => {
  if (isActiveFor(props.gameName)) return dlState.progress;
  return null;
});

const progressPercent = computed(() => {
  if (!progress.value || progress.value.total_size === 0) return 0;
  return Math.round((progress.value.finished_size / progress.value.total_size) * 100);
});

const stateLabel = computed(() => {
  if (!gameState.value) return '';
  switch (gameState.value.state) {
    case 'startgame': return '✓ 已是最新版本';
    case 'needinstall': return '需要下载安装';
    case 'needupdate':
      return `需要更新 (${gameState.value.local_version} → ${gameState.value.remote_version})`;
    case 'networkerror': return '⚠ 网络错误，请检查网络连接';
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

const isWorking = computed(() => isActiveFor(props.gameName) && (dlState.phase === 'downloading' || dlState.phase === 'verifying'));
const isVerifyPhase = computed(() => dlState.phase === 'verifying' || progress.value?.phase === 'verify');
const workingPhase = computed(() => {
  if (!isActiveFor(props.gameName)) return '';
  if (dlState.phase === 'verifying') return '校验中';
  if (dlState.phase === 'downloading') {
    if (progress.value?.phase === 'install') return '安装中';
    return '下载中';
  }
  return '';
});

watch(() => props.modelValue, (val) => {
  if (val) {
    error.value = '';
    statusMsg.value = '';
    if (!isActiveFor(props.gameName)) {
      gameState.value = null;
    }
    loadState();
  }
}, { immediate: true });
</script>

<template>
  <transition name="modal-fade">
    <div v-if="modelValue" class="dl-overlay" @click.self="close">
      <div class="dl-window">
        <!-- 标题栏 -->
        <div class="dl-header">
          <span class="dl-title">下载 / 安装游戏</span>
          <div class="dl-close" @click="close">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
              fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </div>
        </div>

        <div class="dl-body">
          <!-- 游戏名 -->
          <div class="game-info">
            <span class="game-name">{{ displayName || gameName }}</span>
            <span v-if="isSupported" class="badge supported">支持自动下载</span>
            <span v-else class="badge manual">手动安装</span>
          </div>

          <!-- 加载提示 -->
          <div v-if="statusMsg" class="status-msg">{{ statusMsg }}</div>

          <!-- ========== 支持自动下载的游戏 (鸣潮) ========== -->
          <template v-if="isSupported">
            <!-- 服务器选择 -->
            <div v-if="availableServers.length > 1 && !isWorking" class="server-section">
              <label class="server-label">服务器</label>
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

            <!-- 游戏状态卡片 -->
            <div v-if="gameState" class="state-card" :class="stateClass">
              <div class="state-label">{{ stateLabel }}</div>
              <div class="state-versions" v-if="gameState.remote_version">
                <span v-if="gameState.local_version">本地: {{ gameState.local_version }}</span>
                <span>最新: {{ gameState.remote_version }}</span>
              </div>
            </div>

            <div
              v-if="channelProtection?.supported && channelProtection.channel?.required && !isWorking"
              class="channel-mode-card"
            >
              <div class="channel-mode-head">
                <span class="channel-mode-title">渠道模式</span>
                <span
                  class="channel-mode-pill"
                  :class="{ ok: channelProtection.channel?.enabled, warn: !channelProtection.channel?.enabled }"
                >
                  {{ channelProtection.channel?.enabled ? '已匹配' : '未匹配' }}
                </span>
              </div>
              <div class="channel-mode-meta">
                <span>当前值: {{ channelProtection.channel?.currentValue ?? '未知' }}</span>
                <span>目标值: {{ channelProtection.channel?.expectedValue ?? '未知' }}</span>
                <span>当前模式: {{ channelProtection.channel?.mode || '-' }}</span>
              </div>
              <div class="channel-mode-actions">
                <button class="action-btn sm" @click="setChannelMode('init')" :disabled="isChannelModeBusy || !gameFolder">
                  切换初始化模式(19)
                </button>
                <button class="action-btn sm primary" @click="setChannelMode('protected')" :disabled="isChannelModeBusy || !gameFolder">
                  切换联机模式(205)
                </button>
                <button class="action-btn sm" @click="setChannelMode('init')" :disabled="isChannelModeBusy || !gameFolder">
                  恢复默认(19)
                </button>
              </div>
              <p v-if="channelProtection.channel?.mode === 'init'" class="channel-mode-hint">
                当前处于初始化模式。完成首次初始化后，请手动切换到联机模式(205)。
              </p>
              <p
                v-else-if="!channelProtection.channel?.enabled"
                class="channel-mode-hint warning"
              >
                当前模式与配置值不一致，建议重新应用当前模式。
              </p>
            </div>

            <!-- 安装目录（始终显示，下载前必须确认） -->
            <div v-if="!isWorking" class="install-dir-section">
              <label class="install-dir-label">安装目录</label>
              <div class="install-dir-row">
                <input v-model="gameFolder" type="text" class="dl-input" placeholder="选择游戏安装目录..." />
                <button class="icon-btn" @click="selectGameFolder" title="选择目录">
                  <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
                    fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                  </svg>
                </button>
              </div>
              <p class="install-dir-hint">游戏文件将下载到此目录，请确保有足够磁盘空间（约 130GB+）</p>
            </div>

            <!-- 语言包选择 -->
            <div v-if="availableLanguages.length > 0 && !isWorking" class="lang-section">
              <label class="lang-label">语音包（可多选）</label>
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
                ⚠ 未选择任何语音包，游戏将没有角色语音
              </p>
            </div>

            <!-- 下载/更新按钮 -->
            <div v-if="!isWorking" class="main-actions">
              <div
                v-if="protectionInfo?.hasProtections"
                class="protection-status"
                :class="protectionStatusClass"
              >
                {{ protectionStatusLabel }}
              </div>
              <button
                v-if="canDownload"
                class="action-btn primary large"
                @click="startDownload"
                :disabled="!gameFolder"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
                  fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                  <polyline points="7 10 12 15 17 10"/>
                  <line x1="12" y1="15" x2="12" y2="3"/>
                </svg>
                {{ gameState?.state === 'needupdate' ? '开始更新' : '开始下载' }}
              </button>
              <button class="action-btn" @click="startVerify" v-if="gameState?.state === 'startgame'">
                校验游戏文件
              </button>
              <button
                class="action-btn"
                v-if="protectionInfo?.hasProtections && !protectionApplied"
                @click="applyProtectionAfterDownload"
                :disabled="isProtectionBusy"
              >
                {{ isProtectionBusy ? '处理中...' : '应用安全防护' }}
              </button>
              <button
                class="action-btn danger-soft"
                v-if="protectionInfo?.hasProtections && protectionApplied && !hideDisableProtectionButton"
                @click="disableProtection"
                :disabled="isProtectionBusy"
              >
                {{ isProtectionBusy ? '处理中...' : '删除/禁用防护' }}
              </button>
              <button class="action-btn" @click="checkState" :disabled="isChecking">
                {{ isChecking ? '检查中...' : '刷新状态' }}
              </button>
            </div>

            <!-- 进度条 -->
            <div v-if="isWorking && progress" class="progress-section">
              <div class="progress-phase">{{ workingPhase }}</div>
              <div class="progress-bar-track">
                <div class="progress-bar-fill"
                  :class="{ 'verify-fill': isVerifyPhase }"
                  :style="{ width: progressPercent + '%' }"></div>
              </div>
              <div class="progress-info">
                <span>{{ progressPercent }}%</span>
                <span v-if="progress.phase === 'install'">
                  {{ progress.finished_size }} / {{ progress.total_size }} 条目
                </span>
                <span v-else>
                  {{ formatSize(progress.finished_size) }} / {{ formatSize(progress.total_size) }}
                </span>
              </div>
              <div class="progress-detail">
                <span class="progress-file">{{ progress.current_file }}</span>
                <span v-if="isVerifyPhase">
                  校验速度 {{ formatSize(progress.speed_bps) }}/s · 剩余 {{ formatEta(progress.eta_seconds) }}
                </span>
                <span v-else-if="progress.phase !== 'install'">
                  {{ formatSize(progress.speed_bps) }}/s · 剩余 {{ formatEta(progress.eta_seconds) }}
                </span>
              </div>
              <div class="progress-counts">
                {{ isVerifyPhase ? '文件' : '包' }}: {{ progress.finished_count }} / {{ progress.total_count }}
              </div>
              <button class="action-btn danger" @click="cancelDownload">取消</button>
            </div>

            <!-- 高级配置（折叠） -->
            <details class="config-details">
              <summary>高级配置</summary>
              <div class="config-content">
                <div class="field">
                  <label>启动器 API</label>
                  <input v-model="launcherApi" type="text" class="dl-input" />
                </div>
                <button class="action-btn sm" @click="saveDownloadConfig">保存配置</button>
              </div>
            </details>

            <div class="divider"><span>或者</span></div>
          </template>

          <!-- ========== 手动指定路径 ========== -->
          <div class="section">
            <div class="section-title" v-if="!isSupported">此游戏暂不支持自动下载</div>
            <p class="hint">如果游戏已安装（通过 Steam、Lutris 或手动安装），选择可执行文件即可。</p>
            <button class="action-btn primary" @click="selectGameExe">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
                fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
              选择游戏可执行文件
            </button>
          </div>

          <!-- 错误提示 -->
          <div v-if="error" class="error-msg">{{ error }}</div>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.dl-overlay {
  position: fixed; inset: 0;
  background: rgba(0,0,0,0.6); backdrop-filter: blur(4px);
  z-index: 2000; display: flex; align-items: center; justify-content: center;
}
.dl-window {
  width: 580px; max-height: 82vh;
  background: rgba(30,30,30,0.97); border: 1px solid rgba(255,255,255,0.1);
  border-radius: 12px; box-shadow: 0 8px 32px rgba(0,0,0,0.6);
  display: flex; flex-direction: column; animation: slideUp 0.25s ease-out;
}
@keyframes slideUp {
  from { opacity:0; transform:translateY(16px); }
  to { opacity:1; transform:translateY(0); }
}
.dl-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 18px 24px; border-bottom: 1px solid rgba(255,255,255,0.06);
}
.dl-title { font-size:16px; font-weight:600; color:#fff; }
.dl-close {
  width:28px; height:28px; display:flex; align-items:center; justify-content:center;
  border-radius:4px; cursor:pointer; color:rgba(255,255,255,0.5); transition:all 0.2s;
}
.dl-close:hover { background:rgba(255,255,255,0.1); color:#fff; }
.dl-body { padding:24px; overflow-y:auto; flex:1; }

/* 游戏信息 */
.game-info { margin-bottom:20px; display:flex; align-items:center; gap:10px; }
.game-name { font-size:18px; font-weight:600; color:#F7CE46; }
.badge {
  font-size:11px; padding:2px 8px; border-radius:4px; white-space:nowrap;
}
.badge.supported { background:rgba(103,194,58,0.15); color:#67c23a; border:1px solid rgba(103,194,58,0.3); }
.badge.manual { background:rgba(255,255,255,0.06); color:rgba(255,255,255,0.4); }

.status-msg {
  text-align:center; color:rgba(255,255,255,0.5); font-size:13px;
  padding:16px 0; animation: pulse 1.5s ease-in-out infinite;
}
@keyframes pulse { 0%,100% { opacity:0.5; } 50% { opacity:1; } }

/* 服务器选择 */
.server-section { margin-bottom:16px; }
.server-label {
  display:block; font-size:13px; font-weight:500;
  color:rgba(255,255,255,0.7); margin-bottom:8px;
}
.server-options { display:flex; gap:8px; }
.server-btn {
  padding:6px 16px; border:1px solid rgba(255,255,255,0.12); border-radius:6px;
  background:rgba(255,255,255,0.06); color:rgba(255,255,255,0.7);
  font-size:13px; cursor:pointer; transition:all 0.2s;
}
.server-btn:hover { background:rgba(255,255,255,0.1); border-color:rgba(255,255,255,0.2); }
.server-btn.active {
  background:rgba(247,206,70,0.15); color:#F7CE46;
  border-color:rgba(247,206,70,0.4);
}

/* 状态卡片 */
.state-card {
  background:rgba(0,0,0,0.25); border:1px solid rgba(255,255,255,0.06);
  border-radius:8px; padding:14px 16px; margin-bottom:16px;
}
.state-card.state-ok { border-color:rgba(103,194,58,0.3); }
.state-card.state-install { border-color:rgba(247,206,70,0.3); }
.state-card.state-update { border-color:rgba(0,122,204,0.3); }
.state-card.state-error { border-color:rgba(232,17,35,0.3); }
.state-label { font-size:15px; font-weight:500; color:rgba(255,255,255,0.9); }
.state-versions {
  margin-top:6px; font-size:12px; color:rgba(255,255,255,0.45);
  display:flex; gap:16px;
}

.channel-mode-card {
  background: rgba(0, 0, 0, 0.22);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  padding: 12px 14px;
  margin-bottom: 16px;
}

.channel-mode-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.channel-mode-title {
  color: rgba(255, 255, 255, 0.85);
  font-size: 13px;
  font-weight: 600;
}

.channel-mode-pill {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: rgba(255, 255, 255, 0.75);
}

.channel-mode-pill.ok {
  color: #67c23a;
  border-color: rgba(103, 194, 58, 0.4);
  background: rgba(103, 194, 58, 0.1);
}

.channel-mode-pill.warn {
  color: #e6a23c;
  border-color: rgba(230, 162, 60, 0.4);
  background: rgba(230, 162, 60, 0.1);
}

.channel-mode-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 14px;
  color: rgba(255, 255, 255, 0.55);
  font-size: 12px;
  margin-bottom: 10px;
}

.channel-mode-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.channel-mode-hint {
  margin: 8px 0 0 0;
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
}

.channel-mode-hint.warning {
  color: #e6a23c;
}

/* 安装目录 */
.install-dir-section { margin-bottom:16px; }
.install-dir-label {
  display:block; font-size:13px; font-weight:500;
  color:rgba(255,255,255,0.7); margin-bottom:6px;
}
.install-dir-row { display:flex; gap:6px; }
.install-dir-row .dl-input { flex:1; }
.install-dir-row .icon-btn {
  display:flex; align-items:center; justify-content:center;
  color:rgba(255,255,255,0.6);
}
.install-dir-row .icon-btn:hover { color:#fff; }
.install-dir-hint {
  font-size:11px; color:rgba(255,255,255,0.35); margin-top:6px; line-height:1.4;
}

/* 语言包选择 */
.lang-section { margin-bottom:16px; }
.lang-label {
  display:block; font-size:13px; font-weight:500;
  color:rgba(255,255,255,0.7); margin-bottom:8px;
}
.lang-options {
  display:flex; flex-wrap:wrap; gap:8px;
}
.lang-checkbox {
  display:flex; align-items:center; gap:6px;
  padding:6px 12px; border-radius:6px;
  background:rgba(255,255,255,0.06); border:1px solid rgba(255,255,255,0.08);
  cursor:pointer; transition:all 0.2s; user-select:none;
}
.lang-checkbox:hover { background:rgba(255,255,255,0.1); border-color:rgba(255,255,255,0.15); }
.lang-checkbox input[type="checkbox"] {
  accent-color:#F7CE46; width:14px; height:14px; cursor:pointer;
}
.lang-name { font-size:13px; color:rgba(255,255,255,0.85); }
.lang-hint {
  font-size:11px; color:#f0a030; margin-top:6px; line-height:1.4;
}

/* 主要操作按钮 */
.main-actions { display:flex; gap:8px; margin-bottom:16px; flex-wrap:wrap; }
.protection-status {
  width: 100%;
  font-size: 12px;
  border-radius: 6px;
  padding: 8px 10px;
  border: 1px solid rgba(255,255,255,0.12);
  background: rgba(255,255,255,0.06);
}
.protection-status.enabled {
  color: #67c23a;
  border-color: rgba(103,194,58,0.35);
  background: rgba(103,194,58,0.12);
}
.protection-status.disabled {
  color: #f0a030;
  border-color: rgba(240,160,48,0.4);
  background: rgba(240,160,48,0.12);
}
.protection-status.neutral {
  color: rgba(255,255,255,0.5);
}
.action-btn {
  padding:8px 16px; border:none; border-radius:6px; font-size:13px; cursor:pointer;
  color:#fff; background:rgba(255,255,255,0.1); transition:all 0.2s;
  display:inline-flex; align-items:center; gap:6px;
}
.action-btn:hover { background:rgba(255,255,255,0.18); }
.action-btn:disabled { opacity:0.5; cursor:not-allowed; }
.action-btn.primary {
  background:rgba(247,206,70,0.2); color:#F7CE46; border:1px solid rgba(247,206,70,0.3);
}
.action-btn.primary:hover { background:rgba(247,206,70,0.3); }
.action-btn.primary.large { padding:12px 24px; font-size:15px; font-weight:600; }
.action-btn.danger { background:rgba(232,17,35,0.2); color:#ff6b6b; margin-top:8px; }
.action-btn.danger:hover { background:rgba(232,17,35,0.3); }
.action-btn.danger-soft {
  background: rgba(232, 17, 35, 0.12);
  color: #ff8f8f;
  border: 1px solid rgba(232, 17, 35, 0.24);
}
.action-btn.danger-soft:hover {
  background: rgba(232, 17, 35, 0.2);
}
.action-btn.sm { padding:6px 12px; font-size:12px; margin-top:8px; }

/* 进度条 */
.progress-section {
  background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06);
  border-radius:8px; padding:16px; margin-bottom:16px;
}
.progress-phase {
  font-size:13px; font-weight:600; color:rgba(255,255,255,0.8); margin-bottom:10px;
}
.progress-bar-track {
  width:100%; height:10px; background:rgba(255,255,255,0.08); border-radius:5px; overflow:hidden;
}
.progress-bar-fill {
  height:100%; background:linear-gradient(90deg, #F7CE46, #f0a030); border-radius:5px;
  transition: width 0.3s ease;
}
.progress-bar-fill.verify-fill {
  background:linear-gradient(90deg, #67c23a, #4caf50);
}
.progress-info {
  display:flex; justify-content:space-between; font-size:13px;
  color:rgba(255,255,255,0.7); margin-top:8px;
}
.progress-detail {
  display:flex; justify-content:space-between; font-size:11px;
  color:rgba(255,255,255,0.4); margin-top:4px;
}
.progress-file {
  max-width:280px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
}
.progress-counts {
  font-size:11px; color:rgba(255,255,255,0.35); margin-top:4px;
}

/* 折叠配置 */
.config-details {
  margin:16px 0;
}
.config-details summary {
  font-size:12px; color:rgba(255,255,255,0.4); cursor:pointer;
  padding:6px 0; user-select:none;
}
.config-details summary:hover { color:rgba(255,255,255,0.7); }
.config-content {
  margin-top:10px; padding:12px; background:rgba(0,0,0,0.15);
  border:1px solid rgba(255,255,255,0.04); border-radius:6px;
}
.field { margin-bottom:10px; }
.field label { display:block; font-size:12px; color:rgba(255,255,255,0.5); margin-bottom:4px; }
.dl-input {
  width:100%; box-sizing:border-box; background:rgba(0,0,0,0.3);
  border:1px solid rgba(255,255,255,0.1); border-radius:4px;
  padding:8px 10px; color:#fff; font-size:13px; outline:none;
}
.dl-input:focus { border-color:#F7CE46; }
.input-row { display:flex; gap:6px; }
.input-row .dl-input { flex:1; }
.icon-btn {
  background:rgba(255,255,255,0.08); border:1px solid rgba(255,255,255,0.1);
  border-radius:4px; padding:6px 10px; cursor:pointer; font-size:14px;
}
.icon-btn:hover { background:rgba(255,255,255,0.15); }

/* 分隔线 */
.divider {
  display:flex; align-items:center; gap:12px; margin:20px 0;
  color:rgba(255,255,255,0.3); font-size:12px;
}
.divider::before, .divider::after {
  content:''; flex:1; height:1px; background:rgba(255,255,255,0.08);
}

/* 手动安装区 */
.section { margin-bottom:16px; }
.section-title {
  font-size:14px; font-weight:600; color:rgba(255,255,255,0.8); margin-bottom:8px;
}
.hint { font-size:12px; color:rgba(255,255,255,0.4); margin-bottom:12px; line-height:1.5; }

/* 错误 */
.error-msg {
  background:rgba(232,17,35,0.12); border:1px solid rgba(232,17,35,0.25);
  border-radius:6px; padding:10px 14px; color:#ff6b6b; font-size:13px; margin-top:12px;
}

/* 过渡 */
.modal-fade-enter-active, .modal-fade-leave-active { transition:opacity 0.25s; }
.modal-fade-enter-from, .modal-fade-leave-to { opacity:0; }
</style>
