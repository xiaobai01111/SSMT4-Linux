<script setup lang="ts">
import { onMounted, onUnmounted, provide } from "vue";
import { useRoute } from "vue-router";
import { appSettings, BGType } from "./store";
import TitleBar from "./components/TitleBar.vue";
import { ElMessage, ElNotification } from "element-plus";




const route = useRoute();

/**
 * =========================================================================
 *  Global Notification Entry Point (全局通知中心)
 * =========================================================================
 * 
 * 使用方法 (Using):
 * -----------------
 * 1. 在 Vue 组件中 (Setup Script):
 *    import { inject } from 'vue';
 *    const notify = inject<any>('notify');
 *    notify.success('Title', 'Message content');
 *    // or
 *    notify.error('Title', 'Error message');
 * 
 * 2. 或者直接使用 Element Plus 的 ElNotification / ElMessage 并依赖下方的全局样式修正
 *    (Or just use standard ElNotification/ElMessage imports, as we fix styles below)
 * 
 * 此处我们提供 `notify` 作为统一入口，方便未来可能的替换或扩展。
 */

const notify = {
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
  <!-- Custom Title Bar -->
  <TitleBar>
  </TitleBar>

  <!-- Background Layer -->
  <div class="bg-layer">
    <transition-group name="bg-trans">
      <!-- Image Background -->
      <div 
        v-if="appSettings.bgType === BGType.Image && appSettings.bgImage"
        :key="appSettings.bgImage"
        class="bg-item"
        :style="{ backgroundImage: `url(${appSettings.bgImage})` }"
      ></div>

      <!-- Video Background -->
      <video 
        v-if="appSettings.bgType === BGType.Video && appSettings.bgVideo" 
        :key="appSettings.bgVideo"
        :src="appSettings.bgVideo" 
        autoplay loop muted playsinline 
        class="bg-item"
      ></video>
    </transition-group>
  </div>
  
  <!-- Home & Websites & Settings Ambient Shadow Layer -->
  <div class="home-shadow-layer" v-if="route.path === '/' || route.path === '/websites'"></div>

  <!-- Global Mask Layer for Game Library Page -->
  <transition name="fade">
    <div v-if="route.path === '/games' || route.path === '/settings' || route.path === '/mods'" class="global-dim-layer"></div>
  </transition>

  <el-config-provider>
    <div class="app-container">
      <main class="app-main" :style="{
        '--content-bg-opacity': appSettings.contentOpacity,
        '--content-blur': `${appSettings.contentBlur}px`
      }">
        <div class="content-scroll-wrapper" :class="{ 'no-scroll': route.path === '/' }">
          <router-view v-slot="{ Component }">
            <transition name="page-blur" mode="out-in">
              <component :is="Component" />
            </transition>
          </router-view>
        </div>
      </main>
    </div>
  </el-config-provider>
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

.bg-video {
  /* Removed, replaced by .bg-item */
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
  /* Content area: Configurable */
  background-color: rgba(255, 255, 255, var(--content-bg-opacity, 0.55)); 
  backdrop-filter: blur(var(--content-blur, 3px)); 
  transition: opacity 0.5s ease;
  
  /* Dark Glass Style Overrides */
  background-color: rgba(0, 0, 0, var(--content-bg-opacity, 0.4)); 
  color: #ffffff;
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
  transition: opacity 0.2s ease, filter 0.2s ease, transform 0.2s ease;
}

.page-blur-enter-from {
  opacity: 0;
  filter: blur(10px);
  transform: scale(0.98);
}

.page-blur-leave-to {
  opacity: 0;
  filter: blur(10px);
  transform: scale(1.02);
}
</style>