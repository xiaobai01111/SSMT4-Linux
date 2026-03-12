export type HomePrimaryAction = 'running' | 'update' | 'start' | 'download';

export interface HomePrimaryActionInput {
  isGameRunning: boolean;
  needsUpdate: boolean;
  hasExecutable: boolean;
}

export function resolveHomePrimaryAction({
  isGameRunning,
  needsUpdate,
  hasExecutable,
}: HomePrimaryActionInput): HomePrimaryAction {
  if (isGameRunning) return 'running';
  if (needsUpdate) return 'update';
  return hasExecutable ? 'start' : 'download';
}

export function resolveHomePrimaryLabelKey(action: HomePrimaryAction): string {
  switch (action) {
    case 'running':
      return 'home.status.running';
    case 'update':
      return 'home.css.updategame';
    case 'start':
      return 'home.css.startgame';
    case 'download':
      return 'home.css.downloadgame';
  }
}

export function shouldOpenDownloadForPrimaryAction(action: HomePrimaryAction): boolean {
  return action === 'update' || action === 'download';
}
