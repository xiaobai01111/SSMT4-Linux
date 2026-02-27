<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from 'vue';
import { readGameLogSnapshot, type GameLogSnapshot } from '../api';

const snapshot = ref<GameLogSnapshot>({
  active: false,
  gameName: '',
  startedAt: '',
  lineCount: 0,
  content: '',
});
const autoScroll = ref(true);
const isLoading = ref(false);
const logContainer = ref<HTMLElement | null>(null);
let refreshTimer: ReturnType<typeof setInterval> | null = null;

const scrollToBottom = () => {
  if (!logContainer.value) return;
  logContainer.value.scrollTop = logContainer.value.scrollHeight;
};

const loadLogs = async () => {
  if (isLoading.value) return;
  try {
    isLoading.value = true;
    const next = await readGameLogSnapshot(1800);
    const prev = snapshot.value;
    const changed =
      next.active !== prev.active ||
      next.gameName !== prev.gameName ||
      next.startedAt !== prev.startedAt ||
      next.lineCount !== prev.lineCount ||
      next.content.length !== prev.content.length ||
      next.content.slice(-120) !== prev.content.slice(-120);
    if (!changed) return;

    snapshot.value = next;
    if (autoScroll.value && next.content) {
      await nextTick();
      scrollToBottom();
    }
  } finally {
    isLoading.value = false;
  }
};

const copyLogs = async () => {
  const text = snapshot.value.content || '';
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    document.body.removeChild(ta);
  }
};

onMounted(async () => {
  await loadLogs();
  refreshTimer = setInterval(loadLogs, 1200);
});

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
});
</script>

<template>
  <div class="game-log-viewer">
    <div class="toolbar">
      <span class="title">游戏日志</span>
      <span class="meta" v-if="snapshot.active">
        {{ snapshot.gameName }} | lines={{ snapshot.lineCount }} | started={{ snapshot.startedAt }}
      </span>
      <span class="meta" v-else>未开启日志会话</span>
      <div class="actions">
        <label class="auto-scroll-label">
          <input type="checkbox" v-model="autoScroll" />
          自动滚动
        </label>
        <button class="btn" @click="loadLogs" :disabled="isLoading">刷新</button>
        <button class="btn" @click="copyLogs">复制全部</button>
      </div>
    </div>
    <div class="content" ref="logContainer">
      <pre>{{ snapshot.content || '暂无日志...' }}</pre>
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
