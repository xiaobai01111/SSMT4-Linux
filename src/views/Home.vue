<script setup lang="ts" >
import { computed, ref, onMounted, onUnmounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { gamesList, switchToGame, appSettings, loadGames } from '../store'
import {
  showMessage,
  askConfirm,
  setGameVisibility,
  loadGameConfig,
  ensureDirectory,
  openInExplorer,
  toggleSymlink as apiToggleSymlink,
  check3dmigotoIntegrity,
  startGame as apiStartGame,
  checkGameProtectionStatus,
  getGameProtectionInfo,
  joinPath,
} from '../api'
import GameSettingsModal from '../components/GameSettingsModal.vue'
import GameDownloadModal from '../components/GameDownloadModal.vue'
import { dlState } from '../downloadStore'

import { useI18n  } from 'vue-i18n';



const { t, te } = useI18n()
const router = useRouter()
const getGameName = (game: any) => te(`games.${game.name}`) ? t(`games.${game.name}`) : (game.displayName || game.name)


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

const open3dmigotoFolder = async () => {
  const gameName = appSettings.currentConfigName;
  if (!gameName || gameName === 'Default') return;

  try {
    // We need to load the config to find the path
    const data = await loadGameConfig(gameName);
    let path = data.threeDMigoto?.installDir;

    // Default Logic (Must match Modal logic)
    if (!path && appSettings.cacheDir) {
      path = await joinPath(appSettings.cacheDir, '3Dmigoto', gameName);
    }

    if (path) {
      await ensureDirectory(path);
      await openInExplorer(path);
    } else {
      console.warn('No 3Dmigoto path found and no cache dir set.');
    }
  } catch (e) {
    console.error('Failed to open 3Dmigoto folder:', e);
  }
};

const openD3dxIni = async () => {
  const gameName = appSettings.currentConfigName;
  if (!gameName || gameName === 'Default') return;

  try {
    let path: string | undefined;
    // Load config to find path
    const data = await loadGameConfig(gameName);
    path = data.threeDMigoto?.installDir;

    // Fallback
    if (!path && appSettings.cacheDir) {
      path = await joinPath(appSettings.cacheDir, '3Dmigoto', gameName);
    }

    if (path) {
      await ensureDirectory(path);
      const iniPath = await joinPath(path, 'd3dx.ini');
      await openInExplorer(iniPath);
    }
  } catch (e) {
    console.error('Failed to open d3dx.ini:', e);
  }
};

const showSettings = ref(false);
const showDownload = ref(false);

const currentDisplayName = computed(() => {
  const game = gamesList.find(g => g.name === appSettings.currentConfigName);
  return game?.displayName || appSettings.currentConfigName;
});
const settingsModalRef = ref<InstanceType<typeof GameSettingsModal> | null>(null);

const openSettingsAndUpdate = () => {
  showSettings.value = true;
  // Wait for modal to mount/open
  setTimeout(() => {
    settingsModalRef.value?.runPackageUpdate();
  }, 100);
};

const toggleSymlink = async (enable: boolean) => {
  const gameName = appSettings.currentConfigName;
  if (!gameName || gameName === 'Default') return;

  try {
    const data = await loadGameConfig(gameName);
    let gamePath = data.threeDMigoto?.installDir;
    if (!gamePath && appSettings.cacheDir) {
      gamePath = await joinPath(appSettings.cacheDir, '3Dmigoto', gameName);
    }
    if (!gamePath) {
      await showMessage('无法确定游戏路径', { title: '错误', kind: 'error' });
      return;
    }
    await apiToggleSymlink(gamePath, enable);
    await showMessage(enable ? 'Symlink 已开启' : 'Symlink 已关闭', { title: '成功', kind: 'info' });
  } catch (e) {
    console.error('Failed to toggle symlink:', e);
    await showMessage(`操作失败: ${e}`, { title: '错误', kind: 'error' });
  }
};

// 检查当前游戏是否已配置可执行文件
const gameHasExe = ref(false);

const checkGameExe = async () => {
  const gameName = appSettings.currentConfigName;
  if (!gameName || gameName === 'Default') {
    gameHasExe.value = false;
    return;
  }
  try {
    const data = await loadGameConfig(gameName);
    gameHasExe.value = !!(data.other?.gamePath);
  } catch {
    gameHasExe.value = false;
  }
};

// Start Game Logic
const isLaunching = ref(false);

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
    const info = await getGameProtectionInfo(preset);
    if (!info?.hasProtections) return true;

    const exePath = String(gameConfig?.other?.gamePath || '').trim();
    const gameRoot = (() => {
      if (!exePath) return undefined;
      const normalized = exePath.replace(/\\/g, '/');
      const idx = normalized.lastIndexOf('/');
      return idx > 0 ? normalized.slice(0, idx) : undefined;
    })();

    const status = await checkGameProtectionStatus(preset, gameRoot);
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
  if (isLaunching.value) return;

  const gameName = appSettings.currentConfigName;
  if (!gameName || gameName === 'Default') {
    await showMessage('请先选择一个游戏配置', { title: '提示', kind: 'info' });
    return;
  }

  if (!(await ensureRiskAcknowledged())) {
    return;
  }
  
  try {
      // Load game config to resolve paths
      const data = await loadGameConfig(gameName);
      if (!(await ensureProtectionEnabled(gameName, data))) {
        return;
      }

      let gamePath = data.threeDMigoto?.installDir;
      if (!gamePath && appSettings.cacheDir) {
        gamePath = await joinPath(appSettings.cacheDir, '3Dmigoto', gameName);
      }

      const gameExePath = data.other?.gamePath || '';
      const wineVersionId = data.other?.wineVersionId || '';

      // Check 3Dmigoto Integrity（非强制，用户可选择无 Mod 启动）
      const safe = await check3dmigotoIntegrity(gameName, gamePath || '');
      if (!safe) {
          const install = await askConfirm(
              '未检测到 3Dmigoto 文件 (d3d11.dll 或 d3dx.ini)。\n\nMod 功能将不可用。\n是否现在安装 3Dmigoto？\n\n点击"取消"将直接启动游戏（无 Mod）。', 
              { title: 'Mod 组件缺失', kind: 'warning', okLabel: '安装 3Dmigoto', cancelLabel: '继续启动' }
          );
          
          if (install) {
              openSettingsAndUpdate();
              return;
          }
          // 用户选择"继续启动"，不阻断
      }
      
      if (!gameExePath) {
        await showMessage('请先在游戏设置中配置游戏可执行文件路径', { title: '提示', kind: 'info' });
        return;
      }

      if (!wineVersionId) {
        await showMessage('请先在游戏设置中选择 Wine/Proton 版本', { title: '提示', kind: 'info' });
        return;
      }

      isLaunching.value = true;
      await apiStartGame(gameName, gameExePath, wineVersionId);
      
  } catch (e: any) {
    console.error('Start Game Error:', e);
    await showMessage(`启动失败: ${e}`, { title: '错误', kind: 'error' });
  } finally {
    if(isLaunching.value) {
        setTimeout(() => {
            isLaunching.value = false;
        }, 1500);
    }
  }
}

watch(() => appSettings.currentConfigName, () => {
  checkGameExe();
});

onMounted(() => {
  document.addEventListener('click', closeMenu);
  checkGameExe();
});

onUnmounted(() => {
  document.removeEventListener('click', closeMenu);
});
</script>

<template>
  <div class="home-container">
    <div class="sidebar-wrapper">
      <div class="sidebar-track">
        <!-- Games Loop -->
        <el-tooltip v-for="game in sidebarGames" :key="game.name" :content="getGameName(game)" placement="right" effect="dark"
          popper-class="game-tooltip">
          <div class="sidebar-icon" :class="{ active: isGameActive(game.name) }" @click.stop="handleGameClick(game)"
            @contextmenu.prevent="handleContextMenu($event, game)">
            <img :src="game.iconPath" :alt="game.name" loading="lazy"
              @load="(e) => (e.target as HTMLImageElement).style.opacity = '1'"
              @error="(e) => (e.target as HTMLImageElement).style.opacity = '0'" />
          </div>
        </el-tooltip>

        <!-- Empty state: guide to Game Library -->
        <el-tooltip v-if="sidebarGames.length === 0" content="添加游戏到侧边栏" placement="right" effect="dark" popper-class="game-tooltip">
          <div class="sidebar-icon add-game-btn" @click="router.push('/games')">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
          </div>
        </el-tooltip>
      </div>
    </div>

    <!-- Custom Context Menu -->
    <div v-if="showMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }" @click.stop>
      <div class="menu-item" @click="hideGame">
        {{ t('home.contextmenu.hidegame') }}
      </div>
    </div>

    <div class="content-area">

      <!-- Mini download progress (shown when modal is closed but download is active) -->
      <div v-if="dlState.active && !showDownload" class="mini-dl-bar" @click="showDownload = true">
        <div class="mini-dl-info">
          <span class="mini-dl-name">{{ dlState.displayName || dlState.gameName }}</span>
          <span class="mini-dl-phase">{{ dlState.phase === 'verifying' ? '校验中' : (dlState.progress?.phase === 'install' ? '安装中' : '下载中') }}</span>
          <span class="mini-dl-pct" v-if="dlState.progress && dlState.progress.total_size > 0">
            {{ Math.round((dlState.progress.finished_size / dlState.progress.total_size) * 100) }}%
          </span>
        </div>
        <div class="mini-dl-track">
          <div class="mini-dl-fill" :style="{ width: (dlState.progress && dlState.progress.total_size > 0 ? Math.round((dlState.progress.finished_size / dlState.progress.total_size) * 100) : 0) + '%' }"></div>
        </div>
      </div>

    </div>

    <!-- Settings Modal -->
    <GameSettingsModal ref="settingsModalRef" v-model="showSettings" :game-name="appSettings.currentConfigName" />

    <!-- Download Modal -->
    <GameDownloadModal
      v-model="showDownload"
      :game-name="appSettings.currentConfigName"
      :display-name="currentDisplayName"
      @game-configured="checkGameExe"
    />

    <div class="action-bar">
      <!-- Start Game Button -->
      <div class="start-game-btn" @click="gameHasExe ? launchGame() : (showDownload = true)" :class="{ 'disabled': isLaunching }">
        <div class="icon-wrapper">
          <div class="play-triangle" v-if="gameHasExe"></div>
          <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
        </div>
        <span class="btn-text">{{ gameHasExe ? t('home.css.startgame') : t('home.css.downloadgame') }}</span>
      </div>

      <!-- Settings Menu Button -->
      <el-dropdown trigger="hover" placement="top-end" popper-class="settings-dropdown">
        <div class="settings-btn">
          <div class="menu-lines">
            <div class="line"></div>
            <div class="line"></div>
            <div class="line"></div>
          </div>
        </div>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item @click="showSettings = true">{{ t('home.dropdown.gamesettings') }}</el-dropdown-item>
            <el-dropdown-item @click="open3dmigotoFolder">{{ t('home.dropdown.open3dmigoto') }}</el-dropdown-item>
            <el-dropdown-item @click="openD3dxIni">{{ t('home.dropdown.opend3dx') }}</el-dropdown-item>
            <el-dropdown-item divided @click="toggleSymlink(true)">{{ t('home.dropdown.enablesymlink') }}</el-dropdown-item>
            <el-dropdown-item @click="toggleSymlink(false)">{{ t('home.dropdown.disablesymlink') }}</el-dropdown-item>
            <el-dropdown-item divided @click="showDownload = true">下载/防护管理</el-dropdown-item>
            <el-dropdown-item @click="openSettingsAndUpdate">{{ t('home.dropdown.checkupdate') }}</el-dropdown-item>

          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>





  </div>
</template>

<style scoped>
.home-container {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: row;
  /* Changed to row for Sidebar + Content */
  padding: 0;
  /* Remove padding from container, move to children */
  box-sizing: border-box;
  position: relative;
}

.sidebar-wrapper {
  width: 80px;
  height: 100%;
  /* Gradient Background: Transparent top to Black bottom */
  background: linear-gradient(to bottom, rgba(0, 0, 0, 0) 0%, rgba(0, 0, 0, 0.95) 100%);
  display: flex;
  flex-direction: column;
  /* justify-content: flex-end; Removed to allow scrolling with margin-top: auto */
  padding-bottom: 16px;
  /* Space from bottom matches side margins */
  padding-top: 40px;
  /* Safe area for TitleBar when scrolling */
  box-sizing: border-box;
  z-index: 10;

  overflow-y: auto;
  overflow-x: hidden;

  /* Distinct right border */
  border-right: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 2px 0 10px rgba(0, 0, 0, 0.5);
  /* Adds depth shadow to the right */
}

/* Hide scrollbar for sidebar */
.sidebar-wrapper::-webkit-scrollbar {
  width: 0px;
  background: transparent;
}

.sidebar-track {
  display: flex;
  flex-direction: column-reverse;
  /* Stack from bottom to top as requested */
  gap: 16px;
  align-items: center;
  width: 100%;
  margin-top: auto;
  /* Push content to bottom when not overflowing */
}

.sidebar-icon {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  overflow: hidden;
  cursor: pointer;
  transition: transform 0.2s ease, box-shadow 0.2s ease;
  background-color: rgba(0, 0, 0, 0.3);
  /* Placeholder bg */
}

.sidebar-icon.add-game-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px dashed rgba(255, 255, 255, 0.4);
  background-color: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.6);
}

.sidebar-icon.add-game-btn:hover {
  border-color: rgba(255, 255, 255, 0.7);
  color: rgba(255, 255, 255, 0.9);
  background-color: rgba(255, 255, 255, 0.15);
}

.sidebar-icon img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.sidebar-icon:hover {
  transform: scale(1.1);
  box-shadow: 0 0 10px rgba(255, 255, 255, 0.3);
}

/* Crystal/Amber selection effect */
.sidebar-icon.active {
  /* 
       Layered Box Shadows to create "Gap + Thick Border" effect
       1. Dark gap (simulating distance from icon)
       2. Thick White Border
       3. Outer Glow
       4. Inner Glow (Crystal effect)
    */
  box-shadow:
    0 0 0 2px rgba(0, 0, 0, 0.6),
    /* 2px Distance/Gap */
    0 0 0 4px #ffffff,
    /* 2px Thick White Border (4px total spread - 2px gap) */
    0 0 20px rgba(255, 255, 255, 0.5),
    /* Soft ambient glow */
    inset 0 0 20px rgba(255, 255, 255, 0.5);
  /* Inner crystal glow */

  /* Remove physical border or make it subtle inner edge */
  border: 1px solid rgba(255, 255, 255, 0.3);

  transform: scale(1.05);
  z-index: 2;
  /* Ensure shadow overlaps neighbors if needed */
}

.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 40px;
  /* Restore padding here */
  position: relative;
  z-index: 1;
  /* Ensure content sits above shadow */
}

.action-bar {
  display: flex;
  height: 60px;
  /* Tall button strip */
  /* Remove flex-end self align because now it is inside content-area which needs to be carefully managed */
  /* Or actually, keep it but ensure content-area is full height */
  margin-top: auto;
  /* Push to bottom */
  align-self: flex-end;
  gap: 10px;
  /* Space between buttons */

  /* Ensure it doesn't overlap with sidebar if window is small, though structure prevents it */
  padding-right: 40px;
  /* Right padding from screen edge */
  padding-bottom: 40px;
}

/* --- Start Game Button --- */
.start-game-btn {
  background-color: #F7CE46;
  /* Yellow */
  color: #000000;
  display: flex;
  align-items: center;
  padding: 0 44px 0 12px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-family: 'Microsoft YaHei', sans-serif;

  /* Full rounded capsule shape */
  border-radius: 30px;
}

.start-game-btn .btn-text {
  font-size: 18px;
  font-weight: 900;
  margin-left: 30px;
  letter-spacing: 2px;
}

.icon-wrapper {
  width: 36px;
  height: 36px;
  background-color: #000000;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.play-triangle {
  width: 0;
  height: 0;
  border-style: solid;
  border-width: 7px 0 7px 11px;
  /* Pointing right */
  border-color: transparent transparent transparent #F7CE46;
  /* Yellow triangle */
  margin-left: 3px;
  /* Visual optical adjustment */
  transition: all 0.2s ease;
}

.start-game-btn.disabled {
  pointer-events: none;
  opacity: 0.6;
  filter: grayscale(0.5);
}

/* Hover Effect: Flip Colors for Start Button */
.start-game-btn:hover {
  background-color: #333333;
  color: #F7CE46;
}

.start-game-btn:active {
  background-color: #000000;
}

.start-game-btn:hover .icon-wrapper {
  background-color: #F7CE46;
}

.start-game-btn:hover .play-triangle {
  border-color: transparent transparent transparent #333333;
}

.start-game-btn:active .play-triangle {
  border-color: transparent transparent transparent #000000;
}

.icon-wrapper svg {
  color: #F7CE46;
}

.start-game-btn:hover .icon-wrapper svg {
  color: #333333;
}

.start-game-btn:active .icon-wrapper svg {
  color: #000000;
}


/* --- Settings Button --- */
/* Wrapper for dropdown to behave as flex item */
:deep(.el-dropdown) {
  display: flex;
  align-items: stretch;
}

.settings-btn {
  width: 60px;
  /* Square to make it a circle (height is 60px from parent) */
  background-color: #2D2D2D;
  /* Dark Gray */
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;

  /* Remove default borders/outlines */
  border: none;
  outline: none;

  /* Circle shape */
  border-radius: 50%;

  transition: background-color 0.2s;
}

/* Ensure no outline on focus */
.settings-btn:focus,
.settings-btn:focus-visible {
  outline: none;
  border: none;
}

.settings-btn:hover {
  background-color: #2D2D2D;
  /* Keep background unchanged on hover as requested, or slightly lighter? User said "bg color is gray keep unchanged". Assuming static. */
}

/* The three horizontal lines icon */
.menu-lines {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  height: 16px;
  width: 20px;
}

.line {
  height: 3px;
  background-color: #888888;
  /* Gray lines */
  width: 100%;
  border-radius: 2px;
  transition: background-color 0.2s;
}

.settings-btn:hover .line {
  background-color: #ffffff;
  /* White lines on hover */
}

/* Context Menu */
.context-menu {
  position: fixed;
  z-index: 10000;
  background: rgba(30, 30, 30, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(8px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  border-radius: 6px;
  padding: 4px;
  min-width: 120px;
}

.menu-item {
  padding: 8px 12px;
  cursor: pointer;
  color: #eee;
  font-size: 13px;
  border-radius: 4px;
  transition: background-color 0.1s;
}

.menu-item:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: #fff;
}

/* Mini download toast (top-right notification) */
.mini-dl-bar {
  position: fixed;
  top: 38px;
  right: 16px;
  width: 280px;
  background: rgba(20, 20, 20, 0.92);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  padding: 10px 14px;
  cursor: pointer;
  transition: all 0.25s;
  z-index: 1000;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
  animation: toastSlideIn 0.3s ease-out;
}
@keyframes toastSlideIn {
  from { opacity: 0; transform: translateX(40px); }
  to   { opacity: 1; transform: translateX(0); }
}
.mini-dl-bar:hover {
  background: rgba(30, 30, 30, 0.96);
  border-color: rgba(247, 206, 70, 0.35);
  box-shadow: 0 4px 24px rgba(247, 206, 70, 0.1);
}
.mini-dl-info {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.mini-dl-name {
  font-size: 12px;
  font-weight: 600;
  color: #F7CE46;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 140px;
}
.mini-dl-phase {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.45);
  padding: 1px 6px;
  background: rgba(255, 255, 255, 0.06);
  border-radius: 3px;
}
.mini-dl-pct {
  font-size: 12px;
  font-weight: 700;
  color: rgba(255, 255, 255, 0.9);
  margin-left: auto;
}
.mini-dl-track {
  width: 100%;
  height: 3px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 2px;
  overflow: hidden;
}
.mini-dl-fill {
  height: 100%;
  background: linear-gradient(90deg, #F7CE46, #f0a030);
  border-radius: 2px;
  transition: width 0.3s ease;
}
</style>

<style>
/* Global styles for the settings dropdown */
.settings-dropdown.el-popper {
  border-radius: 12px !important;
  overflow: hidden;
}

.settings-dropdown .el-dropdown-menu {
  border-radius: 12px !important;
  padding: 6px !important;
}

.settings-dropdown .el-dropdown-menu__item {
  border-radius: 8px !important;
  margin-bottom: 2px;
}

.settings-dropdown .el-dropdown-menu__item:last-child {
  margin-bottom: 0;
}
</style>
