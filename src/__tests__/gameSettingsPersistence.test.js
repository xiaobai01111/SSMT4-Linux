import test from 'node:test';
import assert from 'node:assert/strict';
import { useGameSettingsPersistence } from '../composables/useGameSettingsPersistence.ts';

const createPersistence = ({
  selectedWineVersionId = 'wine-ge',
  isSaveBlocked = () => false,
} = {}) => {
  const steps = [];
  const config = {
    basic: {
      gamePreset: 'WutheringWaves',
      runtimeEnv: 'wine',
    },
    other: {},
  };
  const protonSettings = {
    custom_env: {},
  };

  const persistence = useGameSettingsPersistence({
    normalizeGameName: (value) => String(value || '').trim(),
    isEditableGameName: (value) => {
      const normalized = String(value || '').trim();
      return normalized.length > 0 && normalized !== 'Default';
    },
    currentGameName: () => 'WutheringWaves',
    isSaveBlocked,
    config,
    protonSettings,
    selectedWineVersionId: () => selectedWineVersionId,
    syncSystemOptionsIntoConfig: () => {
      steps.push('sync-system');
      config.other.gameLang = 'zh_CN';
    },
    syncInfoConfigToLegacyState: () => {
      steps.push('sync-info');
      config.basic.runtimeEnv = 'steam';
    },
    saveGameConfig: async () => {
      steps.push('save-config');
    },
    saveWineConfig: async (_gameName, wineVersionId) => {
      steps.push(`save-wine:${wineVersionId}`);
    },
    saveInfoRuntime: async () => {
      steps.push('save-info-runtime');
    },
  });

  return {
    steps,
    config,
    persistence,
  };
};

test('persistSystemSection saves runtime profile then legacy config exactly once', async () => {
  const { steps, config, persistence } = createPersistence();

  await persistence.persistSystemSection();

  assert.deepEqual(steps, [
    'save-wine:wine-ge',
    'sync-system',
    'save-config',
  ]);
  assert.equal(config.other.wineVersionId, 'wine-ge');
});

test('persistRuntimeSection saves info runtime before runtime profile and legacy config', async () => {
  const { steps, config, persistence } = createPersistence();

  await persistence.persistRuntimeSection();

  assert.deepEqual(steps, [
    'save-info-runtime',
    'sync-info',
    'save-wine:wine-ge',
    'sync-system',
    'save-config',
  ]);
  assert.equal(config.basic.runtimeEnv, 'steam');
  assert.equal(config.other.wineVersionId, 'wine-ge');
});

test('persistManagedState skips runtime profile when no wine version is selected', async () => {
  const { steps, persistence } = createPersistence({
    selectedWineVersionId: '',
  });

  await persistence.persistManagedState();

  assert.deepEqual(steps, [
    'sync-system',
    'save-config',
  ]);
});
