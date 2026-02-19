<script setup lang="ts">
import { computed, ref } from 'vue';

type DocItem = {
  id: string;
  title: string;
  file: string;
  content: string;
};

const wikiUrl = 'https://github.com/xiaobai01111/SSMT4-Linux/wiki';

const wikiModules: Record<string, string> = {};

const fallbackDocContent = (title: string, file: string): string => [
  `# ${title}`,
  '',
  '本地 Wiki 文档未包含在当前打包目录中。',
  '',
  `- 文档文件：\`${file}\``,
  `- 在线 Wiki：${wikiUrl}`,
  '',
  '可在联网环境中点击“打开 GitHub Wiki”查看最新内容。',
].join('\n');

const loadDocContent = (file: string, title: string): string => {
  const moduleEntry = Object.entries(wikiModules).find(([path]) => path.endsWith(`/${file}?raw`));
  return moduleEntry?.[1] ?? fallbackDocContent(title, file);
};

const docs: DocItem[] = [
  { id: 'home', title: 'Home', file: 'Home.md', content: loadDocContent('Home.md', 'Home') },
  { id: 'risk', title: '项目风险与要求', file: '01-项目风险与要求.md', content: loadDocContent('01-项目风险与要求.md', '项目风险与要求') },
  { id: 'download', title: '游戏下载与主程序配置', file: '02-游戏下载与主程序配置.md', content: loadDocContent('02-游戏下载与主程序配置.md', '游戏下载与主程序配置') },
  { id: 'proton', title: 'Proton 下载、管理与使用', file: '03-Proton-下载管理与使用.md', content: loadDocContent('03-Proton-下载管理与使用.md', 'Proton 下载、管理与使用') },
  { id: 'dxvk', title: 'DXVK 下载、管理与使用', file: '04-DXVK-下载管理与使用.md', content: loadDocContent('04-DXVK-下载管理与使用.md', 'DXVK 下载、管理与使用') },
  { id: 'protection', title: '防护与防封禁管理', file: '05-防护与防封禁管理.md', content: loadDocContent('05-防护与防封禁管理.md', '防护与防封禁管理') },
  { id: 'known', title: '已知问题与不足', file: '06-已知问题与不足.md', content: loadDocContent('06-已知问题与不足.md', '已知问题与不足') },
];

const activeDocId = ref('home');

const docById = new Map(docs.map((d) => [d.id, d]));
const docByFile = new Map(docs.map((d) => [d.file.toLowerCase(), d.id]));

const activeDoc = computed(() => docById.get(activeDocId.value) ?? docs[0]);

const normalizeDocHref = (href: string): string => {
  let value = href.trim();
  if (value.startsWith('./')) value = value.slice(2);
  if (value.startsWith('/')) value = value.slice(1);
  return value.toLowerCase();
};

const escapeHtml = (input: string): string =>
  input
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

const markdownToHtml = (markdown: string): string => {
  const normalized = markdown.replace(/\r\n/g, '\n');
  const codeBlocks: string[] = [];

  let text = normalized.replace(/```([\s\S]*?)```/g, (_all, code: string) => {
    const token = `@@CODE_BLOCK_${codeBlocks.length}@@`;
    codeBlocks.push(code.trimEnd());
    return token;
  });

  text = escapeHtml(text);

  text = text.replace(/^###\s+(.+)$/gm, '<h3>$1</h3>');
  text = text.replace(/^##\s+(.+)$/gm, '<h2>$1</h2>');
  text = text.replace(/^#\s+(.+)$/gm, '<h1>$1</h1>');

  // 先处理链接，内部 md 链接转 data 属性，方便切换内嵌文档
  text = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_all, label: string, href: string) => {
    const safeHref = href.trim();
    if (/\.md(?:#.*)?$/i.test(safeHref)) {
      return `<a href="${safeHref}" data-doc-link="${safeHref}">${label}</a>`;
    }
    return `<a href="${safeHref}" target="_blank" rel="noopener noreferrer">${label}</a>`;
  });

  text = text.replace(/`([^`\n]+)`/g, '<code>$1</code>');

  text = text.replace(/(^|\n)(- .*(?:\n- .*)*)/g, (_all, prefix: string, block: string) => {
    const items = block
      .trim()
      .split('\n')
      .map((line) => `<li>${line.replace(/^- /, '')}</li>`)
      .join('');
    return `${prefix}<ul>${items}</ul>`;
  });

  text = text.replace(/(^|\n)(\d+\. .*(?:\n\d+\. .*)*)/g, (_all, prefix: string, block: string) => {
    const items = block
      .trim()
      .split('\n')
      .map((line) => `<li>${line.replace(/^\d+\. /, '')}</li>`)
      .join('');
    return `${prefix}<ol>${items}</ol>`;
  });

  const blocks = text
    .split(/\n{2,}/)
    .map((b) => b.trim())
    .filter((b) => b.length > 0)
    .map((b) => {
      if (/^<(h1|h2|h3|ul|ol|pre)\b/.test(b)) return b;
      return `<p>${b.replace(/\n/g, '<br/>')}</p>`;
    });

  let html = blocks.join('\n');
  html = html.replace(/@@CODE_BLOCK_(\d+)@@/g, (_all, idx: string) => {
    const code = codeBlocks[Number(idx)] ?? '';
    return `<pre><code>${escapeHtml(code)}</code></pre>`;
  });

  return html;
};

const renderedHtml = computed(() => markdownToHtml(activeDoc.value.content));

const openWiki = async () => {
  const { open } = await import('@tauri-apps/plugin-shell');
  await open(wikiUrl);
};

const handleDocClick = (e: MouseEvent) => {
  const target = e.target as HTMLElement | null;
  if (!target) return;
  const link = target.closest('a[data-doc-link]') as HTMLAnchorElement | null;
  if (!link) return;

  const href = link.getAttribute('data-doc-link');
  if (!href) return;

  const normalized = normalizeDocHref(href.split('#')[0] ?? href);
  const nextId = docByFile.get(normalized);
  if (!nextId) return;

  e.preventDefault();
  activeDocId.value = nextId;
};
</script>

<template>
  <div class="documents-view">
    <aside class="doc-sidebar">
      <div class="doc-sidebar-title">Wiki</div>
      <button
        v-for="doc in docs"
        :key="doc.id"
        class="doc-nav-btn"
        :class="{ active: doc.id === activeDocId }"
        @click="activeDocId = doc.id"
      >
        {{ doc.title }}
      </button>
      <button class="doc-open-btn" @click="openWiki">打开 GitHub Wiki</button>
    </aside>

    <section class="doc-content-wrap">
      <div class="doc-content-header">
        <div class="doc-content-title">{{ activeDoc.title }}</div>
        <a :href="wikiUrl" target="_blank" rel="noopener noreferrer" class="doc-link">{{ wikiUrl }}</a>
      </div>
      <article class="doc-content markdown-body" v-html="renderedHtml" @click="handleDocClick"></article>
    </section>
  </div>
</template>

<style scoped>
.documents-view {
  width: 100%;
  height: 100%;
  display: grid;
  grid-template-columns: 240px 1fr;
  overflow: hidden;
}

.doc-sidebar {
  border-right: 1px solid rgba(255, 255, 255, 0.08);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
}

.doc-sidebar-title {
  color: #f2f2f2;
  font-size: 16px;
  font-weight: 700;
  padding: 4px 6px 8px 6px;
}

.doc-nav-btn {
  text-align: left;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.03);
  color: rgba(255, 255, 255, 0.82);
  border-radius: 8px;
  padding: 9px 10px;
  font-size: 12px;
  cursor: pointer;
}

.doc-nav-btn:hover {
  background: rgba(255, 255, 255, 0.08);
}

.doc-nav-btn.active {
  background: rgba(64, 158, 255, 0.2);
  border-color: rgba(64, 158, 255, 0.45);
  color: #a8d4ff;
}

.doc-open-btn {
  margin-top: auto;
  border: 1px solid rgba(255, 255, 255, 0.18);
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
  font-size: 12px;
  border-radius: 8px;
  padding: 9px 10px;
  cursor: pointer;
}

.doc-open-btn:hover {
  background: rgba(255, 255, 255, 0.14);
}

.doc-content-wrap {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.doc-content-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.doc-content-title {
  color: #f2f2f2;
  font-size: 18px;
  font-weight: 700;
}

.doc-link {
  color: #79bbff;
  font-size: 12px;
  text-decoration: none;
  max-width: 50%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.doc-link:hover {
  text-decoration: underline;
}

.doc-content {
  flex: 1;
  overflow-y: auto;
  padding: 18px 22px 28px 22px;
  color: rgba(255, 255, 255, 0.88);
  line-height: 1.7;
}

:deep(.markdown-body h1),
:deep(.markdown-body h2),
:deep(.markdown-body h3) {
  margin-top: 0;
  color: #f5f5f5;
}

:deep(.markdown-body p) {
  margin: 0 0 10px 0;
}

:deep(.markdown-body ul),
:deep(.markdown-body ol) {
  margin: 0 0 10px 18px;
  padding: 0;
}

:deep(.markdown-body code) {
  background: rgba(255, 255, 255, 0.1);
  padding: 1px 5px;
  border-radius: 4px;
  font-size: 12px;
}

:deep(.markdown-body pre) {
  background: rgba(0, 0, 0, 0.34);
  border: 1px solid rgba(255, 255, 255, 0.08);
  padding: 10px;
  border-radius: 8px;
  overflow-x: auto;
}

:deep(.markdown-body pre code) {
  background: transparent;
  padding: 0;
}

:deep(.markdown-body a) {
  color: #79bbff;
  text-decoration: none;
}

:deep(.markdown-body a:hover) {
  text-decoration: underline;
}

@media (max-width: 980px) {
  .documents-view {
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr;
  }

  .doc-sidebar {
    border-right: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    max-height: 220px;
  }

  .doc-link {
    max-width: 65%;
  }
}
</style>
