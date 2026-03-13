import type { ProtonSettings } from '../api';
import type { GameConfig } from '../types/ipc';

type NormalizeGameName = (value: string | null | undefined) => string;
type IsEditableGameName = (value: string | null | undefined) => boolean;

interface GameSettingsPersistenceOptions {
  normalizeGameName: NormalizeGameName;
  isEditableGameName: IsEditableGameName;
  currentGameName: () => string | null | undefined;
  isSaveBlocked: () => boolean;
  config: GameConfig;
  protonSettings: ProtonSettings;
  selectedWineVersionId: () => string;
  syncSystemOptionsIntoConfig: () => void;
  syncInfoConfigToLegacyState: () => void;
  saveGameConfig: (gameName: string, config: GameConfig) => Promise<void>;
  saveWineConfig: (
    gameName: string,
    wineVersionId: string,
    protonSettings: ProtonSettings,
  ) => Promise<void>;
  saveInfoRuntime: (gameName: string) => Promise<void>;
}

export function useGameSettingsPersistence(
  options: GameSettingsPersistenceOptions,
) {
  const resolveGameName = (
    targetGameName: string | null | undefined = options.currentGameName(),
  ): string => options.normalizeGameName(targetGameName);

  const canPersist = (gameName: string): boolean =>
    options.isEditableGameName(gameName) && !options.isSaveBlocked();

  const persistLegacyConfig = async (
    targetGameName: string | null | undefined = options.currentGameName(),
  ) => {
    const gameName = resolveGameName(targetGameName);
    if (!canPersist(gameName)) {
      return false;
    }

    options.syncSystemOptionsIntoConfig();
    await options.saveGameConfig(gameName, options.config);
    return true;
  };

  const persistRuntimeProfile = async (
    targetGameName: string | null | undefined = options.currentGameName(),
  ) => {
    const gameName = resolveGameName(targetGameName);
    const wineVersionId = options.selectedWineVersionId();
    if (!options.isEditableGameName(gameName) || !wineVersionId) {
      return false;
    }

    await options.saveWineConfig(gameName, wineVersionId, options.protonSettings);
    options.config.other.wineVersionId = wineVersionId;
    return true;
  };

  const persistManagedState = async (
    targetGameName: string | null | undefined = options.currentGameName(),
  ) => {
    const gameName = resolveGameName(targetGameName);
    if (!options.isEditableGameName(gameName)) {
      return false;
    }

    await persistRuntimeProfile(gameName);
    await persistLegacyConfig(gameName);
    return true;
  };

  const persistRuntimeSection = async (
    targetGameName: string | null | undefined = options.currentGameName(),
  ) => {
    const gameName = resolveGameName(targetGameName);
    if (!options.isEditableGameName(gameName)) {
      return false;
    }

    await options.saveInfoRuntime(gameName);
    options.syncInfoConfigToLegacyState();
    await persistManagedState(gameName);
    return true;
  };

  const persistSystemSection = async (
    targetGameName: string | null | undefined = options.currentGameName(),
  ) => persistManagedState(targetGameName);

  return {
    persistLegacyConfig,
    persistRuntimeProfile,
    persistManagedState,
    persistRuntimeSection,
    persistSystemSection,
  };
}
