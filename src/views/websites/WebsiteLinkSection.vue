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
  <div class="link-section">
    <div class="section-header">
      <h2 class="section-title">{{ t(titleKey, fallbackTitle) }}</h2>
    </div>
    
    <div class="link-grid" v-if="items.length > 0">
      <div v-for="item in items" :key="item.key" class="glass-card">
        
        <div class="card-top">
          <div class="card-title">{{ t(`websites.items.${item.key}.name`) }}</div>
          <el-tag size="small" :type="category === 'project' ? 'primary' : 'info'" effect="light" class="glass-tag">
            {{ t(tagKey, fallbackTag) }}
          </el-tag>
        </div>
        
        <div class="card-desc">{{ t(`websites.items.${item.key}.desc`) }}</div>

        <div class="card-links">
          <template v-if="category === 'project'">
            <a :href="item.url" target="_blank" rel="noopener noreferrer" class="link-url-box">
              <i class="el-icon-link mr-1"></i> {{ item.url }}
            </a>
          </template>

          <template v-else>
            <div class="server-row">
              <span class="server-label">{{ t('websites.server_cn') }}</span>
              <a
                v-if="item.cnUrl"
                :href="item.cnUrl"
                target="_blank"
                rel="noopener noreferrer"
                class="link-url-box"
              >
                <i class="el-icon-link mr-1"></i> {{ item.cnUrl }}
              </a>
              <span v-else class="server-empty">{{ t('websites.server_unavailable') }}</span>
            </div>
            
            <div class="server-row mt-3">
              <span class="server-label">{{ t('websites.server_global') }}</span>
              <a
                v-if="item.globalUrl"
                :href="item.globalUrl"
                target="_blank"
                rel="noopener noreferrer"
                class="link-url-box"
              >
                <i class="el-icon-link mr-1"></i> {{ item.globalUrl }}
              </a>
              <span v-else class="server-empty">{{ t('websites.server_unavailable') }}</span>
            </div>
          </template>
        </div>
        
      </div>
    </div>
  </div>
</template>

<style scoped>
.link-section {
  margin-top: 32px;
  margin-bottom: 24px;
}

.section-header {
  margin-bottom: 20px;
}

.section-title {
  color: var(--el-text-color-primary);
  font-size: 18px;
  font-weight: 600;
  letter-spacing: 0.5px;
  margin: 0;
}

.link-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 20px;
}

/* =========== 核心：毛玻璃材质卡片 =========== */
.glass-card {
  background-color: rgba(20, 25, 30, 0.45); /* 半透明深色底 */
  backdrop-filter: blur(16px) saturate(120%);
  -webkit-backdrop-filter: blur(16px) saturate(120%);
  border: 1px solid rgba(255, 255, 255, 0.08); /* 极细反光边框 */
  border-radius: 12px;
  padding: 20px;
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  display: flex;
  flex-direction: column;
}

.glass-card:hover {
  background-color: rgba(30, 35, 45, 0.6);
  border-color: var(--el-color-primary-light-5);
  transform: translateY(-4px);
  box-shadow: 0 12px 24px rgba(0, 0, 0, 0.2);
}

.card-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.card-title {
  color: #fff;
  font-size: 16px;
  font-weight: 600;
  line-height: 1.4;
}

.glass-tag {
  background-color: rgba(255, 255, 255, 0.1) !important;
  border: 1px solid rgba(255, 255, 255, 0.2) !important;
  color: #fff !important;
  flex-shrink: 0;
}

.card-desc {
  margin-bottom: 20px;
  color: rgba(255, 255, 255, 0.7);
  font-size: 13px;
  line-height: 1.6;
  flex-grow: 1; /* 让描述区域自动撑开，保证底部链接对齐 */
}

/* 链接区域排版 */
.card-links {
  margin-top: auto;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  padding-top: 16px;
}

.link-url-box {
  display: inline-block;
  width: 100%;
  box-sizing: border-box;
  background-color: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.05);
  color: var(--el-color-primary-light-3);
  font-family: monospace;
  font-size: 12px;
  padding: 8px 12px;
  border-radius: 6px;
  word-break: break-all;
  text-decoration: none;
  transition: all 0.2s ease;
}

.link-url-box:hover {
  background-color: rgba(var(--el-color-primary-rgb), 0.15);
  border-color: var(--el-color-primary-light-5);
  color: var(--el-color-primary-light-7);
}

.server-row {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.mt-3 {
  margin-top: 16px;
}
.mr-1 {
  margin-right: 4px;
}

.server-label {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.server-empty {
  color: rgba(255, 255, 255, 0.3);
  font-size: 13px;
  font-style: italic;
  padding: 4px 0;
}
</style>