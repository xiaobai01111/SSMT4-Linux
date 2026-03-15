<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{
  showMenu: boolean;
  menuX: number;
  menuY: number;
}>();

defineEmits<{
  (event: 'hide-game'): void;
}>();

const { t } = useI18n();
</script>

<template>
  <div
    v-if="showMenu"
    class="context-menu glass-toast"
    :style="{ top: `${menuY}px`, left: `${menuX}px` }"
    @click.stop
  >
    <div class="menu-item" @click="$emit('hide-game')">
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none"
        stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="mr-2">
        <path d="m2 2 20 20"></path>
        <path d="M6.71 6.71A10.61 10.61 0 0 0 2 12a10.54 10.54 0 0 0 11.29 9 10.46 10.46 0 0 0 5-1.71"></path>
        <path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a10.16 10.16 0 0 1-2.39 3.46"></path>
        <path d="M8 12a4 4 0 0 0 4 4"></path>
        <path d="M12 8a4 4 0 0 0-4 4"></path>
      </svg>
      {{ t('home.contextmenu.hidegame') }}
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

.context-menu {
  position: fixed;
  z-index: 10000;
  padding: 6px;
  min-width: 140px;
  border-radius: 8px;
}

.menu-item {
  display: flex;
  align-items: center;
  padding: 10px 14px;
  cursor: pointer;
  color: #fff;
  font-size: 13px;
  border-radius: 6px;
  transition: all 0.2s;
}

.menu-item:hover {
  background: rgba(var(--el-color-danger-rgb), 0.2);
  color: var(--el-color-danger-light-3);
}

.mr-2 {
  margin-right: 8px;
}
</style>
