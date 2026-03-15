<script setup lang="ts">
import type { ComponentPublicInstance } from 'vue';
import { useGameLogViewer } from './useGameLogViewer';

const {
  t,
  snapshot,
  autoScroll,
  isLoading,
  logContainer,
  loadLogs,
  copyLogs,
} = useGameLogViewer();

const setLogContainer = (element: Element | ComponentPublicInstance | null) => {
  logContainer.value = element as HTMLElement | null;
};
</script>

<template>
  <div class="game-log-viewer">
    <div class="toolbar">
      <span class="title">{{ t('gamelogviewer.title') }}</span>
      <span class="meta" v-if="snapshot.active">
        {{ snapshot.gameName }} | lines={{ snapshot.lineCount }} | started={{ snapshot.startedAt }}
      </span>
      <span class="meta" v-else>{{ t('gamelogviewer.noSession') }}</span>
      <div class="actions">
        <label class="auto-scroll-label">
          <input type="checkbox" v-model="autoScroll" />
          {{ t('gamelogviewer.autoScroll') }}
        </label>
        <button class="btn" @click="loadLogs" :disabled="isLoading">{{ t('gamelogviewer.refresh') }}</button>
        <button class="btn" @click="copyLogs">{{ t('gamelogviewer.copyAll') }}</button>
      </div>
    </div>
    <div class="content" :ref="setLogContainer">
      <pre>{{ snapshot.content || t('gamelogviewer.empty') }}</pre>
    </div>
  </div>
</template>

<style scoped>
.game-log-viewer {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #0d0d0d;
  color: #e0e0e0;
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 16px;
  background: #1a1a1a;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
}

.title {
  font-size: 13px;
  font-weight: 700;
  color: #f7ce46;
}

.meta {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.45);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.auto-scroll-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  display: flex;
  align-items: center;
  gap: 4px;
  user-select: none;
}

.auto-scroll-label input {
  accent-color: #f7ce46;
}

.btn {
  padding: 4px 12px;
  font-size: 11px;
  border: 1px solid rgba(255, 255, 255, 0.15);
  background: rgba(255, 255, 255, 0.05);
  color: rgba(255, 255, 255, 0.75);
  border-radius: 4px;
  cursor: pointer;
}

.btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.content {
  flex: 1;
  overflow: auto;
  padding: 12px 16px;
}

.content pre {
  margin: 0;
  font-size: 11px;
  line-height: 1.6;
  white-space: pre;
  word-break: normal;
  color: #c8c8c8;
}
</style>
