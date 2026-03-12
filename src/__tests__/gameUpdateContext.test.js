import assert from 'node:assert/strict';
import test from 'node:test';

import {
  normalizePathForCompare,
  parentDir,
  resolveGameUpdateFolder,
  resolveGameUpdateSource,
  trimFolderPart,
} from '../utils/gameUpdateContext.ts';

test('path helpers normalize separators and parent paths', () => {
  assert.equal(trimFolderPart('/WutheringWaves/'), 'WutheringWaves');
  assert.equal(
    normalizePathForCompare('C:\\Games\\\\WutheringWaves\\Client\\'),
    'C:/Games/WutheringWaves/Client',
  );
  assert.equal(parentDir('C:\\Games\\WutheringWaves\\Client.exe'), 'C:/Games/WutheringWaves');
  assert.equal(parentDir('Client.exe'), '');
});

test('resolveGameUpdateFolder prefers saved folder over all fallbacks', () => {
  const folder = resolveGameUpdateFolder(
    ' /games/custom-wuwa ',
    '/games/WutheringWaves/Client.exe',
    '/games',
    'WutheringWaves',
  );

  assert.equal(folder, '/games/custom-wuwa');
});

test('resolveGameUpdateFolder falls back to parent of saved executable path', () => {
  const folder = resolveGameUpdateFolder(
    '',
    '/games/WutheringWaves/Client.exe',
    '/games',
    'WutheringWaves',
  );

  assert.equal(folder, '/games/WutheringWaves');
});

test('resolveGameUpdateFolder finally builds default folder from base dir and launcher folder', () => {
  assert.equal(
    resolveGameUpdateFolder('', '', '/games', '/WutheringWaves/'),
    '/games/WutheringWaves',
  );
  assert.equal(
    resolveGameUpdateFolder('', '', '/games', ''),
    '/games',
  );
  assert.equal(
    resolveGameUpdateFolder('', '', '', '/WutheringWaves/'),
    '',
  );
});

test('resolveGameUpdateSource keeps saved launcher api and preserves matched biz prefix', () => {
  const source = resolveGameUpdateSource(
    ' https://api.example/global ',
    [
      { launcherApi: 'https://api.example/cn', bizPrefix: 'cn' },
      { launcherApi: 'https://api.example/global', bizPrefix: 'global' },
    ],
    'https://fallback.example',
  );

  assert.deepEqual(source, {
    launcherApi: 'https://api.example/global',
    bizPrefix: 'global',
  });
});

test('resolveGameUpdateSource falls back to first server then top-level launcher api', () => {
  assert.deepEqual(
    resolveGameUpdateSource('', [
      { launcherApi: 'https://api.example/cn', bizPrefix: ' cn ' },
    ], 'https://fallback.example'),
    {
      launcherApi: 'https://api.example/cn',
      bizPrefix: 'cn',
    },
  );

  assert.deepEqual(
    resolveGameUpdateSource('', [], ' https://fallback.example '),
    {
      launcherApi: 'https://fallback.example',
      bizPrefix: undefined,
    },
  );

  assert.equal(resolveGameUpdateSource('', [], '   '), null);
});
