export type RuntimeFocusTarget = 'all' | 'wine_version' | 'dxvk' | 'vkd3d';

export type GlobalSettingsMenu = 'proton' | 'dxvk' | 'vkd3d';

export type GameSettingsTab = 'info' | 'game' | 'runtime' | 'system';

export interface GameSettingsOpenRequest {
  tab: GameSettingsTab;
  reason?: string;
  runtimeFocus?: RuntimeFocusTarget;
}

export type OnboardingHomeAction =
  | { type: 'open_download_modal' }
  | { type: 'open_game_settings'; tab: GameSettingsTab; runtimeFocus?: RuntimeFocusTarget }
  | { type: 'close_modals' };
