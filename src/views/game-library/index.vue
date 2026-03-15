<script setup lang="ts">
import { gamesLoading } from '../../store';
import GameLibraryEffects from './GameLibraryEffects.vue';
import GameLibraryMainContent from './GameLibraryMainContent.vue';
import GameLibraryMenus from './GameLibraryMenus.vue';
import { useGameLibraryView } from './useGameLibraryView';

const {
  addToFavorites,
  appSettings,
  bgHearts,
  blankMenuX,
  blankMenuY,
  closeImportDialog,
  deleteGame,
  emptyStateText,
  filteredGames,
  getMeteorWrapperStyle,
  handleBlankContextMenu,
  handleContextMenu,
  handleGameSelect,
  handleImport,
  importLoading,
  menuX,
  menuY,
  meteorStars,
  openImportDialog,
  openTemplatesFolder,
  particles,
  searchQuery,
  showBlankMenu,
  showImportDialog,
  showMenu,
  templateList,
} = useGameLibraryView();
</script>

<template>
  <div class="game-library-container" data-onboarding="library-root" @contextmenu="handleBlankContextMenu">
    <GameLibraryEffects
      :bg-hearts="bgHearts"
      :meteor-stars="meteorStars"
      :particles="particles"
      :get-meteor-wrapper-style="getMeteorWrapperStyle"
    />

    <GameLibraryMainContent
      :search-query="searchQuery"
      :filtered-games="filteredGames"
      :current-config-name="appSettings.currentConfigName"
      :games-loading="gamesLoading"
      :empty-state-text="emptyStateText"
      @update:search-query="searchQuery = $event"
      @open-import-dialog="openImportDialog"
      @handle-game-select="handleGameSelect"
      @handle-context-menu="handleContextMenu"
    />

    <GameLibraryMenus
      :show-menu="showMenu"
      :menu-x="menuX"
      :menu-y="menuY"
      :show-blank-menu="showBlankMenu"
      :blank-menu-x="blankMenuX"
      :blank-menu-y="blankMenuY"
      :show-import-dialog="showImportDialog"
      :import-loading="importLoading"
      :template-list="templateList"
      @add-to-favorites="addToFavorites"
      @delete-game="deleteGame"
      @open-import-dialog="openImportDialog"
      @open-templates-folder="openTemplatesFolder"
      @close-import-dialog="closeImportDialog"
      @import-template="handleImport"
    />
  </div>
</template>

<style scoped>
.game-library-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  padding-top: 60px;
  padding-bottom: 72px;
  background: transparent;
  overflow-y: auto;
  overflow-x: hidden;
}

.game-library-container::-webkit-scrollbar {
  width: 6px;
}

.game-library-container::-webkit-scrollbar-track {
  background: transparent;
}

.game-library-container::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
  border-radius: 3px;
}

.game-library-container::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.3);
}
</style>
