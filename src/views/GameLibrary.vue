<script setup lang="ts">
import { gamesList, switchToGame, appSettings, loadGames } from '../store';
import { type CSSProperties, ref, onMounted, onUnmounted, computed } from 'vue';
import { setGameVisibility, deleteGameConfigFolder, askConfirm, listGameTemplates, importGameTemplate, getGameTemplatesDir, type GameTemplateInfo } from '../api';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { convertFileSrc } from '@tauri-apps/api/core';

const { t, te } = useI18n();

// Router
const router = useRouter();

// Context Menu State
const showMenu = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const targetGame = ref<any>(null);

// Search and Filter
const searchQuery = ref('');

const filteredGames = computed(() => {
    if (!searchQuery.value) return gamesList;
    const q = searchQuery.value.toLowerCase();
    return gamesList.filter((g: any) => {
        const name = g.name.toLowerCase();
        const display = (te(`games.${g.name}`) ? t(`games.${g.name}`) : (g.displayName || g.name)).toLowerCase();
        return name.includes(q) || display.includes(q);
    });
});

const handleContextMenu = (e: MouseEvent, game: any) => {
  e.preventDefault();
  targetGame.value = game;
  menuX.value = e.clientX;
  menuY.value = e.clientY;
  showMenu.value = true;
};

const closeMenu = () => {
  showMenu.value = false;
};

const addToFavorites = async () => {
  if (!targetGame.value) return;
  
  const gameName = targetGame.value.name;
  
  try {
    // hidden=false → remove from hidden list → show in sidebar
    await setGameVisibility(gameName, false);
    await loadGames();
    
    // Switch to home page
    router.push('/');
  } catch (err) {
    console.error('Failed to add game to favorites:', err);
  }
  
  closeMenu();
};

// Delete Game
const deleteGame = async () => {
  if (!targetGame.value) return;
  const gameName = targetGame.value.name;
  const displayName = te(`games.${gameName}`) ? t(`games.${gameName}`) : gameName;
  const yes = await askConfirm(
    t('gamelibrary.deleteConfirmMsg', { name: displayName }),
    { title: t('gamelibrary.deleteConfirmTitle'), kind: 'warning' }
  );
  if (!yes) { closeMenu(); return; }
  try {
    await deleteGameConfigFolder(gameName);
    await loadGames();
  } catch (err) {
    console.error('Failed to delete game:', err);
  }
  closeMenu();
};

// Blank Area Right-Click Menu
const showBlankMenu = ref(false);
const blankMenuX = ref(0);
const blankMenuY = ref(0);

const handleBlankContextMenu = (e: MouseEvent) => {
  // Only trigger on the container itself, not on game cards
  if ((e.target as HTMLElement).closest('.game-card')) return;
  e.preventDefault();
  blankMenuX.value = e.clientX;
  blankMenuY.value = e.clientY;
  showBlankMenu.value = true;
};

const closeBlankMenu = () => {
  showBlankMenu.value = false;
};

// Import Dialog
const showImportDialog = ref(false);
const templateList = ref<GameTemplateInfo[]>([]);
const importLoading = ref(false);

const openImportDialog = async () => {
  closeBlankMenu();
  importLoading.value = true;
  showImportDialog.value = true;
  try {
    templateList.value = await listGameTemplates();
  } catch (err) {
    console.error('Failed to list templates:', err);
  } finally {
    importLoading.value = false;
  }
};

const handleImport = async (tmpl: GameTemplateInfo) => {
  if (tmpl.alreadyExists) {
    const yes = await askConfirm(
      t('gamelibrary.importOverwriteMsg', { name: tmpl.name }),
      { title: t('gamelibrary.importOverwriteTitle'), kind: 'warning' }
    );
    if (!yes) return;
  }
  try {
    await importGameTemplate(tmpl.name, tmpl.alreadyExists);
    await loadGames();
    templateList.value = await listGameTemplates();
  } catch (err) {
    console.error('Failed to import template:', err);
  }
};

const openTemplatesFolder = async () => {
  try {
    const dir = await getGameTemplatesDir();
    const { open } = await import('@tauri-apps/plugin-shell');
    await open(dir);
  } catch (err) {
    console.error('Failed to open templates folder:', err);
  }
};

const handleCloseAll = () => {
  closeMenu();
  closeBlankMenu();
};

onMounted(() => {
  document.addEventListener('click', handleCloseAll);
});

onUnmounted(() => {
  document.removeEventListener('click', handleCloseAll);
});

// Love Particle System
interface Particle {
    id: number;
    x: number;
    y: number;
    text: string;
    style: CSSProperties;
}
const particles = ref<Particle[]>([]);
let particleId = 0;
const colors = ['#ff7eb3', '#ff758c', '#ff7eb3', '#fgbdff', '#ff9999', '#ffffff'];

// Background Hearts
interface BgHeart {
    id: number;
    x: number;
    y: number;
    size: number;
    rotation: number;
    color: string;
    opacity: number;
}
const bgHearts = ref<BgHeart[]>([]);

// Meteor Star System
interface MeteorStar {
    id: number;
    x: number;     // Start X
    y: number;     // Start Y
    tx: number;    // Translate X
    ty: number;    // Translate Y
    angle: number; // Movement angle for tail rotation
    color: string;
    emoji: string; // New: Random Emoji
    rotationDuration: string;
    flickerDuration: string; // New: Flicker speed
    flyDuration: string; 
    size: number;
    // opacity removed
    trail: Array<{ x: number, y: number, s: number, o: number }>; 
}
const meteorStars = ref<MeteorStar[]>([]);
let meteorId = 0;
const starColors = ['#ff0000', '#ffaf00', '#ffff00', '#00ff00', '#0000ff', '#4b0082', '#8f00ff', '#ff00ff', '#00ffff'];
const meteorEmojis = ['⭐', '🌟', '💫', '✨', '☄️', '🪐', '🦄', '🌈', '🍭', '🌸', '🍩', '🍪', '🍕', '🚀', '🛸', '🧚', '💎', '🍄', '🐱', '🐶'];

const consecutiveClickCount = ref(0);
const lastClickedGameId = ref('');

const spawnMeteorStars = () => {
    // Spawn a few stars to create a "shower" feel
    const count = 3 + Math.floor(Math.random() * 4); 
    const w = window.innerWidth;
    const h = window.innerHeight;
    
    for (let i = 0; i < count; i++) {
        // Pick a random start edge
        const edge = Math.floor(Math.random() * 4);
        let startX = 0, startY = 0;
        let endX = 0, endY = 0;
        
        switch(edge) {
            case 0: // Top -> Bottom
                startX = Math.random() * w; startY = -80; endX = Math.random() * w; endY = h + 150; break;
            case 1: // Right -> Left
                startX = w + 80; startY = Math.random() * h; endX = -150; endY = Math.random() * h; break;
            case 2: // Bottom -> Top
                startX = Math.random() * w; startY = h + 80; endX = Math.random() * w; endY = -150; break;
            case 3: // Left -> Right
                startX = -80; startY = Math.random() * h; endX = w + 150; endY = Math.random() * h; break;
        }

        // Calculate relative translation
        const tx = endX - startX;
        const ty = endY - startY;
        const angle = Math.atan2(ty, tx);

        // Speed: 1.2s - 2.5s
        const durationSec = 1.2 + Math.random() * 1.3;

        const ms: MeteorStar = {
            id: meteorId++,
            x: startX,
            y: startY,
            tx,
            ty,
            angle,
            color: starColors[Math.floor(Math.random() * starColors.length)],
            emoji: meteorEmojis[Math.floor(Math.random() * meteorEmojis.length)],
            rotationDuration: `${0.2 + Math.random() * 0.3}s`, // Fast Spin
            flickerDuration: `${0.1 + Math.random() * 0.2}s`, // Fast Flicker
            flyDuration: `${durationSec}s`,
            size: 24 + Math.random() * 32, 
            trail: [] // Empty
        };

        meteorStars.value.push(ms);
        
        // Remove after animation
        setTimeout(() => {
            meteorStars.value = meteorStars.value.filter(s => s.id !== ms.id);
        }, durationSec * 1000 + 100); 
    }
};

const handleGameSelect = (game: any, event: MouseEvent) => {
    // 0. Track Consecutive Clicks
    if (lastClickedGameId.value === game.name) {
        consecutiveClickCount.value++;
    } else {
        lastClickedGameId.value = game.name;
        consecutiveClickCount.value = 1;
    }

    // Trigger Meteor Shower if 3+ clicks
    if (consecutiveClickCount.value >= 3) {
        spawnMeteorStars();
    }

    // If detecting consecutive click on already selected item
    if (appSettings.currentConfigName === game.name) {
        spawnLoveExplosion(event);
    }
    
    // Always switch (or refresh) and trigger animations
    switchToGame(game);
};

const spawnLoveExplosion = (e: MouseEvent) => {
    // Determine coordinates based on click event source, or just use mouse position
    const x = e.clientX;
    const y = e.clientY;

    // 1. Background Giant Heart
    bgHearts.value.push({
        id: particleId++,
        x: x, 
        y: y,
        size: 300 + Math.random() * 300, // 300-600px
        rotation: (Math.random() - 0.5) * 60,
        color: '#ff7eb3',
        opacity: 0.1 + Math.random() * 0.2 // Random opacity
    });
    // Remove bg heart after animation
    setTimeout(() => {
        bgHearts.value.shift();
    }, 2000);

    // 2. Exploding Particles
    for (let i = 0; i < 15; i++) {
        const angle = Math.random() * Math.PI * 2;
        const velocity = 100 + Math.random() * 200;
        const tx = Math.cos(angle) * velocity;
        const ty = Math.sin(angle) * velocity;
        const color = colors[Math.floor(Math.random() * colors.length)];
        const kaomojisArr = [
            '(｡♥‿♥｡)', '( ˘ ³˘)♥', 'OwO', 'UwU', '(*♡∀♡)', 
            '(◕‿◕✿)', '♥', '❤', '❥', '(≧◡≦)', '(>ω<)', 
            'Ciallo～(∠・ω< )⌒☆', 'Ciallo', '(∠・ω< )⌒★', 
            '(・ω< )', '☆⌒(ゝ。∂)', '(★^O^★)'
        ];
        const text = kaomojisArr[Math.floor(Math.random() * kaomojisArr.length)];
        
        const p: Particle = {
            id: particleId++,
            x: x,
            y: y,
            text: text,
            style: {
                '--tx': `${tx}px`,
                '--ty': `${ty}px`,
                color: color,
                fontSize: `${12 + Math.random() * 16}px`,
                fontWeight: 'bold',
                textShadow: '0 0 5px rgba(255,100,100,0.5)'
            } as CSSProperties
        };
        
        particles.value.push(p);
        
        // Remove particle
        setTimeout(() => {
            particles.value = particles.value.filter(item => item.id !== p.id);
        }, 1500);
    }
};
</script>

<template>
    <div class="game-library-container" @contextmenu="handleBlankContextMenu">
        <!-- Background Effects Layer -->
        <div class="effects-layer">
            <div 
                v-for="h in bgHearts" 
                :key="h.id" 
                class="bg-heart"
                :style="{
                    left: h.x + 'px',
                    top: h.y + 'px',
                    width: h.size + 'px',
                    height: h.size + 'px',
                    transform: `translate(-50%, -50%) rotate(${h.rotation}deg)`,
                    opacity: h.opacity
                }"
            >
                <svg viewBox="0 0 32 29.6" fill="currentColor">
                    <path d="M23.6,0c-3.4,0-6.3,2.7-7.6,5.6C14.7,2.7,11.8,0,8.4,0C3.8,0,0,3.8,0,8.4c0,9.4,9.5,11.9,16,21.2
                    c6.1-9.3,16-11.8,16-21.2C32,3.8,28.2,0,23.6,0z"/>
                </svg>
            </div>
        </div>

        <!-- Meteor Star Layer -->
        <div class="meteor-layer">
            <div 
                v-for="s in meteorStars" 
                :key="s.id" 
                class="meteor-star-wrapper"
                :style="{
                    left: s.x + 'px',
                    top: s.y + 'px',
                    '--tx': s.tx + 'px',
                    '--ty': s.ty + 'px',
                    animationDuration: s.flyDuration
                } as any"
            >
                <div 
                    class="meteor-star-inner"
                    :style="{
                        color: s.color,
                        fontSize: s.size + 'px',
                        animation: `starSpin ${s.rotationDuration} linear infinite, starFlicker ${s.flickerDuration} ease-in-out infinite alternate`,
                        textShadow: `0 0 10px ${s.color}`
                    }"
                >{{ s.emoji }}</div>
            </div>
        </div>

        <!-- Particle Layer -->
        <div class="particles-layer">
             <div 
                v-for="p in particles" 
                :key="p.id" 
                class="love-particle"
                :style="{
                    left: p.x + 'px',
                    top: p.y + 'px',
                    ...p.style
                }"
            >
                {{ p.text }}
            </div>
        </div>

        <!-- Toolbar -->
        <div class="toolbar" @click.stop v-if="gamesList.length > 0 || searchQuery !== ''">
            <button class="tech-btn" @click="openImportDialog">
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px; vertical-align: -2px;">
                    <line x1="12" y1="5" x2="12" y2="19"></line>
                    <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
                {{ t('gamelibrary.importConfig') }}
            </button>
            <div style="flex-grow: 1;"></div>
            <input 
                type="text" 
                class="search-input" 
                v-model="searchQuery"
                :placeholder="t('gamelibrary.searchGames') || 'Search games...'"
            />
        </div>

        <div class="games-grid">
            <div 
                v-for="(game, index) in filteredGames" 
                :key="game.name"
                class="game-card"
                :class="{ active: appSettings.currentConfigName === game.name }"
                :style="{ animationDelay: `${index * 0.05}s` }"
                @click="handleGameSelect(game, $event)"
                @contextmenu.prevent="handleContextMenu($event, game)"
            >
                <div class="game-icon-wrapper">
                    <img 
                        :src="game.iconPath" 
                        class="game-icon" 
                        alt="icon" 
                        @load="(e) => (e.target as HTMLImageElement).style.opacity = '1'"
                        @error="(e) => (e.target as HTMLImageElement).style.opacity = '0'"
                    />
                </div>
                <div class="game-label">{{ te(`games.${game.name}`) ? t(`games.${game.name}`) : (game.displayName || game.name) }}</div>
            </div>
            
            <div v-if="filteredGames.length === 0" class="empty-state">
                <div class="empty-icon">!</div>
                <div class="empty-text">No matches found.</div>
            </div>
        </div>

        <!-- Custom Context Menu for Game Library -->
        <div 
          v-if="showMenu" 
          class="context-menu" 
          :style="{ top: menuY + 'px', left: menuX + 'px' }"
          @click.stop
        >
          <div class="menu-item" @click="addToFavorites">
            {{ t('gamelibrary.addToSidebar') }}
          </div>
          <div class="menu-item menu-item-danger" @click="deleteGame">
            {{ t('gamelibrary.deleteGame') }}
          </div>
        </div>

        <!-- Blank Area Context Menu -->
        <div
          v-if="showBlankMenu"
          class="context-menu"
          :style="{ top: blankMenuY + 'px', left: blankMenuX + 'px' }"
          @click.stop
        >
          <div class="menu-item" @click="openImportDialog">
            {{ t('gamelibrary.importConfig') }}
          </div>
          <div class="menu-item" @click="openTemplatesFolder">
            {{ t('gamelibrary.openTemplatesFolder') }}
          </div>
        </div>

        <!-- Import Dialog -->
        <div v-if="showImportDialog" class="import-overlay" @click.self="showImportDialog = false">
          <div class="import-dialog">
            <div class="import-header">
              <span>{{ t('gamelibrary.importConfig') }}</span>
              <button class="import-close" @click="showImportDialog = false">✕</button>
            </div>
            <div class="import-body">
              <div v-if="importLoading" class="import-loading">Loading...</div>
              <div v-else-if="templateList.length === 0" class="import-empty">{{ t('gamelibrary.noTemplates') }}</div>
              <div v-else class="import-list">
                <div
                  v-for="tmpl in templateList"
                  :key="tmpl.name"
                  class="import-item"
                  :class="{ 'import-item-exists': tmpl.alreadyExists }"
                  @click="handleImport(tmpl)"
                >
                  <img
                    v-if="tmpl.hasIcon && tmpl.iconPath"
                    :src="convertFileSrc(tmpl.iconPath)"
                    class="import-icon"
                    alt=""
                  />
                  <div v-else class="import-icon-placeholder">?</div>
                  <div class="import-info">
                    <div class="import-name">{{ te(`games.${tmpl.gameId}`) ? t(`games.${tmpl.gameId}`) : (tmpl.displayName || tmpl.name) }}</div>
                    <div class="import-name-sub">{{ tmpl.name }} ({{ tmpl.gameId }})</div>
                  </div>
                  <div v-if="tmpl.alreadyExists" class="import-badge">{{ t('gamelibrary.alreadyExists') }}</div>
                </div>
              </div>
            </div>
            <div class="import-footer">
              <button class="import-open-folder" @click="openTemplatesFolder">
                {{ t('gamelibrary.openTemplatesFolder') }}
              </button>
            </div>
          </div>
        </div>
    </div>
</template>

<style scoped>
.game-library-container {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%; /* Constrain height to viewport to enable internal scrolling */
    box-sizing: border-box;
    padding-top: 60px; /* TitleBar Safe Area */
    padding-bottom: 72px; /* Increased bottom padding (40px + 32px TitleBar height) */
    
    /* Background moved to App.vue for global coverage including TitleBar */
    background: transparent; 
    
    overflow-y: auto;
    overflow-x: hidden; /* Prevent horizontal scrollbar caused by scaled breathing effects */
}

/* 
   Toolbar & Inputs (Bright Tech)
*/
.toolbar { /* Assuming there will be a toolbar div, this acts as preparation or applies if it exists */
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(0, 240, 255, 0.3);
  border-radius: 8px;
  padding: 8px 16px;
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 24px;
}

.search-input {
  background: rgba(0, 0, 0, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: #fff;
  padding: 8px 12px;
  border-radius: 4px;
  transition: all 0.2s;
  outline: none;
}
.search-input:focus {
  border-color: #00f0ff;
  box-shadow: 0 0 10px rgba(0, 240, 255, 0.3);
}

.tech-btn {
  background: rgba(0, 240, 255, 0.1);
  border: 1px solid #00f0ff;
  color: #00f0ff;
  padding: 8px 16px;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
  text-transform: uppercase;
  font-weight: 600;
  font-size: 12px;
  letter-spacing: 1px;
}
.tech-btn:hover {
  background: #00f0ff;
  color: #000;
  box-shadow: 0 0 15px rgba(0, 240, 255, 0.5);
}

/* Context Menu */
.context-menu {
  position: fixed;
  z-index: 10000;
  background: rgba(15, 15, 20, 0.95);
  border: 1px solid rgba(0, 240, 255, 0.5); /* bright border */
  backdrop-filter: blur(8px);
  box-shadow: 0 4px 20px rgba(0, 240, 255, 0.2);
  border-radius: 4px; /* sharp */
  padding: 4px;
  min-width: 140px;
}

.menu-item {
  padding: 8px 12px;
  cursor: pointer;
  color: #fff;
  font-size: 13px;
  border-radius: 2px;
  transition: background-color 0.1s;
}

.menu-item:hover {
  background-color: #00f0ff;
  color: #000;
}

.menu-item-danger {
  color: #ff0055;
}
.menu-item-danger:hover {
  background-color: #ff0055;
  color: #fff;
}

/* Import Dialog (Tech Style) */
.import-overlay {
  position: fixed;
  top: 0; left: 0;
  width: 100vw; height: 100vh;
  background: rgba(0,0,0,0.8);
  z-index: 20000;
  display: flex;
  align-items: center;
  justify-content: center;
}
.import-dialog {
  background: rgba(10, 15, 20, 0.98);
  border: 1px solid #00f0ff;
  border-radius: 8px; /* Sharper */
  width: 420px;
  max-height: 70vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 0 30px rgba(0, 240, 255, 0.2), inset 0 0 20px rgba(255,255,255,0.02);
}
.import-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid rgba(0, 240, 255, 0.3);
  font-size: 16px;
  font-weight: 600;
  color: #00f0ff;
  text-transform: uppercase;
  letter-spacing: 1px;
}
.import-close {
  background: none;
  border: none;
  color: rgba(255,255,255,0.5);
  font-size: 18px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 2px;
}
.import-close:hover { color: #ff0055; background: rgba(255, 0, 85, 0.1); }
.import-body {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}
.import-loading, .import-empty {
  text-align: center;
  color: #00f0ff;
  padding: 32px 0;
  font-size: 14px;
}
.import-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.import-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
  border: 1px solid transparent;
}
.import-item:hover { 
  background: rgba(0, 240, 255, 0.05); 
  border-color: rgba(0, 240, 255, 0.3);
}
.import-item-exists { opacity: 0.5; filter: grayscale(1); }
.import-icon {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  object-fit: cover;
  border: 1px solid rgba(255,255,255,0.2);
}
.import-icon-placeholder {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  background: rgba(0, 240, 255, 0.1);
  border: 1px dashed rgba(0, 240, 255, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #00f0ff;
  font-size: 18px;
}
.import-info { flex: 1; min-width: 0; }
.import-name {
  color: #fff;
  font-size: 14px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.import-name-sub {
  color: rgba(255,255,255,0.5);
  font-size: 11px;
  margin-top: 4px;
  font-family: monospace;
}
.import-badge {
  font-size: 10px;
  color: #ff0055;
  border: 1px solid #ff0055;
  background: rgba(255, 0, 85, 0.1);
  padding: 2px 6px;
  border-radius: 2px;
  white-space: nowrap;
  text-transform: uppercase;
}
.import-footer {
  padding: 16px;
  border-top: 1px solid rgba(0, 240, 255, 0.3);
  text-align: center;
}
.import-open-folder {
  background: transparent;
  border: 1px solid #00f0ff;
  color: #00f0ff;
  padding: 8px 16px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 1px;
  transition: all 0.2s;
}
.import-open-folder:hover { background: #00f0ff; color: #000; box-shadow: 0 0 10px rgba(0,240,255,0.5); }

/* Meteor Star CSS */
.meteor-layer {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 300; /* Above cards and explosions */
}
.meteor-star-wrapper {
    position: absolute;
    /* Use ease-in to simulate gravity or accelerating streak currently mapped to flying across whole screen */
    animation: flyAcross linear forwards; 
}
.meteor-star-inner {
    /* Self-rotation handled by inline style */
    display: inline-block;
    width: 100%;
    height: 100%;
    line-height: 1;
    font-weight: bold;
    transform-origin: 50% 55%; /* Fix visual center rotation */
    text-align: center;
}
.meteor-tail-container {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 0;
    height: 0;
    z-index: -1;
    pointer-events: none;
}
.meteor-trail-dot {
    position: absolute;
    border-radius: 50%;
    /* Create a pulsing effect for trail particles */
    animation: trailPulse 0.5s ease-in-out infinite alternate;
}

@keyframes flyAcross {
    to { transform: translate(var(--tx), var(--ty)); }
}
@keyframes starSpin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
}
@keyframes starFlicker {
    from { opacity: 1; }
    to { opacity: 0.3; }
}
@keyframes trailPulse {
    from { transform: scale(0.8); opacity: 0.3; }
    to { transform: scale(1.2); opacity: 0.8; }
}

/* Keyframes for Impact Shake */
@keyframes impactShake {
    0% { transform: scale(1.5) translate(0, 0); }
    20% { transform: scale(1.5) translate(-4px, 4px); }
    40% { transform: scale(1.5) translate(4px, -4px); }
    60% { transform: scale(1.5) translate(-3px, 0); }
    80% { transform: scale(1.5) translate(3px, 0); }
    100% { transform: scale(1.5) translate(0, 0); }
}

/* Custom Scrollbar */
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

.games-grid {
    display: flex;
    flex-wrap: wrap; 
    justify-content: flex-start;
    gap: 32px 24px;
    padding: 24px 60px 60px 60px;
    margin: 0 auto;
    max-width: 1400px;
}

/* --- Bright Tech Sci-Fi Game Cards --- */

.game-card {
    position: relative;
    width: 110px; /* Fixed width */
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    cursor: pointer;
    
    /* 
      Staggered Entrance Animation 
    */
    animation: cardEntranceFade 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
}
.game-card:nth-child(1)  { animation-delay: 0.05s; }
.game-card:nth-child(2)  { animation-delay: 0.10s; }
.game-card:nth-child(3)  { animation-delay: 0.15s; }
.game-card:nth-child(4)  { animation-delay: 0.20s; }
.game-card:nth-child(5)  { animation-delay: 0.25s; }
.game-card:nth-child(6)  { animation-delay: 0.30s; }
.game-card:nth-child(7)  { animation-delay: 0.35s; }
.game-card:nth-child(8)  { animation-delay: 0.40s; }
.game-card:nth-child(9)  { animation-delay: 0.45s; }
.game-card:nth-child(10) { animation-delay: 0.50s; }
.game-card:nth-child(11) { animation-delay: 0.55s; }
.game-card:nth-child(12) { animation-delay: 0.60s; }
.game-card:nth-child(n+13) { animation-delay: 0.65s; }

@keyframes cardEntranceFade {
    0% { opacity: 0; transform: translateY(20px); }
    100% { opacity: 1; transform: translateY(0); }
}

.game-icon-wrapper {
    position: relative;
    width: 100%;
    /* 1:1 Aspect ratio based on width */
    aspect-ratio: 1 / 1;
    
    background: rgba(255, 255, 255, 0.05);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px; /* Sharper Tech Corners */
    padding: 6px;

    /* Base depth */
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
    
    transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.game-card:hover .game-icon-wrapper {
    transform: translateY(-6px) scale(1.05); /* Snappy scale */
    border-color: #00f0ff; /* Bright cyan hover */
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.4), 
                0 0 15px rgba(0, 240, 255, 0.6);
}

.game-card.active .game-icon-wrapper {
    transform: translateY(-4px) scale(1.08); /* Snappy scale */
    border-color: #fff; /* Crisp white active state */
    box-shadow: 0 8px 15px rgba(0, 0, 0, 0.5), 
                0 0 20px rgba(255, 255, 255, 0.8);
    background: rgba(255, 255, 255, 0.1);
}

/* Mechanical Scanning Line for Active Game */
.game-card.active .game-icon-wrapper::after {
  content: '';
  position: absolute;
  top: 0; left: 0; right: 0; bottom: 0;
  border-radius: 11px;
  z-index: 4;
  pointer-events: none;
  background: linear-gradient(to bottom, transparent 40%, rgba(0, 240, 255, 0.4) 50%, transparent 60%);
  background-size: 100% 200%;
  animation: techScan 2s linear infinite;
}

@keyframes techScan {
  0% { background-position: 0% -100%; }
  100% { background-position: 0% 200%; }
}

.game-icon {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 8px; /* Sharper image inside wrapper */
    display: block;
    z-index: 1;
    filter: brightness(0.9);
    transition: filter 0.2s ease, transform 0.2s ease;
}

.game-card:hover .game-icon {
    filter: brightness(1.1);
}
.game-card.active .game-icon {
    filter: brightness(1);
}

/* 
  Game Label Below Icon
*/
.game-label {
    margin-top: 12px;
    width: 100%;
    text-align: center;
    font-size: 13px;
    font-weight: 500;
    color: #e0e0e0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color 0.3s ease, text-shadow 0.3s ease;
}

/* Hover effect on Label: Reveal text completely if previously hidden? 
   Actually, just slightly highlighting the text works better. */
.game-card:hover .game-label {
    color: #fff;
    text-shadow: 0 0 8px rgba(255, 255, 255, 0.6);
}
.game-card.active .game-label {
    color: #00f0ff; /* Tech cyan instead of yellow */
    text-shadow: 0 0 8px rgba(0, 240, 255, 0.6);
}

/* Active breathing light removed to favor the sleek border and shadow */
/* Only keep the animation shake/impact logic defined in <script> by hiding the CSS pseudo elements */
.game-card.active::before,
.game-card.active::after,
.game-icon-wrapper::before,
.game-icon-wrapper::after {
    display: none !important;
}

/* Animations and Layers */
.effects-layer, .particles-layer {
    position: fixed;
    top: 0; left: 0;
    width: 100vw; height: 100vh;
    pointer-events: none;
    z-index: 5; /* Below game cards (z-index 10) but above background */
    overflow: hidden;
}

.bg-heart {
    position: absolute;
    color: #ff7eb3;
    animation: bgHeartFade 2s ease-out forwards;
    filter: blur(5px);
}

@keyframes bgHeartFade {
    0% { transform: translate(-50%, -50%) scale(0.5); opacity: 0; }
    20% { opacity: 0.5; transform: translate(-50%, -50%) scale(1); }
    100% { transform: translate(-50%, -50%) scale(1.5); opacity: 0; }
}

.love-particle {
    position: absolute;
    pointer-events: none;
    animation: particleFly 1.5s cubic-bezier(0.25, 1, 0.5, 1) forwards;
    white-space: nowrap;
}

@keyframes particleFly {
    0% { transform: translate(-50%, -50%) scale(0); opacity: 1; }
    50% { opacity: 1; }
    100% { transform: translate(calc(-50% + var(--tx)), calc(-50% + var(--ty))) scale(1.5); opacity: 0; }
}
</style>