<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{
  version: string;
  gamePath: string;
  loading: boolean;
  error: string;
}>();

const emit = defineEmits<{
  (e: 'refresh'): void;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="setting-group">
    <div class="setting-label">{{ t('gamesettingsmodal.info.gameVersion') }}</div>
    <div class="version-value" v-if="!loading">
      {{ version || t('gamesettingsmodal.info.versionUnknown') }}
    </div>
    <div class="info-sub" v-else>
      {{ t('gamesettingsmodal.info.versionLoading') }}
    </div>
    <div class="info-sub" v-if="gamePath">
      {{ t('gamesettingsmodal.info.versionPath') }}: {{ gamePath }}
    </div>
    <div class="button-row">
      <button class="action-btn" :disabled="loading" @click="emit('refresh')">
        {{ loading ? t('gamesettingsmodal.info.versionRefreshing') : t('gamesettingsmodal.info.refreshVersion') }}
      </button>
    </div>
    <div v-if="error" class="info-sub text-err">{{ error }}</div>
  </div>
</template>

<style scoped>
.setting-group {
  margin-bottom: 24px;
}

.setting-label {
  display: block;
  font-size: 14px;
  color: rgba(255, 255, 255, 0.8);
  margin-bottom: 8px;
}

.version-value {
  border-radius: 4px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(0, 0, 0, 0.25);
  color: rgba(255, 255, 255, 0.92);
  padding: 9px 12px;
  font-size: 14px;
}

.button-row {
  display: flex;
  gap: 12px;
  margin-top: 12px;
}

.action-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 4px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
  color: #fff;
  background: rgba(255, 255, 255, 0.1);
}

.action-btn:hover {
  background: rgba(255, 255, 255, 0.2);
}

.action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.info-sub {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.45);
  margin-top: 6px;
  word-break: break-all;
}

.text-err {
  color: #f56c6c;
}
</style>
