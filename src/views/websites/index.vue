<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import WebsiteLinkSection from './WebsiteLinkSection.vue';
import { useWebsitesView } from './useWebsitesView';

const { t } = useI18n();
const { gameLinks, keyword, projectLinks } = useWebsitesView();
</script>

<template>
  <div class="websites-page">
    <div class="websites-header">
      <h1 class="title">{{ t('websites.title') }}</h1>
      <p class="desc">{{ t('websites.desc') }}</p>
      <el-input
        v-model="keyword"
        class="search-input"
        :placeholder="t('websites.search_placeholder')"
        clearable
      />
    </div>

    <WebsiteLinkSection
      title-key="websites.project_links"
      fallback-title="项目链接"
      tag-key="websites.tag_project"
      fallback-tag="项目"
      :items="projectLinks"
      category="project"
    />

    <WebsiteLinkSection
      title-key="websites.game_links"
      fallback-title="游戏官网"
      tag-key="websites.tag_game"
      fallback-tag="游戏"
      :items="gameLinks"
      category="games"
    />

    <div v-if="projectLinks.length === 0 && gameLinks.length === 0" class="empty-text">
      {{ t('websites.empty') }}
    </div>
  </div>
</template>

<style scoped>
.websites-page {
  padding: 32px 40px 60px 40px;
  animation: fadeIn 0.15s ease-out;
  background: rgba(10, 15, 20, 0.92);
  will-change: transform;
  contain: layout style;
  width: 100%;
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.websites-header {
  margin-bottom: 32px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
}

.title {
  margin: 0 0 8px 0;
  font-size: 28px;
  font-weight: 700;
  color: #00f0ff;
  letter-spacing: 1px;
  text-transform: uppercase;
}

.desc {
  margin: 0 0 20px 0;
  color: rgba(255, 255, 255, 0.65);
  font-size: 14px;
}

.search-input {
  max-width: 460px;
}

:deep(.search-input .el-input__wrapper) {
  background-color: rgba(10, 15, 20, 0.6) !important;
  border: 1px solid rgba(0, 240, 255, 0.3) !important;
  box-shadow: none !important;
  border-radius: 4px;
}

:deep(.search-input .el-input__wrapper.is-focus) {
  border-color: #00f0ff !important;
  box-shadow: none !important;
  background-color: rgba(0, 240, 255, 0.05) !important;
}

:deep(.search-input .el-input__inner) {
  color: #00f0ff !important;
  font-family: monospace;
  font-size: 14px;
}

:deep(.search-input .el-input__inner::placeholder) {
  color: rgba(0, 240, 255, 0.4);
}

.empty-text {
  margin-top: 40px;
  text-align: center;
  color: rgba(255, 255, 255, 0.45);
}
</style>
