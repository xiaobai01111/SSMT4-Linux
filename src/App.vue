<script setup lang="ts">
import { computed, onMounted, onUnmounted, provide } from "vue";
import { useRoute } from "vue-router";
import { appSettings, BGType } from "./store";
import TitleBar from "./components/TitleBar.vue";
import FeatureOnboarding from "./components/FeatureOnboarding.vue";
import { ElMessage, ElNotification } from "element-plus";
import "element-plus/es/components/message/style/css";
import "element-plus/es/components/notification/style/css";
import { dismissTask, taskNotifications } from "./taskNotifications";
import { NOTIFY_KEY, type NotifyApi } from "./types/notify";




const route = useRoute();
const blurEnabledRoutes = new Set(['/', '/websites']);
const isBlurRoute = computed(() => blurEnabledRoutes.has(route.path));
const effectiveContentBlur = computed(() => {
  if (!isBlurRoute.value) return 0;
  return Math.min(Math.max(appSettings.contentBlur || 0, 0), 3);
});

/**
 * =========================================================================
 *  Global Notification Entry Point (全局通知中心)
 * =========================================================================
 * 
 * 使用方法 (Using):
 * -----------------
 * 1. 在 Vue 组件中 (Setup Script):
 *    import { inject } from 'vue';
 *    import { NOTIFY_KEY } from './types/notify';
 *    const notify = inject(NOTIFY_KEY, null);
 *    notify.success('Title', 'Message content');
 *    // or
 *    notify.error('Title', 'Error message');
 * 
 * 2. 或者直接使用 Element Plus 的 ElNotification / ElMessage 并依赖下方的全局样式修正
 *    (Or just use standard ElNotification/ElMessage imports, as we fix styles below)
 * 
 * 此处我们提供 `notify` 作为统一入口，方便未来可能的替换或扩展。
 */

const notify: NotifyApi = {
  success: (title: string, message?: string) => {
    ElNotification({
      title: title,
      message: message || '',
      type: 'success',
      position: 'top-right',
      zIndex: 99999 // Ensure on top of TitleBar
    });
  },
  warning: (title: string, message?: string) => {
    ElNotification({
      title: title,
      message: message || '',
      type: 'warning',
      position: 'top-right',
      zIndex: 99999
    });
  },
  info: (title: string, message?: string) => {
    ElNotification({
      title: title,
      message: message || '',
      type: 'info',
      position: 'top-right',
      zIndex: 99999
    });
  },
  error: (title: string, message?: string) => {
    ElNotification({
      title: title,
      message: message || '',
      type: 'error',
      position: 'top-right',
      zIndex: 99999
    });
  },
  // Legacy support for simple message toasts
  toast: (message: string, type: 'success' | 'warning' | 'info' | 'error' = 'info') => {
      ElMessage({
          message,
          type,
          zIndex: 99999
      });
  }
};

// Provide notify globally to all child components
provide(NOTIFY_KEY, notify);
provide('notify', notify);
// Disable default right-click context menu
const preventContextMenu = (event: Event) => {
  event.preventDefault();
};

onMounted(() => {
  document.addEventListener('contextmenu', preventContextMenu);
});

onUnmounted(() => {
  document.removeEventListener('contextmenu', preventContextMenu);
});

/* bgStyle removed, handled in template */
</script>

<template>
  <!-- 日志查看器独立窗口：无 TitleBar、无背景层 -->
  <template v-if="route.path === '/log-viewer' || route.path === '/game-log-viewer'">
    <router-view />
  </template>

  <!-- 主应用布局 -->
  <template v-else>
    <!-- Custom Title Bar -->
    <TitleBar>
    </TitleBar>

    <!-- Background Layer -->
    <div class="bg-layer">
      <!-- Image Background -->
      <div
        v-if="appSettings.bgType === BGType.Image && appSettings.bgImage"
        class="bg-item"
        :style="{ backgroundImage: `url(${appSettings.bgImage})` }"
      ></div>
    </div>
    
    <!-- Home & Websites & Settings Ambient Shadow Layer -->
    <div class="home-shadow-layer" v-show="route.path === '/' || route.path === '/websites'"></div>

    <!-- Global Mask Layer for Game Library Page -->
    <div v-show="route.path === '/games'" class="global-dim-layer"></div>

    <el-config-provider>
      <div class="app-container">
        <main class="app-main" :style="{
          '--content-bg-opacity': appSettings.contentOpacity,
          '--content-blur': `${effectiveContentBlur}px`
        }" :class="{ 'enable-content-blur': effectiveContentBlur > 0.1 }">
          <div class="content-scroll-wrapper" :class="{ 'no-scroll': route.path === '/' }">
            <router-view />
          </div>
        </main>
      </div>
    </el-config-provider>

    <FeatureOnboarding />

    <div class="task-toast-stack">
      <transition-group name="task-toast">
        <div
          v-for="task in taskNotifications"
          :key="task.id"
          class="task-toast"
          :class="`is-${task.status}`"
        >
          <div class="task-toast-head">
            <div class="task-toast-title">{{ task.title }}</div>
            <button
              v-if="task.status !== 'running'"
              class="task-toast-close"
              type="button"
              @click="dismissTask(task.id)"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24"
                fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
          <div class="task-toast-body">
            <div class="task-toast-text">{{ task.message }}</div>
            <div class="task-toast-progress">
              <div
                class="task-toast-progress-bar"
                :class="{ indeterminate: task.status === 'running' && task.progress == null }"
                :style="task.progress == null ? undefined : { width: `${task.progress}%` }"
              ></div>
            </div>
            <div v-if="task.status === 'running' && task.progress != null" class="task-toast-meta">
              {{ Math.round(task.progress) }}%
            </div>
          </div>
        </div>
      </transition-group>
    </div>
  </template>
</template>

<style>
/* Global Resets */
html, body {
  margin: 0;
  padding: 0;
  height: 100%;
  font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  
  /* Disable text selection */
  user-select: none;

  /* Cyberpunk Black Fallback: Deep dark with subtle neon glows */
  background-color: #030305;
  background-image: 
    radial-gradient(circle at 50% 50%, rgba(60, 20, 100, 0.2) 0%, transparent 60%),
    radial-gradient(circle at 50% 50%, rgba(0, 100, 180, 0.1) 0%, transparent 70%);

  overflow: hidden;
}

/* Re-enable selection for inputs */
input, textarea {
  user-select: text;
}

/* 
  Global Notification/Message Fix 
  Ensure they sit ABOVE the TitleBar (which likely has z-index ~1000-2000)
*/
.el-message, .el-notification, .el-message-box__wrapper {
  z-index: 99999 !important;
}

.task-toast-stack {
  position: fixed;
  top: 44px;
  right: 16px;
  z-index: 100001;
  display: flex;
  flex-direction: column;
  gap: 12px;
  pointer-events: none;
}

.task-toast {
  width: min(360px, calc(100vw - 24px));
  padding: 14px 14px 12px;
  border-radius: 10px;
  border: 1px solid rgba(0, 240, 255, 0.24);
  background: rgba(8, 14, 18, 0.96);
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.4);
  pointer-events: auto;
  backdrop-filter: blur(10px);
}

.task-toast.is-success {
  border-color: rgba(103, 194, 58, 0.34);
}

.task-toast.is-error {
  border-color: rgba(245, 108, 108, 0.34);
}

.task-toast-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.task-toast-title {
  color: #00f0ff;
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0.4px;
}

.task-toast.is-success .task-toast-title {
  color: #67c23a;
}

.task-toast.is-error .task-toast-title {
  color: #f56c6c;
}

.task-toast-close {
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: rgba(255, 255, 255, 0.5);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.task-toast-close:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.1);
}

.task-toast-body {
  margin-top: 8px;
}

.task-toast-text {
  color: rgba(255, 255, 255, 0.78);
  font-size: 13px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}

.task-toast-progress {
  margin-top: 10px;
  height: 6px;
  border-radius: 999px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.08);
}

.task-toast-progress-bar {
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, rgba(0, 240, 255, 0.45), #00f0ff);
  transition: width 0.18s ease;
}

.task-toast.is-success .task-toast-progress-bar {
  background: linear-gradient(90deg, rgba(103, 194, 58, 0.45), #67c23a);
}

.task-toast.is-error .task-toast-progress-bar {
  background: linear-gradient(90deg, rgba(245, 108, 108, 0.4), #f56c6c);
}

.task-toast-progress-bar.indeterminate {
  width: 42%;
  animation: task-toast-indeterminate 1.15s ease-in-out infinite;
}

.task-toast-meta {
  margin-top: 7px;
  color: rgba(255, 255, 255, 0.48);
  font-size: 12px;
  text-align: right;
}

.task-toast-enter-active,
.task-toast-leave-active {
  transition: all 0.2s ease;
}

.task-toast-enter-from,
.task-toast-leave-to {
  opacity: 0;
  transform: translateY(-8px) translateX(12px);
}

@keyframes task-toast-indeterminate {
  0% { transform: translateX(-65%); }
  100% { transform: translateX(220%); }
}


#app {
  height: 100%;
  position: relative; /* Need relative for absolute children */
}
.bg-layer {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 0;
  overflow: hidden;
  background-color: #050505; /* Black fallback for transitions */
}

/* Background Transition Items */
.bg-item {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  background-size: cover;
  background-position: center;
  will-change: opacity;
  contain: strict;
}

/* Transition Classes */
.bg-trans-enter-active,
.bg-trans-leave-active {
  transition: opacity 0.6s ease; /* Smooth 0.6s fade */
}

.bg-trans-enter-from,
.bg-trans-leave-to {
  opacity: 0;
}

.home-shadow-layer {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 0; /* On top of bg-layer (also 0, but later in DOM), behind app-container (1) */
  pointer-events: none;
  /* 
     Fix: Removed 'multiply' blend mode which made things look dirty.
     New Style: Clean cinematic vignette + bottom fade for UI readability.
     Keeps the center bright and clean.
  */
  background: 
    /* 1. Seamless smooth fade from bottom (for potential footer text) */
    linear-gradient(to top, rgba(0, 0, 0, 0.5) 0%, transparent 25%),
    
    /* 2. Very subtle cinematic vignette (corners only, center is pure clean) */
    radial-gradient(circle at 50% 50%, transparent 75%, rgba(0, 0, 0, 0.4) 140%);
}

.global-dim-layer {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 0; /* Above bg-layer (0 via DOM order), below App Content (1) */
  pointer-events: none; /* Let clicks pass through if needed, though standard bg doesn't need interactions */
  
  /* Center Radiating Light Background (Copied from GameLibrary.vue) */
    background: radial-gradient(
        circle at 50% 50%, 
        rgba(0, 0, 0, 0.6) 0%, 
        rgba(0, 0, 0, 0.9) 50%, 
        rgba(0, 0, 0, 0.98) 90%
    );
}
</style>

<style scoped>
.app-container {
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  position: relative;
  z-index: 1; /* Above bg */
  padding-top: 32px; /* TitleBar height */
}

.app-main {
  width: 100%;
  height: 100%;
  padding: 0;
  overflow: hidden;
  position: relative;
  contain: layout style;
  /* Content area: Configurable */
  background-color: rgba(255, 255, 255, var(--content-bg-opacity, 0.55)); 
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  transition: opacity 0.3s ease;
  
  /* Dark Glass Style Overrides */
  background-color: rgba(0, 0, 0, var(--content-bg-opacity, 0.4)); 
  color: #ffffff;
}

.app-main.enable-content-blur {
  backdrop-filter: blur(var(--content-blur, 3px));
  -webkit-backdrop-filter: blur(var(--content-blur, 3px));
}

.content-scroll-wrapper {
  margin-top: 0;
  height: 100%;
  overflow-y: auto;
  padding: 0 0 32px 0; /* Add 32px bottom padding globally */
  box-sizing: border-box; /* Ensures padding doesn't cause overflow */
}

/* Custom Scrollbar for Content */
.content-scroll-wrapper::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
.content-scroll-wrapper::-webkit-scrollbar-track {
  background: transparent;
}
.content-scroll-wrapper::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2); 
  border-radius: 4px;
}
.content-scroll-wrapper::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.4); 
}

.no-scroll {
  overflow-y: hidden !important;
}

/* Glassmorphism for Element Plus Components */
:deep(.el-card) {
  background-color: rgba(30, 30, 30, 0.6) !important;
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #ffffff;
  --el-card-bg-color: transparent;
}
:deep(.el-card__header) {
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  color: #fff;
}
/* Form Labels */
:deep(.el-form-item__label) {
  color: #e0e0e0 !important;
}

/* Page Transition Effects */
.page-blur-enter-active,
.page-blur-leave-active {
  transition: opacity 0.2s ease;
}

.page-blur-enter-from,
.page-blur-leave-to {
  opacity: 0;
}
</style>
