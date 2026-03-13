import { useI18n } from 'vue-i18n';
import { askConfirm, showMessage } from '../api';
import type {
  HomeLaunchGuardPlan,
  HomeLaunchGuardStep,
  HomeLaunchNavigationAction,
  HomeLaunchStepParams,
} from '../types/homeLaunch';
import type { GlobalSettingsMenu, RuntimeFocusTarget } from '../types/gameSettings';

interface HomeLaunchNavigation {
  openGlobalSettingsMenu(menu: GlobalSettingsMenu, reason?: string): Promise<void>;
  openRuntimeSettings(reason?: string, focusTarget?: RuntimeFocusTarget): Promise<void>;
  openGameSettingsGameTab(): Promise<void>;
  openDownloadModal(): void | Promise<void>;
}

function formatMissingItems(
  items: string[] | undefined,
  includeLabel: boolean,
  missingItemsLabel: string,
): string | undefined {
  if (!items || items.length === 0) {
    return includeLabel ? '' : undefined;
  }

  const bulletList = items.join('\n- ');
  if (!includeLabel) {
    return bulletList;
  }
  return `\n\n${missingItemsLabel}:\n- ${bulletList}`;
}

export function useHomeLaunchGuardUi(navigation: HomeLaunchNavigation) {
  const { t } = useI18n();

  const applyNavigation = async (
    action: HomeLaunchNavigationAction | undefined,
  ) => {
    if (!action) return;

    switch (action.kind) {
      case 'open_global_settings':
        await navigation.openGlobalSettingsMenu(
          action.menu,
          action.reasonKey ? t(action.reasonKey) : undefined,
        );
        return;
      case 'open_runtime_settings':
        await navigation.openRuntimeSettings(
          action.reasonKey ? t(action.reasonKey) : undefined,
          action.focusTarget,
        );
        return;
      case 'open_game_settings_game_tab':
        await navigation.openGameSettingsGameTab();
        return;
      case 'open_download_modal':
        await Promise.resolve(navigation.openDownloadModal());
        return;
    }
  };

  const buildMessageParams = (
    step: HomeLaunchGuardStep,
  ): HomeLaunchStepParams | undefined => {
    const params = step.params ? { ...step.params } : undefined;
    const missing = formatMissingItems(
      step.missingItems,
      !!step.includeMissingItemsLabel,
      t('home.messages.missingItemsLabel'),
    );
    if (missing === undefined) {
      return params;
    }
    return {
      ...(params || {}),
      missing,
    };
  };

  const executeGuardPlan = async (
    plan: HomeLaunchGuardPlan,
  ): Promise<boolean> => {
    for (const step of plan.steps) {
      const message = t(step.messageKey, buildMessageParams(step) || {});
      const title = t(step.titleKey);

      if (step.kind === 'message') {
        await showMessage(message, { title, kind: step.dialogKind });
        await applyNavigation(step.navigation);
        if (!step.continueLaunch) {
          return false;
        }
        continue;
      }

      const confirmed = await askConfirm(message, {
        title,
        kind: step.dialogKind,
        okLabel: t(step.okLabelKey),
        cancelLabel: t(step.cancelLabelKey),
      });

      if (confirmed) {
        await applyNavigation(step.navigationOnConfirm);
        if (!step.continueOnConfirm) {
          return false;
        }
        continue;
      }

      await applyNavigation(step.navigationOnCancel);
      if (!step.continueOnCancel) {
        return false;
      }
    }

    return true;
  };

  const ensureRiskAcknowledged = async (
    alreadyAcknowledged: boolean,
    markAcknowledged: () => void,
  ) => {
    if (alreadyAcknowledged) return true;

    const accepted = await askConfirm(
      t('home.messages.tosPrimary'),
      {
        title: t('home.messages.title.tosRisk'),
        kind: 'warning',
        okLabel: t('home.messages.ok.riskUnderstood'),
        cancelLabel: t('home.messages.cancel.cancel'),
      },
    );
    if (!accepted) return false;

    const second = await askConfirm(
      t('home.messages.tosSecondary'),
      {
        title: t('home.messages.title.secondConfirm'),
        kind: 'warning',
        okLabel: t('home.messages.ok.confirmContinue'),
        cancelLabel: t('home.messages.cancel.back'),
      },
    );
    if (!second) return false;

    markAcknowledged();
    return true;
  };

  return {
    executeGuardPlan,
    ensureRiskAcknowledged,
  };
}
