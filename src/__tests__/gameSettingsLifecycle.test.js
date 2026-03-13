import test from 'node:test';
import assert from 'node:assert/strict';
import { useGameSettingsLifecycle } from '../composables/useGameSettingsLifecycle.ts';

const createLifecycle = (askConfirm = async () => true) =>
  useGameSettingsLifecycle({
    askConfirm,
    tr: (_key, fallback) => fallback,
  });

test('requestClose blocks closing when unsaved info changes are not discarded', async () => {
  let closed = false;
  let asked = 0;
  const { requestClose } = createLifecycle(async () => {
    asked += 1;
    return false;
  });

  const result = await requestClose({
    hasUnsavedInfoChanges: true,
    onClose: () => {
      closed = true;
    },
  });

  assert.equal(result, false);
  assert.equal(closed, false);
  assert.equal(asked, 1);
});

test('handleGameNameChange reverts switch when unsaved info changes are not discarded', async () => {
  let revertedTo = '';
  let saved = 0;
  let loaded = 0;
  const { handleGameNameChange } = createLifecycle(async () => false);

  await handleGameNameChange({
    newGame: 'NewGame',
    oldGame: 'OldGame',
    isModalOpen: true,
    hasLoadedConfig: true,
    hasUnsavedInfoChanges: true,
    saveManagedSections: async () => {
      saved += 1;
    },
    loadAllSections: async () => {
      loaded += 1;
    },
    revertToGame: (gameName) => {
      revertedTo = gameName;
    },
  });

  assert.equal(revertedTo, 'OldGame');
  assert.equal(saved, 0);
  assert.equal(loaded, 0);
});
