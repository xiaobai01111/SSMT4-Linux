export type Locale = 'en' | 'zhs' | 'zht';
export type BackgroundType = 'Image' | 'Video';

export interface AppSettings {
  bgType: BackgroundType;
  bgImage: string;
  bgVideo: string;
  contentOpacity: number;
  contentBlur: number;
  cacheDir: string;
  currentConfigName: string;
  githubToken: string;
  showWebsites: boolean;
  showDocuments: boolean;
  migotoEnabled: boolean;
  locale: Locale;
  dataDir: string;
  initialized: boolean;
  tosRiskAcknowledged: boolean;
  onboardingCompleted: boolean;
  onboardingVersion: number;
  snowbreakSourcePolicy: 'official_first' | 'community_first';
}

export interface GameInfo {
  name: string;
  displayName: string;
  iconPath: string;
  bgPath: string;
  bgVideoPath?: string;
  bgVideoRawPath?: string;
  bgType: BackgroundType;
  showSidebar: boolean;
  migotoSupported: boolean;
}

export interface GameConfigOther {
  displayName?: string;
  wineVersionId?: string;
  gpuIndex?: number;
  gameLang?: string;
  gamePath?: string;
  gameFolder?: string;
  launcherApi?: string;
  launcherInstallerVersion?: string;
  launcherInstallerPath?: string;
  launchArgs?: string;
  workingDir?: string;
  migoto?: Record<string, unknown> & { enabled?: boolean };
  [key: string]: unknown;
}

export interface GameConfig {
  basic: {
    gamePreset: string;
    runtimeEnv?: 'wine' | 'steam' | 'linux';
    backgroundType?: BackgroundType;
  };
  other: GameConfigOther;
}

export type DialogKind = 'info' | 'warning' | 'error' | 'success';
export type ChannelProtectionMode = 'init' | 'protected';

export interface ChannelProtectionState {
  required: boolean;
  enabled: boolean;
  mode?: ChannelProtectionMode | string;
  launchEnforcement?: 'warn' | 'block' | string;
  channelKey?: string;
  currentValue?: number;
  initValue?: number;
  expectedValue?: number;
  protectedValue?: number;
  configPath?: string;
  error?: string;
  backupExists?: boolean;
}

export interface ChannelProtectionStatus {
  gamePreset: string;
  supported: boolean;
  gameRoot?: string;
  channel: ChannelProtectionState;
}

export interface ProtectionTelemetryStatus {
  required: boolean;
  allBlocked: boolean;
  blocked: string[];
  unblocked: string[];
  totalServers: number;
}

export interface ProtectionFilesStatus {
  required: boolean;
  allRemoved: boolean;
  removed: string[];
  existing: string[];
  totalFiles: number;
  error?: string | null;
}

export interface GameProtectionStatus {
  gamePreset: string;
  supported: boolean;
  enforceAtLaunch: boolean;
  hasProtections: boolean;
  enabled: boolean;
  allProtected: boolean;
  missing: string[];
  gameRoot?: string | null;
  telemetry: ProtectionTelemetryStatus;
  files: ProtectionFilesStatus;
  channel: ChannelProtectionState;
}

export interface TelemetryStatus {
  supported: boolean;
  message?: string;
  allBlocked?: boolean;
  blocked?: string[];
  unblocked?: string[];
  totalServers?: number;
  channel?: ChannelProtectionState;
}

export interface TelemetryMutationSection {
  supported: boolean;
  message: string;
  newlyBlocked?: number;
  removedEntries?: number;
  servers?: string[];
  removed?: string[];
  notFound?: string[];
  state?: ChannelProtectionState;
}

export interface TelemetryMutationResult {
  success: boolean;
  gamePreset: string;
  telemetry: TelemetryMutationSection;
  channel: TelemetryMutationSection;
}

export interface TelemetryFilesResult {
  supported: boolean;
  message?: string;
  removed?: string[];
  notFound?: string[];
}

export interface GameProtectionApplyResult {
  success: boolean;
  gamePreset: string;
  results: {
    telemetry?: TelemetryMutationResult;
    telemetryFiles?: TelemetryFilesResult;
    channel?: {
      mode: ChannelProtectionMode | string;
      state: ChannelProtectionState;
    };
    [key: string]: unknown;
  };
}

export interface GameProtectionDescriptor {
  type: string;
  name: string;
  description: string;
  servers?: string[];
  files?: string[];
  channelKey?: string;
  initValue?: number | null;
  targetValue?: number;
  defaultMode?: ChannelProtectionMode | string;
  launchEnforcement?: 'warn' | 'block' | string;
  configRelativePath?: string;
}

export interface GameProtectionInfo {
  gamePreset: string;
  category: string;
  protections: GameProtectionDescriptor[];
  hasProtections: boolean;
}

export interface LauncherPatchConfig {
  version: string;
  base_url: string;
  index_file: string;
  ext: unknown;
}

export interface LauncherInfo {
  version: string;
  resources_base_path: string;
  cdn_url: string;
  index_file_url: string;
  patch_configs: LauncherPatchConfig[];
  raw: unknown;
}
