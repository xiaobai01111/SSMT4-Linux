<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import type { GameInfo, XxmiLocalPackage, XxmiPackageSource, XxmiRemoteVersion } from '../../../api';
import type { MigotoGameConfig, MigotoPathOverrideField } from '../../../composables/useSettingsMigotoConfig';

type MigotoSelectablePathField =
  | 'migoto_path'
  | 'importer_folder'
  | 'mod_folder'
  | 'shader_fixes_folder'
  | 'd3dx_ini_path'
  | 'bridge_exe_path';

defineProps<{
  globalMigotoEnabled: boolean;
  isMigotoTogglePending: boolean;
  isMigotoSaving: boolean;
  migotoRiskStatement: string;
  migotoSelectedGame: string;
  migotoGamesList: GameInfo[];
  migotoConfig: MigotoGameConfig;
  migotoAvailableImporterOptions: Array<{ value: string; label: string }>;
  migotoImporterHint: string;
  migotoInjectionHint: string;
  migotoStartArgsHint: string;
  migotoRequiredStartArgs: string;
  migotoEditableStartArgs: string;
  migotoHasLockedCustomization: boolean;
  isMigotoWwmi: boolean;
  isMigotoImporterLocked: boolean;
  isMigotoInjectionLocked: boolean;
  xxmiSelectedSource: string;
  xxmiFilteredSources: XxmiPackageSource[];
  xxmiRemoteVersions: XxmiRemoteVersion[];
  xxmiFilteredLocal: XxmiLocalPackage[];
  xxmiSources: XxmiPackageSource[];
  isXxmiFetching: boolean;
  isXxmiDownloading: boolean;
  xxmiDownloadingVersion: string;
  xxmiMessage: string;
  xxmiMessageType: 'success' | 'error' | '';
  handleMigotoGlobalToggle: (value: boolean) => unknown | Promise<unknown>;
  openDocumentsDoc: (docId: string) => void | Promise<void>;
  showMigotoRiskRestatement: () => void | Promise<void>;
  getLocalizedGameName: (game: Pick<GameInfo, 'name'> | string) => string;
  isMigotoPathOverridden: (field: MigotoPathOverrideField) => boolean;
  getMigotoPathDisplayValue: (field: MigotoPathOverrideField) => string;
  getMigotoAutoPathDescription: (field: MigotoPathOverrideField) => string;
  selectMigotoPath: (field: MigotoSelectablePathField) => void | Promise<void>;
  restoreMigotoPathAuto: (field: MigotoPathOverrideField) => void;
  saveMigotoGameConfig: () => unknown | Promise<unknown>;
  loadMigotoGameConfig: () => unknown | Promise<unknown>;
  refreshXxmiRemote: () => void | Promise<void>;
  doDownloadXxmi: (ver: XxmiRemoteVersion) => void | Promise<void>;
  doDeployXxmi: (pkg: XxmiLocalPackage) => void | Promise<void>;
  doDeleteXxmi: (pkg: XxmiLocalPackage) => void | Promise<void>;
}>();

const emit = defineEmits<{
  (event: 'update:selectedGame', value: string): void;
  (event: 'update:editableStartArgs', value: string): void;
  (event: 'update:selectedSource', value: string): void;
}>();

const { t, te } = useI18n();

const tr = (key: string, fallback: string) => {
  return te(key) ? String(t(key)) : fallback;
};

const formatBytes = (bytes: number) => {
  if (!Number.isFinite(bytes) || bytes <= 0) return '-';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};
</script>

<template>
  <div class="settings-panel migoto-panel w-full">
    <div class="panel-header">
      <h2 class="panel-title">{{ tr('settings.migoto.panelTitle', '3DMIGOTO 管理') }}</h2>
      <el-tag type="danger" effect="dark" class="experimental-badge">{{ tr('settings.migoto.experimentalBadge', '实验性') }}</el-tag>
    </div>

    <el-alert
      type="error"
      show-icon
      :closable="false"
      class="risk-alert custom-alert mt-4"
    >
      <template #title>
        <span class="alert-title">{{ tr('settings.migoto.riskTitle', '严重警告') }}</span>
      </template>
      <div class="alert-desc">{{ migotoRiskStatement }}</div>
      <div class="alert-actions mt-3">
        <el-button size="small" type="danger" plain @click="openDocumentsDoc('terms')">{{ tr('settings.migoto.viewTerms', '查看《服务条款》') }}</el-button>
        <el-button size="small" plain @click="openDocumentsDoc('risk')">{{ tr('settings.migoto.viewProjectRisk', '查看项目风险') }}</el-button>
        <el-button size="small" type="warning" @click="showMigotoRiskRestatement">{{ tr('settings.migoto.restateRisk', '再次查看声明') }}</el-button>
      </div>
    </el-alert>

    <el-card class="setting-card mt-5 full-width-card" shadow="never">
      <div class="setting-row">
        <div class="setting-info">
          <div class="setting-name">{{ tr('settings.migoto.globalToggleTitle', '3DMigoto 全局开关') }}</div>
          <div class="setting-desc">{{ tr('settings.migoto.globalToggleHint', '关闭后将全局禁用 3DMigoto 的相关配置、桥接和注入；游戏设置页也不会显示 3DMigoto 入口。') }}</div>
        </div>
        <div class="setting-control pl-4">
          <el-switch
            :model-value="globalMigotoEnabled"
            :loading="isMigotoTogglePending"
            :disabled="isMigotoTogglePending"
            size="large"
            @update:model-value="handleMigotoGlobalToggle(Boolean($event))"
          />
        </div>
      </div>
    </el-card>

    <el-alert
      v-if="!globalMigotoEnabled"
      type="info"
      show-icon
      :closable="false"
      class="mt-4 custom-alert"
    >
      {{ tr('settings.migoto.disabledSummary', '3DMigoto 当前已全局禁用。现有游戏配置会被保留，但启动时不会加载桥接、不会注入，也不会在游戏设置中显示 3DMigoto 管理。') }}
    </el-alert>

    <template v-else>
      <el-card class="setting-card mt-5 full-width-card" shadow="never">
        <div class="setting-header">
          <h3 class="setting-title">{{ $t('settings.migoto.gameConfigTitle') }}</h3>
          <p class="setting-subtitle">{{ $t('settings.migoto.gameConfigHint') }}</p>
        </div>

        <div class="setting-block">
          <div class="setting-name mb-2">{{ $t('settings.migoto.selectGame') }}</div>
          <el-select
            :model-value="migotoSelectedGame"
            @update:model-value="emit('update:selectedGame', String($event))"
            :placeholder="$t('settings.migoto.selectGamePlaceholder')"
            class="full-width-select"
            filterable
            style="width: 100%; max-width: 500px;"
          >
            <el-option
              v-for="g in migotoGamesList"
              :key="g.name"
              :label="getLocalizedGameName(g)"
              :value="g.name"
            />
          </el-select>
          <div class="input-hint mt-2">{{ tr('settings.migoto.supportedGamesOnlyHint', '这里只显示当前支持 3DMigoto 的游戏。') }}</div>
        </div>
      </el-card>

      <div v-if="migotoSelectedGame" class="game-config-container mt-5">
        
        <el-card class="setting-card full-width-card" shadow="never">
          <template #header>
            <div class="card-header-title">{{ $t('settings.migoto.pathConfig') }}</div>
          </template>
          <div class="path-grid">
            <div class="input-group">
              <label class="input-label">{{ $t('settings.migoto.migotoPath') }}</label>
              <div class="flex-row">
                <el-input v-model="migotoConfig.migoto_path" :placeholder="$t('settings.migoto.migotoPathPlaceholder')" class="flex-1" />
                <el-button @click="selectMigotoPath('migoto_path')">{{ $t('settings.migoto.browse') }}</el-button>
              </div>
              <div class="input-hint">{{ $t('settings.migoto.migotoPathHint') }}</div>
            </div>

            <div v-for="field in (['importer_folder', 'mod_folder', 'shader_fixes_folder', 'd3dx_ini_path'] as const)" :key="field" class="input-group">
              <label class="input-label">{{ $t(`settings.migoto.${field.replace(/_([a-z])/g, g => g[1].toUpperCase())}`) }}</label>
              <div class="flex-row">
                <el-input
                  :model-value="getMigotoPathDisplayValue(field)"
                  readonly
                  :class="{ 'auto-path': !isMigotoPathOverridden(field) }"
                  :placeholder="$t(`settings.migoto.${field.replace(/_([a-z])/g, g => g[1].toUpperCase())}Placeholder`)"
                  class="flex-1"
                />
                <el-button @click="selectMigotoPath(field)">{{ $t('settings.migoto.browse') }}</el-button>
                <el-button v-if="isMigotoPathOverridden(field)" type="danger" plain @click="restoreMigotoPathAuto(field)">
                  {{ $t('settings.migoto.restoreAuto') }}
                </el-button>
              </div>
              <div class="input-hint">
                {{ $t(`settings.migoto.${field.replace(/_([a-z])/g, g => g[1].toUpperCase())}Hint`) }}
                <div v-if="!isMigotoPathOverridden(field)" class="text-primary mt-1">{{ getMigotoAutoPathDescription(field) }}</div>
              </div>
            </div>
          </div>
        </el-card>

        <el-card class="setting-card full-width-card" shadow="never">
          <template #header>
            <div class="flex-between">
              <div class="card-header-title">{{ $t('settings.migoto.importerAndInjection') }}</div>
              <el-tag v-if="migotoHasLockedCustomization" type="warning" effect="light">
                {{ tr('settings.migoto.lockedTag', '配置锁定中') }}
              </el-tag>
            </div>
          </template>
          
          <div class="two-column-grid">
            <div class="input-group">
              <label class="input-label">{{ $t('settings.migoto.importerLabel') }}</label>
              <div class="flex-row">
                <el-select v-model="migotoConfig.importer" :disabled="isMigotoImporterLocked" class="flex-1">
                  <el-option v-for="opt in migotoAvailableImporterOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
                </el-select>
                <el-tag v-if="isMigotoImporterLocked" type="warning" plain class="ml-2">{{ tr('settings.migoto.lockedBadge', '不可修改') }}</el-tag>
              </div>
              <div class="input-hint">{{ migotoImporterHint }}</div>
            </div>

            <div class="input-group">
              <label class="input-label">{{ $t('settings.migoto.injectionLabel') }}</label>
              <div class="flex-row align-center pt-2">
                <el-radio-group v-model="migotoConfig.use_hook" :disabled="isMigotoInjectionLocked">
                  <el-radio :value="true">{{ $t('settings.migoto.injectionHook') }}</el-radio>
                  <el-radio :value="false">{{ $t('settings.migoto.injectionDirect') }}</el-radio>
                </el-radio-group>
                <el-tag v-if="isMigotoInjectionLocked" type="warning" plain class="ml-2">{{ tr('settings.migoto.lockedBadge', '不可修改') }}</el-tag>
              </div>
              <div class="input-hint">{{ migotoInjectionHint }}</div>
            </div>
          </div>
        </el-card>

        <el-card class="setting-card full-width-card" shadow="never">
          <template #header>
            <div class="card-header-title">{{ $t('settings.migoto.bridgeConfig') }}</div>
          </template>
          <div class="path-grid">
            <div class="input-group">
              <label class="input-label">{{ $t('settings.migoto.bridgeExe') }}</label>
              <div class="flex-row">
                <el-input v-model="migotoConfig.bridge_exe_path" :placeholder="$t('settings.migoto.bridgeExePlaceholder')" class="flex-1" />
                <el-button @click="selectMigotoPath('bridge_exe_path')">{{ $t('settings.migoto.browse') }}</el-button>
              </div>
              <div class="input-hint">{{ $t('settings.migoto.bridgeExeHint') }}</div>
            </div>

            <div class="input-group">
              <label class="input-label">{{ $t('settings.migoto.startArgs') }}</label>
              <el-input :model-value="migotoEditableStartArgs" @update:model-value="emit('update:editableStartArgs', String($event))" :placeholder="$t('settings.migoto.startArgsPlaceholder')" />
              <div v-if="migotoRequiredStartArgs" class="input-hint text-warning font-bold mt-1">
                {{ $t('settings.migoto.startArgsRequiredValue', { args: migotoRequiredStartArgs }) }}
              </div>
              <div class="input-hint">{{ migotoStartArgsHint }}</div>
            </div>

            <div class="two-column-grid">
              <div class="input-group">
                <label class="input-label">{{ $t('settings.migoto.processStartMethod') }}</label>
                <el-select v-model="migotoConfig.process_start_method">
                  <el-option value="Native" :label="$t('settings.migoto.startMethodNative')" />
                  <el-option value="CreateProcess" :label="$t('settings.migoto.startMethodCreateProcess')" />
                  <el-option value="ShellExecute" :label="$t('settings.migoto.startMethodShellExecute')" />
                </el-select>
              </div>
              <div class="input-group">
                <label class="input-label">{{ $t('settings.migoto.processPriority') }}</label>
                <el-select v-model="migotoConfig.process_priority">
                  <el-option value="Normal" :label="$t('settings.migoto.priorityNormal')" />
                  <el-option value="AboveNormal" :label="$t('settings.migoto.priorityAboveNormal')" />
                  <el-option value="High" :label="$t('settings.migoto.priorityHigh')" />
                  <el-option value="Realtime" :label="$t('settings.migoto.priorityRealtime')" />
                  <el-option value="BelowNormal" :label="$t('settings.migoto.priorityBelowNormal')" />
                  <el-option value="Idle" :label="$t('settings.migoto.priorityIdle')" />
                </el-select>
              </div>
            </div>
            
            <div class="input-group">
              <label class="input-label">{{ $t('settings.migoto.dllInitDelay') }}</label>
              <div class="flex-row align-center">
                <el-input-number v-model="migotoConfig.xxmi_dll_init_delay" :min="0" :max="5000" :step="50" style="width: 160px;" />
                <span class="text-secondary text-sm ml-2">{{ $t('settings.migoto.dllInitDelayUnit') }}</span>
              </div>
              <div class="input-hint">{{ $t('settings.migoto.dllInitDelayHint') }}</div>
            </div>
          </div>
        </el-card>

        <el-card class="setting-card full-width-card" shadow="never">
          <template #header>
            <div class="card-header-title">{{ $t('settings.migoto.advancedOptions') }}</div>
          </template>
          <div class="switch-grid">
            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.enforceRendering') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.enforceRenderingHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.enforce_rendering" />
            </div>

            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.muteWarnings') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.muteWarningsHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.mute_warnings" />
            </div>

            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.enableHunting') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.enableHuntingHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.enable_hunting" />
            </div>

            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.dumpShaders') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.dumpShadersHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.dump_shaders" />
            </div>

            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.callsLogging') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.callsLoggingHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.calls_logging" />
            </div>

            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.debugLogging') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.debugLoggingHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.debug_logging" />
            </div>
            
            <div class="switch-box process-timeout-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.processTimeout') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.processTimeoutHint') }}</div>
              </div>
              <el-input-number v-model="migotoConfig.process_timeout" :min="5" :max="120" :step="5" />
            </div>

            <div class="switch-box unsafe-box">
              <div class="switch-info">
                <div class="switch-title text-danger">
                  <i class="el-icon-warning-outline"></i> {{ $t('settings.migoto.unsafeMode') }}
                </div>
                <div class="switch-desc">{{ $t('settings.migoto.unsafeModeHint') }}</div>
                <div v-if="migotoConfig.unsafe_mode" class="switch-desc text-danger font-bold mt-1">{{ $t('settings.migoto.unsafeModeWarn') }}</div>
              </div>
              <el-switch v-model="migotoConfig.unsafe_mode" active-color="#f56c6c" />
            </div>
          </div>
        </el-card>

        <el-card v-if="isMigotoWwmi" class="setting-card full-width-card border-secondary" shadow="never">
          <template #header>
            <div class="card-header-title text-secondary">{{ $t('settings.migoto.wwmiTitle') }}</div>
          </template>
          <div class="switch-grid">
            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.wwmiConfigureGame') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.wwmiConfigureGameHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.wwmi_configure_game" />
            </div>
            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.wwmiUnlockFps') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.wwmiUnlockFpsHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.wwmi_unlock_fps" />
            </div>
            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.wwmiPerfTweaks') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.wwmiPerfTweaksHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.wwmi_perf_tweaks" />
            </div>
            <div class="switch-box">
              <div class="switch-info">
                <div class="switch-title">{{ $t('settings.migoto.wwmiDisableWoundedFx') }}</div>
                <div class="switch-desc">{{ $t('settings.migoto.wwmiDisableWoundedFxHint') }}</div>
              </div>
              <el-switch v-model="migotoConfig.wwmi_disable_wounded_fx" />
            </div>
          </div>
        </el-card>

        <el-card class="setting-card full-width-card" shadow="never">
          <template #header>
            <div class="card-header-title">{{ $t('settings.migoto.customLaunch') }}</div>
          </template>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">{{ $t('settings.migoto.customLaunchEnable') }}</div>
              <div class="setting-desc">{{ $t('settings.migoto.customLaunchEnableHint') }}</div>
            </div>
            <div class="setting-control pl-4">
              <el-switch v-model="migotoConfig.custom_launch_enabled" />
            </div>
          </div>

          <div v-if="migotoConfig.custom_launch_enabled" class="inner-form-block mt-4">
            <div class="input-group">
              <label class="input-label">{{ $t('settings.migoto.customLaunchCmd') }}</label>
              <el-input v-model="migotoConfig.custom_launch_cmd" :placeholder="$t('settings.migoto.customLaunchCmdPlaceholder')" />
              <div class="input-hint">{{ $t('settings.migoto.customLaunchCmdHint') }}</div>
            </div>
            <div class="input-group mt-4">
              <label class="input-label">{{ $t('settings.migoto.customLaunchInjectMode') }}</label>
              <el-select v-model="migotoConfig.custom_launch_inject_mode" style="width: 200px;">
                <el-option value="Hook" :label="$t('settings.migoto.customLaunchInjectHook')" />
                <el-option value="Direct" :label="$t('settings.migoto.customLaunchInjectDirect')" />
              </el-select>
            </div>
          </div>
        </el-card>

        <el-card class="setting-card full-width-card" shadow="never">
          <template #header>
            <div class="card-header-title">{{ $t('settings.migoto.scriptHooks') }}</div>
          </template>
          <div class="path-grid">
            <div class="input-group">
              <div class="flex-between align-center mb-2">
                <label class="input-label mb-0">{{ $t('settings.migoto.preLaunchScript') }}</label>
                <el-switch v-model="migotoConfig.pre_launch_enabled" />
              </div>
              <template v-if="migotoConfig.pre_launch_enabled">
                <el-input v-model="migotoConfig.pre_launch_cmd" :placeholder="$t('settings.migoto.preLaunchCmdPlaceholder')" />
                <el-checkbox v-model="migotoConfig.pre_launch_wait" class="mt-2">{{ $t('settings.migoto.preLaunchWait') }}</el-checkbox>
              </template>
            </div>

            <el-divider style="margin: 0;" />

            <div class="input-group">
              <div class="flex-between align-center mb-2">
                <label class="input-label mb-0">{{ $t('settings.migoto.postLoadScript') }}</label>
                <el-switch v-model="migotoConfig.post_load_enabled" />
              </div>
              <template v-if="migotoConfig.post_load_enabled">
                <el-input v-model="migotoConfig.post_load_cmd" :placeholder="$t('settings.migoto.postLoadCmdPlaceholder')" />
                <el-checkbox v-model="migotoConfig.post_load_wait" class="mt-2">{{ $t('settings.migoto.postLoadWait') }}</el-checkbox>
              </template>
            </div>
          </div>
        </el-card>

        <el-card class="setting-card full-width-card" shadow="never">
          <template #header>
            <div class="card-header-title">{{ $t('settings.migoto.extraLibraries') }}</div>
          </template>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">{{ $t('settings.migoto.extraLibrariesEnable') }}</div>
              <div class="setting-desc">{{ $t('settings.migoto.extraLibrariesEnableHint') }}</div>
            </div>
            <div class="setting-control pl-4">
              <el-switch v-model="migotoConfig.extra_libraries_enabled" />
            </div>
          </div>
          
          <div v-if="migotoConfig.extra_libraries_enabled" class="input-group mt-4">
            <label class="input-label">{{ $t('settings.migoto.extraLibrariesPaths') }}</label>
            <el-input
              v-model="migotoConfig.extra_libraries_paths"
              type="textarea"
              :rows="3"
              :placeholder="$t('settings.migoto.extraLibrariesPathsPlaceholder')"
            />
            <div class="input-hint">{{ $t('settings.migoto.extraLibrariesPathsHint') }}</div>
          </div>
        </el-card>

        <div class="action-footer mt-2 mb-8">
          <el-button type="primary" size="large" @click="saveMigotoGameConfig" :loading="isMigotoSaving">
            <i class="el-icon-check mr-1" v-if="!isMigotoSaving"></i>
            {{ isMigotoSaving ? $t('settings.migoto.saving') : $t('settings.migoto.saveConfig') }}
          </el-button>
          <el-button size="large" @click="loadMigotoGameConfig" :disabled="isMigotoSaving">
            <i class="el-icon-refresh-right mr-1"></i> {{ $t('settings.migoto.reload') }}
          </el-button>
        </div>
      </div>

      <el-card v-if="globalMigotoEnabled" class="setting-card xxmi-card mt-6 full-width-card" shadow="never">
        <template #header>
          <div class="card-header-title text-primary">{{ $t('settings.migoto.xxmiTitle') }}</div>
          <div class="setting-desc mt-1">{{ $t('settings.migoto.xxmiHint') }}</div>
        </template>

        <div class="xxmi-toolbar">
          <div class="toolbar-label">{{ $t('settings.migoto.xxmiSource') }}</div>
          <el-select
            :model-value="xxmiSelectedSource"
            @update:model-value="emit('update:selectedSource', String($event))"
            class="flex-1"
            style="max-width: 400px;"
          >
            <el-option v-for="src in xxmiFilteredSources" :key="src.id" :label="src.display_name" :value="src.id" />
          </el-select>
          <el-button type="primary" plain @click="refreshXxmiRemote" :loading="isXxmiFetching">
            {{ isXxmiFetching ? $t('settings.migoto.xxmiFetching') : $t('settings.migoto.xxmiRefresh') }}
          </el-button>
        </div>

        <el-alert
          v-if="xxmiMessage"
          :type="xxmiMessageType || 'info'"
          :closable="false"
          class="mt-4"
        >
          {{ xxmiMessage }}
        </el-alert>

        <div class="xxmi-section-top mt-6">
          <div class="section-subtitle mb-3">
            <span class="font-bold text-base">{{ $t('settings.migoto.xxmiLocalTitle') }}</span>
            <el-tag size="small" type="info" class="ml-2" effect="plain" round>{{ xxmiFilteredLocal.length }}</el-tag>
          </div>
          
          <div v-if="xxmiFilteredLocal.length > 0" class="local-grid">
            <div v-for="pkg in xxmiFilteredLocal" :key="`${pkg.source_id}-${pkg.version}`" class="local-card">
              <div class="local-card-info">
                <span class="font-mono font-bold text-base">{{ pkg.version }}</span>
                <span class="text-secondary text-sm ml-3">{{ formatBytes(pkg.size_bytes) }}</span>
              </div>
              <div class="local-card-actions">
                <el-button size="small" type="primary" plain @click="doDeployXxmi(pkg)">{{ $t('settings.migoto.xxmiDeploy') }}</el-button>
                <el-button size="small" type="danger" text bg @click="doDeleteXxmi(pkg)">
                  {{ tr('settings.migoto.xxmiDelete', '删除') }}
                </el-button>
              </div>
            </div>
          </div>
          <div v-else class="xxmi-empty-box">
            {{ tr('settings.migoto.xxmiLocalEmpty', '暂无本地下载') }}
          </div>
        </div>

        <el-divider border-style="dashed" class="my-6" />

        <div class="xxmi-section-bottom">
          <div class="section-subtitle mb-3">
            <span class="font-bold text-base">{{ $t('settings.migoto.xxmiRemoteTitle') }}</span>
            <el-tag size="small" type="info" class="ml-2" effect="plain" round>{{ xxmiRemoteVersions.length }}</el-tag>
          </div>
          
          <el-table 
            v-if="xxmiRemoteVersions.length > 0"
            :data="xxmiRemoteVersions" 
            max-height="450" 
            stripe
            class="remote-table full-width"
          >
            <el-table-column :label="tr('settings.migoto.xxmiColumnVersion', '版本')" min-width="140">
              <template #default="{ row }">
                <span class="font-mono font-bold">{{ row.version }}</span>
                <el-tag v-if="row.installed" size="small" type="success" effect="light" class="ml-2">
                  {{ tr('settings.migoto.xxmiInstalledReady', '已就绪') }}
                </el-tag>
              </template>
            </el-table-column>
            
            <el-table-column :label="tr('settings.migoto.xxmiColumnSize', '大小')" min-width="100">
              <template #default="{ row }">
                <span class="text-secondary text-sm">{{ formatBytes(row.asset_size) }}</span>
              </template>
            </el-table-column>
            
            <el-table-column :label="tr('settings.migoto.xxmiColumnPublishedAt', '发布日期')" min-width="120">
              <template #default="{ row }">
                <span class="text-secondary text-sm">{{ row.published_at ? row.published_at.substring(0, 10) : '-' }}</span>
              </template>
            </el-table-column>
            
            <el-table-column :label="tr('settings.migoto.xxmiColumnActions', '操作')" width="120" fixed="right" align="right">
              <template #default="{ row }">
                <el-button 
                  v-if="!row.installed" 
                  size="small" 
                  type="primary" 
                  :loading="isXxmiDownloading && xxmiDownloadingVersion === row.version"
                  :disabled="isXxmiDownloading"
                  @click="doDownloadXxmi(row)"
                >
                  {{
                    isXxmiDownloading && xxmiDownloadingVersion === row.version
                      ? tr('settings.migoto.xxmiDownloading', '下载中...')
                      : tr('settings.migoto.xxmiDownload', '下载')
                  }}
                </el-button>
                <span v-else class="text-success text-sm font-medium">
                  <i class="el-icon-check mr-1"></i>{{ tr('settings.migoto.xxmiInstalled', '已下载') }}
                </span>
              </template>
            </el-table-column>
          </el-table>
          
          <div v-else-if="!isXxmiFetching" class="xxmi-empty-box">{{ $t('settings.migoto.xxmiEmpty') }}</div>
        </div>
      </el-card>

    </template>
  </div>
</template>

<style scoped>
/* 全局基础配置 - 去除强制约束确保占满宽度 */
.migoto-panel {
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  color: var(--el-text-color-primary);
  width: 100%;
  max-width: none;
  min-width: 0;
}

.full-width { width: 100%; }
.full-width-card {
  width: 100%;
  box-sizing: border-box;
}

/* 文字与辅助类 */
.text-primary { color: var(--el-color-primary); }
.text-danger { color: var(--el-color-danger); }
.text-warning { color: var(--el-color-warning); }
.text-secondary { color: var(--el-text-color-secondary); }
.text-success { color: var(--el-color-success); }
.font-medium { font-weight: 500; }
.font-mono { font-family: monospace; }
.font-bold { font-weight: 600; }
.text-sm { font-size: 13px; }
.text-base { font-size: 15px; }
.mt-1 { margin-top: 4px; }
.mt-2 { margin-top: 8px; }
.mt-3 { margin-top: 12px; }
.mt-4 { margin-top: 16px; }
.mt-5 { margin-top: 20px; }
.mt-6 { margin-top: 24px; }
.mb-0 { margin-bottom: 0px; }
.mb-2 { margin-bottom: 8px; }
.mb-3 { margin-bottom: 12px; }
.mb-8 { margin-bottom: 32px; }
.my-6 { margin: 24px 0; }
.ml-1 { margin-left: 4px; }
.ml-2 { margin-left: 8px; }
.ml-3 { margin-left: 12px; }
.mr-1 { margin-right: 4px; }
.pl-4 { padding-left: 16px; }
.pt-2 { padding-top: 8px; }
.flex-1 { flex: 1; }

/* 弹性布局工具类 */
.flex-row {
  display: flex;
  gap: 8px;
  width: 100%;
}
.flex-between {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.align-center {
  align-items: center;
}

/* 头部样式 */
.panel-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.panel-title {
  font-size: 22px;
  font-weight: 600;
  margin: 0;
}

/* 自定义 Alert 样式 */
.custom-alert {
  background-color: var(--el-color-danger-light-9);
  border: 1px solid var(--el-color-danger-light-7);
}
.risk-alert .alert-title {
  font-weight: bold;
  font-size: 15px;
}
.risk-alert .alert-desc {
  margin-top: 4px;
  line-height: 1.5;
  color: var(--el-text-color-regular);
}
.risk-alert .alert-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

/* 卡片组件 */
.setting-card {
  border: 1px solid var(--el-border-color-lighter);
  background-color: var(--el-bg-color-overlay);
  border-radius: 8px;
  overflow: visible;
}
.border-secondary {
  border-color: #534530; 
}

.card-header-title {
  font-size: 16px;
  font-weight: 600;
}
.setting-header {
  margin-bottom: 16px;
}
.setting-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0 0 4px 0;
}
.setting-subtitle {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  margin: 0;
}

/* 行级设置项 */
.setting-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.setting-info {
  flex: 1;
}
.setting-name {
  font-size: 15px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}
.setting-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
  line-height: 1.4;
}

/* 游戏配置容器 */
.game-config-container {
  display: flex;
  flex-direction: column;
  gap: 20px;
  width: 100%;
}

/* 表单与输入框 */
.input-group {
  display: flex;
  flex-direction: column;
}
.input-label {
  font-size: 14px;
  font-weight: 500;
  margin-bottom: 8px;
  color: var(--el-text-color-regular);
}
.input-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 6px;
  line-height: 1.4;
}

/* 路径配置网格 */
.path-grid {
  display: flex;
  flex-direction: column;
  gap: 24px;
  width: 100%;
}

/* 两列等宽网格 */
.two-column-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 24px;
}
@media (max-width: 900px) {
  .two-column-grid { grid-template-columns: 1fr; }
}

@media (min-width: 1280px) {
  .migoto-panel {
    max-width: 1400px;
  }
}

/* 开关网格 */
.switch-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 16px;
}
.switch-box {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 16px;
  background-color: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  transition: border-color 0.2s;
}
.switch-box:hover {
  border-color: var(--el-border-color);
}
.process-timeout-box {
  grid-column: span 1;
}
.unsafe-box {
  background-color: var(--el-color-danger-light-9);
  border-color: var(--el-color-danger-light-7);
  grid-column: 1 / -1; 
}
.switch-info {
  padding-right: 16px;
}
.switch-title {
  font-size: 14px;
  font-weight: 500;
}
.switch-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
  line-height: 1.3;
}

/* 内嵌表单区域 */
.inner-form-block {
  padding: 16px;
  background-color: var(--el-fill-color-light);
  border-radius: 6px;
}

/* 底部操作按钮 */
.action-footer {
  display: flex;
  gap: 12px;
}

/* --- 全新 XXMI 上下堆叠全宽排版 --- */
.xxmi-card {
  background-color: var(--el-fill-color-light);
}
.xxmi-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  background-color: var(--el-bg-color);
  padding: 14px 16px;
  border-radius: 8px;
  border: 1px solid var(--el-border-color-lighter);
}
.toolbar-label {
  font-weight: 500;
  font-size: 14px;
}

/* 本地网格卡片排列 */
.local-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
}
.local-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px;
  background-color: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  transition: all 0.2s;
}
.local-card:hover {
  border-color: var(--el-color-primary-light-5);
  box-shadow: var(--el-box-shadow-light);
}

/* 远程表格样式 */
.remote-table {
  border-radius: 8px;
  border: 1px solid var(--el-border-color-lighter);
}
.remote-table :deep(th.el-table__cell) {
  background-color: var(--el-fill-color-darker);
  color: var(--el-text-color-regular);
  font-weight: 500;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

/* 空状态提示框 */
.xxmi-empty-box {
  text-align: center;
  padding: 32px 20px;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  border: 1px dashed var(--el-border-color);
  border-radius: 8px;
  background-color: var(--el-bg-color);
}
</style>
