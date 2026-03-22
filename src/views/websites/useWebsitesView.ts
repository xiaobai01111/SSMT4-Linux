import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { LinkItem } from './types';

const baseLinks: LinkItem[] = [
  { key: 'github', category: 'project', url: 'https://github.com/xiaobai01111/SSMT4-Linux' },
  { key: 'gitee', category: 'project', url: 'https://gitee.com/xiaobai01111/ssmt4-linux' },
  { key: 'dataMirror', category: 'project', url: 'https://github.com/xiaobai01111/data-linux' },
  { key: 'releases', category: 'project', url: 'https://github.com/xiaobai01111/SSMT4-Linux/releases' },
  { key: 'issues', category: 'project', url: 'https://github.com/xiaobai01111/SSMT4-Linux/issues' },
  { key: 'docs', category: 'project', url: 'https://github.com/xiaobai01111/SSMT4-Linux/wiki' },
  { key: 'hsr', category: 'games', cnUrl: 'https://sr.mihoyo.com/', globalUrl: 'https://hsr.hoyoverse.com/' },
  { key: 'zzz', category: 'games', cnUrl: 'https://zzz.mihoyo.com/', globalUrl: 'https://zenless.hoyoverse.com/' },
  { key: 'ww', category: 'games', cnUrl: 'https://mc.kurogames.com/', globalUrl: 'https://wutheringwaves.kurogames.com/' },
  { key: 'snowbreak', category: 'games', cnUrl: 'https://snowbreak.amazingseasun.com/', globalUrl: 'https://snowbreak.amazingseasun.com/' },
];

export function useWebsitesView() {
  const { t } = useI18n();
  const keyword = ref('');

  const links = computed(() => baseLinks);
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

  return {
    gameLinks,
    keyword,
    projectLinks,
  };
}
