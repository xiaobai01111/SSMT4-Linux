<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core';
import { useI18n } from 'vue-i18n';
import type { GameTemplateInfo } from '../../api';

defineProps<{
  showMenu: boolean;
  menuX: number;
  menuY: number;
  showBlankMenu: boolean;
  blankMenuX: number;
  blankMenuY: number;
  showImportDialog: boolean;
  importLoading: boolean;
  templateList: GameTemplateInfo[];
}>();

defineEmits<{
  (event: 'add-to-favorites'): void;
  (event: 'delete-game'): void;
  (event: 'open-import-dialog'): void;
  (event: 'open-templates-folder'): void;
  (event: 'close-import-dialog'): void;
  (event: 'import-template', template: GameTemplateInfo): void;
}>();

const { t, te } = useI18n();
</script>

<template>
  <div
    v-if="showMenu"
    class="context-menu"
    :style="{ top: `${menuY}px`, left: `${menuX}px` }"
    @click.stop
  >
    <div class="menu-item" @click="$emit('add-to-favorites')">
      {{ t('gamelibrary.addToSidebar') }}
    </div>
    <div class="menu-item menu-item-danger" @click="$emit('delete-game')">
      {{ t('gamelibrary.deleteGame') }}
    </div>
  </div>

  <div
    v-if="showBlankMenu"
    class="context-menu"
    :style="{ top: `${blankMenuY}px`, left: `${blankMenuX}px` }"
    @click.stop
  >
    <div class="menu-item" @click="$emit('open-import-dialog')">
      {{ t('gamelibrary.importConfig') }}
    </div>
    <div class="menu-item" @click="$emit('open-templates-folder')">
      {{ t('gamelibrary.openTemplatesFolder') }}
    </div>
  </div>

  <div v-if="showImportDialog" class="import-overlay" @click.self="$emit('close-import-dialog')">
    <div class="import-dialog">
      <div class="import-header">
        <span>{{ t('gamelibrary.importConfig') }}</span>
        <button class="import-close" @click="$emit('close-import-dialog')">✕</button>
      </div>
      <div class="import-body">
        <div v-if="importLoading" class="import-loading">Loading...</div>
        <div v-else-if="templateList.length === 0" class="import-empty">{{ t('gamelibrary.noTemplates') }}</div>
        <div v-else class="import-list">
          <div
            v-for="tmpl in templateList"
            :key="tmpl.name"
            class="import-item"
            :class="{ 'import-item-exists': tmpl.alreadyExists }"
            @click="$emit('import-template', tmpl)"
          >
            <img
              v-if="tmpl.hasIcon && tmpl.iconPath"
              :src="convertFileSrc(tmpl.iconPath)"
              class="import-icon"
              alt=""
            />
            <div v-else class="import-icon-placeholder">?</div>
            <div class="import-info">
              <div class="import-name">{{ te(`games.${tmpl.gameId}`) ? t(`games.${tmpl.gameId}`) : (tmpl.displayName || tmpl.name) }}</div>
              <div class="import-name-sub">{{ tmpl.name }} ({{ tmpl.gameId }})</div>
            </div>
            <div v-if="tmpl.alreadyExists" class="import-badge">{{ t('gamelibrary.alreadyExists') }}</div>
          </div>
        </div>
      </div>
      <div class="import-footer">
        <button class="import-open-folder" @click="$emit('open-templates-folder')">
          {{ t('gamelibrary.openTemplatesFolder') }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.context-menu {
  position: fixed;
  z-index: 10000;
  background: rgba(15, 15, 20, 0.98);
  border: 1px solid rgba(0, 240, 255, 0.5);
  border-radius: 4px;
  padding: 4px;
  min-width: 140px;
}

.menu-item {
  padding: 8px 12px;
  cursor: pointer;
  color: #fff;
  font-size: 13px;
  border-radius: 2px;
  transition: background-color 0.1s;
}

.menu-item:hover {
  background-color: #00f0ff;
  color: #000;
}

.menu-item-danger {
  color: #ff0055;
}

.menu-item-danger:hover {
  background-color: #ff0055;
  color: #fff;
}

.import-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.8);
  z-index: 20000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.import-dialog {
  background: rgba(10, 15, 20, 0.98);
  border: 1px solid #00f0ff;
  border-radius: 8px;
  width: 420px;
  max-height: 70vh;
  display: flex;
  flex-direction: column;
}

.import-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
  font-size: 16px;
  font-weight: 600;
  color: #00f0ff;
  text-transform: uppercase;
  letter-spacing: 1px;
}

.import-close {
  background: none;
  border: none;
  color: rgba(255,255,255,0.5);
  font-size: 18px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 2px;
}

.import-close:hover {
  color: #ff0055;
  background: rgba(255, 0, 85, 0.1);
}

.import-body {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.import-loading,
.import-empty {
  text-align: center;
  color: #00f0ff;
  padding: 32px 0;
  font-size: 14px;
}

.import-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.import-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
  border: 1px solid transparent;
}

.import-item:hover {
  background: rgba(0, 240, 255, 0.05);
  border-color: rgba(0, 240, 255, 0.3);
}

.import-item-exists {
  opacity: 0.5;
  filter: grayscale(1);
}

.import-icon {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  object-fit: cover;
  border: 1px solid rgba(255,255,255,0.2);
}

.import-icon-placeholder {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  background: rgba(0, 240, 255, 0.1);
  border: 1px dashed rgba(0, 240, 255, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #00f0ff;
  font-size: 18px;
}

.import-info {
  flex: 1;
  min-width: 0;
}

.import-name {
  color: #fff;
  font-size: 14px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.import-name-sub {
  color: rgba(255,255,255,0.5);
  font-size: 11px;
  margin-top: 4px;
  font-family: monospace;
}

.import-badge {
  font-size: 10px;
  color: #ff0055;
  border: 1px solid #ff0055;
  background: rgba(255, 0, 85, 0.1);
  padding: 2px 6px;
  border-radius: 2px;
  white-space: nowrap;
  text-transform: uppercase;
}

.import-footer {
  padding: 16px;
  border-top: 1px solid rgba(0, 240, 255, 0.3);
  text-align: center;
}

.import-open-folder {
  background: transparent;
  border: 1px solid #00f0ff;
  color: #00f0ff;
  padding: 8px 16px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 1px;
  transition: all 0.2s;
}

.import-open-folder:hover {
  background: #00f0ff;
  color: #000;
}
</style>
