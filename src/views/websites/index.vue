<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import WebsiteLinkSection from './WebsiteLinkSection.vue';
import { useWebsitesView } from './useWebsitesView';

const { t } = useI18n();
const { gameLinks, keyword, projectLinks } = useWebsitesView();
</script>

<template>
  <div class="websites-page glass-panel w-full">
    <div class="page-header">
      <div class="header-content">
        <h1 class="page-title">{{ t('websites.title') }}</h1>
        <p class="page-desc">{{ t('websites.desc') }}</p>
      </div>
      <div class="header-actions">
        <el-input
          v-model="keyword"
          class="search-input"
          :placeholder="t('websites.search_placeholder')"
          clearable
        >
          <template #prefix>
            <i class="el-icon-search"></i>
          </template>
        </el-input>
      </div>
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

    <div v-if="projectLinks.length === 0 && gameLinks.length === 0" class="empty-hint">
      <i class="el-icon-document-remove" style="font-size: 24px; margin-bottom: 8px; opacity: 0.8;"></i>
      <div>{{ t('websites.empty') }}</div>
    </div>
  </div>
</template>

<style scoped>
/* =========== 核心修复：毛玻璃玻璃态容器 =========== */
.glass-panel {
  background-color: rgba(20, 25, 30, 0.75) !important; /* 加上半透明深色底色，压盖住刺眼壁纸 */
  backdrop-filter: blur(20px) saturate(120%); /* 加入高级毛玻璃模糊效果 */
  -webkit-backdrop-filter: blur(20px) saturate(120%);
}

/* 页面主容器 */
.websites-page {
  padding: 32px 40px 60px 40px;
  animation: smoothFadeIn 0.25s ease-out;
  width: 100%;
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
  color: var(--el-text-color-primary);
}
.w-full {
  width: 100%;
}

/* 丝滑的进入动画 */
@keyframes smoothFadeIn {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 头部布局：弹性排版，右侧放置搜索框 */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 20px;
  margin-bottom: 32px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  flex-wrap: wrap;
}

.header-content {
  flex: 1;
  min-width: 280px;
}

.page-title {
  margin: 0;
  font-size: 26px;
  font-weight: 600;
  color: #fff;
}

.page-desc {
  margin: 8px 0 0 0;
  color: rgba(255, 255, 255, 0.65);
  font-size: 14px;
  line-height: 1.5;
}

/* 搜索框 */
.header-actions {
  display: flex;
  align-items: center;
}

.search-input {
  width: 320px;
  flex-shrink: 0;
}

/* 适配毛玻璃的搜索框 */
:deep(.el-input__wrapper) {
  background-color: rgba(0, 0, 0, 0.2) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}
:deep(.el-input__inner) {
  color: #fff !important;
}

/* 统一的空状态提示 */
.empty-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  margin-top: 40px;
  padding: 60px 20px;
  color: rgba(255, 255, 255, 0.65);
  font-size: 14px;
  border: 2px dashed rgba(255, 255, 255, 0.15);
  border-radius: 8px;
  background-color: rgba(0, 0, 0, 0.15);
  transition: all 0.3s;
}

.empty-hint:hover {
  border-color: rgba(255, 255, 255, 0.3);
}

/* 响应式调整 */
@media (max-width: 768px) {
  .websites-page {
    padding: 24px 20px 40px 20px;
  }
  .page-header {
    flex-direction: column;
    align-items: stretch;
  }
  .search-input {
    width: 100%;
  }
}
</style>
