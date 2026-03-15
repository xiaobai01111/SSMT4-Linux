import { onMounted, onScopeDispose, ref, watch } from 'vue';
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
  const idleCallbackIds: number[] = [];
  const idleTimeoutIds: ReturnType<typeof setTimeout>[] = [];

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
  const migotoLoaded = ref(false);

  let versionLoadPromise: Promise<void> | null = null;
  let resourceLoadPromise: Promise<void> | null = null;
  let protonLoadPromise: Promise<void> | null = null;
  let dxvkLoadPromise: Promise<void> | null = null;
  let vkd3dLoadPromise: Promise<void> | null = null;
  let migotoLoadPromise: Promise<void> | null = null;

  const waitNextFrame = () =>
    new Promise<void>((resolve) => {
      if (
        typeof window !== 'undefined' &&
        typeof window.requestAnimationFrame === 'function'
      ) {
        window.requestAnimationFrame(() => resolve());
        return;
      }
      setTimeout(resolve, 0);
    });

  const scheduleIdleTask = (task: () => Promise<void> | void) => {
    if (
      typeof window !== 'undefined' &&
      typeof window.requestIdleCallback === 'function'
    ) {
      const id = window.requestIdleCallback(() => {
        void task();
      }, { timeout: 1600 });
      idleCallbackIds.push(id);
      return;
    }
    const id = setTimeout(() => {
      void task();
    }, 700);
    idleTimeoutIds.push(id);
  };

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

  const ensureVersionLoaded = async () => {
    if (versionCheckLoaded.value) return;
    if (!versionLoadPromise) {
      versionLoadPromise = checkVersionInfo().finally(() => {
        versionLoadPromise = null;
      });
    }
    await versionLoadPromise;
  };

  const ensureResourceLoaded = async () => {
    if (resourceCheckLoaded.value) return;
    if (!resourceLoadPromise) {
      resourceLoadPromise = checkResourceInfo().finally(() => {
        resourceLoadPromise = null;
      });
    }
    await resourceLoadPromise;
  };

  const ensureProtonLoaded = async () => {
    if (protonLoaded.value) return;
    if (!protonLoadPromise) {
      protonLoadPromise = options.loadProton().then(() => {
        protonLoaded.value = true;
      }).finally(() => {
        protonLoadPromise = null;
      });
    }
    await protonLoadPromise;
  };

  const ensureDxvkLoaded = async () => {
    if (dxvkLoaded.value) return;
    if (!dxvkLoadPromise) {
      dxvkLoadPromise = options.loadDxvk().then(() => {
        dxvkLoaded.value = true;
      }).finally(() => {
        dxvkLoadPromise = null;
      });
    }
    await dxvkLoadPromise;
  };

  const ensureVkd3dLoaded = async () => {
    if (vkd3dLoaded.value) return;
    if (!vkd3dLoadPromise) {
      vkd3dLoadPromise = options.loadVkd3d().then(() => {
        vkd3dLoaded.value = true;
      }).finally(() => {
        vkd3dLoadPromise = null;
      });
    }
    await vkd3dLoadPromise;
  };

  const ensureMigotoLoaded = async () => {
    if (migotoLoaded.value) return;
    if (!migotoLoadPromise) {
      migotoLoadPromise = options.loadMigoto().then(() => {
        migotoLoaded.value = true;
      }).finally(() => {
        migotoLoadPromise = null;
      });
    }
    await migotoLoadPromise;
  };

  const warmMenuData = async (menu: string, migotoEnabled: boolean) => {
    await waitNextFrame();

    if (menu === 'version') {
      await ensureVersionLoaded();
      return;
    }
    if (menu === 'resource') {
      await ensureResourceLoaded();
      return;
    }
    if (menu === 'proton') {
      await ensureProtonLoaded();
      return;
    }
    if (menu === 'dxvk') {
      await ensureDxvkLoaded();
      return;
    }
    if (menu === 'vkd3d') {
      await ensureVkd3dLoaded();
      return;
    }
    if (menu === 'migoto' && migotoEnabled) {
      await ensureMigotoLoaded();
    }
  };

  watch(
    () => [options.route.query.menu, options.route.query.guide, options.route.query.t],
    () => applyMenuFromRoute(),
    { immediate: true },
  );

  watch(
    () => [activeMenu.value, options.globalMigotoEnabled()] as const,
    ([menu, migotoEnabled]) => {
      if (menu === 'proton') {
        options.setProtonEditorCollapsed();
      }
      void warmMenuData(menu, migotoEnabled);
    },
    { immediate: true },
  );

  onMounted(() => {
    const backgroundTasks = [
      () => ensureProtonLoaded(),
      () => ensureDxvkLoaded(),
      () => ensureVkd3dLoaded(),
      () =>
        options.globalMigotoEnabled() ? ensureMigotoLoaded() : Promise.resolve(),
    ];

    let taskIndex = 0;
    const runNext = () => {
      if (taskIndex >= backgroundTasks.length) return;
      scheduleIdleTask(async () => {
        const currentTask = backgroundTasks[taskIndex];
        taskIndex += 1;
        await currentTask();
        runNext();
      });
    };

    runNext();
  });

  onScopeDispose(() => {
    if (guideMenuTimer) {
      clearTimeout(guideMenuTimer);
      guideMenuTimer = null;
    }
    if (
      typeof window !== 'undefined' &&
      typeof window.cancelIdleCallback === 'function'
    ) {
      idleCallbackIds.forEach((id) => window.cancelIdleCallback(id));
    }
    idleTimeoutIds.forEach((id) => clearTimeout(id));
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
    migotoLoaded,
    applyMenuFromRoute,
    checkVersionInfo,
    checkResourceInfo,
    pullResources,
    reenterOnboarding,
  };
}
