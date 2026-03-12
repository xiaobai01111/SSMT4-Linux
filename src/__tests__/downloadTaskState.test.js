import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildFailedFilesPreviewSection,
  buildTaskKey,
  buildVerifyFailureKey,
  isCancellationLikeError,
  phaseFromProgressOperation,
  phaseFromTaskOperation,
  taskFailureMessage,
  taskFailureTitle,
} from '../utils/downloadTaskState.ts';

test('phase mapping keeps verify and repair in verifying phase', () => {
  assert.equal(phaseFromTaskOperation('verify_game'), 'verifying');
  assert.equal(phaseFromTaskOperation('repair_game'), 'verifying');
  assert.equal(phaseFromTaskOperation('update_game'), 'downloading');

  assert.equal(phaseFromProgressOperation('verify-game'), 'verifying');
  assert.equal(phaseFromProgressOperation('repair-game'), 'verifying');
  assert.equal(phaseFromProgressOperation('update-game-patch'), 'downloading');
});

test('buildTaskKey includes source, langs and repair file scope', () => {
  const key = buildTaskKey({
    operation: 'repair_game',
    gameName: 'WutheringWaves',
    gameFolder: '/games/wuwa',
    launcherApi: 'https://kuro.example/api',
    bizPrefix: 'cn',
    gamePreset: 'WutheringWaves',
    languages: ['zh-cn', 'ja-jp'],
    repairFiles: ['A.pak', 'B.pak'],
  });

  assert.equal(
    key,
    'repair_game::WutheringWaves::/games/wuwa::https://kuro.example/api::cn::WutheringWaves::zh-cn,ja-jp::A.pak,B.pak',
  );
});

test('buildVerifyFailureKey stays scoped by source and biz prefix', () => {
  assert.equal(
    buildVerifyFailureKey('ZZZ', '/games/zzz', 'https://api.example', 'global'),
    'ZZZ::/games/zzz::https://api.example::global',
  );
  assert.equal(
    buildVerifyFailureKey('ZZZ', '/games/zzz', 'https://api.example'),
    'ZZZ::/games/zzz::https://api.example::',
  );
});

test('task failure message and title match operation families', () => {
  assert.equal(taskFailureTitle('download_game'), '下载错误');
  assert.equal(taskFailureTitle('verify_game'), '校验错误');
  assert.equal(taskFailureTitle('repair_game'), '修复错误');

  assert.equal(taskFailureMessage('download_game', 'boom'), '下载失败: boom');
  assert.equal(
    taskFailureMessage('download_launcher_installer', 'boom'),
    '下载启动器失败: boom',
  );
  assert.equal(taskFailureMessage('verify_game', 'boom'), '校验失败: boom');
  assert.equal(taskFailureMessage('repair_game', 'boom'), '修复失败: boom');
});

test('cancellation detection recognizes english and chinese markers', () => {
  assert.equal(isCancellationLikeError('request cancelled by user'), true);
  assert.equal(isCancellationLikeError(new Error('operation aborted')), true);
  assert.equal(isCancellationLikeError('用户已取消下载'), true);
  assert.equal(isCancellationLikeError('network timeout'), false);
});

test('failed files preview truncates and annotates overflow', () => {
  const preview = buildFailedFilesPreviewSection(
    ['a', 'b', 'c'],
    '异常文件',
    2,
  );

  assert.equal(preview, '\n\n异常文件（部分）：\na\nb\n... 还有 1 个文件未展示');
  assert.equal(buildFailedFilesPreviewSection([], '异常文件'), '');
});
