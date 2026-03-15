<script setup lang="ts">
import { computed, markRaw, type Component } from 'vue';
import SettingsAppearancePanel from './components/SettingsAppearancePanel.vue';
import SettingsBasicPanel from './components/SettingsBasicPanel.vue';
import SettingsDxvkPanel from './components/SettingsDxvkPanel.vue';
import SettingsDisplayPanel from './components/SettingsDisplayPanel.vue';
import SettingsMigotoPanel from './components/SettingsMigotoPanel.vue';
import SettingsProtonPanel from './components/SettingsProtonPanel.vue';
import SettingsResourcePanel from './components/SettingsResourcePanel.vue';
import SettingsSidebarMenu from './components/SettingsSidebarMenu.vue';
import SettingsVkd3dPanel from './components/SettingsVkd3dPanel.vue';
import SettingsVersionPanel from './components/SettingsVersionPanel.vue';
import { useSettingsView } from './useSettingsView';

const {
  appSettings,
  updateLocaleAndReload,
  openLogWindow,
  activeMenu,
  guideMenu,
  versionInfo,
  isVersionChecking,
  checkVersionInfo,
  resourceInfo,
  isResourceChecking,
  checkResourceInfo,
  isResourcePulling,
  pullResources,
  reenterOnboarding,
  protonCatalog,
  isCatalogLoading,
  isCatalogSaving,
  isLocalLoading,
  isRemoteLoading,
  isDownloading,
  downloadingFamilyKey,
  downloadingTag,
  showProtonCatalogEditor,
  selectedLocalByFamily,
  selectedRemoteByFamily,
  editableFamilies,
  editableSources,
  deletingProtonIds,
  remoteItemKey,
  hasSourceByFamily,
  selectedLocalItems,
  selectedRemoteItems,
  refreshLocalGrouped,
  refreshRemoteGrouped,
  saveCatalogChanges,
  reloadCatalogEditor,
  addFamily,
  removeFamily,
  addSource,
  removeSource,
  isManagedProtonItem,
  installSelectedForFamily,
  familyCards,
  removeLocalProtonItem,
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
  globalMigotoEnabled,
  handleMigotoGlobalToggle,
  isMigotoImporterLocked,
  isMigotoInjectionLocked,
  isMigotoPathOverridden,
  isMigotoSaving,
  isMigotoTogglePending,
  isMigotoWwmi,
  getMigotoAutoPathDescription,
  getMigotoPathDisplayValue,
  loadMigotoGameConfig,
  migotoAvailableImporterOptions,
  migotoConfig,
  migotoEditableStartArgs,
  migotoGamesList,
  migotoImporterHint,
  migotoInjectionHint,
  migotoRequiredStartArgs,
  migotoRiskStatement,
  migotoSelectedGame,
  migotoStartArgsHint,
  restoreMigotoPathAuto,
  saveMigotoGameConfig,
  selectMigotoPath,
  showMigotoRiskRestatement,
  migotoHasLockedCustomization,
  openDocumentsDoc,
  getLocalizedGameName,
  xxmiSources,
  xxmiSelectedSource,
  xxmiRemoteVersions,
  isXxmiFetching,
  isXxmiDownloading,
  xxmiDownloadingVersion,
  xxmiMessage,
  xxmiMessageType,
  refreshXxmiRemote,
  doDownloadXxmi,
  doDeployXxmi,
  doDeleteXxmi,
  xxmiFilteredLocal,
  xxmiFilteredSources,
  selectCacheDir,
  selectDataDir,
} = useSettingsView();

const settingsPanels = {
  basic: markRaw(SettingsBasicPanel),
  appearance: markRaw(SettingsAppearancePanel),
  display: markRaw(SettingsDisplayPanel),
  version: markRaw(SettingsVersionPanel),
  resource: markRaw(SettingsResourcePanel),
  proton: markRaw(SettingsProtonPanel),
  dxvk: markRaw(SettingsDxvkPanel),
  vkd3d: markRaw(SettingsVkd3dPanel),
  migoto: markRaw(SettingsMigotoPanel),
} as const;

const activeSettingsPanel = computed(
  () =>
    settingsPanels[
      activeMenu.value as keyof typeof settingsPanels
    ] ?? settingsPanels.basic,
);

const activeSettingsPanelProps = computed<Record<string, unknown>>(() => {
  switch (activeMenu.value) {
    case 'appearance':
      return {
        appSettings,
      };
    case 'display':
      return {
        appSettings,
      };
    case 'version':
      return {
        versionInfo: versionInfo.value,
        isVersionChecking: isVersionChecking.value,
        checkVersionInfo,
      };
    case 'resource':
      return {
        resourceInfo: resourceInfo.value,
        isResourceChecking: isResourceChecking.value,
        isResourcePulling: isResourcePulling.value,
        checkResourceInfo,
        pullResources,
      };
    case 'proton':
      return {
        guideMenu: guideMenu.value,
        protonCatalog: protonCatalog.value,
        familyCards: familyCards.value,
        selectedLocalByFamily,
        selectedRemoteByFamily,
        showCatalogEditor: showProtonCatalogEditor.value,
        isCatalogLoading: isCatalogLoading.value,
        isCatalogSaving: isCatalogSaving.value,
        isLocalLoading: isLocalLoading.value,
        isRemoteLoading: isRemoteLoading.value,
        isDownloading: isDownloading.value,
        downloadingFamilyKey: downloadingFamilyKey.value,
        downloadingTag: downloadingTag.value,
        editableFamilies: editableFamilies.value,
        editableSources: editableSources.value,
        deletingProtonIds,
        refreshLocalGrouped,
        refreshRemoteGrouped,
        selectedLocalItems: selectedLocalItems.value,
        selectedRemoteItems: selectedRemoteItems.value,
        remoteItemKey,
        hasSourceByFamily: hasSourceByFamily.value,
        isManagedProtonItem,
        installSelectedForFamily,
        removeLocalProtonItem,
        reloadCatalogEditor,
        saveCatalogChanges,
        addFamily,
        removeFamily,
        addSource,
        removeSource,
        'onUpdate:showCatalogEditor': (value: boolean) => {
          showProtonCatalogEditor.value = value;
        },
      };
    case 'dxvk':
      return {
        guideMenu: guideMenu.value,
        dxvkLocalVersions: dxvkLocalVersions.value,
        dxvkGroupedList: dxvkGroupedList.value,
        selectedKey: dxvkSelectedKey.value,
        selectedDxvkItem: selectedDxvkItem.value,
        isDxvkFetching: isDxvkFetching.value,
        isDxvkDownloading: isDxvkDownloading.value,
        dxvkFetchWarning: dxvkFetchWarning.value,
        dxvkLocalCount: dxvkLocalCount.value,
        deletingDxvkKeys,
        refreshDxvkLocal,
        refreshDxvkRemote,
        doDownloadDxvk,
        removeLocalDxvkItem,
        'onUpdate:selectedKey': (value: string) => {
          dxvkSelectedKey.value = value;
        },
      };
    case 'vkd3d':
      return {
        vkd3dLocalVersions: vkd3dLocalVersions.value,
        vkd3dVersionList: vkd3dVersionList.value,
        selectedVersion: vkd3dSelectedVersion.value,
        selectedVkd3dItem: selectedVkd3dItem.value,
        isVkd3dFetching: isVkd3dFetching.value,
        isVkd3dDownloading: isVkd3dDownloading.value,
        vkd3dFetchWarning: vkd3dFetchWarning.value,
        vkd3dLocalCount: vkd3dLocalCount.value,
        deletingVkd3dVersions,
        refreshVkd3dLocal,
        refreshVkd3dRemote,
        doDownloadVkd3d,
        removeLocalVkd3dItem,
        'onUpdate:selectedVersion': (value: string) => {
          vkd3dSelectedVersion.value = value;
        },
      };
    case 'migoto':
      return {
        globalMigotoEnabled: globalMigotoEnabled.value,
        isMigotoTogglePending: isMigotoTogglePending.value,
        isMigotoSaving: isMigotoSaving.value,
        migotoRiskStatement,
        migotoSelectedGame: migotoSelectedGame.value,
        migotoGamesList: migotoGamesList.value,
        migotoConfig,
        migotoAvailableImporterOptions: migotoAvailableImporterOptions.value,
        migotoImporterHint: migotoImporterHint.value,
        migotoInjectionHint: migotoInjectionHint.value,
        migotoStartArgsHint: migotoStartArgsHint.value,
        migotoRequiredStartArgs: migotoRequiredStartArgs.value,
        migotoEditableStartArgs: migotoEditableStartArgs.value,
        migotoHasLockedCustomization: migotoHasLockedCustomization.value,
        isMigotoWwmi: isMigotoWwmi.value,
        isMigotoImporterLocked: isMigotoImporterLocked.value,
        isMigotoInjectionLocked: isMigotoInjectionLocked.value,
        xxmiSelectedSource: xxmiSelectedSource.value,
        xxmiFilteredSources: xxmiFilteredSources.value,
        xxmiRemoteVersions: xxmiRemoteVersions.value,
        xxmiFilteredLocal: xxmiFilteredLocal.value,
        xxmiSources: xxmiSources.value,
        isXxmiFetching: isXxmiFetching.value,
        isXxmiDownloading: isXxmiDownloading.value,
        xxmiDownloadingVersion: xxmiDownloadingVersion.value,
        xxmiMessage: xxmiMessage.value,
        xxmiMessageType: xxmiMessageType.value,
        handleMigotoGlobalToggle,
        openDocumentsDoc,
        showMigotoRiskRestatement,
        getLocalizedGameName,
        isMigotoPathOverridden,
        getMigotoPathDisplayValue,
        getMigotoAutoPathDescription,
        selectMigotoPath,
        restoreMigotoPathAuto,
        saveMigotoGameConfig,
        loadMigotoGameConfig,
        refreshXxmiRemote,
        doDownloadXxmi,
        doDeployXxmi,
        doDeleteXxmi,
        'onUpdate:selectedGame': (value: string) => {
          migotoSelectedGame.value = value;
        },
        'onUpdate:editableStartArgs': (value: string) => {
          migotoEditableStartArgs.value = value;
        },
        'onUpdate:selectedSource': (value: string) => {
          xxmiSelectedSource.value = value;
        },
      };
    case 'basic':
    default:
      return {
        appSettings,
        selectDataDir,
        selectCacheDir,
        updateLocaleAndReload,
        openLogWindow,
        reenterOnboarding,
      };
  }
});

const activeSettingsPanelComponent = computed(
  () => activeSettingsPanel.value as Component,
);

const activeSettingsPanelBindings = computed(
  () => activeSettingsPanelProps.value as Record<string, unknown>,
);

</script>

<template>
  <div class="settings-layout">
    <aside class="settings-sidebar-shell">
      <SettingsSidebarMenu
        :active-menu="activeMenu"
        :guide-menu="guideMenu"
        @select="(value) => activeMenu = value"
      />
    </aside>

    <div class="settings-layout-divider" aria-hidden="true"></div>

    <section class="settings-content-shell">
      <div class="settings-content">
        <KeepAlive :max="9">
          <component
            :is="activeSettingsPanelComponent"
            v-bind="activeSettingsPanelBindings"
          />
        </KeepAlive>
      </div>
    </section>

  </div>
</template>

<style>
.settings-layout {
  display: flex;
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: relative;
  gap: 0;
  padding: 18px;
  box-sizing: border-box;
  background: rgba(10, 15, 20, 0.92);
  will-change: transform;
  contain: layout style;
}

.settings-sidebar-shell {
  width: 272px;
  min-width: 272px;
  max-width: 272px;
  flex-shrink: 0;
  border-radius: 22px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  background:
    linear-gradient(180deg, rgba(26, 32, 40, 0.88), rgba(11, 15, 20, 0.94));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 20px 48px rgba(0, 0, 0, 0.34);
  backdrop-filter: blur(18px);
  overflow: hidden;
}

.settings-layout-divider {
  position: relative;
  width: 18px;
  min-width: 18px;
  flex-shrink: 0;
}

.settings-layout-divider::before {
  content: '';
  position: absolute;
  top: 18px;
  bottom: 18px;
  left: 50%;
  width: 2px;
  transform: translateX(-50%);
  border-radius: 999px;
  background: linear-gradient(
    180deg,
    rgba(0, 240, 255, 0.05),
    rgba(0, 240, 255, 0.34) 18%,
    rgba(0, 240, 255, 0.42) 50%,
    rgba(0, 240, 255, 0.18) 82%,
    rgba(0, 240, 255, 0.04)
  );
  box-shadow:
    0 0 16px rgba(0, 240, 255, 0.2),
    0 0 28px rgba(0, 240, 255, 0.08);
}

.settings-content-shell {
  flex: 1;
  min-width: 0;
  border-radius: 24px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  background:
    linear-gradient(180deg, rgba(12, 18, 24, 0.82), rgba(8, 12, 18, 0.9));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.04),
    0 16px 40px rgba(0, 0, 0, 0.22);
  overflow: hidden;
}

.settings-menu {
  width: 220px;
  min-width: 220px;
  border-right: 1px solid rgba(0, 240, 255, 0.3); /* Tech Cyan Line */
  background: rgba(0, 5, 10, 0.4);
  overflow-y: auto;
  padding: 24px 12px;
}

.settings-el-menu {
  border-right: none;
  background-color: transparent;
}

.settings-el-menu .el-menu-item {
  height: 48px;
  line-height: 48px;
  margin: 4px 0;
  border-radius: 4px; /* Sharp */
  color: rgba(255, 255, 255, 0.65);
  font-size: 14px;
  font-weight: 500;
  transition: all 0.2s ease;
  position: relative;
  overflow: hidden;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.menu-guide-dot {
  position: absolute;
  left: 8px;
  top: 50%;
  width: 8px;
  height: 8px;
  margin-top: -4px;
  border-radius: 999px;
  background: #f59e0b;
  animation: menuGuideBlink 0.7s ease-in-out 0s 6;
}

@keyframes menuGuideBlink {
  0% { opacity: 0.3; transform: scale(0.9); }
  50% { opacity: 1; transform: scale(1.25); }
  100% { opacity: 0.35; transform: scale(0.9); }
}

.settings-el-menu .el-menu-item:hover {
  background-color: rgba(0, 240, 255, 0.1);
  color: #fff;
}

.settings-el-menu .el-menu-item.is-active {
  background-color: rgba(0, 240, 255, 0.15);
  color: #00f0ff; /* Glowing cyan text */
  font-weight: 600;
  border-left: 4px solid #00f0ff;
}

.settings-content {
  height: 100%;
  overflow-y: auto;
  padding: 32px 40px 60px 40px;
  will-change: scroll-position;
}

.settings-content::-webkit-scrollbar {
  width: 10px;
}

.settings-content::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.12);
  border: 2px solid transparent;
  background-clip: padding-box;
}

.settings-content::-webkit-scrollbar-track {
  background: transparent;
}

.settings-panel {
  max-width: 800px;
  animation: fadeIn 0.15s ease-out;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.panel-title {
  font-size: 24px;
  font-weight: 600;
  color: #00f0ff; /* Tech cyan */
  margin-bottom: 32px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
  letter-spacing: 1px;
  text-transform: uppercase;
}

.panel-title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin-bottom: 32px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
}

.panel-title-inline {
  margin-bottom: 0;
  padding-bottom: 0;
  border-bottom: none;
}

.panel-title-badge {
  letter-spacing: 0.5px;
}

.migoto-risk-block {
  border: 1px solid rgba(255, 99, 71, 0.45);
  background: linear-gradient(135deg, rgba(120, 12, 12, 0.32), rgba(55, 12, 12, 0.22));
}

.migoto-risk-title {
  color: #ff9b8a;
}

.migoto-risk-text {
  color: rgba(255, 226, 220, 0.92);
  line-height: 1.7;
}

.migoto-risk-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 16px;
}

.settings-guide-banner {
  margin-bottom: 14px;
  border-radius: 6px;
  border: 1px solid rgba(245, 158, 11, 0.45);
  background: rgba(245, 158, 11, 0.14);
  color: #fbbf24;
  padding: 10px 12px;
  font-size: 13px;
  animation: guideBannerPulse 0.8s ease-in-out 0s 4;
}

@keyframes guideBannerPulse {
  0% { opacity: 0.65; }
  50% { opacity: 1; }
  100% { opacity: 0.68; }
}

.form-item-vertical {
  display: flex;
  flex-direction: column;
  width: 100%;
}

.form-item-hint {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.45);
  margin-top: 6px;
  line-height: 1.5;
}

.settings-divider {
  display: flex;
  align-items: center;
  margin: 30px 0 20px 0;
  color: #fff;
  font-size: 15px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.settings-divider::after {
  content: '';
  flex: 1;
  height: 1px;
  background: linear-gradient(to right, rgba(255, 255, 255, 0.15), transparent);
  margin-left: 20px;
}

.proton-panel {
  max-width: 1100px;
}

@media (max-width: 960px) {
  .settings-layout {
    padding: 12px;
  }

  .settings-sidebar-shell {
    width: 248px;
    min-width: 248px;
    max-width: 248px;
  }

  .settings-layout-divider {
    width: 12px;
    min-width: 12px;
  }

  .settings-content {
    padding: 24px 28px 44px;
  }
}

/* Glass Panels / Cards / Tech blocks */
.section-block, .family-card, .dxvk-status-card {
  margin-bottom: 24px;
  border: 1px solid rgba(0, 240, 255, 0.2);
  border-radius: 4px; /* Sharper */
  background: rgba(10, 15, 20, 0.6); /* Solid translucent */
  padding: 20px;
}

.section-block:hover, .family-card:hover {
  background: rgba(15, 20, 25, 0.8);
}

.section-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.toolbar-actions {
  display: flex;
  gap: 12px;
}

.section-title {
  font-size: 16px;
  font-weight: 600;
  color: #fff;
}

.section-hint {
  margin-top: 6px;
  color: rgba(255, 255, 255, 0.55);
  font-size: 13px;
  line-height: 1.5;
}

.version-panel {
  max-width: 900px;
}

.version-grid {
  margin-top: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.version-row {
  display: grid;
  grid-template-columns: 140px 1fr;
  gap: 16px;
  align-items: start;
}

.version-label {
  font-size: 14px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.7);
}

.version-value {
  font-size: 14px;
  color: #fff;
}

.version-log-row {
  align-items: stretch;
}

.version-log-content {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 13px;
  line-height: 1.6;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  padding: 16px;
  color: rgba(255, 255, 255, 0.85);
  font-family: 'Fira Code', monospace;
}

.family-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 12px;
}

.family-card {
  content-visibility: auto;
  contain-intrinsic-size: 240px;
}

.proton-editor-wrap {
  content-visibility: auto;
  contain-intrinsic-size: 720px;
}

.family-title {
  font-size: 16px;
  color: #fff;
  font-weight: 600;
}

.family-key {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.45);
  background: rgba(0, 0, 0, 0.3);
  padding: 2px 8px;
  border-radius: 12px;
}

.family-row {
  display: grid;
  grid-template-columns: 130px 1fr auto;
  gap: 12px;
  align-items: center;
  margin-top: 16px;
}

.row-label {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.75);
}

.family-select {
  width: 100%;
}

.row-sub {
  margin-top: 8px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
  word-break: break-all;
}

.remote-option-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
}

.remote-option-meta {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
}

.editor-subtitle {
  font-size: 15px;
  color: #fff;
  font-weight: 600;
  margin-bottom: 12px;
}

.migoto-locked-summary {
  margin-bottom: 12px;
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.03);
}

.migoto-locked-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.migoto-locked-badge {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 0 10px;
  border-radius: 999px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.05);
  color: rgba(255, 255, 255, 0.52);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.2px;
}

.migoto-form-item-locked {
  opacity: 0.72;
}

.migoto-form-item-locked .form-item-hint {
  color: rgba(255, 255, 255, 0.48);
}

.editor-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.editor-row {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  padding: 12px;
  display: grid;
  gap: 12px;
}

.family-editor-row {
  grid-template-columns: 180px 200px 140px 90px 90px auto;
}

.source-editor-row {
  grid-template-columns: 190px 170px 200px 260px 260px 130px 180px 180px 130px 100px 90px 180px auto;
}

.patterns-input {
  grid-column: 1 / -1;
}

.source-family-select, .provider-select {
  width: 100%;
}

.dxvk-panel {
  max-width: 900px;
}

.dxvk-section {
  margin-top: 12px;
}

.dxvk-local-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 12px;
}

.dxvk-local-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
}


.dxvk-local-ver {
  font-size: 15px;
  font-weight: 600;
  color: #fff;
  min-width: 70px;
}

.dxvk-local-path {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.45);
  word-break: break-all;
  flex: 1;
}

.dxvk-local-actions {
  flex: 0 0 auto;
}

.row-sub-path {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.dxvk-count {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
  font-weight: 400;
  margin-left: 8px;
}

.dxvk-download-row {
  display: flex;
  gap: 12px;
  align-items: center;
}

.dxvk-version-select {
  flex: 1;
}

.dxvk-status-row {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 14px;
  color: #fff;
}

.dxvk-status-label {
  min-width: 80px;
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
}

.text-ok { color: #67c23a; font-weight: 600;}
.text-err { color: #f56c6c; font-weight: 600;}

.dxvk-fetch-warning {
  font-size: 13px;
  color: #e6a23c;
  background: rgba(230, 162, 60, 0.15);
  border: 1px solid rgba(230, 162, 60, 0.3);
  border-radius: 6px;
  padding: 8px 12px;
}

/* 
  Deep customization for Element Plus components inside settings 
  to match the Bright Tech HUD theme.
*/
:deep(.el-form-item__label) {
  color: rgba(255, 255, 255, 0.85);
  font-weight: 500;
}

:deep(.el-input__wrapper), :deep(.el-select__wrapper) {
  background-color: rgba(0, 0, 0, 0.5) !important;
  border: 1px solid rgba(255, 255, 255, 0.2) !important;
  box-shadow: none !important;
  border-radius: 4px;
}

:deep(.el-input__wrapper:hover), :deep(.el-select__wrapper:hover) {
  border-color: rgba(0, 240, 255, 0.5) !important;
  box-shadow: none !important;
}

:deep(.el-input__wrapper.is-focus), :deep(.el-select__wrapper.is-focus) {
  border-color: #00f0ff !important;
  box-shadow: none !important;
  background-color: rgba(0, 240, 255, 0.05) !important;
}

:deep(.el-input__inner) {
  color: #fff !important;
}

/* Switches as Mechanical Toggles */
:deep(.el-switch__core) {
  background-color: rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 4px;
}
:deep(.el-switch.is-checked .el-switch__core) {
  background-color: #00f0ff;
  border-color: #00f0ff;
}
:deep(.el-switch.is-checked .el-switch__core .el-switch__action) {
  background-color: #000;
  border-radius: 2px;
}
:deep(.el-switch__core .el-switch__action) {
  border-radius: 2px;
}

/* Tech Buttons */
:deep(.el-button) {
  background-color: rgba(0, 240, 255, 0.05);
  border: 1px solid rgba(0, 240, 255, 0.5);
  color: #00f0ff;
  border-radius: 4px;
  text-transform: uppercase;
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

:deep(.el-button:hover:not(.is-disabled)) {
  background-color: #00f0ff;
  color: #000;
  border-color: #00f0ff;
}

:deep(.el-button--primary) {
  background-color: rgba(0, 240, 255, 0.2);
  color: #00f0ff;
  border: 1px solid #00f0ff;
}

:deep(.el-button--primary:hover:not(.is-disabled)) {
  background-color: #00f0ff;
  color: #000;
}

:deep(.el-button.is-disabled) {
  background-color: rgba(255, 255, 255, 0.05);
  color: rgba(255, 255, 255, 0.2);
  border-color: rgba(255, 255, 255, 0.1);
}

/* XXMI 资源包列表 */
.xxmi-pkg-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
}

.xxmi-pkg-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  background: rgba(0, 240, 255, 0.03);
  border: 1px solid rgba(0, 240, 255, 0.15);
  border-radius: 4px;
  transition: border-color 0.2s;
}

.xxmi-pkg-item:hover {
  border-color: rgba(0, 240, 255, 0.4);
}

.xxmi-pkg-info {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
  min-width: 0;
}

.xxmi-pkg-version {
  font-weight: 600;
  color: #00f0ff;
  font-size: 14px;
  font-family: 'JetBrains Mono', monospace;
}

.xxmi-pkg-size {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
}

.xxmi-pkg-date {
  color: rgba(255, 255, 255, 0.4);
  font-size: 12px;
  margin-left: 8px;
}

.xxmi-pkg-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

@media (max-width: 1280px) {
  .family-editor-row,
  .source-editor-row,
  .family-row,
  .version-row {
    grid-template-columns: 1fr;
    gap: 12px;
  }
}
</style>
