<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { BgHeart, MeteorStar, MeteorWrapperStyle, Particle } from './types';

const props = defineProps<{
  bgHearts: BgHeart[];
  meteorStars: MeteorStar[];
  particles: Particle[];
  getMeteorWrapperStyle: (star: MeteorStar) => MeteorWrapperStyle;
}>();

const HEART_PATH = 'M23.6,0c-3.4,0-6.3,2.7-7.6,5.6C14.7,2.7,11.8,0,8.4,0C3.8,0,0,3.8,0,8.4c0,9.4,9.5,11.9,16,21.2 c6.1-9.3,16-11.8,16-21.2C32,3.8,28.2,0,23.6,0z';
const HEART_DURATION_MS = 2000;
const PARTICLE_DURATION_MS = 1500;
const MAX_DEVICE_PIXEL_RATIO = 2;

const canvasRef = ref<HTMLCanvasElement | null>(null);
let ctx: CanvasRenderingContext2D | null = null;
let heartPath: Path2D | null = null;
let viewportWidth = 0;
let viewportHeight = 0;
let rafId = 0;
let isMounted = false;
let isDocumentHidden = false;

const heartBirthTimes = new Map<number, number>();
const particleBirthTimes = new Map<number, number>();
const meteorBirthTimes = new Map<number, number>();

const hasActiveEffects = computed(
  () => props.bgHearts.length > 0 || props.particles.length > 0 || props.meteorStars.length > 0,
);

const effectSignature = computed(() => [
  props.bgHearts.map((heart) => heart.id).join(','),
  props.particles.map((particle) => particle.id).join(','),
  props.meteorStars.map((star) => star.id).join(','),
].join('|'));

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));
const easeOutCubic = (progress: number) => 1 - ((1 - progress) ** 3);
const easeOutQuad = (progress: number) => 1 - ((1 - progress) ** 2);

const parseStyleNumber = (style: Particle['style'], key: string, fallback: number) => {
  const styleMap = style as Record<string, unknown>;
  const rawValue = styleMap[key];
  if (typeof rawValue === 'number' && Number.isFinite(rawValue)) return rawValue;
  if (typeof rawValue === 'string') {
    const parsed = Number.parseFloat(rawValue);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
};

const parseStyleString = (style: Particle['style'], key: string, fallback: string) => {
  const styleMap = style as Record<string, unknown>;
  const rawValue = styleMap[key];
  return typeof rawValue === 'string' && rawValue.length > 0 ? rawValue : fallback;
};

const parseDurationMs = (value: string, fallbackMs: number) => {
  const normalized = value.trim();
  if (!normalized) return fallbackMs;
  if (normalized.endsWith('ms')) {
    const parsed = Number.parseFloat(normalized);
    return Number.isFinite(parsed) ? parsed : fallbackMs;
  }
  if (normalized.endsWith('s')) {
    const parsed = Number.parseFloat(normalized);
    return Number.isFinite(parsed) ? parsed * 1000 : fallbackMs;
  }
  const parsed = Number.parseFloat(normalized);
  return Number.isFinite(parsed) ? parsed : fallbackMs;
};

const resizeCanvas = () => {
  const canvas = canvasRef.value;
  if (!canvas) return;

  const pixelRatio = Math.min(window.devicePixelRatio || 1, MAX_DEVICE_PIXEL_RATIO);
  viewportWidth = window.innerWidth;
  viewportHeight = window.innerHeight;

  canvas.width = Math.max(1, Math.round(viewportWidth * pixelRatio));
  canvas.height = Math.max(1, Math.round(viewportHeight * pixelRatio));
  canvas.style.width = `${viewportWidth}px`;
  canvas.style.height = `${viewportHeight}px`;

  ctx = canvas.getContext('2d');
  if (!ctx) return;

  ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  ctx.imageSmoothingEnabled = true;
  drawFrame(performance.now());
};

const clearCanvas = () => {
  if (!ctx) return;
  ctx.clearRect(0, 0, viewportWidth, viewportHeight);
};

const syncBirthTimes = (now = performance.now()) => {
  const syncMap = <T extends { id: number }>(target: Map<number, number>, items: T[]) => {
    const activeIds = new Set(items.map((item) => item.id));
    for (const item of items) {
      if (!target.has(item.id)) {
        target.set(item.id, now);
      }
    }
    for (const existingId of target.keys()) {
      if (!activeIds.has(existingId)) {
        target.delete(existingId);
      }
    }
  };

  syncMap(heartBirthTimes, props.bgHearts);
  syncMap(particleBirthTimes, props.particles);
  syncMap(meteorBirthTimes, props.meteorStars);
};

const drawHeart = (heart: BgHeart, now: number) => {
  if (!ctx || !heartPath) return;

  const bornAt = heartBirthTimes.get(heart.id) ?? now;
  const progress = clamp((now - bornAt) / HEART_DURATION_MS, 0, 1);
  const scale = 0.5 + easeOutQuad(progress);
  const opacity = progress < 0.2
    ? heart.opacity * (progress / 0.2)
    : heart.opacity * (1 - ((progress - 0.2) / 0.8));

  if (opacity <= 0.001) return;

  ctx.save();
  ctx.globalAlpha = opacity;
  ctx.translate(heart.x, heart.y);
  ctx.rotate((heart.rotation * Math.PI) / 180);
  ctx.scale((heart.size / 32) * scale, (heart.size / 32) * scale);
  ctx.translate(-16, -14.8);
  ctx.fillStyle = heart.color;
  ctx.shadowColor = heart.color;
  ctx.shadowBlur = 18;
  ctx.fill(heartPath);
  ctx.restore();
};

const drawParticle = (particle: Particle, now: number) => {
  if (!ctx) return;

  const bornAt = particleBirthTimes.get(particle.id) ?? now;
  const progress = clamp((now - bornAt) / PARTICLE_DURATION_MS, 0, 1);
  const eased = easeOutCubic(progress);
  const tx = parseStyleNumber(particle.style, '--tx', 0);
  const ty = parseStyleNumber(particle.style, '--ty', 0);
  const x = particle.x + tx * eased;
  const y = particle.y + ty * eased;
  const scale = 0.3 + eased * 1.2;
  const opacity = progress < 0.5 ? 1 : 1 - ((progress - 0.5) / 0.5);
  const color = parseStyleString(particle.style, 'color', '#ff9fbf');
  const fontSize = parseStyleNumber(particle.style, 'fontSize', 16);
  const fontWeight = parseStyleString(particle.style, 'fontWeight', '700');

  if (opacity <= 0.001) return;

  ctx.save();
  ctx.globalAlpha = opacity;
  ctx.translate(x, y);
  ctx.scale(scale, scale);
  ctx.fillStyle = color;
  ctx.font = `${fontWeight} ${fontSize}px "Noto Sans SC", "PingFang SC", sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.shadowColor = color;
  ctx.shadowBlur = 8;
  ctx.fillText(particle.text, 0, 0);
  ctx.restore();
};

const drawMeteor = (star: MeteorStar, now: number) => {
  if (!ctx) return;

  const bornAt = meteorBirthTimes.get(star.id) ?? now;
  const flyDurationMs = Math.max(1, parseDurationMs(star.flyDuration, 1800));
  const spinDurationMs = Math.max(1, parseDurationMs(star.rotationDuration, 400));
  const flickerDurationMs = Math.max(1, parseDurationMs(star.flickerDuration, 220));
  const progress = clamp((now - bornAt) / flyDurationMs, 0, 1);
  const x = star.x + (star.tx * progress);
  const y = star.y + (star.ty * progress);
  const spinAngle = ((now - bornAt) / spinDurationMs) * Math.PI * 2;
  const flickerPhase = ((now - bornAt) / flickerDurationMs) * Math.PI * 2;
  const opacity = 0.65 + (0.35 * ((Math.sin(flickerPhase) + 1) / 2));
  const directionX = Math.cos(star.angle);
  const directionY = Math.sin(star.angle);
  const trailLength = star.size * 2.6;

  const trailGradient = ctx.createLinearGradient(
    x - (directionX * trailLength),
    y - (directionY * trailLength),
    x,
    y,
  );
  trailGradient.addColorStop(0, 'rgba(255, 255, 255, 0)');
  trailGradient.addColorStop(0.55, `${star.color}33`);
  trailGradient.addColorStop(1, `${star.color}dd`);

  ctx.save();
  ctx.globalAlpha = opacity;
  ctx.strokeStyle = trailGradient;
  ctx.lineCap = 'round';
  ctx.lineWidth = Math.max(2, star.size * 0.18);
  ctx.beginPath();
  ctx.moveTo(x - (directionX * trailLength), y - (directionY * trailLength));
  ctx.lineTo(x, y);
  ctx.stroke();
  ctx.restore();

  ctx.save();
  ctx.globalAlpha = opacity;
  ctx.translate(x, y);
  ctx.rotate(star.angle + spinAngle);
  ctx.fillStyle = star.color;
  ctx.font = `${Math.max(18, star.size)}px "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.shadowColor = star.color;
  ctx.shadowBlur = Math.max(10, star.size * 0.45);
  ctx.fillText(star.emoji, 0, 0);
  ctx.restore();
};

const drawFrame = (now: number) => {
  if (!ctx || viewportWidth <= 0 || viewportHeight <= 0) return;

  clearCanvas();

  for (const heart of props.bgHearts) {
    drawHeart(heart, now);
  }

  for (const particle of props.particles) {
    drawParticle(particle, now);
  }

  for (const star of props.meteorStars) {
    drawMeteor(star, now);
  }
};

const stopAnimation = () => {
  if (!rafId) return;
  window.cancelAnimationFrame(rafId);
  rafId = 0;
};

const animationTick = (now: number) => {
  rafId = 0;
  drawFrame(now);

  if (!isMounted || isDocumentHidden || !hasActiveEffects.value) return;
  rafId = window.requestAnimationFrame(animationTick);
};

const startAnimation = () => {
  if (!isMounted || isDocumentHidden || rafId || !hasActiveEffects.value) return;
  drawFrame(performance.now());
  rafId = window.requestAnimationFrame(animationTick);
};

const handleVisibilityChange = () => {
  isDocumentHidden = document.hidden;
  if (isDocumentHidden) {
    stopAnimation();
    return;
  }
  if (hasActiveEffects.value) {
    startAnimation();
  } else {
    drawFrame(performance.now());
  }
};

watch(
  effectSignature,
  () => {
    syncBirthTimes();
    if (!isMounted) return;
    if (hasActiveEffects.value) {
      startAnimation();
    } else {
      stopAnimation();
      clearCanvas();
    }
  },
  { immediate: true },
);

watch(
  hasActiveEffects,
  (active) => {
    if (!isMounted) return;
    if (active) {
      startAnimation();
    } else {
      stopAnimation();
      clearCanvas();
    }
  },
  { immediate: true },
);

onMounted(() => {
  isMounted = true;
  isDocumentHidden = document.hidden;
  heartPath = new Path2D(HEART_PATH);
  resizeCanvas();
  syncBirthTimes();
  window.addEventListener('resize', resizeCanvas, { passive: true });
  document.addEventListener('visibilitychange', handleVisibilityChange, { passive: true });
  if (hasActiveEffects.value) {
    startAnimation();
  }
});

onBeforeUnmount(() => {
  isMounted = false;
  stopAnimation();
  window.removeEventListener('resize', resizeCanvas);
  document.removeEventListener('visibilitychange', handleVisibilityChange);
  heartBirthTimes.clear();
  particleBirthTimes.clear();
  meteorBirthTimes.clear();
});
</script>

<template>
  <canvas ref="canvasRef" class="effects-canvas"></canvas>
</template>

<style scoped>
.effects-canvas {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  pointer-events: none;
  z-index: 40;
  contain: strict;
  will-change: transform, opacity;
  transform: translateZ(0);
}
</style>
