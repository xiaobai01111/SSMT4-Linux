import { computed, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { openFileDialog } from '../../api';
import { appSettings } from '../../store';

export const useSetupView = () => {
  const router = useRouter();
  const route = useRoute();
  const { t } = useI18n();
  const restartMode = String(route.query.restart || '').trim() === '1';
  const step = ref(restartMode ? 1 : appSettings.initialized ? 3 : 1);
  const selectedDir = ref(appSettings.dataDir || '');
  const confirmUnofficial = ref(restartMode ? false : appSettings.tosRiskAcknowledged);
  const confirmRisk = ref(restartMode ? false : appSettings.tosRiskAcknowledged);
  const canFinish = computed(() => confirmUnofficial.value && confirmRisk.value);

  const selectDir = async () => {
    const selected = await openFileDialog({
      directory: true,
      multiple: false,
      title: t('setup.selectDataDirTitle'),
    });
    if (selected && typeof selected === 'string') {
      selectedDir.value = selected;
    }
  };

  const useDefault = () => {
    selectedDir.value = '';
    step.value = 3;
  };

  const finish = () => {
    if (!canFinish.value) return;
    appSettings.dataDir = selectedDir.value;
    appSettings.tosRiskAcknowledged = true;
    appSettings.initialized = true;
    void router.replace('/');
  };

  return {
    t,
    appSettings,
    step,
    selectedDir,
    confirmUnofficial,
    confirmRisk,
    canFinish,
    selectDir,
    useDefault,
    finish,
  };
};
