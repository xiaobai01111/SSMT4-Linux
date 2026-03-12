import assert from 'node:assert/strict';
import test from 'node:test';

import {
  beginGameUpdateCheck,
  buildErrorGameUpdateCheckPatch,
  buildIdleGameUpdateCheckPatch,
  buildReadyGameUpdateCheckPatch,
  createGameUpdateCheckState,
  normalizeGameUpdateError,
  shouldApplyGameUpdateResult,
} from '../utils/gameUpdateCheckState.ts';

test('createGameUpdateCheckState starts from clean idle state', () => {
  assert.deepEqual(createGameUpdateCheckState(), {
    phase: 'idle',
    state: null,
    error: '',
    updatedAt: 0,
    requestId: 0,
  });
});

test('beginGameUpdateCheck enters checking state with request id', () => {
  assert.deepEqual(beginGameUpdateCheck(7, 123), {
    phase: 'checking',
    error: '',
    updatedAt: 123,
    requestId: 7,
  });
});

test('shouldApplyGameUpdateResult only accepts matching request id', () => {
  assert.equal(shouldApplyGameUpdateResult({ requestId: 9 }, 9), true);
  assert.equal(shouldApplyGameUpdateResult({ requestId: 9 }, 8), false);
});

test('ready and idle patches clear stale error and set expected payload', () => {
  const readyPatch = buildReadyGameUpdateCheckPatch(
    {
      state: 'needupdate',
      local_version: '1.0.0',
      remote_version: '1.1.0',
      supports_incremental: true,
    },
    456,
  );
  assert.deepEqual(readyPatch, {
    phase: 'ready',
    state: {
      state: 'needupdate',
      local_version: '1.0.0',
      remote_version: '1.1.0',
      supports_incremental: true,
    },
    error: '',
    updatedAt: 456,
  });

  assert.deepEqual(buildIdleGameUpdateCheckPatch(789), {
    phase: 'idle',
    state: null,
    error: '',
    updatedAt: 789,
  });
});

test('error patch normalizes Error objects and unknown values', () => {
  assert.equal(normalizeGameUpdateError(new Error('network error')), 'network error');
  assert.equal(normalizeGameUpdateError('plain failure'), 'plain failure');
  assert.equal(normalizeGameUpdateError(null), '');

  assert.deepEqual(buildErrorGameUpdateCheckPatch(new Error('boom'), 999), {
    phase: 'error',
    state: null,
    error: 'boom',
    updatedAt: 999,
  });
});
