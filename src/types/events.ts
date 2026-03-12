export type ComponentDownloadPhase = 'downloading' | 'extracting' | 'done';

export interface ComponentDownloadProgressEvent {
  componentId: string;
  componentName?: string | null;
  phase: ComponentDownloadPhase;
  downloaded: number;
  total: number;
}

export type GameDownloadOperation =
  | 'download-game'
  | 'update-game'
  | 'update-game-patch'
  | 'download-launcher-installer'
  | 'update-launcher-installer'
  | 'verify-game'
  | 'repair-game';

export interface GameDownloadProgressEvent {
  task_id: string;
  operation: GameDownloadOperation;
  phase: string;
  total_size: number;
  finished_size: number;
  total_count: number;
  finished_count: number;
  current_file: string;
  speed_bps: number;
  eta_seconds: number;
}

export interface GameLifecycleStartedEvent {
  event: 'started';
  game: string;
  pid: number;
  runner: string;
  region: string;
  launchedAt: string;
  configuredExe: string;
  launchExe: string;
  runnerExe: string;
  commandProgram: string;
}

export interface GameLifecycleExitedEvent {
  event: 'exited';
  game: string;
  pid: number;
  runner: string;
  region: string;
  launchedAt: string;
  finishedAt: string;
  exitCode: number | null;
  signal: number | null;
  crashed: boolean;
}

export interface BridgeStatusLifecycleEvent {
  event: 'bridge-status';
  game: string;
  message: string;
}

export interface BridgeProgressLifecycleEvent {
  event: 'bridge-progress';
  game: string;
  stage: string;
  current: number;
  total: number;
}

export interface BridgeErrorLifecycleEvent {
  event: 'bridge-error';
  game: string;
  code: string;
  message: string;
}

export type GameLifecycleEvent =
  | GameLifecycleStartedEvent
  | GameLifecycleExitedEvent
  | BridgeStatusLifecycleEvent
  | BridgeProgressLifecycleEvent
  | BridgeErrorLifecycleEvent;

export interface GameAnticheatWarningEvent {
  message: string;
}

export interface AppEventMap {
  'component-download-progress': ComponentDownloadProgressEvent;
  'game-download-progress': GameDownloadProgressEvent;
  'game-lifecycle': GameLifecycleEvent;
  'game-anticheat-warning': GameAnticheatWarningEvent;
}
