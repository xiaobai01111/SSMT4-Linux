import type { CSSProperties } from 'vue';

export interface Particle {
  id: number;
  x: number;
  y: number;
  text: string;
  style: CSSProperties;
}

export interface BgHeart {
  id: number;
  x: number;
  y: number;
  size: number;
  rotation: number;
  color: string;
  opacity: number;
}

export interface MeteorStar {
  id: number;
  x: number;
  y: number;
  tx: number;
  ty: number;
  angle: number;
  color: string;
  emoji: string;
  rotationDuration: string;
  flickerDuration: string;
  flyDuration: string;
  size: number;
  trail: Array<{ x: number; y: number; s: number; o: number }>;
}

export type MeteorWrapperStyle = CSSProperties & { '--tx': string; '--ty': string };
