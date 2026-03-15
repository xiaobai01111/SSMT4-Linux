<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { LinkCategory, LinkItem } from './types';

defineProps<{
  titleKey: string;
  fallbackTitle: string;
  tagKey: string;
  fallbackTag: string;
  items: LinkItem[];
  category: LinkCategory;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="section">
    <div class="section-title">{{ t(titleKey, fallbackTitle) }}</div>
    <div class="link-grid" v-if="items.length > 0">
      <el-card v-for="item in items" :key="item.key" class="link-card" shadow="hover">
        <div class="card-top">
          <div class="card-title">{{ t(`websites.items.${item.key}.name`) }}</div>
          <el-tag size="small" :type="category === 'project' ? 'success' : undefined">
            {{ t(tagKey, fallbackTag) }}
          </el-tag>
        </div>
        <div class="card-desc">{{ t(`websites.items.${item.key}.desc`) }}</div>

        <template v-if="category === 'project'">
          <a :href="item.url" target="_blank" rel="noopener noreferrer" class="link-url">{{ item.url }}</a>
        </template>

        <template v-else>
          <div class="server-row">
            <span class="server-label">{{ t('websites.server_cn') }}</span>
            <a
              v-if="item.cnUrl"
              :href="item.cnUrl"
              target="_blank"
              rel="noopener noreferrer"
              class="link-url"
            >
              {{ item.cnUrl }}
            </a>
            <span v-else class="server-empty">{{ t('websites.server_unavailable') }}</span>
          </div>
          <div class="server-row">
            <span class="server-label">{{ t('websites.server_global') }}</span>
            <a
              v-if="item.globalUrl"
              :href="item.globalUrl"
              target="_blank"
              rel="noopener noreferrer"
              class="link-url"
            >
              {{ item.globalUrl }}
            </a>
            <span v-else class="server-empty">{{ t('websites.server_unavailable') }}</span>
          </div>
        </template>
      </el-card>
    </div>
  </div>
</template>

<style scoped>
.section {
  margin-top: 30px;
}

.section-title {
  margin-bottom: 16px;
  color: #fff;
  font-size: 16px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  display: flex;
  align-items: center;
}

.section-title::before {
  content: '';
  display: inline-block;
  width: 4px;
  height: 16px;
  background-color: #00f0ff;
  margin-right: 8px;
}

.link-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 16px;
}

:deep(.link-card) {
  border: 1px solid rgba(0, 240, 255, 0.2) !important;
  background: rgba(10, 15, 20, 0.5) !important;
  border-radius: 4px !important;
  transition: all 0.2s ease !important;
  cursor: default;
}

:deep(.link-card:hover) {
  border-color: #00f0ff !important;
  transform: translateY(-2px) scale(1.02);
  background: rgba(15, 20, 25, 0.8) !important;
}

.card-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 12px;
}

.card-title {
  color: #fff;
  font-size: 15px;
  font-weight: 600;
}

:deep(.el-tag) {
  background-color: rgba(0, 240, 255, 0.1) !important;
  border-color: rgba(0, 240, 255, 0.3) !important;
  color: #00f0ff !important;
  border-radius: 2px !important;
}

.card-desc {
  margin-bottom: 12px;
  color: rgba(255, 255, 255, 0.68);
  line-height: 1.7;
  min-height: 48px;
}

.link-url {
  color: #8fefff;
  font-family: monospace;
  font-size: 12px;
  word-break: break-all;
  text-decoration: none;
}

.link-url:hover {
  color: #fff;
  text-decoration: underline;
}

.server-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 12px;
}

.server-label {
  color: rgba(255, 255, 255, 0.5);
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.8px;
}

.server-empty {
  color: rgba(255, 255, 255, 0.3);
  font-size: 12px;
}
</style>
