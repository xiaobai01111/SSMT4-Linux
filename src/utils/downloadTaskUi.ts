export interface CompletedTaskLike {
  key: string;
  phase: string;
  updatedAt: number;
}

export function buildCompletedTaskMarker(task: CompletedTaskLike | null | undefined): string {
  if (!task || task.phase !== 'done') return '';
  return `${task.key}:${task.updatedAt}`;
}

export function shouldHandleCompletedTaskMarker(
  marker: string,
  lastHandledMarker: string,
  isModalOpen: boolean,
): boolean {
  return !!marker && marker !== lastHandledMarker && isModalOpen;
}
