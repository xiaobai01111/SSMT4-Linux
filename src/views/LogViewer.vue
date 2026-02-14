<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue';
import { readLogFile, getLogDir } from '../api';

const logContent = ref('');
const logDir = ref('');
const autoScroll = ref(true);
const isLoading = ref(false);
const logContainer = ref<HTMLElement | null>(null);
let refreshTimer: ReturnType<typeof setInterval> | null = null;

const loadLogs = async () => {
  try {
    isLoading.value = true;
    logContent.value = await readLogFile(2000);
    if (autoScroll.value) {
      await nextTick();
      scrollToBottom();
    }
  } catch (e) {
    logContent.value = `读取日志失败: ${e}`;
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
  // 每 2 秒自动刷新
  refreshTimer = setInterval(loadLogs, 2000);
});

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
});
</script>

<template>
  <div class="log-viewer">
    <div class="log-toolbar">
      <span class="log-title">SSMT4 日志查看器</span>
      <span class="log-dir">{{ logDir }}</span>
      <div class="log-actions">
        <label class="auto-scroll-label">
          <input type="checkbox" v-model="autoScroll" />
          自动滚动
        </label>
        <button class="log-btn" @click="loadLogs" :disabled="isLoading">刷新</button>
        <button class="log-btn" @click="copyLogs">复制全部</button>
      </div>
    </div>
    <div class="log-content" ref="logContainer">
      <pre>{{ logContent || '暂无日志...' }}</pre>
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
  overflow-y: auto;
  padding: 12px 16px;
}

.log-content pre {
  margin: 0;
  font-size: 11px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
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
