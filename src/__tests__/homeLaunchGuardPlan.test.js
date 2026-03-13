import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildExecutableGuardSteps,
  buildProtectionGuardSteps,
  buildRuntimeGuardSteps,
  findBlockingStep,
} from '../composables/homeLaunchGuardPlan.ts';

test('protection required builds a blocking download-modal step', () => {
  const steps = buildProtectionGuardSteps({
    kind: 'required_missing',
    missing: ['ACE', 'Ids.dat'],
  });

  assert.equal(steps.length, 1);
  assert.equal(steps[0]?.kind, 'message');
  assert.equal(steps[0]?.continueLaunch, false);
  assert.equal(steps[0]?.navigation?.kind, 'open_download_modal');
  assert.deepEqual(steps[0]?.missingItems, ['ACE', 'Ids.dat']);
  assert.equal(findBlockingStep(steps), steps[0]);
});

test('dxvk-not-installed confirm keeps one launch-continue branch', () => {
  const steps = buildRuntimeGuardSteps({
    kind: 'dxvk_not_installed',
  });

  assert.equal(steps.length, 1);
  assert.equal(steps[0]?.kind, 'confirm');
  assert.equal(steps[0]?.continueOnConfirm, false);
  assert.equal(steps[0]?.continueOnCancel, true);
  assert.equal(steps[0]?.navigationOnConfirm?.kind, 'open_runtime_settings');
  assert.equal(findBlockingStep(steps), null);
});

test('executable mismatch becomes a non-blocking game-settings warning step', () => {
  const steps = buildExecutableGuardSteps({
    kind: 'mismatch',
    configuredPath: '/games/wuwa/old.exe',
    detectedPath: '/games/wuwa/new.exe',
  });

  assert.equal(steps.length, 1);
  assert.equal(steps[0]?.kind, 'message');
  assert.equal(steps[0]?.continueLaunch, true);
  assert.equal(steps[0]?.navigation?.kind, 'open_game_settings_game_tab');
  assert.deepEqual(steps[0]?.params, {
    configuredPath: '/games/wuwa/old.exe',
    detected: '/games/wuwa/new.exe',
  });
});
