import type { GameState } from '../api';

export type GameUpdateCheckPhase = 'idle' | 'checking' | 'ready' | 'error';

export interface GameUpdateCheckState {
  phase: GameUpdateCheckPhase;
  state: GameState | null;
  error: string;
  updatedAt: number;
  requestId: number;
}

export function createGameUpdateCheckState(): GameUpdateCheckState {
  return {
    phase: 'idle',
    state: null,
    error: '',
    updatedAt: 0,
    requestId: 0,
  };
}

export function beginGameUpdateCheck(
  requestId: number,
  updatedAt: number,
): Pick<GameUpdateCheckState, 'phase' | 'error' | 'updatedAt' | 'requestId'> {
  return {
    phase: 'checking',
    error: '',
    updatedAt,
    requestId,
  };
}

export function shouldApplyGameUpdateResult(
  state: Pick<GameUpdateCheckState, 'requestId'>,
  requestId: number,
): boolean {
  return state.requestId === requestId;
}

export function buildIdleGameUpdateCheckPatch(
  updatedAt: number,
): Pick<GameUpdateCheckState, 'phase' | 'state' | 'error' | 'updatedAt'> {
  return {
    phase: 'idle',
    state: null,
    error: '',
    updatedAt,
  };
}

export function buildReadyGameUpdateCheckPatch(
  gameState: GameState,
  updatedAt: number,
): Pick<GameUpdateCheckState, 'phase' | 'state' | 'error' | 'updatedAt'> {
  return {
    phase: 'ready',
    state: gameState,
    error: '',
    updatedAt,
  };
}

export function normalizeGameUpdateError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error || '');
}

export function buildErrorGameUpdateCheckPatch(
  error: unknown,
  updatedAt: number,
): Pick<GameUpdateCheckState, 'phase' | 'state' | 'error' | 'updatedAt'> {
  return {
    phase: 'error',
    state: null,
    error: normalizeGameUpdateError(error),
    updatedAt,
  };
}
