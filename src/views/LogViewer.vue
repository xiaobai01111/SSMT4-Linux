<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue';
import { readLogFile, getLogDir } from '../api';
import { useI18n } from 'vue-i18n';

const { t } = useI18n();
const logContent = ref('');
const logDir = ref('');
const autoScroll = ref(true);
const isLoading = ref(false);
const logContainer = ref<HTMLElement | null>(null);
let refreshTimer: ReturnType<typeof setInterval> | null = null;

const loadLogs = async () => {
  if (isLoading.value) return;
  try {
    isLoading.value = true;
    const nextContent = await readLogFile(1200);
    const changed =
      nextContent.length !== logContent.value.length ||
      nextContent.slice(-120) !== logContent.value.slice(-120);
    if (!changed) return;
    logContent.value = nextContent;
    if (autoScroll.value) {
      await nextTick();
      scrollToBottom();
    }
  } catch (e) {
    logContent.value = `${t('logviewer.readFailed')}: ${e}`;
  } finally {
    isLoading.value = false;
  }
};

const scrollToBottom = () => {
  if (logContainer.value) {
    logContainer.value.scrollTop = logContainer.value.scrollHeight;
  }
};

const copyLogs = async () => {
  try {
    await navigator.clipboard.writeText(logContent.value);
  } catch {
    // fallback
    const ta = document.createElement('textarea');
    ta.value = logContent.value;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    document.body.removeChild(ta);
  }
};

onMounted(async () => {
  logDir.value = await getLogDir();
  await loadLogs();
  // 降低刷新频率，避免长日志文本高频重排
  refreshTimer = setInterval(loadLogs, 2500);
});

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
});
</script>

<template>
  <div class="log-viewer">
    <div class="log-toolbar">
      <span class="log-title">{{ t('logviewer.title') }}</span>
      <span class="log-dir">{{ logDir }}</span>
      <div class="log-actions">
        <label class="auto-scroll-label">
          <input type="checkbox" v-model="autoScroll" />
          {{ t('logviewer.autoScroll') }}
        </label>
        <button class="log-btn" @click="loadLogs" :disabled="isLoading">{{ t('logviewer.refresh') }}</button>
        <button class="log-btn" @click="copyLogs">{{ t('logviewer.copyAll') }}</button>
      </div>
    </div>
    <div class="log-content" ref="logContainer">
      <pre>{{ logContent || t('logviewer.empty') }}</pre>
    </div>
  </div>
</template>

<style scoped>
.log-viewer {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #0d0d0d;
  color: #e0e0e0;
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
}

.log-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 16px;
  background: #1a1a1a;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
}

.log-title {
  font-size: 13px;
  font-weight: 700;
  color: #F7CE46;
}

.log-dir {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.log-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.auto-scroll-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.5);
  display: flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  user-select: none;
}

.auto-scroll-label input {
  accent-color: #F7CE46;
}

.log-btn {
  padding: 4px 12px;
  font-size: 11px;
  border: 1px solid rgba(255, 255, 255, 0.15);
  background: rgba(255, 255, 255, 0.05);
  color: rgba(255, 255, 255, 0.7);
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s;
}

.log-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.log-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.log-content {
  flex: 1;
  overflow: auto;
  padding: 12px 16px;
}

.log-content pre {
  margin: 0;
  font-size: 11px;
  line-height: 1.6;
  white-space: pre;
  word-break: normal;
  color: #c8c8c8;
}

/* 滚动条样式 */
.log-content::-webkit-scrollbar {
  width: 6px;
}
.log-content::-webkit-scrollbar-track {
  background: transparent;
}
.log-content::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
  border-radius: 3px;
}
.log-content::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.25);
}
</style>
