<script setup lang="ts">
import VirtualLogViewer from '../../components/VirtualLogViewer.vue';
import { useLogViewer } from './useLogViewer';

const {
  t,
  logContent,
  logDir,
  autoScroll,
  isLoading,
  loadLogs,
  copyLogs,
} = useLogViewer();
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
    <!-- 使用虚拟日志查看器替代原来的 <pre> 标签 -->
    <VirtualLogViewer
      :content="logContent || t('logviewer.empty')"
      :auto-scroll="autoScroll"
      :estimate-line-height="25.6"
      :overscan="5"
    />
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
</style>
