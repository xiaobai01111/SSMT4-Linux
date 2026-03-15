<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{
  currentGameNeedsUpdate: boolean;
  isGameRunning: boolean;
  gameHasExe: boolean;
  isLaunching: boolean;
  hasCurrentGame: boolean;
  startButtonLabel: string;
}>();

const emit = defineEmits<{
  (event: 'primary-action'): void;
  (event: 'open-game-settings'): void;
  (event: 'open-download'): void;
  (event: 'open-game-log'): void;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="action-bar">
    <div class="start-game-wrapper">
      <div
        class="start-game-btn"
        data-onboarding="home-start-button"
        :class="{ disabled: isLaunching, running: isGameRunning }"
        @click="emit('primary-action')"
      >
        <div class="btn-background-fx"></div>
        <div class="icon-wrapper">
          <svg
            v-if="currentGameNeedsUpdate && !isGameRunning"
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M12 3v12"></path>
            <path d="m7 10 5 5 5-5"></path>
            <path d="M5 21h14"></path>
          </svg>
          <div v-else-if="gameHasExe && !isGameRunning" class="play-triangle"></div>
          <div v-else-if="isGameRunning" class="running-indicator"></div>
          <svg v-else xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="7 10 12 15 17 10"></polyline>
            <line x1="12" y1="15" x2="12" y2="3"></line>
          </svg>
        </div>
        <span class="btn-text">{{ startButtonLabel }}</span>
      </div>
    </div>

    <el-dropdown trigger="click" placement="top-end" popper-class="settings-dropdown">
      <div class="settings-btn" data-onboarding="home-settings-button">
        <svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20 7h-9"></path>
          <path d="M14 17H5"></path>
          <circle cx="17" cy="7" r="3"></circle>
          <circle cx="8" cy="17" r="3"></circle>
        </svg>
      </div>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item :disabled="!hasCurrentGame" @click="emit('open-game-settings')">
            <span class="dropdown-row">
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path
                  d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z">
                </path>
                <circle cx="12" cy="12" r="3"></circle>
              </svg>
              {{ t('home.dropdown.gamesettings') }}
            </span>
          </el-dropdown-item>

          <el-dropdown-item divided :disabled="!hasCurrentGame" @click="emit('open-download')">
            <span class="dropdown-row">
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 17V3"></path>
                <path d="m6 11 6 6 6-6"></path>
                <path d="M19 21H5"></path>
              </svg>
              {{ t('home.dropdown.downloadProtection') }}
            </span>
          </el-dropdown-item>

          <el-dropdown-item divided :disabled="!hasCurrentGame" @click="emit('open-game-log')">
            <span class="dropdown-row">
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 20v-6"></path>
                <path d="M9 20h6"></path>
                <path d="M5 8a7 7 0 1 1 14 0v6a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2z"></path>
              </svg>
              {{ t('home.dropdown.openGameLog') }}
            </span>
          </el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>
  </div>
</template>

<style scoped>
.action-bar {
  display: flex;
  align-items: center;
  gap: 16px;
}

.start-game-wrapper {
  position: relative;
}

.start-game-btn {
  position: relative;
  background: var(--el-color-primary);
  color: #fff;
  display: flex;
  align-items: center;
  padding: 0 32px 0 8px;
  width: 220px;
  height: 56px;
  border-radius: 28px;
  cursor: pointer;
  overflow: hidden;
  transition: all 0.2s ease;
  box-shadow: 0 8px 20px rgba(var(--el-color-primary-rgb), 0.3);
}

.btn-background-fx {
  position: absolute;
  top: 0;
  left: -50%;
  width: 50%;
  height: 100%;
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
  opacity: 0;
  transition: opacity 0.3s;
}

.start-game-btn:hover {
  transform: translateY(-2px);
  box-shadow: 0 12px 24px rgba(var(--el-color-primary-rgb), 0.4);
  background: var(--el-color-primary-light-3);
}

.start-game-btn:hover .btn-background-fx {
  opacity: 1;
  animation: pulseSweep 1.5s infinite;
}

.start-game-btn:active {
  transform: translateY(1px);
}

@keyframes pulseSweep {
  0% {
    left: -50%;
  }

  100% {
    left: 150%;
  }
}

.icon-wrapper {
  width: 40px;
  height: 40px;
  background-color: #fff;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--el-color-primary);
  z-index: 2;
  transition: transform 0.2s ease;
}

.start-game-btn:hover .icon-wrapper {
  transform: scale(1.05);
}

.play-triangle {
  width: 0;
  height: 0;
  border-style: solid;
  border-width: 7px 0 7px 11px;
  border-color: transparent transparent transparent currentColor;
  margin-left: 3px;
}

.btn-text {
  font-size: 16px;
  font-weight: 600;
  margin-left: 16px;
  letter-spacing: 1px;
  z-index: 2;
  flex: 1;
  text-align: center;
}

.start-game-btn.disabled {
  pointer-events: none;
  background: var(--el-text-color-placeholder);
  box-shadow: none;
}

.start-game-btn.disabled .icon-wrapper {
  color: var(--el-text-color-placeholder);
}

.start-game-btn.running {
  pointer-events: none;
  background: var(--el-color-success);
  box-shadow: 0 8px 20px rgba(var(--el-color-success-rgb), 0.3);
}

.start-game-btn.running .icon-wrapper {
  color: var(--el-color-success);
}

.running-indicator {
  width: 14px;
  height: 14px;
  background: currentColor;
  border-radius: 50%;
}

.settings-btn {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.15);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  cursor: pointer;
  transition: all 0.2s;
}

.settings-btn:hover {
  background: rgba(255, 255, 255, 0.2);
  transform: rotate(45deg);
}

.dropdown-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>

<style>
.settings-dropdown.el-popper {
  background: rgba(25, 30, 35, 0.85) !important;
  backdrop-filter: blur(20px) !important;
  border: 1px solid rgba(255, 255, 255, 0.1) !important;
  border-radius: 12px !important;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.3) !important;
}

.settings-dropdown .el-popper__arrow::before {
  background: rgba(25, 30, 35, 0.85) !important;
  border: 1px solid rgba(255, 255, 255, 0.1) !important;
}

.settings-dropdown .el-dropdown-menu {
  background: transparent !important;
  border: none !important;
  padding: 8px !important;
}

.settings-dropdown .el-dropdown-menu__item {
  border-radius: 8px !important;
  color: rgba(255, 255, 255, 0.85) !important;
  padding: 10px 16px !important;
  font-size: 14px !important;
  transition: all 0.2s !important;
}

.settings-dropdown .el-dropdown-menu__item:hover,
.settings-dropdown .el-dropdown-menu__item:focus {
  background: rgba(255, 255, 255, 0.1) !important;
  color: #fff !important;
}

.settings-dropdown .el-dropdown-menu__item--divided {
  border-top: 1px solid rgba(255, 255, 255, 0.08) !important;
  margin-top: 6px !important;
}
</style>
