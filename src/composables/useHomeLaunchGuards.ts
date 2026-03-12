import { useI18n } from 'vue-i18n';
import {
  askConfirm,
  showMessage,
  type GameConfig,
} from '../api';
import {
  checkProtectionRequirement,
  checkRuntimeReadiness,
  probeExecutablePathMismatch,
  resolveWineVersionIdForLaunch,
} from './homeLaunchPolicy';
import type { GlobalSettingsMenu, RuntimeFocusTarget } from '../types/gameSettings';

interface HomeLaunchNavigation {
  openGlobalSettingsMenu(menu: GlobalSettingsMenu, reason?: string): Promise<void>;
  openRuntimeSettings(reason?: string, focusTarget?: RuntimeFocusTarget): Promise<void>;
  openGameSettingsGameTab(): Promise<void>;
  openDownloadModal(): void | Promise<void>;
}

export function useHomeLaunchGuards(navigation: HomeLaunchNavigation) {
  const { t } = useI18n();

  const openDownloadModal = async () => {
    await Promise.resolve(navigation.openDownloadModal());
  };

  const resolveWineVersionId = resolveWineVersionIdForLaunch;

  const checkExecutablePathMismatch = async (gameName: string, gameConfig: GameConfig) => {
    const result = await probeExecutablePathMismatch(gameName, gameConfig);
    if (result.kind !== 'mismatch') {
      return;
    }

    await showMessage(
      t('home.messages.exeMismatch', {
        configuredPath: result.configuredPath,
        detected: result.detectedPath,
      }),
      { title: t('home.messages.title.exeMismatch'), kind: 'warning' },
    );
    await navigation.openGameSettingsGameTab();
  };

  const ensureRuntimeReady = async (
    gameName: string,
    gameConfig: GameConfig,
    wineVersionId: string,
  ) => {
    const result = await checkRuntimeReadiness(gameName, gameConfig, wineVersionId);
    switch (result.kind) {
      case 'ready':
        return true;
      case 'runtime_scan_failed':
        await showMessage(t('home.messages.runtimeScanFailed', { reason: result.reason }), {
          title: t('home.messages.title.runtimeScanFailed'),
          kind: 'error',
        });
        await navigation.openGlobalSettingsMenu('proton', t('home.messages.reason.runtimeScanFailed'));
        return false;
      case 'no_proton':
        await showMessage(
          t('home.messages.noProton'),
          { title: t('home.messages.title.noProton'), kind: 'warning' },
        );
        await navigation.openGlobalSettingsMenu('proton', t('home.messages.reason.noProton'));
        return false;
      case 'no_dxvk':
        await showMessage(
          t('home.messages.noDxvk'),
          { title: t('home.messages.title.noDxvk'), kind: 'warning' },
        );
        await navigation.openGlobalSettingsMenu('dxvk', t('home.messages.reason.noDxvk'));
        return false;
      case 'proton_not_configured':
        await showMessage(
          t('home.messages.protonNotConfigured'),
          { title: t('home.messages.title.protonNotConfigured'), kind: 'warning' },
        );
        await navigation.openRuntimeSettings(t('home.messages.reason.protonNotConfigured'), 'wine_version');
        return false;
      case 'invalid_proton_version':
        await showMessage(
          t('home.messages.invalidProtonVersion', { wineVersionId: result.wineVersionId }),
          { title: t('home.messages.title.invalidProtonVersion'), kind: 'warning' },
        );
        await navigation.openRuntimeSettings(t('home.messages.reason.invalidProtonVersion'), 'wine_version');
        return false;
      case 'dxvk_not_installed': {
        const openNow = await askConfirm(
          t('home.messages.dxvkNotInstalledConfirm'),
          {
            title: t('home.messages.title.noDxvk'),
            kind: 'warning',
            okLabel: t('home.messages.ok.openRuntime'),
            cancelLabel: t('home.messages.cancel.continueLaunch'),
          },
        );
        if (openNow) {
          await navigation.openRuntimeSettings(t('home.messages.reason.installDxvk'), 'dxvk');
          return false;
        }
        return true;
      }
    }
  };

  const ensureProtectionEnabled = async (gameName: string, gameConfig: GameConfig) => {
    const result = await checkProtectionRequirement(gameName, gameConfig);
    switch (result.kind) {
      case 'ready':
        return true;
      case 'warning_missing_optional':
        await showMessage(
          t('home.messages.protectionWarnMode', {
            missing: result.missing.join('\n- '),
          }),
          { title: t('home.messages.title.protectionWarn'), kind: 'warning' },
        );
        return true;
      case 'required_missing': {
        const missing = result.missing.length > 0
          ? `\n\n${t('home.messages.missingItemsLabel')}:\n- ${result.missing.join('\n- ')}`
          : '';
        await showMessage(
          t('home.messages.protectionRequired', { missing }),
          { title: t('home.messages.title.protectionRequired'), kind: 'warning' },
        );
        await openDownloadModal();
        return false;
      }
      case 'check_failed':
        await showMessage(
          t('home.messages.protectionCheckFailed', { error: result.error }),
          {
            title: t('home.messages.title.protectionCheckFailed'),
            kind: 'error',
          },
        );
        await openDownloadModal();
        return false;
    }
  };

  return {
    checkExecutablePathMismatch,
    ensureProtectionEnabled,
    ensureRuntimeReady,
    resolveWineVersionId,
  };
}
