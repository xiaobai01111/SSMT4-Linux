<script setup lang="ts">
import GameDownloadModal from '../../components/GameDownloadModal.vue';
import GameSettingsModal from '../../components/GameSettingsModal.vue';
import HomeActionBar from './components/HomeActionBar.vue';
import HomeContextMenu from './components/HomeContextMenu.vue';
import HomeGamesDock from './components/HomeGamesDock.vue';
import HomeNotifications from './components/HomeNotifications.vue';
import { useHomeView } from './useHomeView';

const {
  router,
  appSettings,
  targetGame,
  gamesLoading,
  sidebarGames,
  hasCurrentGame,
  showMenu,
  menuX,
  menuY,
  showSettings,
  showDownload,
  currentDisplayName,
  currentGameNeedsUpdate,
  startButtonLabel,
  settingsModalRef,
  gameHasExe,
  isLaunching,
  isGameRunning,
  componentDlProgress,
  getGameName,
  isGameActive,
  handleGameClick,
  handleContextMenu,
  hideGame,
  openGameSettingsTab,
  openCurrentGameLog,
  handlePrimaryAction,
  checkGameExe,
  activeDownloadTask,
} = useHomeView();

type HomeSettingsModalRef = NonNullable<(typeof settingsModalRef)['value']>;

const setSettingsModalRef = (value: unknown) => {
  settingsModalRef.value = value as HomeSettingsModalRef | null;
};

const handleAddGame = () => {
  void router.push('/games');
};

const handleOpenGameSettings = () => {
  void openGameSettingsTab('info');
};

const handleOpenDownload = () => {
  showDownload.value = true;
};
</script>

<template>
  <div class="home-container">
    <div class="hero-layer">
      <div
        class="hero-image"
        :style="{ backgroundImage: (targetGame && targetGame.iconPath) ? `url(${targetGame.iconPath})` : 'none' }"
        :class="{ 'has-image': targetGame && targetGame.iconPath }"
      ></div>
      <div class="tech-grid-overlay"></div>
      <div class="hero-overlay"></div>
    </div>

    <div class="content-area">
      <div class="dashboard-panel">
        <HomeGamesDock
          :games-loading="gamesLoading"
          :sidebar-games="sidebarGames"
          :get-game-name="getGameName"
          :is-game-active="isGameActive"
          @add-game="handleAddGame"
          @select-game="handleGameClick"
          @open-context-menu="handleContextMenu"
        />

        <div class="divider"></div>

        <HomeActionBar
          :current-game-needs-update="currentGameNeedsUpdate"
          :is-game-running="isGameRunning"
          :game-has-exe="gameHasExe"
          :is-launching="isLaunching"
          :has-current-game="hasCurrentGame"
          :start-button-label="startButtonLabel"
          @primary-action="handlePrimaryAction"
          @open-game-settings="handleOpenGameSettings"
          @open-download="handleOpenDownload"
          @open-game-log="openCurrentGameLog"
        />
      </div>

      <HomeNotifications
        :active-download-task="activeDownloadTask"
        :component-dl-progress="componentDlProgress"
        :show-download="showDownload"
        @open-download="handleOpenDownload"
      />
    </div>

    <HomeContextMenu
      :show-menu="showMenu"
      :menu-x="menuX"
      :menu-y="menuY"
      @hide-game="hideGame"
    />

    <GameSettingsModal
      :ref="setSettingsModalRef"
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
.home-container {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  position: relative;
  overflow: hidden;
  color: #fff;
}

.hero-layer {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  overflow: hidden;
  background-color: transparent;
}

.hero-image {
  width: 100%;
  height: 100%;
  background-size: cover;
  background-position: center;
  transition: opacity 0.35s ease-in-out;
  opacity: 0;
}

.hero-image.has-image {
  opacity: 0.95;
}

.tech-grid-overlay {
  position: absolute;
  inset: 0;
  background-image: radial-gradient(rgba(255, 255, 255, 0.05) 1px, transparent 1px);
  background-size: 24px 24px;
  z-index: 1;
}

.hero-overlay {
  position: absolute;
  inset: 0;
  background: radial-gradient(circle at center 40%, transparent 10%, rgba(10, 12, 16, 0.8) 100%);
  z-index: 2;
}

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

.dashboard-panel {
  display: flex;
  align-items: center;
  gap: 24px;
  border-radius: 20px;
  padding: 12px 28px;
  background-color: rgba(20, 25, 30, 0.45);
  backdrop-filter: blur(24px) saturate(120%);
  -webkit-backdrop-filter: blur(24px) saturate(120%);
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.25);
}

.divider {
  width: 1px;
  height: 48px;
  background: linear-gradient(to bottom, transparent, rgba(255, 255, 255, 0.2), transparent);
}
</style>
