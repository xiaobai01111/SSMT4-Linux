<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{
  iconFile?: string | null;
  backgroundFile?: string | null;
  readOnly: boolean;
  saving: boolean;
  error: string;
}>();

const emit = defineEmits<{
  (e: 'selectIcon'): void;
  (e: 'selectBackground'): void;
  (e: 'resetBackground'): void;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="setting-group">
    <div class="setting-label">{{ t('gamesettingsmodal.info.backgroundType') }}</div>
    <div class="value-text">{{ t('gamesettingsmodal.image') }}</div>
    <div class="info-sub">{{ t('gamesettingsmodal.info.backgroundImage') }}（仅支持图片）</div>
  </div>

  <div class="setting-group">
    <div class="setting-label">{{ t('gamesettingsmodal.info.gameIcon') }}</div>
    <div class="info-sub" v-if="iconFile">{{ iconFile }}</div>
    <button v-if="!readOnly" class="action-btn" @click="emit('selectIcon')">
      {{ t('gamesettingsmodal.info.selectIcon') }}
    </button>
  </div>

  <div class="setting-group">
    <div class="setting-label">{{ t('gamesettingsmodal.info.backgroundImage') }}</div>
    <div class="info-sub" v-if="backgroundFile">{{ backgroundFile }}</div>
    <div class="button-row" v-if="!readOnly">
      <button class="action-btn" @click="emit('selectBackground')">
        {{ t('gamesettingsmodal.info.selectBackground') }}
      </button>
      <button class="action-btn" :disabled="saving" @click="emit('resetBackground')">
        {{ t('gamesettingsmodal.info.resetBackground') }}
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

.value-text {
  width: 100%;
  box-sizing: border-box;
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  padding: 8px 12px;
  color: rgba(255, 255, 255, 0.92);
  font-size: 14px;
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
}

.text-err {
  color: #f56c6c;
}
</style>
