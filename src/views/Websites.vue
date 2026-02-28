<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';

type LinkItem = {
  key: string;
  category: 'project' | 'games';
  url?: string;
  cnUrl?: string;
  globalUrl?: string;
};

const { t } = useI18n();
const keyword = ref('');

const links = computed<LinkItem[]>(() => [
  { key: 'github', category: 'project', url: 'https://github.com/xiaobai01111/SSMT4-Linux' },
  { key: 'releases', category: 'project', url: 'https://github.com/xiaobai01111/SSMT4-Linux/releases' },
  { key: 'issues', category: 'project', url: 'https://github.com/xiaobai01111/SSMT4-Linux/issues' },
  { key: 'docs', category: 'project', url: 'https://github.com/xiaobai01111/SSMT4-Linux/wiki' },

  { key: 'hsr', category: 'games', cnUrl: 'https://sr.mihoyo.com/', globalUrl: 'https://hsr.hoyoverse.com/' },
  { key: 'zzz', category: 'games', cnUrl: 'https://zzz.mihoyo.com/', globalUrl: 'https://zenless.hoyoverse.com/' },
  { key: 'ww', category: 'games', cnUrl: 'https://mc.kurogames.com/', globalUrl: 'https://wutheringwaves.kurogames.com/' },
  { key: 'snowbreak', category: 'games', cnUrl: 'https://snowbreak.amazingseasun.com/', globalUrl: 'https://snowbreak.amazingseasun.com/' },
]);

const normalizedKeyword = computed(() => keyword.value.trim().toLowerCase());

const matches = (item: LinkItem) => {
  const q = normalizedKeyword.value;
  if (!q) return true;

  const name = t(`websites.items.${item.key}.name`).toLowerCase();
  const desc = t(`websites.items.${item.key}.desc`).toLowerCase();
  const projectUrl = (item.url || '').toLowerCase();
  const cnUrl = (item.cnUrl || '').toLowerCase();
  const globalUrl = (item.globalUrl || '').toLowerCase();
  return (
    name.includes(q)
    || desc.includes(q)
    || projectUrl.includes(q)
    || cnUrl.includes(q)
    || globalUrl.includes(q)
  );
};

const projectLinks = computed(() =>
  links.value.filter((item) => item.category === 'project' && matches(item)),
);

const gameLinks = computed(() =>
  links.value.filter((item) => item.category === 'games' && matches(item)),
);
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

    <div class="section">
      <div class="section-title">{{ t('websites.project_links') }}</div>
      <div class="link-grid" v-if="projectLinks.length > 0">
        <el-card v-for="item in projectLinks" :key="item.key" class="link-card" shadow="hover">
          <div class="card-top">
            <div class="card-title">{{ t(`websites.items.${item.key}.name`) }}</div>
            <el-tag size="small" type="success">{{ t('websites.tag_project') }}</el-tag>
          </div>
          <div class="card-desc">{{ t(`websites.items.${item.key}.desc`) }}</div>
          <a :href="item.url" target="_blank" rel="noopener noreferrer" class="link-url">{{ item.url }}</a>
        </el-card>
      </div>
    </div>

    <div class="section">
      <div class="section-title">{{ t('websites.game_links') }}</div>
      <div class="link-grid" v-if="gameLinks.length > 0">
        <el-card v-for="item in gameLinks" :key="item.key" class="link-card" shadow="hover">
          <div class="card-top">
            <div class="card-title">{{ t(`websites.items.${item.key}.name`) }}</div>
            <el-tag size="small">{{ t('websites.tag_game') }}</el-tag>
          </div>
          <div class="card-desc">{{ t(`websites.items.${item.key}.desc`) }}</div>
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
        </el-card>
      </div>
    </div>

    <div v-if="projectLinks.length === 0 && gameLinks.length === 0" class="empty-text">
      {{ t('websites.empty') }}
    </div>
  </div>
</template>

<style scoped>
.websites-page {
  padding: 32px 40px 60px 40px;
  animation: fadeIn 0.15s ease-out;

  /* Tech Glass Wrapper for Readability */
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

/* Deep customize search input for Tech HUD */
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
  border-radius: 2px !important; /* Sharp tag */
  text-transform: uppercase;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.card-desc {
  margin-bottom: 12px;
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
  line-height: 1.5;
}

.link-url {
  color: #00f0ff;
  text-decoration: none;
  font-size: 13px;
  word-break: break-all;
  transition: all 0.2s;
  font-weight: 500;
}

.link-url:hover {
  text-decoration: underline;
}

.server-row {
  display: grid;
  grid-template-columns: 80px 1fr;
  align-items: start;
  gap: 8px;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid rgba(255, 255, 255, 0.05); /* subtle divider */
}

.server-label {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.server-empty {
  color: rgba(255, 255, 255, 0.3);
  font-size: 12px;
}

.empty-text {
  margin-top: 40px;
  color: #00f0ff;
  font-size: 14px;
  text-align: center;
  text-transform: uppercase;
  letter-spacing: 1px;
  opacity: 0.7;
}
</style>
