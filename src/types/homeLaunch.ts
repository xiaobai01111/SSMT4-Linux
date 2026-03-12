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
