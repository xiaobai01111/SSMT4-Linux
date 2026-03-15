<script setup lang="ts">
import type { BgHeart, MeteorStar, MeteorWrapperStyle, Particle } from './types';

defineProps<{
  bgHearts: BgHeart[];
  meteorStars: MeteorStar[];
  particles: Particle[];
  getMeteorWrapperStyle: (star: MeteorStar) => MeteorWrapperStyle;
}>();
</script>

<template>
  <div class="effects-layer">
    <div
      v-for="heart in bgHearts"
      :key="heart.id"
      class="bg-heart"
      :style="{
        left: `${heart.x}px`,
        top: `${heart.y}px`,
        width: `${heart.size}px`,
        height: `${heart.size}px`,
        transform: `translate(-50%, -50%) rotate(${heart.rotation}deg)`,
        opacity: heart.opacity,
      }"
    >
      <svg viewBox="0 0 32 29.6" fill="currentColor">
        <path d="M23.6,0c-3.4,0-6.3,2.7-7.6,5.6C14.7,2.7,11.8,0,8.4,0C3.8,0,0,3.8,0,8.4c0,9.4,9.5,11.9,16,21.2
        c6.1-9.3,16-11.8,16-21.2C32,3.8,28.2,0,23.6,0z"/>
      </svg>
    </div>
  </div>

  <div class="meteor-layer">
    <div
      v-for="star in meteorStars"
      :key="star.id"
      class="meteor-star-wrapper"
      :style="getMeteorWrapperStyle(star)"
    >
      <div
        class="meteor-star-inner"
        :style="{
          color: star.color,
          fontSize: `${star.size}px`,
          animation: `starSpin ${star.rotationDuration} linear infinite, starFlicker ${star.flickerDuration} ease-in-out infinite alternate`,
          textShadow: `0 0 10px ${star.color}`,
        }"
      >{{ star.emoji }}</div>
    </div>
  </div>

  <div class="particles-layer">
    <div
      v-for="particle in particles"
      :key="particle.id"
      class="love-particle"
      :style="{
        left: `${particle.x}px`,
        top: `${particle.y}px`,
        ...particle.style,
      }"
    >
      {{ particle.text }}
    </div>
  </div>
</template>

<style scoped>
.effects-layer,
.particles-layer {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  pointer-events: none;
  z-index: 5;
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

.meteor-layer {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 300;
}

.meteor-star-wrapper {
  position: absolute;
  animation: flyAcross linear forwards;
}

.meteor-star-inner {
  display: inline-block;
  width: 100%;
  height: 100%;
  line-height: 1;
  font-weight: bold;
  transform-origin: 50% 55%;
  text-align: center;
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
</style>
