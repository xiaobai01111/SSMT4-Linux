import { computed, ref, shallowRef, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { builtinDocCatalog, loadBuiltinDocContent, type BuiltinDocDefinition } from '../../documents/builtinDocs';

export const useDocumentsView = () => {
  const { t, te } = useI18n();
  const route = useRoute();
  const router = useRouter();
  const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);
  const wikiUrl = 'https://github.com/xiaobai01111/SSMT4-Linux/wiki';

  const fallbackDocContent = (title: string, file: string): string => [
    `# ${title}`,
    '',
    t('documents.fallback.localMissing'),
    '',
    `- ${t('documents.fallback.file')}: \`${file}\``,
    `- ${t('documents.fallback.onlineWiki')}: ${wikiUrl}`,
    '',
    t('documents.fallback.openWikiHint'),
  ].join('\n');

  const docs = builtinDocCatalog;

  const docById = new Map(docs.map((doc) => [doc.id, doc]));
  const docByFile = new Map(docs.map((doc) => [doc.file.toLowerCase(), doc.id]));
  const defaultDocId = docs[0]?.id ?? 'home';
  const docContentCache = new Map<string, string>();
  const docHtmlCache = new Map<string, string>();

  const resolveDocId = (value: unknown): string => {
    const raw = Array.isArray(value) ? value[0] : value;
    if (typeof raw !== 'string') return defaultDocId;
    const normalized = raw.trim();
    return docById.has(normalized) ? normalized : defaultDocId;
  };

  const activeDocId = ref(resolveDocId(route.query.doc));
  const activeDoc = computed(() => docById.get(activeDocId.value) ?? docs[0]);
  const renderedHtml = shallowRef('');
  const isDocLoading = ref(false);
  let activeLoadToken = 0;

  const normalizeDocHref = (href: string): string => {
    let value = href.trim();
    if (value.startsWith('./')) value = value.slice(2);
    if (value.startsWith('/')) value = value.slice(1);
    return value.toLowerCase();
  };

  const escapeHtml = (input: string): string =>
    input.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

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
      .map((block) => block.trim())
      .filter((block) => block.length > 0)
      .map((block) => {
        if (/^<(h1|h2|h3|ul|ol|pre)\b/.test(block)) return block;
        return `<p>${block.replace(/\n/g, '<br/>')}</p>`;
      });

    let html = blocks.join('\n');
    html = html.replace(/@@CODE_BLOCK_(\d+)@@/g, (_all, idx: string) => {
      const code = codeBlocks[Number(idx)] ?? '';
      return `<pre><code>${escapeHtml(code)}</code></pre>`;
    });
    return html;
  };

  const yieldToBrowser = async () => {
    await new Promise<void>((resolve) => {
      if (typeof window !== 'undefined' && typeof window.requestAnimationFrame === 'function') {
        window.requestAnimationFrame(() => resolve());
        return;
      }
      setTimeout(resolve, 0);
    });
  };

  const loadDocContent = async (doc: BuiltinDocDefinition): Promise<string> => {
    const cached = docContentCache.get(doc.file);
    if (cached != null) return cached;

    let builtin: string | null = null;
    try {
      builtin = await loadBuiltinDocContent(doc.file);
    } catch {
      builtin = null;
    }

    const content = builtin ?? fallbackDocContent(doc.fallbackTitle, doc.file);
    docContentCache.set(doc.file, content);
    return content;
  };

  const loadDocHtml = async (doc: BuiltinDocDefinition): Promise<string> => {
    const cached = docHtmlCache.get(doc.file);
    if (cached != null) return cached;

    const content = await loadDocContent(doc);
    await yieldToBrowser();
    const html = markdownToHtml(content);
    docHtmlCache.set(doc.file, html);
    return html;
  };

  const refreshRenderedDoc = async (docId: string) => {
    const doc = docById.get(docId) ?? docs[0];
    if (!doc) {
      renderedHtml.value = '';
      isDocLoading.value = false;
      return;
    }

    const cachedHtml = docHtmlCache.get(doc.file);
    if (cachedHtml != null) {
      renderedHtml.value = cachedHtml;
      isDocLoading.value = false;
      return;
    }

    const loadToken = ++activeLoadToken;
    isDocLoading.value = true;
    renderedHtml.value = '';

    try {
      const html = await loadDocHtml(doc);
      if (loadToken !== activeLoadToken) return;
      renderedHtml.value = html;
    } finally {
      if (loadToken === activeLoadToken) {
        isDocLoading.value = false;
      }
    }
  };

  const openWiki = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      await open(wikiUrl);
    } catch {
      if (typeof window !== 'undefined') {
        window.open(wikiUrl, '_blank', 'noopener,noreferrer');
      }
    }
  };

  const handleDocClick = (event: MouseEvent) => {
    const target = event.target as HTMLElement | null;
    if (!target) return;
    const link = target.closest('a[data-doc-link]') as HTMLAnchorElement | null;
    if (!link) return;

    const href = link.getAttribute('data-doc-link');
    if (!href) return;

    const normalized = normalizeDocHref(href.split('#')[0] ?? href);
    const nextId = docByFile.get(normalized);
    if (!nextId) return;

    event.preventDefault();
    activeDocId.value = nextId;
  };

  watch(
    () => route.query.doc,
    (value) => {
      const nextId = resolveDocId(value);
      if (nextId !== activeDocId.value) {
        activeDocId.value = nextId;
      }
    },
    { immediate: true },
  );

  watch(activeDocId, async (value) => {
    const current = resolveDocId(route.query.doc);
    if (current === value) return;
    await router.replace({
      name: 'Documents',
      query: {
        ...route.query,
        doc: value,
      },
    });
  });

  watch(
    activeDocId,
    (value) => {
      void refreshRenderedDoc(value);
    },
    { immediate: true },
  );

  return {
    t,
    tr,
    docs,
    activeDocId,
    activeDoc,
    renderedHtml,
    isDocLoading,
    wikiUrl,
    openWiki,
    handleDocClick,
  };
};
