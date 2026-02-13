<script setup lang="ts">
import { gamesList, switchToGame, appSettings, loadGames } from '../store';
import { reactive, type CSSProperties, ref, onMounted, onUnmounted } from 'vue';
import { setGameVisibility, deleteGameConfigFolder, askConfirm, listGameTemplates, importGameTemplate, getGameTemplatesDir, type GameTemplateInfo } from '../api';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { convertFileSrc } from '@tauri-apps/api/core';

const { t, te } = useI18n();

// Router
const router = useRouter();

// Reactive styles for animation
const cardStyles = reactive<Record<string, CSSProperties>>({});

// Context Menu State
const showMenu = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const targetGame = ref<any>(null);

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

// Animation Timer Management
let animationTimers: any[] = [];
const clearTimers = () => { 
    animationTimers.forEach(id => clearTimeout(id)); 
    animationTimers = []; 
};
const addTimer = (callback: () => void, delay: number) => {
    const id = setTimeout(callback, delay);
    animationTimers.push(id);
    return id;
};

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
    
    // Clear any pending return/cleanup timers to prevent conflict/snapping
    clearTimers();
    
    // Animate others being "blown away"
    const others = gamesList.filter(g => g.name !== game.name);
    
    // 1. Others: Blast away + Gray out
    others.forEach(g => {
        // If already blown away (and not returning), maintain current momentum/position
        const current = cardStyles[g.name];
        const isAlreadyOut = current && current.transform && !(current.transform as string).includes('translate(0, 0)');

        if (!isAlreadyOut) {
            // Random direction (complete 360 scatter)
            const angle = Math.random() * Math.PI * 2;
            const distance = 600 + Math.random() * 900;
            
            const tx = Math.cos(angle) * distance;
            const ty = Math.sin(angle) * distance;
            
            // Random rotation for chaotic effect
            const rot = (Math.random() - 0.5) * 180; 

            cardStyles[g.name] = {
                transform: `translate(${tx}px, ${ty}px) rotate(${rot}deg) scale(0.5)`,
                opacity: '0.4', // Fade slightly
                filter: 'grayscale(1) brightness(0.5)', // Turn gray and dark
                // Fast, explosive movement out
                transition: 'transform 0.4s cubic-bezier(0.1, 0.9, 0.2, 1), opacity 0.4s ease, filter 0.4s ease'
            };
        }
    });

    // 2. Selected: Instant Pop + Shake Sequence
    // Phase A: Instant expansion
    cardStyles[game.name] = {
        transform: 'scale(1.5)',
        transition: 'transform 0.1s ease-out',
        zIndex: '200',
        filter: 'brightness(1.5)' // Flash bright
    };

    addTimer(() => {
        // Phase B: Vibration (Shake)
        // using animation property
        cardStyles[game.name] = {
            animation: 'impactShake 0.3s linear', // defined in CSS
            zIndex: '200',
            filter: 'brightness(1.2)'
        };

        // Phase C: Settle to Active State (Slow shrink)
        addTimer(() => {
            cardStyles[game.name] = {
                transform: 'scale(1.2)',
                transition: 'transform 0.6s ease-out',
                zIndex: '200',
                filter: 'none'
            };
            
            // Cleanup selected after settle
            addTimer(() => {
                delete cardStyles[game.name];
            }, 600);
            
        }, 300); // 300ms shake duration
    }, 100);

    // 3. Others: Schedule return (Traction force)
    addTimer(() => {
        others.forEach(g => {
            cardStyles[g.name] = {
                transform: 'translate(0, 0) rotate(0deg) scale(1)',
                opacity: '', // Revert to class-controlled opacity
                filter: 'grayscale(1) brightness(0.5)', // Keep gray during return!
                // Elastic/Springy return
                transition: 'transform 1.0s cubic-bezier(0.34, 1.56, 0.64, 1), opacity 0.8s ease'
            };
        });

        // 4. Others: Recover color with random delay (GRADUAL FADE IN)
        addTimer(() => {
            others.forEach(g => {
                const randomDelay = Math.random() * 1000; // 0-1s random delay
                addTimer(() => {
                    // Transition to color (restore to default dim state)
                    cardStyles[g.name] = {
                         // Ensure we don't jump positions if they are still settling
                         transform: 'translate(0, 0) rotate(0deg) scale(1)', 
                         filter: 'grayscale(0.2) brightness(0.7)',
                         transition: 'filter 1.5s ease-in-out' // Smooth color transition
                    };
                    
                    // Final cleanup
                    addTimer(() => {
                        delete cardStyles[g.name]; 
                    }, 1500);
                }, randomDelay);
            });
        }, 1000); // Wait for return transition
    }, 350); // Wait for blast
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

        <div class="games-grid">
            <div 
                v-for="game in gamesList" 
                :key="game.name"
                class="game-card"
                :class="{ active: appSettings.currentConfigName === game.name }"
                :style="cardStyles[game.name]"
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
                    <div class="game-label">{{ te(`games.${game.name}`) ? t(`games.${game.name}`) : (game.displayName || game.name) }}</div>
                </div>
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

/* Context Menu */
.context-menu {
  position: fixed;
  z-index: 10000;
  background: rgba(30, 30, 30, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(8px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  border-radius: 6px;
  padding: 4px;
  min-width: 140px;
}

.menu-item {
  padding: 8px 12px;
  cursor: pointer;
  color: #eee;
  font-size: 13px;
  border-radius: 4px;
  transition: background-color 0.1s;
}

.menu-item:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.menu-item-danger {
  color: #ff6b6b;
}
.menu-item-danger:hover {
  background-color: rgba(255, 80, 80, 0.2);
  color: #ff4444;
}

/* Import Dialog */
.import-overlay {
  position: fixed;
  top: 0; left: 0;
  width: 100vw; height: 100vh;
  background: rgba(0,0,0,0.6);
  z-index: 20000;
  display: flex;
  align-items: center;
  justify-content: center;
}
.import-dialog {
  background: rgba(30, 30, 30, 0.98);
  border: 1px solid rgba(255,255,255,0.1);
  border-radius: 12px;
  width: 420px;
  max-height: 70vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 8px 32px rgba(0,0,0,0.6);
}
.import-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid rgba(255,255,255,0.08);
  font-size: 16px;
  font-weight: 600;
  color: #fff;
}
.import-close {
  background: none;
  border: none;
  color: #888;
  font-size: 18px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
}
.import-close:hover { color: #fff; background: rgba(255,255,255,0.1); }
.import-body {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}
.import-loading, .import-empty {
  text-align: center;
  color: #888;
  padding: 32px 0;
  font-size: 14px;
}
.import-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.import-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s;
}
.import-item:hover { background: rgba(255,255,255,0.08); }
.import-item-exists { opacity: 0.6; }
.import-icon {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  object-fit: cover;
}
.import-icon-placeholder {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  background: rgba(255,255,255,0.1);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #666;
  font-size: 18px;
}
.import-info { flex: 1; min-width: 0; }
.import-name {
  color: #eee;
  font-size: 14px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.import-name-sub {
  color: #888;
  font-size: 11px;
  margin-top: 2px;
}
.import-badge {
  font-size: 11px;
  color: #F7CE46;
  background: rgba(247,206,70,0.15);
  padding: 2px 8px;
  border-radius: 4px;
  white-space: nowrap;
}
.import-footer {
  padding: 12px 16px;
  border-top: 1px solid rgba(255,255,255,0.08);
  text-align: center;
}
.import-open-folder {
  background: rgba(255,255,255,0.08);
  border: 1px solid rgba(255,255,255,0.15);
  color: #ccc;
  padding: 8px 16px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
}
.import-open-folder:hover { background: rgba(255,255,255,0.15); color: #fff; }

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
    width: 8px;
}
.game-library-container::-webkit-scrollbar-track {
    background: rgba(0, 0, 0, 0.2);
}
.game-library-container::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.15);
    border-radius: 4px;
}
.game-library-container::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.3);
}

.games-grid {
    display: flex;
    flex-wrap: wrap; 
    justify-content: center;
    gap: 30px;
    padding: 20px 60px;
}

/* --- Crystal Icon Styles (Reused & Adapted) --- */

.game-card {
    position: relative;
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    cursor: pointer;
    transition: transform 0.4s cubic-bezier(0.25, 1, 0.5, 1), opacity 0.4s ease, filter 0.4s ease;
    width: 110px; /* Slightly larger for grid */
    opacity: 0.8; /* Slightly dim (visible but allows active to pop) */
    filter: brightness(0.7) grayscale(0.2); 
    transform-origin: center center;
    z-index: 10; /* Ensure cards are above the effects layer */
}

.game-card:hover {
    transition: transform 0.15s ease-out, opacity 0.2s ease, filter 0.2s ease;
    transform: scale(1.1);
    opacity: 1;
    filter: none;
    z-index: 100;
}

.game-card.active {
    opacity: 1;
    filter: none;
    transform: scale(1.05); /* Slight highlight for active */
    z-index: 10;
}

.game-icon-wrapper {
    position: relative;
    width: 90px;
    height: 90px;

    /* Crystal filling texture */
    background: radial-gradient(circle at 50% 0%,
            rgba(255, 255, 255, 0.15) 0%,
            rgba(255, 255, 255, 0.05) 40%,
            rgba(255, 255, 255, 0.02) 100%);
    backdrop-filter: blur(3px);
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 14px;
    padding: 3px;

    display: flex;
    align-items: center;
    justify-content: center;

    /* Constant radiating light */
    box-shadow:
        0 0 12px rgba(130, 200, 255, 0.15),
        inset 0 0 15px rgba(255, 255, 255, 0.1);

    transition: all 0.2s;
    overflow: hidden;
    /* Performance: Disable infinite animation on idle */
    /* animation: crystalPulse 5s ease-in-out infinite; */
}

.game-card:hover .game-icon-wrapper {
    animation: crystalPulse 5s ease-in-out infinite;
    transform: translateY(-2px);
    border-color: rgba(255, 255, 255, 0.6);
    box-shadow: 0 0 25px rgba(150, 220, 255, 0.4), inset 0 0 25px rgba(255, 255, 255, 0.3);
}

.game-card.active .game-icon-wrapper {
    animation: crystalPulse 5s ease-in-out infinite;
}

@keyframes crystalPulse {
    0%, 100% {
        box-shadow: 0 0 12px rgba(130, 200, 255, 0.15), inset 0 0 15px rgba(255, 255, 255, 0.1);
        border-color: rgba(255, 255, 255, 0.25);
    }
    50% {
        box-shadow: 0 0 22px rgba(130, 210, 255, 0.35), inset 0 0 22px rgba(255, 255, 255, 0.25);
        border-color: rgba(255, 255, 255, 0.5);
    }
}

/* Magic fluid/particle flow */
.game-icon-wrapper::before {
    content: "";
    position: absolute;
    top: -50%;
    left: -50%;
    width: 200%;
    height: 200%;
    background:
        conic-gradient(from 0deg at 50% 50%,
            transparent 0deg,
            rgba(255, 255, 255, 0.05) 40deg,
            rgba(100, 200, 255, 0.1) 90deg,
            transparent 135deg,
            rgba(255, 255, 255, 0.05) 200deg,
            transparent 360deg);
    filter: blur(15px);
    /* Performance: Disable infinite animation on idle */
    /* animation: magicRotate 7s linear infinite; */
    z-index: 2;
    pointer-events: none;
    mix-blend-mode: screen;
    opacity: 0; /* Hide by default to save blend cost */
    transition: opacity 0.3s;
}

.game-card:hover .game-icon-wrapper::before,
.game-card.active .game-icon-wrapper::before {
    opacity: 1;
    animation: magicRotate 7s linear infinite;
}

@keyframes magicRotate {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}

/* Crystal reflection sheen */
.game-icon-wrapper::after {
    content: "";
    position: absolute;
    top: 0;
    left: -150%;
    width: 200%;
    height: 100%;
    background: linear-gradient(115deg,
            transparent 40%,
            rgba(255, 255, 255, 0.05) 45%,
            rgba(255, 255, 255, 0.4) 50%,
            rgba(255, 255, 255, 0.05) 55%,
            transparent 60%);
    transform: skewX(-20deg);
    pointer-events: none;
    z-index: 5;
    /* Performance: Disable infinite animation on idle */
    /* animation: subtleSheen 6s ease-in-out infinite; */
}

.game-card:hover .game-icon-wrapper::after,
.game-card.active .game-icon-wrapper::after {
    animation: subtleSheen 6s ease-in-out infinite;
}

@keyframes subtleSheen {
    0% { left: -150%; opacity: 0.3; }
    40% { left: 150%; opacity: 0.3; }
    100% { left: 150%; opacity: 0.3; }
}

.game-card:hover .game-icon-wrapper::after {
    /* Override animation for hover specific behavior if needed, otherwise use keyframes above */
    animation: subtleSheen 2s ease-in-out infinite; /* Faster sheen on hover */
}

/* Stronger hover effects - merged with .game-card:hover .game-icon-wrapper rule above */
/* .game-card:hover .game-icon-wrapper { ... } */

.game-card.active {
    opacity: 1;
    filter: none;
    transform: scale(1.2); /* Moderately larger */
    z-index: 100;
}

/* Radiating Warm White Breathing Light */
.game-card.active::before {
    content: "";
    position: absolute;
    top: 50%; left: 50%;
    /* Significantly increased range */
    width: 250%; height: 250%;
    transform: translate(-50%, -50%);
    
    /* Warm white radial spectrum */
    background: radial-gradient(
        circle closest-side, 
        rgba(255, 250, 230, 0.4) 0%,
        rgba(255, 240, 200, 0.25) 30%,
        rgba(255, 230, 180, 0.1) 60%,
        transparent 80%
    );
    
    z-index: -1;
    border-radius: 50%; /* Soft circular glow */
    filter: blur(20px);
    animation: radiateBreath 4s ease-in-out infinite;
    pointer-events: none; /* Critical: Prevent blocking clicks on adjacent items */
}

/* Remove the sharp second border */
.game-card.active::after {
    display: none;
}

/* No rotation, just pulsing outwards from center */
@keyframes radiateBreath {
    0%, 100% {
        opacity: 0.5;
        transform: translate(-50%, -50%) scale(0.9);
    }
    50% {
        opacity: 0.9;
        transform: translate(-50%, -50%) scale(1.1);
    }
}

.game-card.active .game-icon-wrapper {
    /* Hide the default white border so the rainbow shows nicely around it */
    border-color: rgba(255, 255, 255, 0.2); 
    /* Add an inner glow to blend with the outer rainbow */
    box-shadow: inset 0 0 20px rgba(255, 255, 255, 0.5);
}

.game-icon {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 12px;
    display: block;
    z-index: 1;
    position: relative;
}

.game-label {
    position: absolute;
    left: 0;
    bottom: 0;
    width: 100%;
    text-align: center;
    font-size: 11px;
    font-weight: 600;
    color: #fff;
    text-shadow: 0 1px 3px rgba(0, 0, 0, 0.9);
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(2px);
    padding: 3px 0;
    line-height: 1.2;
    z-index: 10;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-bottom-left-radius: 14px;
    border-bottom-right-radius: 14px;
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