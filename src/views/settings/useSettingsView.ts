import { computed, inject } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import {
  appSettings,
  startFeatureOnboarding,
  updateLocaleAndReload,
} from '../../store';
import {
  getResourceVersionInfo,
  getVersionCheckInfo,
  openFileDialog,
  openLogWindow,
  pullResourceUpdates,
  showMessage,
  type GameInfo,
} from '../../api';
import { useSettingsResourceTasks } from '../../composables/useSettingsResourceTasks';
import { useSettingsMigotoConfig } from '../../composables/useSettingsMigotoConfig';
import { useSettingsPageController } from '../../composables/useSettingsPageController';
import { NOTIFY_KEY } from '../../types/notify';
import { useSettingsGraphicsManager } from './useSettingsGraphicsManager';
import { useSettingsProtonManager } from './useSettingsProtonManager';
import { useSettingsXxmiManager } from './useSettingsXxmiManager';

export function useSettingsView() {
  const { t, te } = useI18n();
  const route = useRoute();
  const router = useRouter();
  const notify = inject(NOTIFY_KEY, null);

  const tr = (
    key: string,
    fallback: string,
    params?: Record<string, unknown>,
  ) => {
    if (!te(key)) return fallback;
    return params ? t(key, params) : t(key);
  };

  const { runDownloadTask, runDeleteTask } = useSettingsResourceTasks(tr);

  const toast = async (
    kind: 'success' | 'warning' | 'info' | 'error',
    title: string,
    message: string,
  ) => {
    const handler = notify?.[kind];
    if (typeof handler === 'function') {
      handler(title, message);
      return;
    }
    await showMessage(message, { title, kind });
  };

  const protonManager = useSettingsProtonManager({
    tr,
    toast,
    runDownloadTask,
    runDeleteTask,
    getDataDir: () => String(appSettings.dataDir || ''),
  });

  const graphicsManager = useSettingsGraphicsManager({
    tr,
    toast,
    runDownloadTask,
    runDeleteTask,
  });

  const migotoManager = useSettingsMigotoConfig({ t, tr, toast });

  const migotoHasLockedCustomization = computed(
    () =>
      migotoManager.isMigotoImporterLocked.value ||
      migotoManager.isMigotoInjectionLocked.value,
  );

  const openDocumentsDoc = async (docId: string) => {
    if (!appSettings.showDocuments) {
      appSettings.showDocuments = true;
    }
    await router.push({
      name: 'Documents',
      query: {
        doc: docId,
      },
    });
  };

  const getLocalizedGameName = (game: Pick<GameInfo, 'name'> | string) => {
    const gameName = typeof game === 'string' ? game : game.name;
    return te(`games.${gameName}`) ? t(`games.${gameName}`) : gameName;
  };

  const xxmiManager = useSettingsXxmiManager({
    tr,
    getSelectedGame: () => migotoManager.migotoSelectedGame.value,
    getDeployTargetDir: () =>
      migotoManager.getMigotoPathDisplayValue('importer_folder'),
  });

  const pageController = useSettingsPageController({
    route,
    tr,
    toast,
    getVersionCheckInfo,
    getResourceVersionInfo,
    pullResourceUpdates,
    reenterOnboarding: () => startFeatureOnboarding(0),
    globalMigotoEnabled: () => migotoManager.globalMigotoEnabled.value,
    loadProton: async () => {
      await protonManager.loadCatalog();
      await protonManager.refreshLocalGrouped();
    },
    loadDxvk: async () => {
      await graphicsManager.refreshDxvkLocal();
    },
    loadVkd3d: async () => {
      await graphicsManager.refreshVkd3dLocal();
    },
    loadMigoto: async () => {
      await Promise.all([
        migotoManager.ensureMigotoLoaded(),
        xxmiManager.loadXxmiSources(),
        xxmiManager.refreshXxmiLocal(),
      ]);
    },
    setProtonEditorCollapsed: () => {
      protonManager.showProtonCatalogEditor.value = false;
    },
  });

  const selectCacheDir = async () => {
    const selected = await openFileDialog({
      directory: true,
      multiple: false,
      title: t('settings.selectcachedir'),
    });

    if (selected && typeof selected === 'string') {
      appSettings.cacheDir = selected;
    }
  };

  const selectDataDir = async () => {
    const selected = await openFileDialog({
      directory: true,
      multiple: false,
      title: t('settings.selectdatadir'),
    });

    if (selected && typeof selected === 'string') {
      appSettings.dataDir = selected;
    }
  };

  return {
    appSettings,
    updateLocaleAndReload,
    openLogWindow,
    migotoHasLockedCustomization,
    openDocumentsDoc,
    getLocalizedGameName,
    selectCacheDir,
    selectDataDir,
    ...protonManager,
    ...graphicsManager,
    ...migotoManager,
    ...xxmiManager,
    ...pageController,
  };
}
