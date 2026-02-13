<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { gamesList, switchToGame, appSettings, isDrawerOpen } from '../store';

const scrollContainer = ref<HTMLElement | null>(null);
let closeTimer: ReturnType<typeof setTimeout> | null = null;

// Drawer Logic
const startAutoCloseTimer = () => {
    if (closeTimer) clearTimeout(closeTimer);
    closeTimer = setTimeout(() => {
        isDrawerOpen.value = false;
    }, 3000); // 3 seconds of inactivity closes the drawer
};

// Watch global drawer state to manage timer
watch(isDrawerOpen, (isOpen) => {
    if (isOpen) {
        startAutoCloseTimer();
    } else {
        if (closeTimer) {
            clearTimeout(closeTimer);
            closeTimer = null;
        }
    }
});

const onWindowMouseDown = (e: MouseEvent) => {
    // 1. Check if clicking inside the drawer (ignore closure, allow interaction)
    const drawer = document.querySelector('.games-drawer-panel');
    const target = e.target as HTMLElement;

    if (drawer && drawer.contains(target)) {
        return;
    }

    // 2. Click on TitleBar area
    // If click target is inside TitleBar, let's play safe and NOT trigger close, 
    // to avoid conflict with the toggle button click inside App.vue/TitleBar.vue
    if (e.clientY <= 40) {
        return;
    }

    // 3. Click elsewhere (Content area) -> Close
    if (isDrawerOpen.value) {
        isDrawerOpen.value = false;
    }
};

const onDrawerMouseMove = () => {
    // If mouse is moving inside drawer, keep it open and reset timer
    if (!isDrawerOpen.value) isDrawerOpen.value = true;
    startAutoCloseTimer();
};

const onWheel = (e: WheelEvent) => {
    // Scrolling counts as activity
    startAutoCloseTimer();

    if (scrollContainer.value) {
        e.preventDefault();
        // Increase scroll distance per wheel tick for faster horizontal navigation
        scrollContainer.value.scrollLeft += e.deltaY * 3;
    }
};

const handleGameSelect = (game: any) => {
    console.log('[GamesDrawer] select game', game && game.name);
    switchToGame(game);
    startAutoCloseTimer(); // Keep open for a moment after select
};

onMounted(() => {
    // Using mousedown to capture interactions on the TitleBar even if dragging starts
    window.addEventListener('mousedown', onWindowMouseDown, true);
});

onUnmounted(() => {
    window.removeEventListener('mousedown', onWindowMouseDown, true);
    if (closeTimer) clearTimeout(closeTimer);
});
</script>

<template>
    <Teleport to="body">
        <Transition name="drawer-slide">
            <div 
                v-if="isDrawerOpen"
                class="games-drawer-panel"
                @mousemove="onDrawerMouseMove"
                @mouseleave="startAutoCloseTimer"
            >
                <div class="games-slider-wrapper">
                    <div 
                        ref="scrollContainer" 
                        class="games-slider"
                        @wheel="onWheel"
                    >
                        <div 
                            v-for="game in gamesList" 
                            :key="game.name"
                            class="game-card"
                            :class="{ active: appSettings.currentConfigName === game.name }"
                            @click="handleGameSelect(game)"
                        >
                            <div class="game-icon-wrapper">
                                <img :src="game.iconPath" class="game-icon" alt="icon" />
                                <div class="game-label">{{ game.name }}</div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.games-drawer-panel {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    z-index: 9000;
    /* High enough to be above content, but below TitleBar (9999) if we want interaction */
    /* Wait, TitleBar is transparent region but has buttons. 
       If we want to interact with drawer, it must be clickable.
       If we put it below TitleBar visually, we add padding.
    */
    padding-top: 35px;
    /* Clear TitleBar height */
    background: linear-gradient(to bottom, rgba(0, 0, 0, 0.8) 0%, rgba(0, 0, 0, 0) 100%);
    /* Optional shadow gradient */
    display: flex;
    justify-content: center;
}

/* Transition for the drawer */
.drawer-slide-enter-active,
.drawer-slide-leave-active {
    transition: transform 0.3s ease, opacity 0.3s ease;
}

.drawer-slide-enter-from,
.drawer-slide-leave-to {
    transform: translateY(-100%);
    opacity: 0;
}

.games-slider-wrapper {
    width: 100%;
    min-width: 0;
    max-width: none;
    /* Reduced vertical padding to fit larger icons */
    padding: 10px 40px;
    overflow: hidden;
    background: transparent;
    backdrop-filter: none;
    border: none;
    box-shadow: none;
}


.games-slider {
    display: flex;
    justify-content: center;
    gap: 12px;
    overflow-x: auto;
    overflow-y: visible;
    /* Reduced vertical padding, added horizontal safe area */
    padding: 8px 40px;
    scroll-behavior: smooth;
    /* Hide scrollbar for cleaner look */
    scrollbar-width: none;
    -ms-overflow-style: none;
    scrollbar-color: transparent transparent;
    align-items: center;
}

.games-slider::-webkit-scrollbar {
    display: none;
    width: 0;
    height: 0;
}

.game-card {
    position: relative;
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    cursor: pointer;
    /* 离开时：延迟 0.2s 后再开始慢速复原 */
    transition: transform 0.5s cubic-bezier(0.25, 1, 0.5, 1) 0.2s, opacity 0.5s ease 0.2s, filter 0.5s ease 0.2s;
    border-radius: 8px;
    justify-content: center;
    width: 100px;
    /* height: 100px;  Removed fixed height to accommodate label flow */
    opacity: 0.7;
    /* Dusty/Sealed look: Darker, desaturated - REMOVED BLUR as requested */
    filter: brightness(0.6) grayscale(0.4); 
    transform-origin: center top;
}

.game-card:hover {
    /* 进入时：立即响应，没有延迟 */
    transition: transform 0.15s ease-out 0s, opacity 0.2s ease 0s, filter 0.2s ease 0s;
    transform: scale(1.1);
    opacity: 1;
    /* Restore clarity */
    filter: none;
}

.game-card.active {
    transform: none;
    opacity: 1;
    filter: none;
    z-index: 10;
}

.game-icon-wrapper {
    position: relative;
    width: 80px;
    height: 80px;

    /* Crystal filling texture: Refractive glass look */
    background: radial-gradient(circle at 50% 0%,
            rgba(255, 255, 255, 0.15) 0%,
            rgba(255, 255, 255, 0.05) 40%,
            rgba(255, 255, 255, 0.02) 100%);
    backdrop-filter: blur(3px);

    /* Delicate but distinct border */
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 12px;

    /* Padding creates the "encased" look */
    padding: 2px;

    display: flex;
    align-items: center;
    justify-content: center;

    /* Constant radiating light (Flowing Light Overflowing) */
    box-shadow:
        0 0 12px rgba(130, 200, 255, 0.15),
        inset 0 0 15px rgba(255, 255, 255, 0.1);

    transition: transform 0.2s, border-color 0.2s, box-shadow 0.2s;
    overflow: hidden;

    /* Breathing light effect */
    animation: crystalPulse 4s ease-in-out infinite;
}

@keyframes crystalPulse {

    0%,
    100% {
        box-shadow:
            0 0 12px rgba(130, 200, 255, 0.15),
            inset 0 0 15px rgba(255, 255, 255, 0.1);
        border-color: rgba(255, 255, 255, 0.25);
    }

    50% {
        box-shadow:
            0 0 20px rgba(130, 210, 255, 0.3),
            inset 0 0 20px rgba(255, 255, 255, 0.2);
        border-color: rgba(255, 255, 255, 0.45);
    }
}

/* Pseudo-element for magic fluid/particle flow */
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
    animation: magicRotate 6s linear infinite;
    /* Slightly faster rotation */
    z-index: 2;
    pointer-events: none;
    mix-blend-mode: screen;
}

@keyframes magicRotate {
    0% {
        transform: rotate(0deg);
    }

    100% {
        transform: rotate(360deg);
    }
}

/* Crystal reflection sheen - constantly moving */
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
    animation: subtleSheen 5s ease-in-out infinite;
}

@keyframes subtleSheen {
    0% {
        left: -150%;
        opacity: 0.3;
    }

    50% {
        opacity: 0.6;
    }

    100% {
        left: 150%;
        opacity: 0.3;
    }
}

.game-card:hover .game-icon-wrapper::after {
    left: 150%;
    /* Swipe the reflection across */
    transition: left 0.6s ease-in-out;
}

.game-card:hover .game-icon-wrapper {
    transform: translateY(-4px);
    /* Brighter crystal effect on hover */
    border-color: rgba(255, 255, 255, 0.5);
    box-shadow: inset 0 0 20px rgba(255, 255, 255, 0.2);
}

.game-card.active .game-icon-wrapper {
    /* Bright glossy border for active state */
    border-color: rgba(255, 255, 255, 0.9);
    box-shadow: inset 0 0 15px rgba(255, 255, 255, 0.3), 0 0 10px rgba(255, 255, 255, 0.3);
}

.game-icon {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 11px;
    /* Tighter radius for smaller gap (12-1=11) */
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
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
    background: rgba(0, 0, 0, 0.65);
    /* Dark semi-transparent bar */
    backdrop-filter: blur(2px);

    padding: 2px 0;
    /* Minimal vertical padding */
    margin: 0;

    line-height: 1.1;
    z-index: 10;
    /* Above all crystal effects */
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-bottom-left-radius: 11px;
    /* Match wrapper radius - 1px border */
    border-bottom-right-radius: 11px;
}
</style>