<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { GameInfo } from '../../types/ipc';

defineProps<{
  searchQuery: string;
  filteredGames: GameInfo[];
  currentConfigName: string;
  gamesLoading: boolean;
  emptyStateText: string;
}>();

defineEmits<{
  (event: 'update:searchQuery', value: string): void;
  (event: 'open-import-dialog'): void;
  (event: 'handle-game-select', game: GameInfo, mouseEvent: MouseEvent): void;
  (event: 'handle-context-menu', mouseEvent: MouseEvent, game: GameInfo): void;
}>();

const { t, te } = useI18n();
</script>

<template>
  <div>
    <div class="toolbar" data-onboarding="library-toolbar" @click.stop v-if="filteredGames.length > 0 || searchQuery !== ''">
      <button class="tech-btn" @click="$emit('open-import-dialog')">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px; vertical-align: -2px;">
          <line x1="12" y1="5" x2="12" y2="19"></line>
          <line x1="5" y1="12" x2="19" y2="12"></line>
        </svg>
        {{ t('gamelibrary.importConfig') }}
      </button>
      <div style="flex-grow: 1;"></div>
      <input
        :value="searchQuery"
        type="text"
        class="search-input"
        :placeholder="t('gamelibrary.searchGames') || 'Search games...'"
        @input="$emit('update:searchQuery', ($event.target as HTMLInputElement).value)"
      />
    </div>

    <div class="games-grid">
      <div
        v-for="(game, index) in filteredGames"
        :key="game.name"
        class="game-card"
        :class="{ active: currentConfigName === game.name }"
        :style="{ animationDelay: `${index * 0.05}s` }"
        @click="$emit('handle-game-select', game, $event)"
        @contextmenu.prevent="$emit('handle-context-menu', $event, game)"
      >
        <div class="game-icon-wrapper">
          <img
            :src="game.iconPath"
            class="game-icon"
            alt="icon"
            @load="(e) => ((e.target as HTMLImageElement).style.opacity = '1')"
            @error="(e) => ((e.target as HTMLImageElement).style.opacity = '0')"
          />
        </div>
        <div class="game-label">{{ te(`games.${game.name}`) ? t(`games.${game.name}`) : (game.displayName || game.name) }}</div>
      </div>

      <div v-if="filteredGames.length === 0" class="empty-state" :class="{ loading: gamesLoading && !searchQuery }">
        <div class="empty-icon">{{ gamesLoading && !searchQuery ? '...' : '!' }}</div>
        <div class="empty-text">{{ emptyStateText }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.toolbar {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(0, 240, 255, 0.3);
  border-radius: 8px;
  padding: 8px 16px;
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 24px;
}

.search-input {
  background: rgba(0, 0, 0, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: #fff;
  padding: 8px 12px;
  border-radius: 4px;
  transition: all 0.2s;
  outline: none;
}

.search-input:focus {
  border-color: #00f0ff;
}

.tech-btn {
  background: rgba(0, 240, 255, 0.1);
  border: 1px solid #00f0ff;
  color: #00f0ff;
  padding: 8px 16px;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
  text-transform: uppercase;
  font-weight: 600;
  font-size: 12px;
  letter-spacing: 1px;
}

.tech-btn:hover {
  background: #00f0ff;
  color: #000;
}

.games-grid {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-start;
  gap: 32px 24px;
  padding: 24px 60px 60px 60px;
  margin: 0 auto;
  max-width: 1400px;
  contain: layout style;
}

.empty-state.loading .empty-icon {
  animation: emptyPulse 1.1s ease-in-out infinite;
}

@keyframes emptyPulse {
  0%, 100% { opacity: 0.4; transform: scale(0.96); }
  50% { opacity: 1; transform: scale(1.04); }
}

.game-card {
  position: relative;
  width: 110px;
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  cursor: pointer;
  animation: cardEntranceFade 0.2s cubic-bezier(0.16, 1, 0.3, 1) both;
}

.game-card:nth-child(1)  { animation-delay: 0.02s; }
.game-card:nth-child(2)  { animation-delay: 0.04s; }
.game-card:nth-child(3)  { animation-delay: 0.06s; }
.game-card:nth-child(4)  { animation-delay: 0.08s; }
.game-card:nth-child(5)  { animation-delay: 0.10s; }
.game-card:nth-child(6)  { animation-delay: 0.12s; }
.game-card:nth-child(7)  { animation-delay: 0.14s; }
.game-card:nth-child(8)  { animation-delay: 0.16s; }
.game-card:nth-child(9)  { animation-delay: 0.18s; }
.game-card:nth-child(10) { animation-delay: 0.20s; }
.game-card:nth-child(11) { animation-delay: 0.22s; }
.game-card:nth-child(12) { animation-delay: 0.24s; }
.game-card:nth-child(n+13) { animation-delay: 0.26s; }

@keyframes cardEntranceFade {
  0% { opacity: 0; }
  100% { opacity: 1; }
}

.game-icon-wrapper {
  position: relative;
  width: 100%;
  aspect-ratio: 1 / 1;
  background: rgba(20, 22, 28, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 6px;
  will-change: transform;
  contain: layout style;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.game-card:hover .game-icon-wrapper {
  transform: translateY(-6px) scale(1.05);
  border-color: #00f0ff;
}

.game-card.active .game-icon-wrapper {
  transform: translateY(-4px) scale(1.08);
  border-color: #fff;
  background: rgba(255, 255, 255, 0.1);
}

.game-card.active .game-icon-wrapper::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: 11px;
  z-index: 4;
  pointer-events: none;
  background: linear-gradient(to bottom, transparent 40%, rgba(0, 240, 255, 0.4) 50%, transparent 60%);
  background-size: 100% 200%;
  animation: techScan 2s linear infinite;
}

@keyframes techScan {
  0% { background-position: 0% -100%; }
  100% { background-position: 0% 200%; }
}

.game-icon {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 8px;
  display: block;
  z-index: 1;
  filter: brightness(0.9);
  transition: filter 0.2s ease, transform 0.2s ease;
}

.game-card:hover .game-icon {
  filter: brightness(1.1);
}

.game-card.active .game-icon {
  filter: brightness(1);
}

.game-label {
  margin-top: 12px;
  width: 100%;
  text-align: center;
  font-size: 13px;
  font-weight: 500;
  color: #e0e0e0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.game-card:hover .game-label {
  color: #fff;
}

.game-card.active .game-label {
  color: #00f0ff;
}

.game-card.active::before,
.game-card.active::after,
.game-icon-wrapper::before,
.game-icon-wrapper::after {
  display: none !important;
}
</style>
