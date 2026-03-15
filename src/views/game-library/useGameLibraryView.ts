import { computed, onMounted, onUnmounted, ref, type CSSProperties } from 'vue';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { gamesList, gamesLoading, switchToGame, appSettings, loadGames } from '../../store';
import {
  setGameVisibility,
  deleteGameConfigFolder,
  askConfirm,
  listGameTemplates,
  importGameTemplate,
  getGameTemplatesDir,
  type GameTemplateInfo,
} from '../../api';
import type { GameInfo } from '../../types/ipc';
import type { BgHeart, MeteorStar, MeteorWrapperStyle, Particle } from './types';

const colors = ['#ff7eb3', '#ff758c', '#ff7eb3', '#fgbdff', '#ff9999', '#ffffff'];
const starColors = ['#ff0000', '#ffaf00', '#ffff00', '#00ff00', '#0000ff', '#4b0082', '#8f00ff', '#ff00ff', '#00ffff'];
const meteorEmojis = ['⭐', '🌟', '💫', '✨', '☄️', '🪐', '🦄', '🌈', '🍭', '🌸', '🍩', '🍪', '🍕', '🚀', '🛸', '🧚', '💎', '🍄', '🐱', '🐶'];

export function useGameLibraryView() {
  const { t, te } = useI18n();
  const router = useRouter();

  const showMenu = ref(false);
  const menuX = ref(0);
  const menuY = ref(0);
  const targetGame = ref<GameInfo | null>(null);

  const searchQuery = ref('');

  const filteredGames = computed(() => {
    if (!searchQuery.value) return gamesList;
    const q = searchQuery.value.toLowerCase();
    return gamesList.filter((g) => {
      const name = g.name.toLowerCase();
      const display = (
        te(`games.${g.name}`) ? t(`games.${g.name}`) : (g.displayName || g.name)
      ).toLowerCase();
      return name.includes(q) || display.includes(q);
    });
  });

  const emptyStateText = computed(() => {
    if (gamesLoading.value && !searchQuery.value) {
      return 'Scanning game library...';
    }
    return 'No matches found.';
  });

  const handleContextMenu = (e: MouseEvent, game: GameInfo) => {
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
    try {
      await setGameVisibility(targetGame.value.name, false);
      await loadGames();
      await router.push('/');
    } catch (err) {
      console.error('Failed to add game to favorites:', err);
    }
    closeMenu();
  };

  const deleteGame = async () => {
    if (!targetGame.value) return;
    const gameName = targetGame.value.name;
    const displayName = te(`games.${gameName}`) ? t(`games.${gameName}`) : gameName;
    const yes = await askConfirm(
      t('gamelibrary.deleteConfirmMsg', { name: displayName }),
      { title: t('gamelibrary.deleteConfirmTitle'), kind: 'warning' },
    );
    if (!yes) {
      closeMenu();
      return;
    }
    try {
      await deleteGameConfigFolder(gameName);
      await loadGames();
    } catch (err) {
      console.error('Failed to delete game:', err);
    }
    closeMenu();
  };

  const showBlankMenu = ref(false);
  const blankMenuX = ref(0);
  const blankMenuY = ref(0);

  const handleBlankContextMenu = (e: MouseEvent) => {
    if ((e.target as HTMLElement).closest('.game-card')) return;
    e.preventDefault();
    blankMenuX.value = e.clientX;
    blankMenuY.value = e.clientY;
    showBlankMenu.value = true;
  };

  const closeBlankMenu = () => {
    showBlankMenu.value = false;
  };

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
        { title: t('gamelibrary.importOverwriteTitle'), kind: 'warning' },
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

  const closeImportDialog = () => {
    showImportDialog.value = false;
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

  const particles = ref<Particle[]>([]);
  const bgHearts = ref<BgHeart[]>([]);
  const meteorStars = ref<MeteorStar[]>([]);
  let particleId = 0;
  let meteorId = 0;
  const consecutiveClickCount = ref(0);
  const lastClickedGameId = ref('');

  const spawnMeteorStars = () => {
    const count = 3 + Math.floor(Math.random() * 4);
    const w = window.innerWidth;
    const h = window.innerHeight;

    for (let i = 0; i < count; i++) {
      const edge = Math.floor(Math.random() * 4);
      let startX = 0;
      let startY = 0;
      let endX = 0;
      let endY = 0;

      switch (edge) {
        case 0:
          startX = Math.random() * w; startY = -80; endX = Math.random() * w; endY = h + 150; break;
        case 1:
          startX = w + 80; startY = Math.random() * h; endX = -150; endY = Math.random() * h; break;
        case 2:
          startX = Math.random() * w; startY = h + 80; endX = Math.random() * w; endY = -150; break;
        default:
          startX = -80; startY = Math.random() * h; endX = w + 150; endY = Math.random() * h; break;
      }

      const tx = endX - startX;
      const ty = endY - startY;
      const angle = Math.atan2(ty, tx);
      const durationSec = 1.2 + Math.random() * 1.3;

      const star: MeteorStar = {
        id: meteorId++,
        x: startX,
        y: startY,
        tx,
        ty,
        angle,
        color: starColors[Math.floor(Math.random() * starColors.length)],
        emoji: meteorEmojis[Math.floor(Math.random() * meteorEmojis.length)],
        rotationDuration: `${0.2 + Math.random() * 0.3}s`,
        flickerDuration: `${0.1 + Math.random() * 0.2}s`,
        flyDuration: `${durationSec}s`,
        size: 24 + Math.random() * 32,
        trail: [],
      };

      meteorStars.value.push(star);
      setTimeout(() => {
        meteorStars.value = meteorStars.value.filter((item) => item.id !== star.id);
      }, durationSec * 1000 + 100);
    }
  };

  const spawnLoveExplosion = (e: MouseEvent) => {
    const x = e.clientX;
    const y = e.clientY;

    bgHearts.value.push({
      id: particleId++,
      x,
      y,
      size: 300 + Math.random() * 300,
      rotation: (Math.random() - 0.5) * 60,
      color: '#ff7eb3',
      opacity: 0.1 + Math.random() * 0.2,
    });

    setTimeout(() => {
      bgHearts.value.shift();
    }, 2000);

    for (let i = 0; i < 15; i++) {
      const angle = Math.random() * Math.PI * 2;
      const velocity = 100 + Math.random() * 200;
      const tx = Math.cos(angle) * velocity;
      const ty = Math.sin(angle) * velocity;
      const color = colors[Math.floor(Math.random() * colors.length)];
      const kaomojis = [
        '(｡♥‿♥｡)', '( ˘ ³˘)♥', 'OwO', 'UwU', '(*♡∀♡)',
        '(◕‿◕✿)', '♥', '❤', '❥', '(≧◡≦)', '(>ω<)',
        'Ciallo～(∠・ω< )⌒☆', 'Ciallo', '(∠・ω< )⌒★',
        '(・ω< )', '☆⌒(ゝ。∂)', '(★^O^★)',
      ];
      const particle: Particle = {
        id: particleId++,
        x,
        y,
        text: kaomojis[Math.floor(Math.random() * kaomojis.length)],
        style: {
          '--tx': `${tx}px`,
          '--ty': `${ty}px`,
          color,
          fontSize: `${12 + Math.random() * 16}px`,
          fontWeight: 'bold',
          textShadow: '0 0 5px rgba(255,100,100,0.5)',
        } as CSSProperties,
      };

      particles.value.push(particle);
      setTimeout(() => {
        particles.value = particles.value.filter((item) => item.id !== particle.id);
      }, 1500);
    }
  };

  const handleGameSelect = (game: GameInfo, event: MouseEvent) => {
    if (lastClickedGameId.value === game.name) {
      consecutiveClickCount.value++;
    } else {
      lastClickedGameId.value = game.name;
      consecutiveClickCount.value = 1;
    }

    if (consecutiveClickCount.value >= 3) {
      spawnMeteorStars();
    }

    if (appSettings.currentConfigName === game.name) {
      spawnLoveExplosion(event);
    }

    switchToGame(game);
  };

  const getMeteorWrapperStyle = (star: MeteorStar): MeteorWrapperStyle => ({
    left: `${star.x}px`,
    top: `${star.y}px`,
    '--tx': `${star.tx}px`,
    '--ty': `${star.ty}px`,
    animationDuration: star.flyDuration,
  });

  return {
    addToFavorites,
    appSettings,
    bgHearts,
    blankMenuX,
    blankMenuY,
    closeImportDialog,
    deleteGame,
    emptyStateText,
    filteredGames,
    gamesList,
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
    targetGame,
    t,
    te,
    templateList,
  };
}
