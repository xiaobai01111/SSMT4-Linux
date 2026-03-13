import { onScopeDispose, ref, watch } from 'vue';
import type { RouteLocationNormalizedLoaded } from 'vue-router';
import type { VersionCheckInfo } from '../api';
import { resolveSettingsRouteMenuState } from '../utils/settingsMenuRoute';

type Toast = (
  kind: 'success' | 'warning' | 'info' | 'error',
  title: string,
  message: string,
) => Promise<void>;
type Translate = (key: string, fallback: string) => string;

interface SettingsPageControllerOptions {
  route: RouteLocationNormalizedLoaded;
  tr: Translate;
  toast: Toast;
  getVersionCheckInfo: () => Promise<VersionCheckInfo>;
  getResourceVersionInfo: () => Promise<VersionCheckInfo>;
  pullResourceUpdates: () => Promise<string>;
  reenterOnboarding: () => void;
  globalMigotoEnabled: () => boolean;
  loadProton: () => Promise<void>;
  loadDxvk: () => Promise<void>;
  loadVkd3d: () => Promise<void>;
  loadMigoto: () => Promise<void>;
  setProtonEditorCollapsed: () => void;
}

export function useSettingsPageController(
  options: SettingsPageControllerOptions,
) {
  const activeMenu = ref('basic');
  const guideMenu = ref('');
  let guideMenuTimer: ReturnType<typeof setTimeout> | null = null;

  const versionInfo = ref<VersionCheckInfo | null>(null);
  const isVersionChecking = ref(false);
  const versionCheckLoaded = ref(false);
  const resourceInfo = ref<VersionCheckInfo | null>(null);
  const isResourceChecking = ref(false);
  const isResourcePulling = ref(false);
  const resourceCheckLoaded = ref(false);

  const protonLoaded = ref(false);
  const dxvkLoaded = ref(false);
  const vkd3dLoaded = ref(false);

  const applyMenuFromRoute = () => {
    const routeState = resolveSettingsRouteMenuState(
      options.route.query.menu,
      options.route.query.guide,
    );
    if (!routeState.activeMenu) {
      return;
    }
    activeMenu.value = routeState.activeMenu;
    if (!routeState.guideMenu) {
      return;
    }
    guideMenu.value = routeState.guideMenu;
    if (guideMenuTimer) {
      clearTimeout(guideMenuTimer);
    }
    guideMenuTimer = setTimeout(() => {
      guideMenu.value = '';
      guideMenuTimer = null;
    }, 2600);
  };

  const checkVersionInfo = async () => {
    if (isVersionChecking.value) return;
    try {
      isVersionChecking.value = true;
      versionInfo.value = await options.getVersionCheckInfo();
      versionCheckLoaded.value = true;
    } catch (e) {
      await options.toast(
        'error',
        options.tr('settings.messages.title.error', '错误'),
        options.tr('settings.messages.versionCheckFailed', `版本检查失败: ${e}`),
      );
    } finally {
      isVersionChecking.value = false;
    }
  };

  const checkResourceInfo = async () => {
    if (isResourceChecking.value) return;
    try {
      isResourceChecking.value = true;
      resourceInfo.value = await options.getResourceVersionInfo();
      resourceCheckLoaded.value = true;
    } catch (e) {
      await options.toast(
        'error',
        options.tr('settings.messages.title.error', '错误'),
        options.tr(
          'settings.messages.resourceCheckFailed',
          `资源检查失败: ${e}`,
        ),
      );
    } finally {
      isResourceChecking.value = false;
    }
  };

  const pullResources = async () => {
    if (isResourcePulling.value) return;
    try {
      isResourcePulling.value = true;
      const msg = await options.pullResourceUpdates();
      await checkResourceInfo();
      await options.toast(
        'info',
        options.tr('settings.resource.title', '资源更新'),
        msg,
      );
    } catch (e) {
      await options.toast(
        'error',
        options.tr('settings.messages.title.error', '错误'),
        options.tr(
          'settings.messages.resourcePullFailed',
          `拉取资源失败: ${e}`,
        ),
      );
    } finally {
      isResourcePulling.value = false;
    }
  };

  const reenterOnboarding = async () => {
    options.reenterOnboarding();
  };

  watch(
    () => [options.route.query.menu, options.route.query.guide, options.route.query.t],
    () => applyMenuFromRoute(),
    { immediate: true },
  );

  watch(
    () => [activeMenu.value, options.globalMigotoEnabled()] as const,
    async ([menu, migotoEnabled]) => {
      if (menu === 'version' && !versionCheckLoaded.value) {
        await checkVersionInfo();
      }
      if (menu === 'resource' && !resourceCheckLoaded.value) {
        await checkResourceInfo();
      }
      if (menu === 'proton' && !protonLoaded.value) {
        await options.loadProton();
        protonLoaded.value = true;
      }
      if (menu === 'proton') {
        options.setProtonEditorCollapsed();
      }
      if (menu === 'dxvk' && !dxvkLoaded.value) {
        await options.loadDxvk();
        dxvkLoaded.value = true;
      }
      if (menu === 'vkd3d' && !vkd3dLoaded.value) {
        await options.loadVkd3d();
        vkd3dLoaded.value = true;
      }
      if (menu === 'migoto' && migotoEnabled) {
        await options.loadMigoto();
      }
    },
    { immediate: true },
  );

  onScopeDispose(() => {
    if (guideMenuTimer) {
      clearTimeout(guideMenuTimer);
      guideMenuTimer = null;
    }
  });

  return {
    activeMenu,
    guideMenu,
    versionInfo,
    isVersionChecking,
    versionCheckLoaded,
    resourceInfo,
    isResourceChecking,
    isResourcePulling,
    resourceCheckLoaded,
    protonLoaded,
    dxvkLoaded,
    vkd3dLoaded,
    applyMenuFromRoute,
    checkVersionInfo,
    checkResourceInfo,
    pullResources,
    reenterOnboarding,
  };
}
