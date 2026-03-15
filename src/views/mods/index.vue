<script setup lang="ts">
import ModsEntryTable from './ModsEntryTable.vue';
import ModsGameControlPanel from './ModsGameControlPanel.vue';
import ModsHeaderStats from './ModsHeaderStats.vue';
import { useModsView } from './useModsView';

const {
  activeModEntryName,
  enabledGameCount,
  filteredGameOptions,
  filteredModEntries,
  formatBytes,
  formatModified,
  gameKeyword,
  isBulkOperating,
  isLoadingGames,
  isLoadingSelectedMods,
  isSavingGameToggle,
  loadGameSummaries,
  modKeyword,
  modStatusFilter,
  openEntryLocation,
  openMigotoSettings,
  openSelectedModsFolder,
  openSelectedShaderFixesFolder,
  selectedDisabledModCount,
  selectedEnabledModCount,
  selectedGameName,
  selectedGameSummary,
  selectedState,
  toggleAllMods,
  toggleGameMigoto,
  toggleModEntry,
  totalGameCount,
} = useModsView();
</script>

<template>
  <div class="mods-page">
    <ModsHeaderStats
      :is-loading-games="isLoadingGames"
      :total-game-count="totalGameCount"
      :enabled-game-count="enabledGameCount"
      :selected-enabled-mod-count="selectedEnabledModCount"
      :selected-disabled-mod-count="selectedDisabledModCount"
      @refresh="loadGameSummaries"
      @open-settings="openMigotoSettings"
    />

    <ModsGameControlPanel
      v-model:game-keyword="gameKeyword"
      v-model:selected-game-name="selectedGameName"
      :filtered-game-options="filteredGameOptions"
      :selected-game-summary="selectedGameSummary"
      :selected-state="selectedState"
      :is-saving-game-toggle="isSavingGameToggle"
      :is-bulk-operating="isBulkOperating"
      @toggle-game-migoto="toggleGameMigoto"
      @open-selected-mods-folder="openSelectedModsFolder"
      @open-selected-shader-fixes-folder="openSelectedShaderFixesFolder"
      @toggle-all="toggleAllMods"
    />

    <ModsEntryTable
      v-model:mod-keyword="modKeyword"
      v-model:mod-status-filter="modStatusFilter"
      :selected-game-name="selectedGameName"
      :selected-game-summary="selectedGameSummary"
      :selected-state="selectedState"
      :filtered-mod-entries="filteredModEntries"
      :is-loading-selected-mods="isLoadingSelectedMods"
      :is-bulk-operating="isBulkOperating"
      :active-mod-entry-name="activeModEntryName"
      :format-bytes="formatBytes"
      :format-modified="formatModified"
      @toggle-mod-entry="toggleModEntry"
      @open-entry-location="openEntryLocation"
    />
  </div>
</template>

<style scoped>
.mods-page {
  padding: 32px 40px 60px 40px;
  animation: fadeIn 0.15s ease-out;
  background: rgba(10, 15, 20, 0.92);
  width: 100%;
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@media (max-width: 900px) {
  .mods-page {
    padding: 24px 20px 48px 20px;
  }
}
</style>
