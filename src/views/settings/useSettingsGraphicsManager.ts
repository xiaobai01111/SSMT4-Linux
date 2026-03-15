import { computed, reactive, ref, shallowRef } from 'vue';
import {
  deleteLocalDxvk,
  deleteLocalVkd3d,
  downloadDxvk,
  downloadVkd3d,
  fetchDxvkVersions,
  fetchVkd3dVersions,
  scanLocalDxvk,
  scanLocalVkd3d,
  type DxvkLocalVersion,
  type DxvkRemoteVersion,
  type Vkd3dLocalVersion,
  type Vkd3dRemoteVersion,
} from '../../api';

type TaskMessage<T> = string | ((value: T) => string);
type TranslateFn = (
  key: string,
  fallback: string,
  params?: Record<string, unknown>,
) => string;
type ToastFn = (
  kind: 'success' | 'warning' | 'info' | 'error',
  title: string,
  message: string,
) => Promise<void>;

type RunDownloadTask = <T>(options: {
  taskId: string;
  title: string;
  pendingMessage?: string;
  componentKey?: string;
  run: () => Promise<T>;
  successMessage: TaskMessage<T>;
  errorMessage: TaskMessage<unknown>;
  refresh?: () => Promise<void>;
}) => Promise<T>;

type RunDeleteTask = <T>(options: {
  taskId: string;
  title?: string;
  run: () => Promise<T>;
  successMessage: TaskMessage<T>;
  errorMessage: TaskMessage<unknown>;
  refresh?: () => Promise<void>;
}) => Promise<T>;

export interface DxvkVersionItem {
  version: string;
  variant: string;
  key: string;
  isLocal: boolean;
  isRemote: boolean;
  fileSize: number;
  publishedAt: string;
}

export interface Vkd3dVersionItem {
  version: string;
  isLocal: boolean;
  isRemote: boolean;
  fileSize: number;
  publishedAt: string;
}

export function useSettingsGraphicsManager({
  tr,
  toast,
  runDownloadTask,
  runDeleteTask,
}: {
  tr: TranslateFn;
  toast: ToastFn;
  runDownloadTask: RunDownloadTask;
  runDeleteTask: RunDeleteTask;
}) {
  const dxvkLocalVersions = shallowRef<DxvkLocalVersion[]>([]);
  const dxvkRemoteVersions = shallowRef<DxvkRemoteVersion[]>([]);
  const dxvkSelectedKey = ref('');
  const isDxvkFetching = ref(false);
  const isDxvkDownloading = ref(false);
  const dxvkFetchWarning = ref('');
  const deletingDxvkKeys = reactive<Record<string, boolean>>({});

  const vkd3dLocalVersions = shallowRef<Vkd3dLocalVersion[]>([]);
  const vkd3dRemoteVersions = shallowRef<Vkd3dRemoteVersion[]>([]);
  const vkd3dSelectedVersion = ref('');
  const isVkd3dFetching = ref(false);
  const isVkd3dDownloading = ref(false);
  const vkd3dFetchWarning = ref('');
  const deletingVkd3dVersions = reactive<Record<string, boolean>>({});

  const dxvkVersionList = computed<DxvkVersionItem[]>(() => {
    const map = new Map<string, DxvkVersionItem>();

    for (const rv of dxvkRemoteVersions.value) {
      const key = `${rv.version}|${rv.variant}`;
      map.set(key, {
        version: rv.version,
        variant: rv.variant,
        key,
        isLocal: rv.is_local,
        isRemote: true,
        fileSize: rv.file_size,
        publishedAt: rv.published_at,
      });
    }

    for (const lv of dxvkLocalVersions.value) {
      const key = `${lv.version}|${lv.variant}`;
      if (!map.has(key)) {
        map.set(key, {
          version: lv.version,
          variant: lv.variant,
          key,
          isLocal: true,
          isRemote: false,
          fileSize: 0,
          publishedAt: '',
        });
      }
    }

    return Array.from(map.values()).sort((a, b) => {
      const cmp = b.version.localeCompare(a.version);
      return cmp !== 0 ? cmp : a.variant.localeCompare(b.variant);
    });
  });

  const selectedDxvkItem = computed(() =>
    dxvkVersionList.value.find((v) => v.key === dxvkSelectedKey.value),
  );

  const dxvkVariantLabel = (variant: string) => {
    const labels: Record<string, string> = {
      dxvk: tr('settings.dxvk_variant_official', 'Official DXVK'),
      gplasync: 'DXVK-GPLAsync',
      async: 'DXVK-Async',
      sarek: 'DXVK-Sarek',
      sarekasync: 'DXVK-Sarek-Async',
    };
    return labels[variant] || `DXVK-${variant}`;
  };

  const dxvkGroupedList = computed(() => {
    const groups = new Map<string, DxvkVersionItem[]>();
    for (const item of dxvkVersionList.value) {
      const list = groups.get(item.variant) || [];
      list.push(item);
      groups.set(item.variant, list);
    }
    return Array.from(groups.entries())
      .sort((a, b) => {
        if (a[0] === 'dxvk') return -1;
        if (b[0] === 'dxvk') return 1;
        return a[0].localeCompare(b[0]);
      })
      .map(([variant, items]) => ({
        variant,
        label: dxvkVariantLabel(variant),
        items,
      }));
  });

  const refreshDxvkLocal = async () => {
    try {
      dxvkLocalVersions.value = await scanLocalDxvk();
    } catch (e) {
      console.warn('[dxvk] 扫描本地版本失败:', e);
    }
  };

  const refreshDxvkRemote = async () => {
    if (isDxvkFetching.value) return;
    dxvkFetchWarning.value = '';
    try {
      isDxvkFetching.value = true;
      dxvkRemoteVersions.value = await fetchDxvkVersions();
      if (!dxvkSelectedKey.value && dxvkRemoteVersions.value.length > 0) {
        const first = dxvkRemoteVersions.value[0];
        dxvkSelectedKey.value = `${first.version}|${first.variant}`;
      }
      if (dxvkRemoteVersions.value.length === 0) {
        dxvkFetchWarning.value = tr(
          'settings.messages.dxvkFetchWarning',
          '未获取到远程版本，请稍后重试。',
        );
      }
    } catch (e) {
      await toast(
        'error',
        tr('settings.messages.title.error', '错误'),
        tr('settings.messages.dxvkFetchFailed', `获取 DXVK 版本列表失败: ${e}`).replace(
          '{error}',
          String(e),
        ),
      );
    } finally {
      isDxvkFetching.value = false;
    }
  };

  const refreshDxvkState = async () => {
    const [, remote] = await Promise.all([refreshDxvkLocal(), fetchDxvkVersions()]);
    dxvkRemoteVersions.value = remote;
  };

  const doDownloadDxvk = async () => {
    const item = selectedDxvkItem.value;
    if (isDxvkDownloading.value || !item) return;
    const label = dxvkVariantLabel(item.variant);
    const taskId = `settings-dxvk-download-${item.variant}-${item.version}`;
    const componentKey = `dxvk:${item.variant}:${item.version}`;
    try {
      isDxvkDownloading.value = true;
      await runDownloadTask({
        taskId,
        componentKey,
        title: tr('settings.messages.downloadLabelTitle', `下载 ${label}`).replace(
          '{label}',
          label,
        ),
        pendingMessage: tr(
          'settings.messages.downloadLabelBody',
          `正在下载 ${label} ${item.version}，请稍候...`,
        )
          .replace('{label}', label)
          .replace('{version}', item.version),
        run: () => downloadDxvk(item.version, item.variant),
        successMessage: tr(
          'settings.messages.downloadLabelDone',
          `${label} ${item.version} 下载完成`,
        )
          .replace('{label}', label)
          .replace('{version}', item.version),
        errorMessage: (e) =>
          tr('settings.messages.downloadFailed', `下载失败: ${e}`).replace(
            '{error}',
            String(e),
          ),
        refresh: refreshDxvkState,
      });
    } finally {
      isDxvkDownloading.value = false;
    }
  };

  const dxvkLocalCount = computed(() => dxvkLocalVersions.value.length);

  const removeLocalDxvkItem = async (version: string, variant: string) => {
    const key = `${version}|${variant}`;
    if (deletingDxvkKeys[key]) return;
    const label = dxvkVariantLabel(variant);
    const taskId = `settings-dxvk-delete-${variant}-${version}`;
    const target = `${label} ${version}`;
    try {
      deletingDxvkKeys[key] = true;
      await runDeleteTask({
        taskId,
        run: () => deleteLocalDxvk(version, variant),
        successMessage: tr(
          'settings.messages.deleteTargetDone',
          `${target} 已删除`,
        ).replace('{target}', target),
        errorMessage: (e) =>
          tr('settings.messages.deleteFailed', `删除失败: ${e}`).replace(
            '{error}',
            String(e),
          ),
        refresh: refreshDxvkState,
      });
    } finally {
      deletingDxvkKeys[key] = false;
    }
  };

  const vkd3dVersionList = computed<Vkd3dVersionItem[]>(() => {
    const map = new Map<string, Vkd3dVersionItem>();

    for (const rv of vkd3dRemoteVersions.value) {
      map.set(rv.version, {
        version: rv.version,
        isLocal: rv.is_local,
        isRemote: true,
        fileSize: rv.file_size,
        publishedAt: rv.published_at,
      });
    }

    for (const lv of vkd3dLocalVersions.value) {
      if (!map.has(lv.version)) {
        map.set(lv.version, {
          version: lv.version,
          isLocal: true,
          isRemote: false,
          fileSize: 0,
          publishedAt: '',
        });
      }
    }

    return Array.from(map.values()).sort((a, b) =>
      b.version.localeCompare(a.version),
    );
  });

  const selectedVkd3dItem = computed(() =>
    vkd3dVersionList.value.find((v) => v.version === vkd3dSelectedVersion.value),
  );

  const refreshVkd3dLocal = async () => {
    try {
      vkd3dLocalVersions.value = await scanLocalVkd3d();
    } catch (e) {
      console.warn('[vkd3d] 扫描本地版本失败:', e);
    }
  };

  const refreshVkd3dRemote = async () => {
    if (isVkd3dFetching.value) return;
    vkd3dFetchWarning.value = '';
    try {
      isVkd3dFetching.value = true;
      vkd3dRemoteVersions.value = await fetchVkd3dVersions();
      if (!vkd3dSelectedVersion.value && vkd3dRemoteVersions.value.length > 0) {
        vkd3dSelectedVersion.value = vkd3dRemoteVersions.value[0].version;
      }
      if (vkd3dRemoteVersions.value.length === 0) {
        vkd3dFetchWarning.value = tr(
          'settings.messages.vkd3dFetchWarning',
          '未获取到远程版本，请稍后重试。',
        );
      }
    } catch (e) {
      await toast(
        'error',
        tr('settings.messages.title.error', '错误'),
        tr('settings.messages.vkd3dFetchFailed', `获取 VKD3D 版本列表失败: ${e}`).replace(
          '{error}',
          String(e),
        ),
      );
    } finally {
      isVkd3dFetching.value = false;
    }
  };

  const refreshVkd3dState = async () => {
    const [, remote] = await Promise.all([
      refreshVkd3dLocal(),
      fetchVkd3dVersions(),
    ]);
    vkd3dRemoteVersions.value = remote;
  };

  const doDownloadVkd3d = async () => {
    const item = selectedVkd3dItem.value;
    if (isVkd3dDownloading.value || !item) return;
    const taskId = `settings-vkd3d-download-${item.version}`;
    const label = 'VKD3D-Proton';
    const componentKey = `vkd3d:${item.version}`;
    try {
      isVkd3dDownloading.value = true;
      await runDownloadTask({
        taskId,
        componentKey,
        title: tr('settings.messages.downloadLabelTitle', `下载 ${label}`).replace(
          '{label}',
          label,
        ),
        pendingMessage: tr(
          'settings.messages.downloadLabelBody',
          `正在下载 ${label} ${item.version}，请稍候...`,
        )
          .replace('{label}', label)
          .replace('{version}', item.version),
        run: () => downloadVkd3d(item.version),
        successMessage: tr(
          'settings.messages.downloadLabelDone',
          `${label} ${item.version} 下载完成`,
        )
          .replace('{label}', label)
          .replace('{version}', item.version),
        errorMessage: (e) =>
          tr('settings.messages.downloadFailed', `下载失败: ${e}`).replace(
            '{error}',
            String(e),
          ),
        refresh: refreshVkd3dState,
      });
    } finally {
      isVkd3dDownloading.value = false;
    }
  };

  const vkd3dLocalCount = computed(() => vkd3dLocalVersions.value.length);

  const removeLocalVkd3dItem = async (version: string) => {
    if (deletingVkd3dVersions[version]) return;
    const taskId = `settings-vkd3d-delete-${version}`;
    const target = `VKD3D-Proton ${version}`;
    try {
      deletingVkd3dVersions[version] = true;
      await runDeleteTask({
        taskId,
        run: () => deleteLocalVkd3d(version),
        successMessage: tr(
          'settings.messages.deleteTargetDone',
          `${target} 已删除`,
        ).replace('{target}', target),
        errorMessage: (e) =>
          tr('settings.messages.deleteFailed', `删除失败: ${e}`).replace(
            '{error}',
            String(e),
          ),
        refresh: refreshVkd3dState,
      });
    } finally {
      deletingVkd3dVersions[version] = false;
    }
  };

  return {
    dxvkLocalVersions,
    dxvkSelectedKey,
    isDxvkFetching,
    isDxvkDownloading,
    dxvkFetchWarning,
    deletingDxvkKeys,
    dxvkGroupedList,
    selectedDxvkItem,
    refreshDxvkLocal,
    refreshDxvkRemote,
    doDownloadDxvk,
    dxvkLocalCount,
    removeLocalDxvkItem,
    vkd3dLocalVersions,
    vkd3dSelectedVersion,
    isVkd3dFetching,
    isVkd3dDownloading,
    vkd3dFetchWarning,
    deletingVkd3dVersions,
    vkd3dVersionList,
    selectedVkd3dItem,
    refreshVkd3dLocal,
    refreshVkd3dRemote,
    doDownloadVkd3d,
    vkd3dLocalCount,
    removeLocalVkd3dItem,
  };
}
