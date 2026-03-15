import { computed, ref, shallowRef, watch } from 'vue';
import {
  deleteLocalXxmiPackage,
  deployXxmiPackage,
  downloadXxmiPackage,
  fetchXxmiRemoteVersions,
  getXxmiPackageSources,
  scanLocalXxmiPackages,
  type XxmiLocalPackage,
  type XxmiPackageSource,
  type XxmiRemoteVersion,
} from '../../api';

type TranslateFn = (
  key: string,
  fallback: string,
  params?: Record<string, unknown>,
) => string;

export function useSettingsXxmiManager({
  tr,
  getSelectedGame,
  getDeployTargetDir,
}: {
  tr: TranslateFn;
  getSelectedGame: () => string;
  getDeployTargetDir: () => string;
}) {
  const xxmiSources = shallowRef<XxmiPackageSource[]>([]);
  const xxmiSelectedSource = ref('xxmi-libs');
  const xxmiRemoteVersions = shallowRef<XxmiRemoteVersion[]>([]);
  const xxmiLocalPackages = shallowRef<XxmiLocalPackage[]>([]);
  const isXxmiFetching = ref(false);
  const isXxmiDownloading = ref(false);
  const xxmiDownloadingVersion = ref('');
  const xxmiMessage = ref('');
  const xxmiMessageType = ref<'success' | 'error' | ''>('');

  const syncXxmiInstalledState = (versions: XxmiRemoteVersion[]) => {
    const installedSet = new Set(
      xxmiLocalPackages.value.map((item) => `${item.source_id}|${item.version}`),
    );
    return versions.map((item) => ({
      ...item,
      installed: installedSet.has(`${item.source_id}|${item.version}`),
    }));
  };

  const loadXxmiSources = async () => {
    try {
      xxmiSources.value = await getXxmiPackageSources();
    } catch (e) {
      console.warn('[xxmi] 获取包源列表失败:', e);
    }
  };

  const refreshXxmiLocal = async () => {
    try {
      const status = await scanLocalXxmiPackages();
      xxmiLocalPackages.value = status.packages;
      xxmiRemoteVersions.value = syncXxmiInstalledState(xxmiRemoteVersions.value);
    } catch (e) {
      console.warn('[xxmi] 扫描本地包失败:', e);
    }
  };

  const refreshXxmiRemote = async () => {
    if (isXxmiFetching.value) return;
    try {
      isXxmiFetching.value = true;
      xxmiMessage.value = '';
      xxmiRemoteVersions.value = syncXxmiInstalledState(
        await fetchXxmiRemoteVersions(xxmiSelectedSource.value),
      );
    } catch (e) {
      xxmiMessage.value = tr(
        'settings.messages.xxmiFetchFailed',
        `获取远程版本失败: ${e}`,
      ).replace('{error}', String(e));
      xxmiMessageType.value = 'error';
    } finally {
      isXxmiFetching.value = false;
    }
  };

  const refreshXxmiState = async () => {
    await refreshXxmiLocal();
  };

  const runXxmiAction = async <T>({
    pendingMessage,
    run,
    successMessage,
    errorMessage,
    refresh,
  }: {
    pendingMessage: string;
    run: () => Promise<T>;
    successMessage: (result: T) => string;
    errorMessage: (error: unknown) => string;
    refresh?: () => Promise<void>;
  }) => {
    try {
      xxmiMessage.value = pendingMessage;
      xxmiMessageType.value = '';
      const result = await run();
      xxmiMessage.value = successMessage(result);
      xxmiMessageType.value = 'success';
      if (refresh) {
        await refresh();
      }
      return result;
    } catch (e) {
      xxmiMessage.value = errorMessage(e);
      xxmiMessageType.value = 'error';
      throw e;
    }
  };

  const doDownloadXxmi = async (ver: XxmiRemoteVersion) => {
    if (isXxmiDownloading.value) return;
    try {
      isXxmiDownloading.value = true;
      xxmiDownloadingVersion.value = ver.version;
      await runXxmiAction({
        pendingMessage: tr(
          'settings.messages.xxmiDownloading',
          `正在下载 ${ver.source_name} ${ver.version}...`,
        )
          .replace('{source}', ver.source_name)
          .replace('{version}', ver.version),
        run: () =>
          downloadXxmiPackage(ver.source_id, ver.version, ver.download_url),
        successMessage: (msg) => msg,
        errorMessage: (e) =>
          tr('settings.messages.downloadFailed', `下载失败: ${e}`).replace(
            '{error}',
            String(e),
          ),
        refresh: refreshXxmiState,
      });
    } finally {
      isXxmiDownloading.value = false;
      xxmiDownloadingVersion.value = '';
    }
  };

  const doDeployXxmi = async (pkg: XxmiLocalPackage) => {
    const targetDir = getDeployTargetDir();
    if (!targetDir) {
      xxmiMessage.value = tr(
        'settings.migoto.xxmiDeployNoTarget',
        '未找到部署目标目录',
      );
      xxmiMessageType.value = 'error';
      return;
    }
    await runXxmiAction({
      pendingMessage: tr('settings.messages.taskDeploying', '正在部署，请稍候...'),
      run: () => deployXxmiPackage(pkg.source_id, pkg.version, targetDir),
      successMessage: (msg) => msg,
      errorMessage: (e) =>
        tr('settings.messages.xxmiDeployFailed', `部署失败: ${e}`).replace(
          '{error}',
          String(e),
        ),
    });
  };

  const doDeleteXxmi = async (pkg: XxmiLocalPackage) => {
    await runXxmiAction({
      pendingMessage: tr('settings.messages.taskDeleting', '正在删除，请稍候...'),
      run: () => deleteLocalXxmiPackage(pkg.source_id, pkg.version),
      successMessage: (msg) => msg,
      errorMessage: (e) =>
        tr('settings.messages.xxmiDeleteFailed', `删除失败: ${e}`).replace(
          '{error}',
          String(e),
        ),
      refresh: refreshXxmiState,
    });
  };

  watch(
    () => xxmiSelectedSource.value,
    () => {
      xxmiRemoteVersions.value = [];
      xxmiMessage.value = '';
      xxmiMessageType.value = '';
    },
  );

  const xxmiFilteredLocal = computed(() =>
    xxmiLocalPackages.value.filter((p) => p.source_id === xxmiSelectedSource.value),
  );

  const gameToXxmiSourceMap: Record<string, string> = {
    WutheringWaves: 'wwmi',
    ZenlessZoneZero: 'zzmi',
    HonkaiStarRail: 'srmi',
    GenshinImpact: 'gimi',
    Genshin: 'gimi',
    HonkaiImpact3rd: 'himi',
    Honkai3rd: 'himi',
    ArknightsEndfield: 'efmi',
  };

  const xxmiFilteredSources = computed(() => {
    const game = getSelectedGame();
    if (!game) return xxmiSources.value;
    const gameSourceId = gameToXxmiSourceMap[game];
    if (!gameSourceId) return xxmiSources.value;
    return xxmiSources.value.filter(
      (s) => s.id === 'xxmi-libs' || s.id === gameSourceId,
    );
  });

  return {
    xxmiSources,
    xxmiSelectedSource,
    xxmiRemoteVersions,
    xxmiLocalPackages,
    isXxmiFetching,
    isXxmiDownloading,
    xxmiDownloadingVersion,
    xxmiMessage,
    xxmiMessageType,
    loadXxmiSources,
    refreshXxmiLocal,
    refreshXxmiRemote,
    refreshXxmiState,
    doDownloadXxmi,
    doDeployXxmi,
    doDeleteXxmi,
    xxmiFilteredLocal,
    xxmiFilteredSources,
  };
}
