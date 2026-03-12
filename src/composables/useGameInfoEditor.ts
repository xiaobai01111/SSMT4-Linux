import { computed, reactive, ref } from 'vue';
import {
  listGamePresetsForInfo,
  loadGameInfoV2,
  saveGameInfoAssets,
  saveGameInfoMeta,
  saveGameInfoRuntime,
  validateGameConfigName,
  type GameBackgroundType,
  type GameInfoAssetsPatch,
  type GameInfoConfigV2,
  type PresetCatalogItem,
  type RuntimeEnv,
  type ValidateNameResult,
} from '../api';

type SaveSection = 'meta' | 'runtime' | 'assets';

const createEmptyInfoConfig = (gameName: string): GameInfoConfigV2 => ({
  schemaVersion: 2,
  gameName,
  meta: {
    displayName: gameName,
    gamePreset: gameName,
  },
  runtime: {
    runtimeEnv: 'wine',
  },
  assets: {
    backgroundType: 'Image',
    iconFile: null,
    backgroundFile: null,
  },
  readOnly: false,
  warningCode: null,
});

export function useGameInfoEditor() {
  const infoConfig = ref<GameInfoConfigV2>(createEmptyInfoConfig(''));
  const presets = ref<PresetCatalogItem[]>([]);
  const loading = ref(false);
  const saving = reactive<Record<SaveSection, boolean>>({
    meta: false,
    runtime: false,
    assets: false,
  });
  const dirty = reactive<Record<SaveSection, boolean>>({
    meta: false,
    runtime: false,
    assets: false,
  });
  const sectionErrors = reactive<Record<SaveSection, string>>({
    meta: '',
    runtime: '',
    assets: '',
  });
  const nameValidation = ref<ValidateNameResult | null>(null);

  const hasUnsavedChanges = computed(
    () => dirty.meta || dirty.runtime || dirty.assets,
  );

  const activePreset = computed(() =>
    presets.value.find((preset) => preset.id === infoConfig.value.meta.gamePreset),
  );

  const canSave = computed(() => !infoConfig.value.readOnly);

  const resetEditorFlags = () => {
    dirty.meta = false;
    dirty.runtime = false;
    dirty.assets = false;
    sectionErrors.meta = '';
    sectionErrors.runtime = '';
    sectionErrors.assets = '';
    nameValidation.value = null;
  };

  const load = async (gameName: string) => {
    loading.value = true;
    try {
      const [presetCatalog, config] = await Promise.all([
        listGamePresetsForInfo(),
        loadGameInfoV2(gameName),
      ]);
      presets.value = presetCatalog;
      infoConfig.value = config;
      resetEditorFlags();
      return config;
    } finally {
      loading.value = false;
    }
  };

  const markDirty = (section: SaveSection) => {
    dirty[section] = true;
    sectionErrors[section] = '';
  };

  const saveMeta = async (gameName: string) => {
    if (!canSave.value) {
      sectionErrors.meta = 'READ_ONLY';
      return;
    }
    saving.meta = true;
    sectionErrors.meta = '';
    try {
      await saveGameInfoMeta(gameName, {
        displayName: infoConfig.value.meta.displayName,
        gamePreset: infoConfig.value.meta.gamePreset,
      });
      dirty.meta = false;
    } catch (error) {
      sectionErrors.meta = String(error);
      throw error;
    } finally {
      saving.meta = false;
    }
  };

  const saveRuntime = async (gameName: string) => {
    if (!canSave.value) {
      sectionErrors.runtime = 'READ_ONLY';
      return;
    }
    saving.runtime = true;
    sectionErrors.runtime = '';
    try {
      await saveGameInfoRuntime(gameName, {
        runtimeEnv: infoConfig.value.runtime.runtimeEnv,
      });
      dirty.runtime = false;
    } catch (error) {
      sectionErrors.runtime = String(error);
      throw error;
    } finally {
      saving.runtime = false;
    }
  };

  const saveAssets = async (gameName: string, patch: GameInfoAssetsPatch = {}) => {
    if (!canSave.value) {
      sectionErrors.assets = 'READ_ONLY';
      return;
    }
    saving.assets = true;
    sectionErrors.assets = '';
    try {
      await saveGameInfoAssets(gameName, {
        backgroundType:
          patch.backgroundType ?? infoConfig.value.assets.backgroundType,
        iconFile: patch.iconFile,
        backgroundFile: patch.backgroundFile,
      });
      dirty.assets = false;
    } catch (error) {
      sectionErrors.assets = String(error);
      throw error;
    } finally {
      saving.assets = false;
    }
  };

  const validateName = async (
    name: string,
    currentGameName?: string,
  ): Promise<ValidateNameResult> => {
    const result = await validateGameConfigName(name, currentGameName ?? null);
    nameValidation.value = result;
    return result;
  };

  const setPreset = (presetId: string) => {
    infoConfig.value.meta.gamePreset = presetId;
    markDirty('meta');
  };

  const setRuntimeEnv = (runtimeEnv: RuntimeEnv) => {
    infoConfig.value.runtime.runtimeEnv = runtimeEnv;
    markDirty('runtime');
  };

  const setBackgroundType = (backgroundType: GameBackgroundType) => {
    infoConfig.value.assets.backgroundType = backgroundType;
    markDirty('assets');
  };

  return {
    infoConfig,
    presets,
    loading,
    saving,
    dirty,
    sectionErrors,
    nameValidation,
    hasUnsavedChanges,
    activePreset,
    canSave,
    load,
    markDirty,
    saveMeta,
    saveRuntime,
    saveAssets,
    validateName,
    setPreset,
    setRuntimeEnv,
    setBackgroundType,
  };
}
