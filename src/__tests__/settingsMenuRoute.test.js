import test from 'node:test';
import assert from 'node:assert/strict';
import { resolveSettingsRouteMenuState } from '../utils/settingsMenuRoute.ts';

test('resolveSettingsRouteMenuState returns menu and guide menu for valid guide request', () => {
  const result = resolveSettingsRouteMenuState('resource', '1');

  assert.deepEqual(result, {
    activeMenu: 'resource',
    guideMenu: 'resource',
  });
});

test('resolveSettingsRouteMenuState rejects invalid menus', () => {
  const result = resolveSettingsRouteMenuState('unknown', '1');

  assert.deepEqual(result, {
    activeMenu: null,
    guideMenu: null,
  });
});

test('resolveSettingsRouteMenuState keeps active menu without guide highlight', () => {
  const result = resolveSettingsRouteMenuState('proton', '0');

  assert.deepEqual(result, {
    activeMenu: 'proton',
    guideMenu: null,
  });
});
