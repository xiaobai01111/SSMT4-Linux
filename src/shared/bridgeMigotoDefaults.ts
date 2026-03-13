import defaultsJson from './bridgeMigotoDefaults.json';

export interface SharedBridgeGameDefaults {
  processStartMethod: string;
  processPriority: string;
  processTimeout: number;
}

export interface SharedBridgeMigotoDefaultsSection {
  useHook: boolean;
  useDllDrop: boolean;
  enforceRendering: boolean;
  enableHunting: boolean;
  dumpShaders: boolean;
  muteWarnings: boolean;
  callsLogging: boolean;
  debugLogging: boolean;
  unsafeMode: boolean;
  xxmiDllInitDelay: number;
}

export interface SharedBridgeCustomLaunchDefaults {
  enabled: boolean;
  injectMode: string;
}

export interface SharedBridgeShellCommandDefaults {
  enabled: boolean;
  wait: boolean;
}

export interface SharedBridgeExtraLibrariesDefaults {
  enabled: boolean;
}

export interface SharedBridgeJadeiteDefaults {
  enabled: boolean;
}

export interface SharedBridgeMigotoDefaults {
  schemaVersion: number;
  game: SharedBridgeGameDefaults;
  migoto: SharedBridgeMigotoDefaultsSection;
  customLaunch: SharedBridgeCustomLaunchDefaults;
  shellCommand: SharedBridgeShellCommandDefaults;
  extraLibraries: SharedBridgeExtraLibrariesDefaults;
  jadeite: SharedBridgeJadeiteDefaults;
}

export const sharedBridgeMigotoDefaults =
  defaultsJson as SharedBridgeMigotoDefaults;
