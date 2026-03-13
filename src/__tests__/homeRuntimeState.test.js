import test from 'node:test';
import assert from 'node:assert/strict';
import {
  applyComponentDownloadProgressEvent,
  applyGameLifecycleEvent,
  createHomeRuntimeSnapshot,
} from '../utils/homeRuntimeState.ts';

test('applyGameLifecycleEvent marks game as running on started', () => {
  const state = createHomeRuntimeSnapshot();

  const next = applyGameLifecycleEvent(state, {
    event: 'started',
    game: 'WutheringWaves',
    pid: 123,
  });

  assert.equal(next.isGameRunning, true);
  assert.equal(next.isLaunching, false);
  assert.equal(next.runningGameName, 'WutheringWaves');
});

test('applyGameLifecycleEvent clears runtime state on exited', () => {
  const next = applyGameLifecycleEvent(
    {
      isLaunching: true,
      isGameRunning: true,
      runningGameName: 'WutheringWaves',
      componentDlProgress: null,
    },
    {
      event: 'exited',
      game: 'WutheringWaves',
      pid: 123,
      code: 0,
    },
  );

  assert.equal(next.isGameRunning, false);
  assert.equal(next.isLaunching, false);
  assert.equal(next.runningGameName, '');
});

test('applyComponentDownloadProgressEvent clears progress on done', () => {
  const next = applyComponentDownloadProgressEvent(createHomeRuntimeSnapshot(), {
    component: 'dxvk',
    phase: 'done',
    progress: 100,
    total: 100,
  });

  assert.equal(next.componentDlProgress, null);
});
