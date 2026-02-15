<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { PresetCatalogItem } from '../../api';

const props = defineProps<{
  modelValue: string;
  presets: PresetCatalogItem[];
  readOnly: boolean;
  canSave: boolean;
  dirty: boolean;
  saving: boolean;
  error: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void;
  (e: 'save'): void;
}>();

const { t } = useI18n();

const selectedPreset = computed(() =>
  props.presets.find((item) => item.id === props.modelValue),
);
</script>

<template>
  <div class="setting-group">
    <div class="setting-label">{{ t('gamesettingsmodal.info.gamePreset') }}</div>
    <div v-if="readOnly" class="value-text">
      {{ selectedPreset?.displayNameEn || selectedPreset?.label || modelValue }}
    </div>
    <el-select
      v-else
      :model-value="modelValue"
      class="custom-select"
      :placeholder="t('gamesettingsmodal.info.gamePresetPlaceholder')"
      @update:model-value="(value: string) => emit('update:modelValue', value)"
    >
      <el-option
        v-for="item in presets"
        :key="item.id"
        :label="item.displayNameEn || item.label || item.id"
        :value="item.id"
      />
    </el-select>
    <div class="capability-row" v-if="selectedPreset">
      <span class="capability-tag" :class="selectedPreset.supportedDownload ? 'ok' : 'warn'">
        {{ selectedPreset.supportedDownload ? t('gamesettingsmodal.info.capabilityDownload') : t('gamesettingsmodal.info.capabilityNoDownload') }}
      </span>
      <span class="capability-tag" :class="selectedPreset.supportedProtection ? 'ok' : 'warn'">
        {{ selectedPreset.supportedProtection ? t('gamesettingsmodal.info.capabilityProtection') : t('gamesettingsmodal.info.capabilityNoProtection') }}
      </span>
      <span class="capability-tag" :class="selectedPreset.supported3dmigoto ? 'ok' : 'warn'">
        {{ selectedPreset.supported3dmigoto ? t('gamesettingsmodal.info.capability3dmigoto') : t('gamesettingsmodal.info.capabilityNo3dmigoto') }}
      </span>
    </div>
    <div v-if="selectedPreset && !selectedPreset.supportedDownload" class="info-sub text-warn">
      {{ t('gamesettingsmodal.info.capabilityLimitedHint') }}
    </div>
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

.capability-row {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 10px;
}

.capability-tag {
  border-radius: 999px;
  padding: 4px 10px;
  font-size: 12px;
  border: 1px solid transparent;
}

.capability-tag.ok {
  color: #67c23a;
  border-color: rgba(103, 194, 58, 0.5);
  background: rgba(103, 194, 58, 0.1);
}

.capability-tag.warn {
  color: #e6a23c;
  border-color: rgba(230, 162, 60, 0.5);
  background: rgba(230, 162, 60, 0.12);
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

.action-btn.highlight {
  background: rgba(0, 122, 204, 0.3);
  border: 1px solid rgba(0, 122, 204, 0.5);
  color: #61afef;
}

.action-btn.highlight:hover {
  background: rgba(0, 122, 204, 0.5);
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

.text-warn {
  color: #e6a23c;
}
</style>
