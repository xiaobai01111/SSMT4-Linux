import { nextTick, onMounted, onUnmounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { getLogDir, readLogFile } from '../../api';

export const useLogViewer = () => {
  const { t } = useI18n();
  const logContent = ref('');
  const logDir = ref('');
  const autoScroll = ref(true);
  const isLoading = ref(false);
  const logContainer = ref<HTMLElement | null>(null);
  let refreshTimer: ReturnType<typeof setInterval> | null = null;

  const scrollToBottom = () => {
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight;
    }
  };

  const loadLogs = async () => {
    if (isLoading.value) return;
    try {
      isLoading.value = true;
      const nextContent = await readLogFile(1200);
      const changed =
        nextContent.length !== logContent.value.length ||
        nextContent.slice(-120) !== logContent.value.slice(-120);
      if (!changed) return;
      logContent.value = nextContent;
      if (autoScroll.value) {
        await nextTick();
        scrollToBottom();
      }
    } catch (error) {
      logContent.value = `${t('logviewer.readFailed')}: ${error}`;
    } finally {
      isLoading.value = false;
    }
  };

  const copyLogs = async () => {
    try {
      await navigator.clipboard.writeText(logContent.value);
    } catch {
      const ta = document.createElement('textarea');
      ta.value = logContent.value;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
  };

  onMounted(async () => {
    logDir.value = await getLogDir();
    await loadLogs();
    refreshTimer = setInterval(loadLogs, 2500);
  });

  onUnmounted(() => {
    if (refreshTimer) clearInterval(refreshTimer);
  });

  return {
    t,
    logContent,
    logDir,
    autoScroll,
    isLoading,
    logContainer,
    loadLogs,
    copyLogs,
  };
};
