<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { DownloadTaskState } from '../../../downloadStore';
import type { ComponentDownloadProgressEvent } from '../../../types/events';

defineProps<{
  activeDownloadTask: DownloadTaskState | null;
  componentDlProgress: ComponentDownloadProgressEvent | null;
  showDownload: boolean;
}>();

const emit = defineEmits<{
  (event: 'open-download'): void;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="notifications-area">
    <div
      v-if="activeDownloadTask && !showDownload"
      class="mini-dl-bar glass-toast"
      @click="emit('open-download')"
    >
      <div class="mini-dl-info">
        <span class="mini-dl-name">{{ activeDownloadTask.displayName || activeDownloadTask.gameName }}</span>
        <span class="mini-dl-phase">
          {{
            activeDownloadTask.phase === 'verifying'
              ? t('home.downloadPhase.verifying')
              : (activeDownloadTask.progress?.phase === 'install'
                ? t('home.downloadPhase.installing')
                : t('home.downloadPhase.downloading'))
          }}
        </span>
        <span
          v-if="activeDownloadTask.progress && activeDownloadTask.progress.total_size > 0"
          class="mini-dl-pct"
        >
          {{
            Math.round(
              (activeDownloadTask.progress.finished_size / activeDownloadTask.progress.total_size) * 100,
            )
          }}%
        </span>
      </div>
      <div class="mini-dl-track">
        <div
          class="mini-dl-fill"
          :class="{ 'mini-dl-verify': activeDownloadTask.phase === 'verifying' || activeDownloadTask.progress?.phase === 'verify' }"
          :style="{
            width: `${
              activeDownloadTask.progress && activeDownloadTask.progress.total_size > 0
                ? Math.round((activeDownloadTask.progress.finished_size / activeDownloadTask.progress.total_size) * 100)
                : 0
            }%`,
          }"
        ></div>
      </div>
    </div>

    <div v-if="componentDlProgress" class="mini-dl-bar glass-toast component-dl">
      <div class="mini-dl-info">
        <span class="mini-dl-name">{{ componentDlProgress.componentName || componentDlProgress.componentId }}</span>
        <span class="mini-dl-phase">
          {{
            componentDlProgress.phase === 'downloading'
              ? t('home.componentPhase.downloading')
              : componentDlProgress.phase === 'extracting'
                ? t('home.componentPhase.extracting')
                : componentDlProgress.phase
          }}
        </span>
        <span
          v-if="componentDlProgress.total > 0 && componentDlProgress.phase === 'downloading'"
          class="mini-dl-pct"
        >
          {{ Math.round(componentDlProgress.downloaded / componentDlProgress.total * 100) }}%
        </span>
      </div>
      <div class="mini-dl-track">
        <div
          class="mini-dl-fill"
          :class="{ 'mini-dl-verify': componentDlProgress.phase === 'extracting' }"
          :style="{
            width: componentDlProgress.total > 0
              ? `${Math.round(componentDlProgress.downloaded / componentDlProgress.total * 100)}%`
              : '100%',
          }"
        ></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.glass-toast {
  background-color: rgba(20, 25, 30, 0.75);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
}

.notifications-area {
  position: absolute;
  top: 32px;
  right: 32px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  z-index: 100;
}

.mini-dl-bar {
  width: 300px;
  padding: 14px 16px;
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.25s;
  animation: toastSlideIn 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}

.mini-dl-bar:hover {
  background: rgba(40, 45, 50, 0.85);
  border-color: var(--el-color-primary-light-5);
  transform: translateY(-2px);
}

.mini-dl-info {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
}

.mini-dl-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-color-primary-light-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 140px;
}

.mini-dl-phase {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.7);
  padding: 2px 8px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
}

.mini-dl-pct {
  font-size: 13px;
  font-weight: 700;
  margin-left: auto;
}

.mini-dl-track {
  width: 100%;
  height: 4px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
  overflow: hidden;
}

.mini-dl-fill {
  height: 100%;
  background: var(--el-color-primary);
  border-radius: 2px;
  transition: width 0.3s ease;
}

.mini-dl-fill.mini-dl-verify {
  background: var(--el-color-success);
}

@keyframes toastSlideIn {
  from {
    opacity: 0;
    transform: translateX(50px) scale(0.95);
  }

  to {
    opacity: 1;
    transform: translateX(0) scale(1);
  }
}
</style>
