<script setup lang="ts" >
import { computed, ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { gamesList, switchToGame, appSettings, loadGames } from '../store'
import {
  showMessage,
  askConfirm,
  setGameVisibility,
  loadGameConfig,
  startGame as apiStartGame,
  checkGameProtectionStatus,
  listenEvent,
  getGameWineConfig,
  scanWineVersions,
  scanLocalDxvk,
  detectDxvkStatus,
  resolveDownloadedGameExecutable,
  openGameLogWindow,
} from '../api'
import GameSettingsModal from '../components/GameSettingsModal.vue'
import GameDownloadModal from '../components/GameDownloadModal.vue'
import { dlState } from '../downloadStore'

import { useI18n  } from 'vue-i18n';



const { t, te } = useI18n()
const router = useRouter()
const getGameName = (game: any) => te(`games.${game.name}`) ? t(`games.${game.name}`) : (game.displayName || game.name)
const hasCurrentGame = computed(() => {
  const gameName = appSettings.currentConfigName;
  return !!gameName && gameName !== 'Default';
});


// Computed property to get sidebar games (filtered and reverse order)
const sidebarGames = computed(() => {
  return gamesList
    .filter(g => g.showSidebar)
    .reverse();
});

const isGameActive = (gameName: string) => {
  return appSettings.currentConfigName === gameName;
};

const handleGameClick = (game: any) => {
  switchToGame(game);
}

// Context Menu State
const showMenu = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const targetGame = ref<any>(null);

const handleContextMenu = (e: MouseEvent, game: any) => {
  e.preventDefault();
  targetGame.value = game;
  menuX.value = e.clientX;
  menuY.value = e.clientY;
  showMenu.value = true;
};

const closeMenu = () => {
  showMenu.value = false;
};

const hideGame = async () => {
  if (!targetGame.value) return;

  const gameName = targetGame.value.name;
  const wasActive = isGameActive(gameName);

  try {
    await setGameVisibility(gameName, true);
    await loadGames();

    // If the hidden game was active, switch to the first available game
    if (wasActive && sidebarGames.value.length > 0) {
      switchToGame(sidebarGames.value[0]);
    }
  } catch (err) {
    console.error(t('home.hidegame.fail'), err);
  }

  closeMenu();
};

const showSettings = ref(false);
const showDownload = ref(false);

const currentDisplayName = computed(() => {
  const game = gamesList.find(g => g.name === appSettings.currentConfigName);
  return game?.displayName || appSettings.currentConfigName;
});
const settingsModalRef = ref<InstanceType<typeof GameSettingsModal> | null>(null);

type RuntimeFocusTarget = 'all' | 'wine_version' | 'dxvk' | 'vkd3d';
type GlobalSettingsMenu = 'proton' | 'dxvk' | 'vkd3d';
type OnboardingSettingsTab = 'info' | 'game' | 'runtime' | 'system';
type OnboardingHomeAction =
  | { type: 'open_download_modal' }
  | { type: 'open_game_settings'; tab: OnboardingSettingsTab; runtimeFocus?: RuntimeFocusTarget }
  | { type: 'close_modals' };

const openGlobalSettingsMenu = async (
  menu: GlobalSettingsMenu,
  reason?: string,
) => {
  showSettings.value = false;
  await router.push({
    path: '/settings',
    query: {
      menu,
      guide: '1',
      reason: reason || '',
      t: String(Date.now()),
    },
  });
};

const openRuntimeSettings = async (reason?: string, focusTarget: RuntimeFocusTarget = 'all') => {
  showSettings.value = true;
  await nextTick();
  settingsModalRef.value?.switchTab?.('runtime');
  settingsModalRef.value?.focusRuntimeSetup?.(reason, focusTarget);

  // 二次触发，避免首次打开时被子组件初始化流程覆盖（导致无动画/未聚焦）
  setTimeout(() => {
    settingsModalRef.value?.switchTab?.('runtime');
    settingsModalRef.value?.focusRuntimeSetup?.(reason, focusTarget);
  }, 260);
};

const openGameSettingsGameTab = async () => {
  showSettings.value = true;
  await nextTick();
  settingsModalRef.value?.switchTab?.('game');
  setTimeout(() => {
    settingsModalRef.value?.switchTab?.('game');
  }, 220);
};

const openGameSettingsTab = async (tab: OnboardingSettingsTab, runtimeFocus: RuntimeFocusTarget = 'all') => {
  showDownload.value = false;
  showSettings.value = true;
  await nextTick();
  settingsModalRef.value?.switchTab?.(tab);
  if (tab === 'runtime') {
    settingsModalRef.value?.focusRuntimeSetup?.('', runtimeFocus);
  }
  setTimeout(() => {
    settingsModalRef.value?.switchTab?.(tab);
    if (tab === 'runtime') {
      settingsModalRef.value?.focusRuntimeSetup?.('', runtimeFocus);
    }
  }, 220);
};

const applyOnboardingHomeAction = async (detail?: OnboardingHomeAction) => {
  if (!detail) return;
  if (detail.type === 'close_modals') {
    showSettings.value = false;
    showDownload.value = false;
    return;
  }
  if (detail.type === 'open_download_modal') {
    showSettings.value = false;
    showDownload.value = true;
    return;
  }
  if (detail.type === 'open_game_settings') {
    await openGameSettingsTab(detail.tab, detail.runtimeFocus || 'all');
  }
};

const onOnboardingActionEvent = (event: Event) => {
  const detail = (event as CustomEvent<OnboardingHomeAction>).detail;
  void applyOnboardingHomeAction(detail);
};

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
  if (!normalized) return;
  if (!arr.includes(normalized)) {
    arr.push(normalized);
  }
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

const SHOULD_CHECK_EXECUTABLE_PRESETS = new Set([
  'wutheringwaves',
  'honkaistarrail',
  'zenlesszonezero',
]);

const checkExecutablePathMismatch = async (gameName: string, gameConfig: any) => {
  const presetRaw = String(
    gameConfig?.basic?.gamePreset ||
      gameConfig?.basic?.GamePreset ||
      gameConfig?.GamePreset ||
      gameName,
  ).trim();
  const preset = presetRaw.toLowerCase();
  if (!SHOULD_CHECK_EXECUTABLE_PRESETS.has(preset)) {
    return;
  }

  const configuredPath = normalizePathForCompare(String(gameConfig?.other?.gamePath || ''));
  if (!configuredPath) return;

  const launcherApi = String(gameConfig?.other?.launcherApi || '').trim();
  const configuredGameFolder = String(gameConfig?.other?.gameFolder || '').trim();
  const roots = buildExecutableCheckRoots(preset, configuredPath, configuredGameFolder);
  if (roots.length === 0) return;

  let detectedPath: string | null = null;
  for (const root of roots) {
    try {
      detectedPath = await resolveDownloadedGameExecutable(
        presetRaw,
        root,
        launcherApi || undefined,
      );
    } catch {
      detectedPath = null;
    }
    if (detectedPath) break;
  }

  const detected = normalizePathForCompare(String(detectedPath || ''));
  if (!detected || detected === configuredPath) {
    return;
  }

  await showMessage(
    `检测到主程序路径可能不匹配。\n当前配置：${configuredPath}\n推荐主程序：${detected}\n\n已自动跳转到“游戏设置 -> 游戏选项”。本次不会阻止启动。`,
    { title: '主程序路径提醒', kind: 'warning' },
  );
  await openGameSettingsGameTab();
};

const ensureRuntimeReady = async (gameName: string, gameConfig: any, wineVersionId: string) => {
  const runtimeEnv = String(gameConfig?.basic?.runtimeEnv || 'wine').toLowerCase();
  if (runtimeEnv !== 'wine') {
    return true;
  }

  let versions: any[] = [];
  let localDxvk: any[] = [];
  let dxvkInstalledInPrefix = false;
  let dxvkStatusCached: any | null = null;
  try {
    versions = await scanWineVersions();
  } catch (e: any) {
    await showMessage(`运行环境扫描失败，请在“游戏设置 -> 运行环境”检查 Proton 配置：${e}`, {
      title: '运行环境检查失败',
      kind: 'error',
    });
    await openGlobalSettingsMenu('proton', '运行环境扫描失败，请先配置 Proton');
    return false;
  }

  try {
    localDxvk = await scanLocalDxvk();
  } catch (e) {
    console.warn('[launch] DXVK 本地缓存扫描失败:', e);
  }

  try {
    const dxvkStatus = await detectDxvkStatus(gameName);
    dxvkStatusCached = dxvkStatus;
    dxvkInstalledInPrefix = !!dxvkStatus.installed;
  } catch (e) {
    console.warn('[launch] DXVK 安装状态检测失败:', e);
  }

  if (versions.length === 0) {
    await showMessage(
      '未检测到任何 Wine/Proton 版本。\n请先打开“设置 -> Proton 管理”下载安装版本后再启动。',
      { title: '缺少 Proton', kind: 'warning' },
    );
    await openGlobalSettingsMenu('proton', '请先在此下载 Proton 版本。');
    return false;
  }

  if (localDxvk.length === 0 && !dxvkInstalledInPrefix) {
    await showMessage(
      '未检测到可用 DXVK（本地无缓存且当前 Prefix 未安装）。\n请先打开“设置 -> DXVK 管理”下载安装版本。',
      { title: '缺少 DXVK', kind: 'warning' },
    );
    await openGlobalSettingsMenu('dxvk', '请先在此下载 DXVK 版本。');
    return false;
  }

  if (!wineVersionId?.trim()) {
    await showMessage(
      '当前游戏尚未选择 Wine/Proton 版本。\n请先打开“游戏设置 -> 运行环境”进行设置。',
      { title: '未设置 Proton', kind: 'warning' },
    );
    await openRuntimeSettings('请先在此选择或下载 Wine/Proton 版本。', 'wine_version');
    return false;
  }

  const selected = versions.find((v) => v.id === wineVersionId);
  if (!selected) {
    await showMessage(
      `当前配置的 Wine/Proton 版本不存在：${wineVersionId}\n请在“游戏设置 -> 运行环境”重新选择可用版本。`,
      { title: 'Proton 配置无效', kind: 'warning' },
    );
    await openRuntimeSettings('当前 Wine/Proton 配置无效，请重新选择。', 'wine_version');
    return false;
  }

  try {
    const dxvkStatus = dxvkStatusCached || (await detectDxvkStatus(gameName));
    if (!dxvkStatus.installed) {
      const openNow = await askConfirm(
        '检测到当前 Prefix 未安装 DXVK。\n这可能导致 DirectX 11/12 游戏黑屏、崩溃或无法启动。\n\n是否现在打开“游戏设置 -> 运行环境”安装 DXVK？',
        {
          title: '缺少 DXVK',
          kind: 'warning',
          okLabel: '打开运行环境',
          cancelLabel: '继续启动',
        },
      );
      if (openNow) {
        await openRuntimeSettings('请在此安装 DXVK（建议优先官方 DXVK）。', 'dxvk');
        return false;
      }
    }
  } catch (e) {
    console.warn('[launch] DXVK 检测失败:', e);
  }

  return true;
};

// 检查当前游戏是否已配置可执行文件
const gameHasExe = ref(false);
const gameExeCache = new Map<string, boolean>();
let checkGameExeToken = 0;

const checkGameExe = async (force = false) => {
  const gameName = appSettings.currentConfigName;
  if (!gameName || gameName === 'Default') {
    gameHasExe.value = false;
    return;
  }
  if (!force && gameExeCache.has(gameName)) {
    gameHasExe.value = !!gameExeCache.get(gameName);
    return;
  }
  const token = ++checkGameExeToken;
  try {
    const data = await loadGameConfig(gameName);
    if (token !== checkGameExeToken) return;
    const hasExe = !!(data.other?.gamePath);
    gameExeCache.set(gameName, hasExe);
    gameHasExe.value = hasExe;
  } catch {
    if (token !== checkGameExeToken) return;
    gameExeCache.set(gameName, false);
    gameHasExe.value = false;
  }
};

// 组件下载进度（Proton/DXVK）
interface ComponentDlProgress {
  component: string;
  phase: string;
  downloaded: number;
  total: number;
}
const componentDlProgress = ref<ComponentDlProgress | null>(null);

// Start Game Logic
const isLaunching = ref(false);
const isGameRunning = ref(false);
const runningGameName = ref('');

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

const ensureProtectionEnabled = async (gameName: string, gameConfig: any) => {
  try {
    const preset = gameConfig?.basic?.gamePreset || gameConfig?.GamePreset || gameName;
    const exePath = String(gameConfig?.other?.gamePath || '').trim();
    const gameRoot = (() => {
      // 优先使用配置的游戏根目录
      const folder = String(gameConfig?.other?.gameFolder || '').trim();
      if (folder) return folder;
      // 回退到可执行文件的父目录
      if (!exePath) return undefined;
      const normalized = exePath.replace(/\\/g, '/');
      const idx = normalized.lastIndexOf('/');
      return idx > 0 ? normalized.slice(0, idx) : undefined;
    })();

    let status = await checkGameProtectionStatus(preset, gameRoot);
    if (status?.enforceAtLaunch !== true && preset !== gameName) {
      const fallback = await checkGameProtectionStatus(gameName, gameRoot);
      if (fallback?.enforceAtLaunch === true) {
        status = fallback;
      }
    }
    const enforceAtLaunch = status?.enforceAtLaunch === true;
    if (!enforceAtLaunch) {
      const missing = Array.isArray(status?.missing) ? status.missing : [];
      if (missing.length > 0) {
        await showMessage(
          `当前为告警模式，不阻止启动。\n建议处理以下项：\n- ${missing.join('\n- ')}`,
          { title: '防护告警', kind: 'warning' },
        );
      }
      return true;
    }

    const enabled = !!status?.enabled;
    if (enabled) return true;

    const missing = Array.isArray(status?.missing) && status.missing.length > 0
      ? `\n\n未满足项：\n- ${status.missing.join('\n- ')}`
      : '';

    await showMessage(
      `未启用应用防护，当前禁止启动游戏。\n请通过菜单"下载/防护管理"应用安全防护。${missing}`,
      { title: '需要应用防护', kind: 'warning' }
    );
    showDownload.value = true;
    return false;
  } catch (e: any) {
    await showMessage(`无法确认防护状态，已阻止启动：${e}`, { title: '防护检查失败', kind: 'error' });
    showDownload.value = true;
    return false;
  }
};

const launchGame = async () => {
  // 立即检查，防止竞态条件
  if (isLaunching.value || isGameRunning.value) {
    console.log('游戏正在启动或已运行，忽略重复点击');
    return;
  }
  // 先置位，避免 await 期间重复触发
  isLaunching.value = true;

  const gameName = appSettings.currentConfigName;
  if (!gameName || gameName === 'Default') {
    await showMessage('请先选择一个游戏配置', { title: '提示', kind: 'info' });
    isLaunching.value = false;
    return;
  }

  if (!(await ensureRiskAcknowledged())) {
    isLaunching.value = false;
    return;
  }

  try {
      // Load game config to resolve paths
      const data = await loadGameConfig(gameName);
      if (!(await ensureProtectionEnabled(gameName, data))) {
        isLaunching.value = false;
        return;
      }

      const gameExePath = data.other?.gamePath || '';
      // wineVersionId 优先从 wine config 读取，回退到 config.other
      let wineVersionId = data.other?.wineVersionId || '';
      if (!wineVersionId) {
        try {
          const wineConfig = await getGameWineConfig(gameName);
          wineVersionId = wineConfig.wine_version_id || '';
        } catch { /* ignore */ }
      }
      
      if (!gameExePath) {
        await showMessage('请先在游戏设置中配置游戏可执行文件路径', { title: '提示', kind: 'info' });
        isLaunching.value = false;
        return;
      }

      if (!(await ensureRuntimeReady(gameName, data, wineVersionId))) {
        isLaunching.value = false;
        return;
      }

      // 主程序路径不匹配仅提示，不阻止启动
      await checkExecutablePathMismatch(gameName, data);

      await apiStartGame(gameName, gameExePath, wineVersionId);
      // 启动成功后，等待 game-lifecycle 事件来更新状态
      // 不在这里重置 isLaunching，由事件处理器负责
      
  } catch (e: any) {
    console.error('Start Game Error:', e);
    // 只在启动失败时重置状态
    isLaunching.value = false;
    const errText = String(e || '');
    if (
      errText.includes('Wine/Proton') ||
      errText.includes('运行环境') ||
      errText.includes('启动配置错误')
    ) {
      const openNow = await askConfirm(
        `启动失败（运行环境问题）：\n${errText}\n\n是否现在打开“游戏设置 -> 运行环境”进行修复？`,
        {
          title: '运行环境错误',
          kind: 'error',
          okLabel: '打开运行环境',
          cancelLabel: '稍后处理',
        },
      );
      if (openNow) {
        await openRuntimeSettings('请先修复运行环境配置（Proton / DXVK）。', 'wine_version');
        return;
      }
    }
    await showMessage(`启动失败: ${errText}`, { title: '错误', kind: 'error' });
  }
}

const openCurrentGameLog = async () => {
  const gameName = appSettings.currentConfigName;
  if (!gameName || gameName === 'Default') {
    await showMessage('请先选择一个游戏配置', { title: '提示', kind: 'info' });
    return;
  }
  try {
    await openGameLogWindow(gameName);
  } catch (e: any) {
    await showMessage(`打开游戏日志窗口失败: ${e}`, { title: '错误', kind: 'error' });
  }
}

watch(() => appSettings.currentConfigName, () => {
  checkGameExe(false);
});

let unlistenLifecycle: (() => void) | null = null;
let unlistenComponentDl: (() => void) | null = null;
let unlistenAnticheat: (() => void) | null = null;

onMounted(async () => {
  document.addEventListener('click', closeMenu);
  window.addEventListener('ssmt4-onboarding-action', onOnboardingActionEvent as EventListener);
  checkGameExe();
  unlistenLifecycle = await listenEvent('game-lifecycle', (event: any) => {
    const data = event.payload;
    if (data.event === 'started') {
      // 游戏启动成功，更新状态
      isGameRunning.value = true;
      runningGameName.value = data.game || '';
      isLaunching.value = false;
    } else if (data.event === 'exited') {
      // 游戏退出，重置所有状态
      isGameRunning.value = false;
      runningGameName.value = '';
      isLaunching.value = false;
    }
  });
  unlistenComponentDl = await listenEvent('component-download-progress', (event: any) => {
    const data = event.payload as ComponentDlProgress;
    if (data.phase === 'done') {
      componentDlProgress.value = null;
    } else {
      componentDlProgress.value = data;
    }
  });
  unlistenAnticheat = await listenEvent('game-anticheat-warning', (event: any) => {
    const data = event.payload;
    showMessage(data.message, { title: '⚠️ 反作弊风险警告', kind: 'warning' });
  });
});

onUnmounted(() => {
  document.removeEventListener('click', closeMenu);
  window.removeEventListener('ssmt4-onboarding-action', onOnboardingActionEvent as EventListener);
  if (unlistenLifecycle) unlistenLifecycle();
  if (unlistenComponentDl) unlistenComponentDl();
  if (unlistenAnticheat) unlistenAnticheat();
});
</script>

<template>
  <div class="home-container">
    
    <!-- Bright Tech Sci-Fi Background Layer -->
    <div class="hero-layer">
      <!-- Clean background image with lighter blur to let global bg peek through when no image -->
      <div 
        class="hero-image" 
        :style="{ backgroundImage: (targetGame && targetGame.iconPath) ? `url(${targetGame.iconPath})` : 'none' }"
        :class="{ 'has-image': targetGame && targetGame.iconPath }"
      ></div>
      
      <!-- Tech Grid Overlay (Subtle) -->
      <div class="tech-grid-overlay"></div>
      
      <!-- Light Vignette -->
      <div class="hero-overlay"></div>
    </div>

    <!-- Main Content Area centered -->
    <div class="content-area">

      <!-- Action & Sidebar Area - Now a floating bottom/center dashboard -->
      <div class="dashboard-panel">
        
        <!-- Game Selection (Dock) -->
        <div class="games-dock" data-onboarding="home-games-dock">
          <!-- Empty state: guide to Game Library -->
          <el-tooltip v-if="sidebarGames.length === 0" content="添加游戏到侧边栏" placement="top" effect="dark" popper-class="game-tooltip">
            <div class="dock-icon add-game-btn" @click="router.push('/games')">
              <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="12" y1="5" x2="12" y2="19"></line>
                <line x1="5" y1="12" x2="19" y2="12"></line>
              </svg>
            </div>
          </el-tooltip>

          <!-- Games Loop -->
          <el-tooltip v-for="game in sidebarGames" :key="game.name" :content="getGameName(game)" placement="top" effect="dark" popper-class="game-tooltip">
            <div class="dock-icon" :class="{ active: isGameActive(game.name) }" @click.stop="handleGameClick(game)"
              @contextmenu.prevent="handleContextMenu($event, game)">
              <div class="icon-glow" v-if="isGameActive(game.name)"></div>
              <img :src="game.iconPath" :alt="game.name" loading="lazy"
                @load="(e) => (e.target as HTMLImageElement).style.opacity = '1'"
                @error="(e) => (e.target as HTMLImageElement).style.opacity = '0'" />
            </div>
          </el-tooltip>
        </div>

        <div class="divider"></div>

        <!-- Start Game Button & Settings -->
        <div class="action-bar">
          <div class="start-game-wrapper">
             <div class="start-game-btn" data-onboarding="home-start-button" @click="(isGameRunning || isLaunching) ? null : (gameHasExe ? launchGame() : (showDownload = true))" :class="{ 'disabled': isLaunching, 'running': isGameRunning }">
               <div class="btn-background-fx"></div>
               <div class="icon-wrapper">
                 <div class="play-triangle" v-if="gameHasExe && !isGameRunning"></div>
                 <div v-else-if="isGameRunning" class="running-indicator"></div>
                 <svg v-else xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
               </div>
               <span class="btn-text">{{ isGameRunning ? '游戏中' : (gameHasExe ? t('home.css.startgame') : t('home.css.downloadgame')) }}</span>
             </div>
          </div>

          <!-- Settings Menu Button -->
          <el-dropdown trigger="hover" placement="top-end" popper-class="settings-dropdown">
            <div class="settings-btn" data-onboarding="home-settings-button">
              <svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-settings-2"><path d="M20 7h-9"/><path d="M14 17H5"/><circle cx="17" cy="7" r="3"/><circle cx="8" cy="17" r="3"/></svg>
            </div>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click="showSettings = true" :disabled="!hasCurrentGame">
                  <span style="display: flex; align-items: center; gap: 8px;">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
                    {{ t('home.dropdown.gamesettings') }}
                  </span>
                </el-dropdown-item>
                <el-dropdown-item divided @click="showDownload = true" :disabled="!hasCurrentGame">
                  <span style="display: flex; align-items: center; gap: 8px;">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17V3"></path><path d="m6 11 6 6 6-6"></path><path d="M19 21H5"></path></svg>
                    下载/防护管理
                  </span>
                </el-dropdown-item>
                <el-dropdown-item divided @click="openCurrentGameLog" :disabled="!hasCurrentGame">
                  <span style="display: flex; align-items: center; gap: 8px;">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20v-6"></path><path d="M9 20h6"></path><path d="M5 8a7 7 0 1 1 14 0v6a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2z"></path></svg>
                    打开游戏日志窗口
                  </span>
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>

      </div>

      <!-- Mini download progress -->
      <div class="notifications-area">
        <div v-if="dlState.active && !showDownload" class="mini-dl-bar glass-panel" @click="showDownload = true">
          <div class="mini-dl-info">
            <span class="mini-dl-name">{{ dlState.displayName || dlState.gameName }}</span>
            <span class="mini-dl-phase">{{ dlState.phase === 'verifying' ? '校验中' : (dlState.progress?.phase === 'install' ? '安装中' : '下载中') }}</span>
            <span class="mini-dl-pct" v-if="dlState.progress && dlState.progress.total_size > 0">
              {{ Math.round((dlState.progress.finished_size / dlState.progress.total_size) * 100) }}%
            </span>
          </div>
          <div class="mini-dl-track">
            <div class="mini-dl-fill"
              :class="{ 'mini-dl-verify': dlState.phase === 'verifying' || dlState.progress?.phase === 'verify' }"
              :style="{ width: (dlState.progress && dlState.progress.total_size > 0 ? Math.round((dlState.progress.finished_size / dlState.progress.total_size) * 100) : 0) + '%' }"></div>
          </div>
        </div>

        <!-- Proton/DXVK 组件下载进度 toast -->
        <div v-if="componentDlProgress" class="mini-dl-bar glass-panel component-dl">
          <div class="mini-dl-info">
            <span class="mini-dl-name">{{ componentDlProgress.component }}</span>
            <span class="mini-dl-phase">{{ componentDlProgress.phase === 'downloading' ? '下载中' : componentDlProgress.phase === 'extracting' ? '解压中' : componentDlProgress.phase }}</span>
            <span class="mini-dl-pct" v-if="componentDlProgress.total > 0 && componentDlProgress.phase === 'downloading'">
              {{ Math.round(componentDlProgress.downloaded / componentDlProgress.total * 100) }}%
            </span>
          </div>
          <div class="mini-dl-track">
            <div class="mini-dl-fill"
              :class="{ 'mini-dl-verify': componentDlProgress.phase === 'extracting' }"
              :style="{ width: componentDlProgress.total > 0 ? Math.round(componentDlProgress.downloaded / componentDlProgress.total * 100) + '%' : '100%' }"></div>
          </div>
        </div>
      </div>

    </div>

    <!-- Custom Context Menu -->
    <div v-if="showMenu" class="context-menu glass-panel" :style="{ top: menuY + 'px', left: menuX + 'px' }" @click.stop>
      <div class="menu-item" @click="hideGame">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px;"><path d="m2 2 20 20"/><path d="M6.71 6.71A10.61 10.61 0 0 0 2 12a10.54 10.54 0 0 0 11.29 9 10.46 10.46 0 0 0 5-1.71"/><path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a10.16 10.16 0 0 1-2.39 3.46"/><path d="M8 12a4 4 0 0 0 4 4"/><path d="M12 8a4 4 0 0 0-4 4"/></svg>
        {{ t('home.contextmenu.hidegame') }}
      </div>
    </div>

    <!-- Modals -->
    <GameSettingsModal
      v-if="showSettings"
      ref="settingsModalRef"
      v-model="showSettings"
      :game-name="appSettings.currentConfigName"
    />
    <GameDownloadModal
      v-if="showDownload"
      v-model="showDownload"
      :game-name="appSettings.currentConfigName"
      :display-name="currentDisplayName"
      @game-configured="checkGameExe(true)"
    />

  </div>
</template>

<style scoped>
/* Base Container */
.home-container {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  position: relative;
  overflow: hidden;
  color: #fff;
}

/* Background/Hero Area (Bright Tech Sci-Fi) */
.hero-layer {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 0;
  pointer-events: none;
  overflow: hidden;
  /* Removed solid #050510 to allow global App.vue bg to show through */
  background-color: transparent; 
}

.hero-image {
  width: 100%;
  height: 100%;
  background-size: cover;
  background-position: center;
  transition: opacity 0.35s ease-in-out;
  opacity: 0; /* Hidden by default if no image to let global BG show */
}

.hero-image.has-image {
  opacity: 0.92;
  filter: none;
}

.tech-grid-overlay {
  position: absolute;
  top: 0; left: 0; width: 100%; height: 100%;
  background-image: 
    linear-gradient(rgba(0, 240, 255, 0.03) 1px, transparent 1px),
    linear-gradient(90deg, rgba(0, 240, 255, 0.03) 1px, transparent 1px);
  background-size: 40px 40px;
  z-index: 1;
  opacity: 0.5;
}

.hero-overlay {
  position: absolute;
  top: 0; left: 0; width: 100%; height: 100%;
  /* Light vignette, not pitch black */
  background: radial-gradient(circle at center 60%, transparent 20%, rgba(0, 15, 25, 0.4) 100%);
  z-index: 2;
}

/* Main Content Area */
.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  align-items: center;
  padding: 40px;
  position: relative;
  z-index: 10;
}

/* 
  Bright Tech Dashboard Panel 
*/
.dashboard-panel {
  display: flex;
  align-items: center;
  gap: 24px;
  background: rgba(255, 255, 255, 0.08); /* Light translucent base */
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  border: 1px solid rgba(0, 240, 255, 0.4); /* Bright cyan border */
  border-radius: 12px; /* Sharper corners for tech feel */
  padding: 8px 24px;
  box-shadow: 0 10px 40px rgba(0, 240, 255, 0.1), 
              inset 0 0 20px rgba(255, 255, 255, 0.05);
  position: relative;
  animation: none;
}

/* Tech Brackets (HUD Corners) */
.dashboard-panel::before,
.dashboard-panel::after {
  content: '';
  position: absolute;
  width: 20px;
  height: 20px;
  border: 2px solid #00f0ff;
  border-radius: 4px;
  z-index: 5;
  pointer-events: none;
}

.dashboard-panel::before {
  top: -2px; left: -2px;
  border-right: none; border-bottom: none;
}

.dashboard-panel::after {
  bottom: -2px; right: -2px;
  border-left: none; border-top: none;
  box-shadow: none;
}

@keyframes slideUpFade {
  0% { opacity: 0; transform: translateY(30px); }
  100% { opacity: 1; transform: translateY(0); }
}

/* Divider inside Dashboard */
.divider {
  width: 2px;
  height: 40px;
  background: linear-gradient(to bottom, transparent, rgba(255, 255, 255, 0.4), transparent);
}

/* Games Dock */
.games-dock {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 24px 8px;
  max-width: 60vw;
  overflow-x: auto;
  overflow-y: hidden;
  scroll-behavior: smooth;
}

.games-dock::-webkit-scrollbar { height: 4px; }
.games-dock::-webkit-scrollbar-track { background: transparent; }
.games-dock::-webkit-scrollbar-thumb { background: rgba(0, 240, 255, 0.5); border-radius: 2px; }
.games-dock::-webkit-scrollbar-thumb:hover { background: #00f0ff; }

/* Sci-Fi Crisp Hover Dock Icons */
.dock-icon {
  flex-shrink: 0;
  width: 64px;
  height: 64px;
  border-radius: 12px; /* Sharper */
  position: relative;
  cursor: pointer;
  background-color: rgba(255,255,255,0.05);
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1); 
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.dock-icon img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 11px;
  position: relative;
  z-index: 3;
  transition: opacity 0.2s ease;
}

/* Crisp Pop Hover */
.dock-icon:hover {
  transform: translateY(-6px) scale(1.1);
  border-color: #00f0ff;
  box-shadow: 0 10px 20px rgba(0, 0, 0, 0.4),
              0 0 15px rgba(0, 240, 255, 0.6);
}

/* Active Game State */
.dock-icon.active {
  transform: translateY(-4px) scale(1.15);
  border-color: #fff;
  box-shadow: 0 8px 15px rgba(0,0,0,0.5),
              0 0 20px #fff;
  z-index: 10;
}

/* Mechanical Scanning Line for Active Game */
.dock-icon.active::after {
  content: '';
  position: absolute;
  top: 0; left: 0; right: 0; bottom: 0;
  border-radius: 11px;
  z-index: 4;
  pointer-events: none;
  background: linear-gradient(to bottom, transparent 40%, rgba(255, 255, 255, 0.4) 50%, transparent 60%);
  background-size: 100% 100%;
  animation: none;
}

@keyframes techScan {
  0% { background-position: 0% -100%; }
  100% { background-position: 0% 200%; }
}

/* Remove pulseGlow */
.icon-glow {
  display: none;
}

/* Add Game Button */
.dock-icon.add-game-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px dashed rgba(255, 255, 255, 0.3);
  color: #fff;
}

.dock-icon.add-game-btn:hover {
  border-color: #00f0ff;
  color: #00f0ff;
  background-color: rgba(0, 240, 255, 0.1);
}

/* Add Game Button */
.dock-icon.add-game-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px dashed rgba(255, 255, 255, 0.2);
  background-color: rgba(255, 255, 255, 0.05);
  color: rgba(255, 255, 255, 0.5);
}

.dock-icon.add-game-btn:hover {
  border-color: rgba(255, 255, 255, 0.6);
  color: #fff;
  background-color: rgba(255, 255, 255, 0.1);
}

/* 
  Action Bar
*/
.action-bar {
  display: flex;
  align-items: center;
  gap: 16px;
}

.start-game-wrapper {
  position: relative;
}

/* Bright Sci-Fi Start Game Button */
.start-game-btn {
  position: relative;
  background: #00f0ff; /* Solid bright cyan */
  color: #000;
  display: flex;
  align-items: center;
  padding: 0 32px 0 8px;
  min-width: 220px;
  height: 60px;
  border-radius: 8px; /* Mechanical corners */
  cursor: pointer;
  overflow: hidden;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: 0 5px 15px rgba(0, 240, 255, 0.4), inset 0 2px 0 rgba(255,255,255,0.6);
  transform: skewX(-10deg); /* Tech slant */
}

/* Ensure content isn't skewed */
.start-game-btn > * {
  transform: skewX(10deg);
}

.btn-background-fx {
  position: absolute;
  top: 0; left: -35%;
  width: 50%; height: 100%;
  background: linear-gradient(90deg, transparent, rgba(255,255,255,0.8), transparent);
  transform: skewX(-20deg);
  opacity: 0.35;
  transition: none;
  animation: none;
}

@keyframes pulseSweep {
  0% { left: -100%; }
  20% { left: 200%; }
  100% { left: 200%; }
}

.start-game-btn:hover {
  background: #fff; /* Flashes white on hover */
  transform: translateY(-2px) skewX(-10deg);
  box-shadow: 0 10px 30px rgba(0, 240, 255, 0.8), inset 0 2px 0 rgba(255,255,255,1);
}

.start-game-btn:active {
  transform: translateY(2px) skewX(-10deg);
  box-shadow: 0 2px 8px rgba(0, 240, 255, 0.3);
}

.icon-wrapper {
  width: 44px;
  height: 44px;
  background-color: #000;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #00f0ff;
  z-index: 2;
  box-shadow: inset 0 0 10px rgba(0, 240, 255, 0.5);
  transition: all 0.2s;
}

.start-game-btn:hover .icon-wrapper {
  background-color: #00f0ff;
  color: #fff;
}

.play-triangle {
  width: 0; height: 0;
  border-style: solid;
  border-width: 8px 0 8px 12px;
  border-color: transparent transparent transparent currentColor;
  margin-left: 4px;
}

.btn-text {
  font-size: 18px;
  font-weight: 800;
  margin-left: 20px;
  letter-spacing: 2px;
  z-index: 2;
  font-family: 'Segoe UI', sans-serif;
  text-transform: uppercase;
}

/* Button States */
.start-game-btn.disabled {
  pointer-events: none;
  filter: grayscale(1) opacity(0.6);
  box-shadow: none;
}

.start-game-btn.running {
  pointer-events: none;
  background: linear-gradient(135deg, #2E7D32 0%, #1B5E20 100%);
  color: #A5D6A7;
  box-shadow: 0 8px 16px rgba(46, 125, 50, 0.3), inset 0 2px 0 rgba(255,255,255,0.1);
}

.start-game-btn.running .icon-wrapper {
  background-color: #0A2B0C;
  color: #4CAF50;
}

.running-indicator {
  width: 14px;
  height: 14px;
  background: #4CAF50;
  border-radius: 50%;
  box-shadow: 0 0 10px #4CAF50;
  animation: none;
}

@keyframes pulse-green {
  0% { transform: scale(0.8); box-shadow: 0 0 0 0 rgba(76, 175, 80, 0.7); }
  70% { transform: scale(1); box-shadow: 0 0 0 10px rgba(76, 175, 80, 0); }
  100% { transform: scale(0.8); box-shadow: 0 0 0 0 rgba(76, 175, 80, 0); }
}

/* Settings Button */
.settings-btn {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: rgba(255,255,255,0.05);
  border: 1px solid rgba(255,255,255,0.1);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #ddd;
  cursor: pointer;
  transition: all 0.2s;
}

.settings-btn:hover {
  background: rgba(255,255,255,0.1);
  color: #fff;
  transform: rotate(30deg);
}

/* 
  Notifications & Toasts 
*/
.notifications-area {
  position: absolute;
  top: 32px;
  right: 32px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  z-index: 100;
}

.glass-panel {
  background: rgba(20, 20, 22, 0.6);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}

.mini-dl-bar {
  width: 300px;
  padding: 12px 16px;
  cursor: pointer;
  transition: all 0.25s;
  animation: toastSlideIn 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}

.mini-dl-bar:hover {
  background: rgba(30, 30, 35, 0.8);
  border-color: rgba(247, 206, 70, 0.4);
  transform: translateY(-2px);
}

.mini-dl-info {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.mini-dl-name {
  font-size: 13px;
  font-weight: 600;
  color: #F7CE46;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 150px;
}

.mini-dl-phase {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  padding: 2px 8px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
}

.mini-dl-pct {
  font-size: 13px;
  font-weight: 700;
  margin-left: auto;
}

.mini-dl-track {
  width: 100%;
  height: 4px;
  background: rgba(0,0,0,0.5);
  border-radius: 2px;
  overflow: hidden;
}

.mini-dl-fill {
  height: 100%;
  background: linear-gradient(90deg, #F5A623, #F7CE46);
  border-radius: 2px;
  transition: width 0.3s ease;
}

.mini-dl-fill.mini-dl-verify {
  background: linear-gradient(90deg, #4CAF50, #8bc34a);
}

@keyframes toastSlideIn {
  from { opacity: 0; transform: translateX(50px) scale(0.95); }
  to   { opacity: 1; transform: translateX(0) scale(1); }
}

/* Context Menu Overrides */
.context-menu {
  position: fixed;
  z-index: 10000;
  padding: 6px;
  min-width: 140px;
}

.menu-item {
  display: flex;
  align-items: center;
  padding: 10px 14px;
  cursor: pointer;
  color: #ddd;
  font-size: 13px;
  border-radius: 6px;
  transition: all 0.2s;
}

.menu-item:hover {
  background: rgba(255, 60, 60, 0.2);
  color: #ff6b6b;
}

</style>

<style>
/* Global Settings Dropdown Overrides */
.settings-dropdown.el-popper {
  background: rgba(20, 20, 22, 0.85) !important;
  backdrop-filter: blur(8px) !important;
  border: 1px solid rgba(255, 255, 255, 0.1) !important;
  border-radius: 16px !important;
  box-shadow: 0 10px 40px rgba(0,0,0,0.5) !important;
}

.settings-dropdown .el-popper__arrow::before {
  background: rgba(20, 20, 22, 0.85) !important;
  border: 1px solid rgba(255, 255, 255, 0.1) !important;
}

.settings-dropdown .el-dropdown-menu {
  background: transparent !important;
  border: none !important;
  padding: 8px !important;
}

.settings-dropdown .el-dropdown-menu__item {
  border-radius: 8px !important;
  color: #eee !important;
  padding: 10px 16px !important;
  font-size: 14px !important;
  transition: all 0.2s !important;
}

.settings-dropdown .el-dropdown-menu__item:hover,
.settings-dropdown .el-dropdown-menu__item:focus {
  background: rgba(255, 255, 255, 0.1) !important;
  color: #fff !important;
  transform: translateX(4px);
}

.settings-dropdown .el-dropdown-menu__item--divided {
  border-top: 1px solid rgba(255, 255, 255, 0.1) !important;
  margin-top: 6px !important;
}
</style>
