<script setup lang="ts">
import { nextTick, onBeforeUnmount, reactive, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { PropType } from 'vue';
import type { GameInfo } from '../../../types/ipc';

defineProps({
  gamesLoading: {
    type: Boolean,
    required: true,
  },
  sidebarGames: {
    type: Array as PropType<GameInfo[]>,
    required: true,
  },
  getGameName: {
    type: Function as PropType<(game: GameInfo) => string>,
    required: true,
  },
  isGameActive: {
    type: Function as PropType<(gameName: string) => boolean>,
    required: true,
  },
});

const emit = defineEmits<{
  (event: 'add-game'): void;
  (event: 'select-game', game: GameInfo): void;
  (event: 'open-context-menu', mouseEvent: MouseEvent, game: GameInfo): void;
}>();

const { t } = useI18n();

const dockTooltipRef = ref<HTMLElement | null>(null);
const dockTooltip = reactive({
  visible: false,
  label: '',
  left: 0,
  top: 0,
  anchorEl: null as HTMLElement | null,
});

const TOOLTIP_MARGIN = 12;
const TOOLTIP_GAP = 10;
let tooltipPositionFrame = 0;

const flushTooltipPosition = (anchorEl?: HTMLElement | null) => {
  tooltipPositionFrame = 0;
  const anchor = anchorEl ?? dockTooltip.anchorEl;
  if (!anchor) return;

  const rect = anchor.getBoundingClientRect();
  const tooltipWidth = dockTooltipRef.value?.offsetWidth ?? 0;
  const halfWidth = tooltipWidth / 2;
  const unclampedLeft = rect.left + rect.width / 2;
  const minLeft = TOOLTIP_MARGIN + halfWidth;
  const maxLeft = window.innerWidth - TOOLTIP_MARGIN - halfWidth;

  dockTooltip.left =
    tooltipWidth > 0
      ? Math.min(Math.max(unclampedLeft, minLeft), Math.max(minLeft, maxLeft))
      : unclampedLeft;
  dockTooltip.top = rect.top - TOOLTIP_GAP;
};

const scheduleTooltipPosition = (anchorEl?: HTMLElement | null) => {
  const anchor = anchorEl ?? dockTooltip.anchorEl;
  if (!anchor || typeof window === 'undefined') return;

  if (tooltipPositionFrame) {
    window.cancelAnimationFrame(tooltipPositionFrame);
  }

  tooltipPositionFrame = window.requestAnimationFrame(() => {
    flushTooltipPosition(anchor);
  });
};

const showDockTooltip = async (label: string, event: MouseEvent) => {
  const anchorEl = event.currentTarget as HTMLElement | null;
  if (!anchorEl || !label) return;

  dockTooltip.visible = true;
  dockTooltip.label = label;
  dockTooltip.anchorEl = anchorEl;

  await nextTick();
  scheduleTooltipPosition(anchorEl);
};

const hideDockTooltip = () => {
  if (tooltipPositionFrame && typeof window !== 'undefined') {
    window.cancelAnimationFrame(tooltipPositionFrame);
    tooltipPositionFrame = 0;
  }
  dockTooltip.visible = false;
  dockTooltip.label = '';
  dockTooltip.anchorEl = null;
};

const handleViewportChange = () => {
  if (!dockTooltip.visible) return;
  scheduleTooltipPosition();
};

if (typeof window !== 'undefined') {
  window.addEventListener('resize', handleViewportChange);
  window.addEventListener('scroll', handleViewportChange, true);
}

onBeforeUnmount(() => {
  if (typeof window === 'undefined') return;
  if (tooltipPositionFrame) {
    window.cancelAnimationFrame(tooltipPositionFrame);
  }
  window.removeEventListener('resize', handleViewportChange);
  window.removeEventListener('scroll', handleViewportChange, true);
});

const handleSelectGame = (game: GameInfo) => {
  hideDockTooltip();
  emit('select-game', game);
};

const handleContextMenu = (event: MouseEvent, game: GameInfo) => {
  hideDockTooltip();
  emit('open-context-menu', event, game);
};
</script>

<template>
  <div class="games-dock" data-onboarding="home-games-dock" @scroll.passive="handleViewportChange">
    <div v-if="sidebarGames.length === 0 && gamesLoading" class="dock-loading">
      <div class="dock-loading-spinner"></div>
      <div class="dock-loading-text">扫描游戏中...</div>
    </div>

    <el-tooltip
      v-else-if="sidebarGames.length === 0"
      :content="t('home.tooltips.addToSidebar')"
      placement="top"
      effect="dark"
      popper-class="game-tooltip"
    >
      <div class="dock-icon add-game-btn" @click="emit('add-game')">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="12" y1="5" x2="12" y2="19"></line>
          <line x1="5" y1="12" x2="19" y2="12"></line>
        </svg>
      </div>
    </el-tooltip>

    <div
      v-for="game in sidebarGames"
      :key="game.name"
      class="dock-icon"
      :class="{ active: isGameActive(game.name) }"
      @mouseenter="showDockTooltip(getGameName(game), $event)"
      @mouseleave="hideDockTooltip"
      @click.stop="handleSelectGame(game)"
      @contextmenu.prevent="handleContextMenu($event, game)"
    >
      <img
        :src="game.iconPath"
        :alt="game.name"
        loading="lazy"
        @load="(e) => (e.target as HTMLImageElement).style.opacity = '1'"
        @error="(e) => (e.target as HTMLImageElement).style.opacity = '0'"
      />
    </div>
  </div>

  <Teleport to="body">
    <transition name="dock-tooltip-fade">
      <div
        v-if="dockTooltip.visible && dockTooltip.label"
        ref="dockTooltipRef"
        class="dock-hover-tooltip"
        :style="{
          left: `${dockTooltip.left}px`,
          top: `${dockTooltip.top}px`,
        }"
      >
        {{ dockTooltip.label }}
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.games-dock {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 8px;
  max-width: 60vw;
  overflow-x: auto;
  overflow-y: hidden;
  scroll-behavior: smooth;
}

.games-dock::-webkit-scrollbar {
  height: 4px;
}

.games-dock::-webkit-scrollbar-track {
  background: transparent;
}

.games-dock::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 2px;
}

.games-dock::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.4);
}

.dock-loading {
  min-width: 160px;
  height: 64px;
  padding: 0 18px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.05);
  display: flex;
  align-items: center;
  gap: 12px;
  color: rgba(255, 255, 255, 0.78);
}

.dock-loading-spinner {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid rgba(255, 255, 255, 0.1);
  border-top-color: var(--el-color-primary);
  animation: dockSpin 0.8s linear infinite;
}

.dock-loading-text {
  font-size: 13px;
  font-weight: 500;
}

@keyframes dockSpin {
  to {
    transform: rotate(360deg);
  }
}

.dock-icon {
  flex-shrink: 0;
  width: 64px;
  height: 64px;
  border-radius: 14px;
  position: relative;
  cursor: pointer;
  background-color: rgba(255, 255, 255, 0.05);
  transition:
    transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1),
    border-color 0.3s ease,
    box-shadow 0.3s ease,
    background-color 0.3s ease,
    color 0.3s ease;
  border: 1px solid rgba(255, 255, 255, 0.05);
  will-change: transform;
}

.dock-icon img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 13px;
  position: relative;
  z-index: 3;
  transition: opacity 0.2s ease;
}

.dock-icon:hover {
  transform: translateY(-8px) scale(1.1);
  border-color: var(--el-color-primary-light-3);
  box-shadow: 0 12px 24px rgba(0, 0, 0, 0.3);
}

.dock-icon.active {
  transform: translateY(-6px) scale(1.15);
  border-color: var(--el-color-primary);
  box-shadow: 0 8px 24px rgba(var(--el-color-primary-rgb), 0.3);
  z-index: 10;
}

.dock-icon.add-game-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px dashed rgba(255, 255, 255, 0.2);
  background-color: transparent;
  color: rgba(255, 255, 255, 0.5);
}

.dock-icon.add-game-btn:hover {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
  background-color: rgba(var(--el-color-primary-rgb), 0.1);
}

.dock-hover-tooltip {
  position: fixed;
  transform: translate(-50%, -100%);
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
  pointer-events: none;
  z-index: 3000;
  max-width: min(320px, calc(100vw - 24px));
  background-color: rgba(20, 25, 30, 0.75);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
  contain: layout style paint;
  will-change: transform, opacity;
}

.dock-tooltip-fade-enter-active,
.dock-tooltip-fade-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.dock-tooltip-fade-enter-from,
.dock-tooltip-fade-leave-to {
  opacity: 0;
  transform: translate(-50%, calc(-100% + 8px));
}
</style>
