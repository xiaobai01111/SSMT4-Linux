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
  box-sizing: border-box;
  animation: fadeIn 0.15s ease-out;

  /* Tech Glass Wrapper */
  background: rgba(10, 15, 20, 0.92);
  will-change: transform;
  contain: layout style;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.doc-sidebar {
  border-right: 1px solid rgba(0, 240, 255, 0.3); /* Tech Cyan Line */
  background: rgba(0, 5, 10, 0.4);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
}

.doc-sidebar-title {
  color: #00f0ff; /* Glowing cyan */
  font-size: 16px;
  font-weight: 700;
  padding: 4px 6px 12px 6px;
  text-transform: uppercase;
  letter-spacing: 1px;
}

.doc-nav-btn {
  text-align: left;
  border: 1px solid transparent;
  background: transparent;
  color: rgba(255, 255, 255, 0.65);
  border-radius: 4px; /* Sharp */
  padding: 10px 12px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s ease;
  position: relative;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.doc-nav-btn:hover {
  background: rgba(0, 240, 255, 0.1);
  color: #fff;
}

.doc-nav-btn.active {
  background: rgba(0, 240, 255, 0.15);
  color: #00f0ff; /* Glowing cyan text */
  font-weight: 600;
  border-left: 4px solid #00f0ff;
}

/* Hard tech button override */
.doc-open-btn {
  margin-top: auto;
  background-color: rgba(0, 240, 255, 0.05);
  border: 1px solid rgba(0, 240, 255, 0.5);
  color: #00f0ff;
  font-size: 12px;
  border-radius: 4px;
  padding: 10px;
  cursor: pointer;
  text-transform: uppercase;
  font-weight: 600;
  letter-spacing: 0.5px;
  transition: all 0.2s ease;
}

.doc-open-btn:hover {
  background: #00f0ff;
  color: #000;
  border-color: #00f0ff;
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
  padding: 16px 24px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3); /* Tech Cyan Line */
}

.doc-content-title {
  color: #fff;
  font-size: 20px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  display: flex;
  align-items: center;
}
.doc-content-title::before {
  content: '';
  display: inline-block;
  width: 4px;
  height: 20px;
  background-color: #00f0ff;
  margin-right: 10px;
}

.doc-link {
  color: #00f0ff;
  font-size: 13px;
  text-decoration: none;
  font-family: monospace;
  max-width: 50%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: all 0.2s;
}

.doc-link:hover {
  text-decoration: underline;
}

.doc-content {
  flex: 1;
  overflow-y: auto;
  padding: 24px 32px 40px 32px;
  color: rgba(255, 255, 255, 0.85); /* Slightly darker for contrast */
  line-height: 1.8;
  font-size: 14px;
}

/* Markdown overrides for Tech Terminal look */
:deep(.markdown-body h1),
:deep(.markdown-body h2),
:deep(.markdown-body h3) {
  margin-top: 0;
  color: #00f0ff;
  font-weight: 600;
  letter-spacing: 0.5px;
}

:deep(.markdown-body h1) { font-size: 26px; border-bottom: 1px solid rgba(0, 240, 255, 0.2); padding-bottom: 8px; margin-bottom: 24px; }
:deep(.markdown-body h2) { font-size: 20px; margin-top: 32px; margin-bottom: 16px; }
:deep(.markdown-body h3) { font-size: 16px; margin-top: 24px; margin-bottom: 12px; color: #fff; }

:deep(.markdown-body p) {
  margin: 0 0 16px 0;
}

:deep(.markdown-body ul),
:deep(.markdown-body ol) {
  margin: 0 0 16px 20px;
  padding: 0;
}
:deep(.markdown-body li) { margin-bottom: 6px; }

:deep(.markdown-body code) {
  background: rgba(0, 240, 255, 0.1);
  color: #00f0ff;
  padding: 2px 6px;
  border-radius: 4px; /* Sharp */
  font-size: 13px;
  font-family: monospace;
  border: 1px solid rgba(0, 240, 255, 0.2);
}

:deep(.markdown-body pre) {
  background: rgba(0, 5, 10, 0.8);
  border: 1px solid rgba(0, 240, 255, 0.3);
  border-left: 4px solid #00f0ff; /* Tech accent */
  padding: 16px;
  border-radius: 4px; /* Sharp */
  overflow-x: auto;
  margin-bottom: 24px;
}

:deep(.markdown-body pre code) {
  background: transparent;
  padding: 0;
  border: none;
  color: rgba(255, 255, 255, 0.9);
  font-size: 13px;
}

:deep(.markdown-body a) {
  color: #00f0ff;
  text-decoration: none;
  transition: all 0.2s;
  border-bottom: 1px solid transparent;
}

:deep(.markdown-body a:hover) {
  text-decoration: none;
  border-bottom-color: #00f0ff;
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
