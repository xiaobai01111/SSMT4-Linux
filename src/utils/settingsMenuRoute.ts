export const SETTINGS_VALID_MENUS = new Set([
  'basic',
  'appearance',
  'display',
  'version',
  'resource',
  'proton',
  'dxvk',
  'vkd3d',
  'migoto',
]);

export interface SettingsRouteMenuState {
  activeMenu: string | null;
  guideMenu: string | null;
}

export const resolveSettingsRouteMenuState = (
  menu: unknown,
  guide: unknown,
): SettingsRouteMenuState => {
  const normalizedMenu = String(menu || '').trim();
  const normalizedGuide = String(guide || '').trim();
  if (!SETTINGS_VALID_MENUS.has(normalizedMenu)) {
    return {
      activeMenu: null,
      guideMenu: null,
    };
  }
  return {
    activeMenu: normalizedMenu,
    guideMenu: normalizedGuide === '1' ? normalizedMenu : null,
  };
};
