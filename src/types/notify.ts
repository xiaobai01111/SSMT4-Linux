import type { InjectionKey } from 'vue';

export type NotifyKind = 'success' | 'warning' | 'info' | 'error';

export interface NotifyApi {
  success: (title: string, message?: string) => void;
  warning: (title: string, message?: string) => void;
  info: (title: string, message?: string) => void;
  error: (title: string, message?: string) => void;
  toast: (message: string, type?: NotifyKind) => void;
}

export const NOTIFY_KEY: InjectionKey<NotifyApi> = Symbol('notify');
