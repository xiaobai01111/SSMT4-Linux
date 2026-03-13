import type {
  ExecutablePathProbeResult,
  HomeLaunchConfirmStep,
  HomeLaunchGuardStep,
  ProtectionRequirementResult,
  RuntimeReadinessResult,
} from '../types/homeLaunch';

export function buildProtectionGuardSteps(
  result: ProtectionRequirementResult,
): HomeLaunchGuardStep[] {
  switch (result.kind) {
    case 'ready':
      return [];
    case 'warning_missing_optional':
      return [
        {
          kind: 'message',
          titleKey: 'home.messages.title.protectionWarn',
          messageKey: 'home.messages.protectionWarnMode',
          dialogKind: 'warning',
          missingItems: result.missing,
          continueLaunch: true,
        },
      ];
    case 'required_missing':
      return [
        {
          kind: 'message',
          titleKey: 'home.messages.title.protectionRequired',
          messageKey: 'home.messages.protectionRequired',
          dialogKind: 'warning',
          missingItems: result.missing,
          includeMissingItemsLabel: true,
          navigation: {
            kind: 'open_download_modal',
          },
          continueLaunch: false,
        },
      ];
    case 'check_failed':
      return [
        {
          kind: 'message',
          titleKey: 'home.messages.title.protectionCheckFailed',
          messageKey: 'home.messages.protectionCheckFailed',
          dialogKind: 'error',
          params: {
            error: result.error,
          },
          navigation: {
            kind: 'open_download_modal',
          },
          continueLaunch: false,
        },
      ];
  }
}

export function buildRuntimeGuardSteps(
  result: RuntimeReadinessResult,
): HomeLaunchGuardStep[] {
  switch (result.kind) {
    case 'ready':
      return [];
    case 'runtime_scan_failed':
      return [
        {
          kind: 'message',
          titleKey: 'home.messages.title.runtimeScanFailed',
          messageKey: 'home.messages.runtimeScanFailed',
          dialogKind: 'error',
          params: {
            reason: result.reason,
          },
          navigation: {
            kind: 'open_global_settings',
            menu: 'proton',
            reasonKey: 'home.messages.reason.runtimeScanFailed',
          },
          continueLaunch: false,
        },
      ];
    case 'no_proton':
      return [
        {
          kind: 'message',
          titleKey: 'home.messages.title.noProton',
          messageKey: 'home.messages.noProton',
          dialogKind: 'warning',
          navigation: {
            kind: 'open_global_settings',
            menu: 'proton',
            reasonKey: 'home.messages.reason.noProton',
          },
          continueLaunch: false,
        },
      ];
    case 'no_dxvk':
      return [
        {
          kind: 'message',
          titleKey: 'home.messages.title.noDxvk',
          messageKey: 'home.messages.noDxvk',
          dialogKind: 'warning',
          navigation: {
            kind: 'open_global_settings',
            menu: 'dxvk',
            reasonKey: 'home.messages.reason.noDxvk',
          },
          continueLaunch: false,
        },
      ];
    case 'proton_not_configured':
      return [
        {
          kind: 'message',
          titleKey: 'home.messages.title.protonNotConfigured',
          messageKey: 'home.messages.protonNotConfigured',
          dialogKind: 'warning',
          navigation: {
            kind: 'open_runtime_settings',
            focusTarget: 'wine_version',
            reasonKey: 'home.messages.reason.protonNotConfigured',
          },
          continueLaunch: false,
        },
      ];
    case 'invalid_proton_version':
      return [
        {
          kind: 'message',
          titleKey: 'home.messages.title.invalidProtonVersion',
          messageKey: 'home.messages.invalidProtonVersion',
          dialogKind: 'warning',
          params: {
            wineVersionId: result.wineVersionId,
          },
          navigation: {
            kind: 'open_runtime_settings',
            focusTarget: 'wine_version',
            reasonKey: 'home.messages.reason.invalidProtonVersion',
          },
          continueLaunch: false,
        },
      ];
    case 'dxvk_not_installed':
      return [
        {
          kind: 'confirm',
          titleKey: 'home.messages.title.noDxvk',
          messageKey: 'home.messages.dxvkNotInstalledConfirm',
          dialogKind: 'warning',
          okLabelKey: 'home.messages.ok.openRuntime',
          cancelLabelKey: 'home.messages.cancel.continueLaunch',
          navigationOnConfirm: {
            kind: 'open_runtime_settings',
            focusTarget: 'dxvk',
            reasonKey: 'home.messages.reason.installDxvk',
          },
          continueOnConfirm: false,
          continueOnCancel: true,
        },
      ];
  }
}

export function buildExecutableGuardSteps(
  result: ExecutablePathProbeResult,
): HomeLaunchGuardStep[] {
  if (result.kind !== 'mismatch') {
    return [];
  }

  return [
    {
      kind: 'message',
      titleKey: 'home.messages.title.exeMismatch',
      messageKey: 'home.messages.exeMismatch',
      dialogKind: 'warning',
      params: {
        configuredPath: result.configuredPath,
        detected: result.detectedPath,
      },
      navigation: {
        kind: 'open_game_settings_game_tab',
      },
      continueLaunch: true,
    },
  ];
}

export function stepAlwaysStopsLaunch(step: HomeLaunchGuardStep): boolean {
  if (step.kind === 'message') {
    return !step.continueLaunch;
  }
  return !step.continueOnConfirm && !step.continueOnCancel;
}

export function findBlockingStep(
  steps: HomeLaunchGuardStep[],
): HomeLaunchGuardStep | null {
  return steps.find(stepAlwaysStopsLaunch) || null;
}

export function isConfirmStep(
  step: HomeLaunchGuardStep,
): step is HomeLaunchConfirmStep {
  return step.kind === 'confirm';
}
