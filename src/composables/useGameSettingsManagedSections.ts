import { ref } from 'vue';

type NormalizeGameName = (value: string | null | undefined) => string;
type IsEditableGameName = (value: string | null | undefined) => boolean;
type SectionLoader = (gameName: string, sessionId: number) => Promise<unknown>;
type SectionLoaderGroup = SectionLoader[];
type ManagedSave = (gameName: string) => Promise<void>;

interface ManagedSectionsOptions {
  normalizeGameName: NormalizeGameName;
  isEditableGameName: IsEditableGameName;
  isModalOpen: () => boolean;
}

interface LoadManagedSectionsOptions {
  beforeLoad?: (gameName: string, sessionId: number) => void;
}

export function useGameSettingsManagedSections(
  options: ManagedSectionsOptions,
) {
  const isLoading = ref(false);
  const hasLoadedConfig = ref(false);
  const activeLoadSession = ref(0);
  let managedSaveInFlight: Promise<void> | null = null;

  const startLoadSession = (): number => {
    activeLoadSession.value += 1;
    return activeLoadSession.value;
  };

  const isActiveLoadSession = (sessionId: number): boolean =>
    options.isModalOpen() && activeLoadSession.value === sessionId;

  const loadManagedSections = (
    targetGameName: string | null | undefined,
    loaders: SectionLoader[],
    loadOptions: LoadManagedSectionsOptions = {},
  ) =>
    loadManagedSectionGroups(targetGameName, [loaders], loadOptions);

  const loadManagedSectionGroups = async (
    targetGameName: string | null | undefined,
    loaderGroups: SectionLoaderGroup[],
    loadOptions: LoadManagedSectionsOptions = {},
  ) => {
    const gameName = options.normalizeGameName(targetGameName);
    if (!gameName) {
      return Promise.resolve([] as PromiseSettledResult<unknown>[]);
    }

    const sessionId = startLoadSession();
    hasLoadedConfig.value = false;
    loadOptions.beforeLoad?.(gameName, sessionId);
    const results: PromiseSettledResult<unknown>[] = [];
    for (const loaders of loaderGroups) {
      const groupResults = await Promise.allSettled(
        loaders.map((loader) => loader(gameName, sessionId)),
      );
      results.push(...groupResults);
      if (!isActiveLoadSession(sessionId)) {
        break;
      }
    }
    return results;
  };

  const saveManagedSections = async (
    targetGameName: string | null | undefined,
    saveManaged: ManagedSave,
  ) => {
    const gameName = options.normalizeGameName(targetGameName);
    if (!options.isEditableGameName(gameName) || !hasLoadedConfig.value) {
      return;
    }

    if (managedSaveInFlight) {
      await managedSaveInFlight;
      return;
    }

    managedSaveInFlight = saveManaged(gameName);
    try {
      await managedSaveInFlight;
    } finally {
      managedSaveInFlight = null;
    }
  };

  return {
    isLoading,
    hasLoadedConfig,
    activeLoadSession,
    startLoadSession,
    isActiveLoadSession,
    loadManagedSections,
    loadManagedSectionGroups,
    saveManagedSections,
  };
}
