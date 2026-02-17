<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { ValidateNameResult } from '../../api';

const props = defineProps<{
  configName: string;
  displayName: string;
  nameValidation: ValidateNameResult | null;
  readOnly: boolean;
  canSave: boolean;
  dirty: boolean;
  saving: boolean;
  error: string;
}>();

const emit = defineEmits<{
  (e: 'update:configName', value: string): void;
  (e: 'update:displayName', value: string): void;
  (e: 'create'): void;
  (e: 'delete'): void;
  (e: 'reset'): void;
  (e: 'save'): void;
  (e: 'validateName'): void;
}>();

const { t } = useI18n();

const validationClass = computed(() => {
  if (!props.nameValidation) return '';
  return props.nameValidation.valid ? 'text-ok' : 'text-err';
});
</script>

<template>
  <div class="setting-group">
    <div class="setting-label">{{ t('gamesettingsmodal.info.configName') }}</div>
    <div v-if="readOnly" class="value-text">{{ configName }}</div>
    <input
      v-else
      :value="configName"
      type="text"
      class="custom-input"
      :placeholder="t('gamesettingsmodal.info.configNamePlaceholder')"
      @input="emit('update:configName', ($event.target as HTMLInputElement).value)"
      @blur="emit('validateName')"
    />
    <div class="info-sub" :class="validationClass" v-if="!readOnly && nameValidation">
      {{ nameValidation.message }}
    </div>
    <div class="button-row" v-if="!readOnly">
      <button class="action-btn create" @click="emit('create')">
        {{ t('gamesettingsmodal.info.createConfig') }}
      </button>
      <button class="action-btn delete" @click="emit('delete')">
        {{ t('gamesettingsmodal.info.deleteConfig') }}
      </button>
      <button class="action-btn" @click="emit('reset')">
        {{ t('gamesettingsmodal.info.resetConfig') }}
      </button>
    </div>
  </div>

  <div class="setting-group">
    <div class="setting-label">{{ t('gamesettingsmodal.info.displayName') }}</div>
    <div v-if="readOnly" class="value-text">{{ displayName }}</div>
    <input
      v-else
      :value="displayName"
      type="text"
      class="custom-input"
      :placeholder="t('gamesettingsmodal.info.displayNamePlaceholder')"
      @input="emit('update:displayName', ($event.target as HTMLInputElement).value)"
    />
    <div class="button-row" v-if="!readOnly">
      <button class="action-btn highlight" :disabled="!canSave || !dirty || saving" @click="emit('save')">
        {{ saving ? t('gamesettingsmodal.info.saving') : t('gamesettingsmodal.info.saveMeta') }}
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

.custom-input {
  width: 100%;
  box-sizing: border-box;
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  padding: 8px 12px;
  color: #fff;
  font-size: 14px;
  outline: none;
}

.custom-input:focus {
  border-color: #f7ce46;
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
  flex: 1;
  color: #fff;
  background: rgba(255, 255, 255, 0.1);
}

.action-btn:hover {
  background: rgba(255, 255, 255, 0.2);
}

.action-btn.create {
  background: rgba(247, 206, 70, 0.2);
  border: 1px solid rgba(247, 206, 70, 0.4);
  color: #f7ce46;
}

.action-btn.create:hover {
  background: rgba(247, 206, 70, 0.3);
}

.action-btn.highlight {
  background: rgba(0, 122, 204, 0.3);
  border: 1px solid rgba(0, 122, 204, 0.5);
  color: #61afef;
}

.action-btn.highlight:hover {
  background: rgba(0, 122, 204, 0.5);
}

.action-btn.delete {
  background: rgba(232, 17, 35, 0.2);
  border: 1px solid rgba(232, 17, 35, 0.4);
  color: #ff6b6b;
}

.action-btn.delete:hover {
  background: rgba(232, 17, 35, 0.3);
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

.text-ok {
  color: #67c23a;
}

.text-err {
  color: #f56c6c;
}
</style>
