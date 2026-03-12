import assert from 'node:assert/strict';
import test from 'node:test';

import {
  resolveHomePrimaryAction,
  resolveHomePrimaryLabelKey,
  shouldOpenDownloadForPrimaryAction,
} from '../utils/gameUpdateUi.ts';
import {
  buildCompletedTaskMarker,
  shouldHandleCompletedTaskMarker,
} from '../utils/downloadTaskUi.ts';

test('resolveHomePrimaryAction prioritizes running over all other states', () => {
  const action = resolveHomePrimaryAction({
    isGameRunning: true,
    needsUpdate: true,
    hasExecutable: true,
  });

  assert.equal(action, 'running');
  assert.equal(resolveHomePrimaryLabelKey(action), 'home.status.running');
  assert.equal(shouldOpenDownloadForPrimaryAction(action), false);
});

test('resolveHomePrimaryAction routes update flow to download modal', () => {
  const action = resolveHomePrimaryAction({
    isGameRunning: false,
    needsUpdate: true,
    hasExecutable: true,
  });

  assert.equal(action, 'update');
  assert.equal(resolveHomePrimaryLabelKey(action), 'home.css.updategame');
  assert.equal(shouldOpenDownloadForPrimaryAction(action), true);
});

test('resolveHomePrimaryAction starts configured games without updates', () => {
  const action = resolveHomePrimaryAction({
    isGameRunning: false,
    needsUpdate: false,
    hasExecutable: true,
  });

  assert.equal(action, 'start');
  assert.equal(resolveHomePrimaryLabelKey(action), 'home.css.startgame');
  assert.equal(shouldOpenDownloadForPrimaryAction(action), false);
});

test('resolveHomePrimaryAction falls back to download when executable is missing', () => {
  const action = resolveHomePrimaryAction({
    isGameRunning: false,
    needsUpdate: false,
    hasExecutable: false,
  });

  assert.equal(action, 'download');
  assert.equal(resolveHomePrimaryLabelKey(action), 'home.css.downloadgame');
  assert.equal(shouldOpenDownloadForPrimaryAction(action), true);
});

test('buildCompletedTaskMarker only emits markers for done tasks', () => {
  assert.equal(buildCompletedTaskMarker(null), '');
  assert.equal(
    buildCompletedTaskMarker({ key: 'task-1', phase: 'downloading', updatedAt: 12 }),
    '',
  );
  assert.equal(
    buildCompletedTaskMarker({ key: 'task-1', phase: 'done', updatedAt: 12 }),
    'task-1:12',
  );
});

test('shouldHandleCompletedTaskMarker requires a new marker and open modal', () => {
  assert.equal(shouldHandleCompletedTaskMarker('', '', true), false);
  assert.equal(shouldHandleCompletedTaskMarker('task-1:12', 'task-1:12', true), false);
  assert.equal(shouldHandleCompletedTaskMarker('task-1:12', '', false), false);
  assert.equal(shouldHandleCompletedTaskMarker('task-1:12', '', true), true);
});
