import type { GlobalSettingsMenu, RuntimeFocusTarget } from './gameSettings';

export type ExecutablePathProbeResult =
  | { kind: 'ok' }
  | {
      kind: 'mismatch';
      configuredPath: string;
      detectedPath: string;
    };

export type RuntimeReadinessResult =
  | { kind: 'ready' }
  | {
      kind: 'runtime_scan_failed';
      reason: string;
    }
  | { kind: 'no_proton' }
  | { kind: 'no_dxvk' }
  | { kind: 'proton_not_configured' }
  | {
      kind: 'invalid_proton_version';
      wineVersionId: string;
    }
  | { kind: 'dxvk_not_installed' };

export type ProtectionRequirementResult =
  | { kind: 'ready' }
  | {
      kind: 'warning_missing_optional';
      missing: string[];
    }
  | {
      kind: 'required_missing';
      missing: string[];
    }
  | {
      kind: 'check_failed';
      error: string;
    };

export type HomeLaunchDialogKind = 'info' | 'warning' | 'error';

export type HomeLaunchStepParams = Record<string, string | number>;

export type HomeLaunchNavigationAction =
  | {
      kind: 'open_global_settings';
      menu: GlobalSettingsMenu;
      reasonKey?: string;
    }
  | {
      kind: 'open_runtime_settings';
      focusTarget: RuntimeFocusTarget;
      reasonKey?: string;
    }
  | {
      kind: 'open_game_settings_game_tab';
    }
  | {
      kind: 'open_download_modal';
    };

interface HomeLaunchBaseStep {
  titleKey: string;
  messageKey: string;
  dialogKind: HomeLaunchDialogKind;
  params?: HomeLaunchStepParams;
  missingItems?: string[];
  includeMissingItemsLabel?: boolean;
}

export interface HomeLaunchMessageStep extends HomeLaunchBaseStep {
  kind: 'message';
  navigation?: HomeLaunchNavigationAction;
  continueLaunch: boolean;
}

export interface HomeLaunchConfirmStep extends HomeLaunchBaseStep {
  kind: 'confirm';
  okLabelKey: string;
  cancelLabelKey: string;
  navigationOnConfirm?: HomeLaunchNavigationAction;
  navigationOnCancel?: HomeLaunchNavigationAction;
  continueOnConfirm: boolean;
  continueOnCancel: boolean;
}

export type HomeLaunchGuardStep =
  | HomeLaunchMessageStep
  | HomeLaunchConfirmStep;

export interface HomeLaunchGuardPlan {
  wineVersionId: string;
  steps: HomeLaunchGuardStep[];
}
