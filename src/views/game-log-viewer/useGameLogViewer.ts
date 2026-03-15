import { nextTick, onMounted, onUnmounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { readGameLogSnapshot, type GameLogSnapshot } from '../../api';

export const useGameLogViewer = () => {
  const { t } = useI18n();
  const snapshot = ref<GameLogSnapshot>({
    active: false,
    gameName: '',
    startedAt: '',
    lineCount: 0,
    content: '',
  });
  const autoScroll = ref(true);
  const isLoading = ref(false);
  const logContainer = ref<HTMLElement | null>(null);
  let refreshTimer: ReturnType<typeof setInterval> | null = null;

  const scrollToBottom = () => {
    if (!logContainer.value) return;
    logContainer.value.scrollTop = logContainer.value.scrollHeight;
  };

  const loadLogs = async () => {
    if (isLoading.value) return;
    try {
      isLoading.value = true;
      const next = await readGameLogSnapshot(1800);
      const prev = snapshot.value;
      const changed =
        next.active !== prev.active ||
        next.gameName !== prev.gameName ||
        next.startedAt !== prev.startedAt ||
        next.lineCount !== prev.lineCount ||
        next.content.length !== prev.content.length ||
        next.content.slice(-120) !== prev.content.slice(-120);
      if (!changed) return;

      snapshot.value = next;
      if (autoScroll.value && next.content) {
        await nextTick();
        scrollToBottom();
      }
    } catch (error: unknown) {
      const text = error instanceof Error ? error.message : String(error);
      snapshot.value = {
        active: false,
        gameName: '',
        startedAt: '',
        lineCount: 0,
        content: `${t('logviewer.readFailed')}: ${text}`,
      };
    } finally {
      isLoading.value = false;
    }
  };

  const copyLogs = async () => {
    const text = snapshot.value.content || '';
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement('textarea');
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
  };

  onMounted(async () => {
    await loadLogs();
    refreshTimer = setInterval(loadLogs, 1200);
  });

  onUnmounted(() => {
    if (refreshTimer) clearInterval(refreshTimer);
  });

  return {
    t,
    snapshot,
    autoScroll,
    isLoading,
    logContainer,
    loadLogs,
    copyLogs,
  };
};
