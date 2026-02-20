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

  { key: 'genshin', category: 'games', cnUrl: 'https://ys.mihoyo.com/', globalUrl: 'https://genshin.hoyoverse.com/' },
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
  padding: 24px 24px 56px 24px;
}

.websites-header {
  margin-bottom: 20px;
}

.title {
  margin: 0 0 8px 0;
  font-size: 24px;
  font-weight: 700;
  color: #f2f2f2;
}

.desc {
  margin: 0 0 12px 0;
  color: rgba(255, 255, 255, 0.62);
  font-size: 13px;
}

.search-input {
  max-width: 460px;
}

.section {
  margin-top: 14px;
}

.section-title {
  margin-bottom: 10px;
  color: #e8e8e8;
  font-size: 15px;
  font-weight: 600;
}

.link-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 12px;
}

.link-card {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(255, 255, 255, 0.03);
}

.card-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.card-title {
  color: #fff;
  font-size: 14px;
  font-weight: 600;
}

.card-desc {
  margin-bottom: 8px;
  color: rgba(255, 255, 255, 0.65);
  font-size: 12px;
}

.link-url {
  color: #79bbff;
  text-decoration: none;
  font-size: 12px;
  word-break: break-all;
}

.link-url:hover {
  text-decoration: underline;
}

.server-row {
  display: grid;
  grid-template-columns: 68px 1fr;
  align-items: start;
  gap: 8px;
  margin-top: 6px;
}

.server-label {
  color: rgba(255, 255, 255, 0.72);
  font-size: 12px;
}

.server-empty {
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
}

.empty-text {
  margin-top: 20px;
  color: rgba(255, 255, 255, 0.5);
  font-size: 13px;
}
</style>
