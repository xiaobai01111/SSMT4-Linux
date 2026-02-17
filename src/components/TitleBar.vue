<script setup lang="ts">
import { ref } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { appSettings } from '../store';
import { getCurrentWindow } from '@tauri-apps/api/window';

const router = useRouter();
const route = useRoute();
const isMaximized = ref(false);
const appWindow = getCurrentWindow();

// 实际 Tauri 窗口控制
const minimize = () => { appWindow.minimize(); };
const toggleMaximize = async () => {
  await appWindow.toggleMaximize();
  isMaximized.value = await appWindow.isMaximized();
};
const close = () => { appWindow.close(); };
const startDrag = () => { appWindow.startDragging(); };

const toggleGamePage = (e: MouseEvent) => {
    e.stopPropagation(); // prevent drag
    if (route.path === '/games') {
        if (window.history.length > 1) {
            router.back();
        } else {
            router.push('/');
        }
    } else {
        if (route.path === '/settings') {
            router.replace('/games');
        } else {
            router.push('/games');
        }
    }
};

const toggleSettingsPage = (e: MouseEvent) => {
    e.stopPropagation();
    if (route.path === '/settings') {
        if (window.history.length > 1) {
            router.back();
        } else {
            router.push('/');
        }
    } else {
        if (route.path === '/games') {
             router.replace('/settings');
        } else {
             router.push('/settings');
        }
    }
};

const navTo = (path: string) => {
    router.push(path);
};
</script>

<template>
  <div class="titlebar">
    <div class="nav-controls">
        <div class="nav-button" :class="{ active: route.path === '/' }" @click="navTo('/')" title="主页">
            <svg xmlns="http://www.w3.org/2000/svg" width="26" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path><polyline points="9 22 9 12 15 12 15 22"></polyline></svg>
            <span class="nav-text">主页</span>
        </div>
        <div v-if="appSettings.showWebsites" class="nav-button" :class="{ active: route.path === '/websites' }" @click="navTo('/websites')" title="常用网址">
            <svg xmlns="http://www.w3.org/2000/svg" width="26" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path></svg>
            <span class="nav-text">常用网址</span>
        </div>
        <div v-if="appSettings.showDocuments" class="nav-button" :class="{ active: route.path === '/documents' }" @click="navTo('/documents')" title="使用文档">
            <svg xmlns="http://www.w3.org/2000/svg" width="26" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline></svg>
            <span class="nav-text">使用文档</span>
        </div>
    </div>

    <div class="drag-region" @mousedown="startDrag">
      <div class="title-content">
          <slot></slot>
      </div>
    </div>
    
    <div class="window-controls">
      <!-- Game List Toggle Button -->
      <div class="control-button game-list-toggle" :class="{ active: route.path === '/games' }" @click="toggleGamePage" title="游戏库">
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="7" height="7"></rect>
          <rect x="14" y="3" width="7" height="7"></rect>
          <rect x="14" y="14" width="7" height="7"></rect>
          <rect x="3" y="14" width="7" height="7"></rect>
        </svg>
      </div>

      <!-- Settings Button (Placed to right of Game Toggle) -->
      <div class="control-button settings-btn" :class="{ active: route.path === '/settings' }" @click="toggleSettingsPage" title="Settings">
         <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1.82 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
      </div>

      <div class="control-button minimize" @click="minimize">
        <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10">
          <path d="M0,5 L10,5 L10,6 L0,6 Z" />
        </svg>
      </div>
      
      <div class="control-button maximize" @click="toggleMaximize">
        <svg v-if="!isMaximized" xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10">
          <path d="M1,1 L9,1 L9,9 L1,9 L1,1 Z M0,0 L0,10 L10,10 L10,0 L0,0 Z" />
        </svg>
        <svg v-else xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10">
           <path d="M2.1,0v2H0v8.1h8.2v-2h2V0H2.1z M7.2,9.2H1V3h6.1V9.2z M9.2,7.1h-1V2H3.1V1h6.1V7.1z" />
        </svg>
      </div>
      
      <div class="control-button close" @click="close">
        <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10">
          <path d="M1,0 L5,4 L9,0 L10,1 L6,5 L10,9 L9,10 L5,6 L1,10 L0,9 L4,5 L0,1 L1,0 Z" />
        </svg>
      </div>
    </div>
  </div>
</template>

<style scoped>
.titlebar {
  height: 32px;
  width: 100%;
  display: flex;
  justify-content: space-between;
  align-items: center; /* Changed to center to align items vertically */
  position: fixed;
  top: 0;
  left: 0;
  z-index: 9999;
  user-select: none;
  background: rgba(0, 0, 0, 0.5); /* Darker background */
  backdrop-filter: blur(12px); /* stronger blur */
  transition: background 0.3s ease, backdrop-filter 0.3s ease;
}

.nav-controls {
    display: flex;
    align-items: center;
    height: 100%;
    padding-left: 0; 
    z-index: 10001; /* Above drag region */
}

.nav-button {
    display: flex;
    align-items: center;
    padding: 0 12px;
    height: 100%;
    cursor: auto; /* It is clickable, but we set to auto to avoid global pointer. Actual clickable is fine. */
    cursor: pointer;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
    transition: all 0.2s;
    border-radius: 4px;
}
.nav-button:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.1);
}
.nav-button.active {
    color: #fff;
    font-weight: 600;
    background: rgba(255, 255, 255, 0.15);
}
.nav-button svg {
    margin-right: 6px;
    opacity: 0.8;
}
.nav-button.active svg {
    opacity: 1;
}

.drag-region {
  flex-grow: 1;
  height: 100%;
  background: transparent; 
}

.title-content {
    height: 100%;
    display: flex;
    align-items: center;
    font-size: 12px;
}

.window-controls {
  display: flex;
  height: 32px;
  flex-shrink: 0;
  z-index: 10001; /* Ensure buttons are top-most */
}

.control-button {
  display: flex;
  justify-content: center;
  align-items: center;
  width: 46px;
  height: 100%;
  cursor: default;
  transition: background-color 0.1s;
}

.control-button svg {
  fill: currentColor;
}

.control-button:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

.control-button.close:hover {
  background-color: #e81123;
}
.control-button.close:hover svg {
    fill: white;
}
</style>
