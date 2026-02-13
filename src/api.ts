/**
 * API Layer
 *
 * 前端与 Tauri 后端之间的抽象层。
 * 所有前端组件通过此文件调用后端功能，而不是直接使用 @tauri-apps/api。
 */

import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc as tauriConvertFileSrc } from '@tauri-apps/api/core'
import { message, ask, open } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import { join } from '@tauri-apps/api/path'

// ============================================================
// Types
// ============================================================

export interface AppSettings {
  bgType: 'Image' | 'Video';
  bgImage: string;
  bgVideo: string;
  contentOpacity: number;
  contentBlur: number;
  cacheDir: string;
  currentConfigName: string;
  githubToken: string;
  showMods: boolean;
  showWebsites: boolean;
  showDocuments: boolean;
  locale: string;
}

export interface GameInfo {
  name: string;
  iconPath: string;
  bgPath: string;
  bgVideoPath?: string;
  bgType: 'Image' | 'Video';
  showSidebar: boolean;
}

export interface GameConfig {
  basic: {
    gamePreset: string;
    backgroundType?: 'Image' | 'Video';
  };
  threeDMigoto: any;
  other: any;
}

export interface ModScanResult {
  mods: any[];
  groups: any[];
}

export interface ArchivePreview {
  root_dirs: string[];
  file_count: number;
  has_ini: boolean;
  format: string;
}

export interface UpdateInfo {
  version: string;
  description: string;
  downloadUrl: string;
}

// ============================================================
// Wine / Proton Types
// ============================================================

export type ProtonVariant =
  | 'official'
  | 'experimental'
  | 'geproton'
  | 'dwproton'
  | 'protontkg'
  | 'lutris'
  | 'systemwine'
  | 'custom';

export type WineArch = 'win32' | 'win64';

export interface WineVersion {
  id: string;
  name: string;
  variant: ProtonVariant;
  path: string;
  version: string;
  arch: WineArch;
  supports_dxvk: boolean;
  timestamp: string | null;
}

export interface ProtonSettings {
  steam_app_id: string;
  use_pressure_vessel: boolean;
  proton_media_use_gst: boolean;
  proton_enable_wayland: boolean;
  proton_no_d3d12: boolean;
  mangohud: boolean;
  steam_deck_compat: boolean;
  custom_env: Record<string, string>;
}

export interface PrefixInfo {
  game_id: string;
  exists: boolean;
  path: string;
  size_bytes: number;
  config: PrefixConfig | null;
}

export interface PrefixConfig {
  wine_version_id: string;
  arch: WineArch;
  created_at: string;
  dxvk: { enabled: boolean; version: string | null };
  vkd3d: { enabled: boolean; version: string | null };
  installed_runtimes: string[];
  env_overrides: Record<string, string>;
  template_id: string | null;
  proton_settings: ProtonSettings;
}

export interface PrefixTemplate {
  id: string;
  name: string;
  description: string;
  recommended_variant: ProtonVariant;
  arch: WineArch;
  dxvk: { enabled: boolean; version: string | null };
  vkd3d: { enabled: boolean; version: string | null };
  required_runtimes: string[];
  env_overrides: Record<string, string>;
  proton_settings: ProtonSettings;
}

export interface GameWineConfig {
  game_id: string;
  wine_version_id: string | null;
  prefix_path: string | null;
  proton_settings: ProtonSettings;
  launcher_api_config: LauncherApiConfig | null;
}

export interface LauncherApiConfig {
  game_id: string;
  launcher_api: string;
  launcher_download_api: string | null;
}

export interface VulkanInfo {
  available: boolean;
  version: string | null;
  driver: string | null;
  device_name: string | null;
}

export interface RuntimeComponent {
  id: string;
  name: string;
  category: string;
  description: string;
}

export interface DisplayInfo {
  server: string;
  wayland_compositor: string | null;
  gpu_driver: string | null;
  vulkan_available: boolean;
  vulkan_version: string | null;
  ime_configured: boolean;
  gamepad_detected: boolean;
}

// ============================================================
// Game Download Types
// ============================================================

export type LauncherState =
  | 'startgame'
  | 'gamerunning'
  | 'needinstall'
  | 'downloading'
  | 'validating'
  | 'needupdate'
  | 'updating'
  | 'merging'
  | 'networkerror';

export interface GameState {
  state: LauncherState;
  local_version: string | null;
  remote_version: string | null;
  supports_incremental: boolean;
}

export interface DownloadProgress {
  phase: string;
  total_size: number;
  finished_size: number;
  total_count: number;
  finished_count: number;
  current_file: string;
  speed_bps: number;
  eta_seconds: number;
}

export interface VerifyResult {
  total_files: number;
  verified_ok: number;
  redownloaded: number;
  failed: string[];
}

// ============================================================
// Settings Commands
// ============================================================

export async function loadSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('load_settings');
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke('save_settings', { settings });
}

// ============================================================
// Common Commands
// ============================================================

export async function getResourcePath(relative: string): Promise<string> {
  return invoke<string>('get_resource_path', { relative });
}

export async function ensureDirectory(path: string): Promise<void> {
  return invoke('ensure_directory', { path });
}

export async function openInExplorer(path: string): Promise<void> {
  return invoke('open_in_explorer', { path });
}

// ============================================================
// Process Commands
// ============================================================

export async function runResourceExecutable(filename: string, args: string[]): Promise<string> {
  return invoke<string>('run_resource_executable', { filename, args });
}

// ============================================================
// Game Scanner Commands
// ============================================================

export async function scanGames(): Promise<GameInfo[]> {
  return invoke<GameInfo[]>('scan_games');
}

export async function setGameVisibility(gameName: string, hidden: boolean): Promise<void> {
  return invoke('set_game_visibility', { gameName, hidden });
}

// ============================================================
// Game Config Commands
// ============================================================

export async function loadGameConfig(gameName: string): Promise<GameConfig> {
  return invoke<GameConfig>('load_game_config', { gameName });
}

export async function saveGameConfig(gameName: string, config: GameConfig): Promise<void> {
  return invoke('save_game_config', { gameName, config });
}

export async function createNewConfig(newName: string, config: GameConfig): Promise<void> {
  return invoke('create_new_config', { newName, config });
}

export async function deleteGameConfigFolder(gameName: string): Promise<void> {
  return invoke('delete_game_config_folder', { gameName });
}

export async function setGameBackground(
  gameName: string, filePath: string, bgType: string
): Promise<void> {
  return invoke('set_game_background', { gameName, filePath, bgType });
}

export async function setGameIcon(gameName: string, filePath: string): Promise<void> {
  return invoke('set_game_icon', { gameName, filePath });
}

export async function updateGameBackground(
  gameName: string, gamePreset: string, bgType: string
): Promise<void> {
  return invoke('update_game_background', { gameName, gamePreset, bgType });
}

export async function get3dmigotoLatestRelease(gamePreset: string): Promise<UpdateInfo> {
  return invoke<UpdateInfo>('get_3dmigoto_latest_release', { gamePreset });
}

export async function install3dmigotoUpdate(
  gameName: string, downloadUrl: string
): Promise<void> {
  return invoke('install_3dmigoto_update', { gameName, downloadUrl });
}

// ============================================================
// Game Launcher Commands
// ============================================================

export async function startGame(
  gameName: string, gameExePath: string, wineVersionId: string
): Promise<string> {
  return invoke<string>('start_game', { gameName, gameExePath, wineVersionId });
}

export async function check3dmigotoIntegrity(gameName: string, gamePath: string): Promise<boolean> {
  return invoke<boolean>('check_3dmigoto_integrity', { gameName, gamePath });
}

export async function toggleSymlink(gamePath: string, enabled: boolean): Promise<boolean> {
  return invoke<boolean>('toggle_symlink', { gamePath, enabled });
}

// ============================================================
// Mod Manager Commands
// ============================================================

export async function scanMods(gameName: string): Promise<ModScanResult> {
  return invoke<ModScanResult>('scan_mods', { gameName });
}

export async function toggleMod(
  gameName: string, modRelativePath: string, enable: boolean
): Promise<string> {
  return invoke<string>('toggle_mod', { gameName, modRelativePath, enable });
}

export async function watchMods(gameName: string): Promise<void> {
  return invoke('watch_mods', { gameName });
}

export async function unwatchMods(): Promise<void> {
  return invoke('unwatch_mods');
}

export async function createModGroup(gameName: string, groupName: string): Promise<void> {
  return invoke('create_mod_group', { gameName, groupName });
}

export async function setModGroupIcon(
  gameName: string, groupPath: string, iconPath: string
): Promise<void> {
  return invoke('set_mod_group_icon', { gameName, groupPath, iconPath });
}

export async function openModGroupFolder(gameName: string, groupPath: string): Promise<void> {
  return invoke('open_mod_group_folder', { gameName, groupPath });
}

export async function openGameModsFolder(gameName: string): Promise<void> {
  return invoke('open_game_mods_folder', { gameName });
}

export async function renameModGroup(
  gameName: string, oldGroup: string, newGroup: string
): Promise<void> {
  return invoke('rename_mod_group', { gameName, oldGroup, newGroup });
}

export async function deleteModGroup(gameName: string, groupName: string): Promise<void> {
  return invoke('delete_mod_group', { gameName, groupName });
}

export async function deleteMod(gameName: string, modRelativePath: string): Promise<void> {
  return invoke('delete_mod', { gameName, modRelativePath });
}

export async function moveModToGroup(
  gameName: string, modId: string, newGroup: string
): Promise<void> {
  return invoke('move_mod_to_group', { gameName, modId, newGroup });
}

export async function previewModArchive(path: string): Promise<ArchivePreview> {
  return invoke<ArchivePreview>('preview_mod_archive', { path });
}

export async function installModArchive(
  gameName: string, archivePath: string, targetName: string,
  targetGroup: string, password?: string | null
): Promise<void> {
  return invoke('install_mod_archive', { gameName, archivePath, targetName, targetGroup, password });
}

// ============================================================
// Wine / Proton Management Commands
// ============================================================

export async function scanWineVersions(): Promise<WineVersion[]> {
  return invoke<WineVersion[]>('scan_wine_versions');
}

export async function getGameWineConfig(gameId: string): Promise<GameWineConfig> {
  return invoke<GameWineConfig>('get_game_wine_config', { gameId });
}

export async function setGameWineConfig(
  gameId: string, wineVersionId: string, protonSettings: ProtonSettings
): Promise<void> {
  return invoke('set_game_wine_config', { gameId, wineVersionId, protonSettings });
}

export async function createPrefix(gameId: string, templateId?: string): Promise<string> {
  return invoke<string>('create_prefix', { gameId, templateId: templateId ?? null });
}

export async function deletePrefix(gameId: string): Promise<void> {
  return invoke('delete_prefix', { gameId });
}

export async function getPrefixInfo(gameId: string): Promise<PrefixInfo> {
  return invoke<PrefixInfo>('get_prefix_info', { gameId });
}

export async function installDxvk(gameId: string, version: string): Promise<string> {
  return invoke<string>('install_dxvk', { gameId, version });
}

export async function uninstallDxvk(gameId: string): Promise<string> {
  return invoke<string>('uninstall_dxvk', { gameId });
}

export async function installVkd3d(gameId: string, version: string): Promise<string> {
  return invoke<string>('install_vkd3d', { gameId, version });
}

export async function checkVulkan(): Promise<VulkanInfo> {
  return invoke<VulkanInfo>('check_vulkan');
}

export async function installRuntime(gameId: string, component: string): Promise<string> {
  return invoke<string>('install_runtime', { gameId, component });
}

export async function listAvailableRuntimes(): Promise<RuntimeComponent[]> {
  return invoke<RuntimeComponent[]>('list_available_runtimes');
}

export async function getInstalledRuntimes(gameId: string): Promise<string[]> {
  return invoke<string[]>('get_installed_runtimes', { gameId });
}

export async function getDisplayInfo(): Promise<DisplayInfo> {
  return invoke<DisplayInfo>('get_display_info');
}

export async function getRecentLogs(lines?: number): Promise<string[]> {
  return invoke<string[]>('get_recent_logs', { lines: lines ?? null });
}

export async function openLogFolder(): Promise<void> {
  return invoke('open_log_folder');
}

export async function listPrefixTemplates(): Promise<PrefixTemplate[]> {
  return invoke<PrefixTemplate[]>('list_prefix_templates');
}

export async function savePrefixTemplate(template: PrefixTemplate): Promise<void> {
  return invoke('save_prefix_template', { template });
}

// ============================================================
// Game Download Commands
// ============================================================

export async function getLauncherInfo(launcherApi: string): Promise<any> {
  return invoke('get_launcher_info', { launcherApi });
}

export async function getGameState(launcherApi: string, gameFolder: string): Promise<GameState> {
  return invoke<GameState>('get_game_state', { launcherApi, gameFolder });
}

export async function downloadGame(launcherApi: string, gameFolder: string): Promise<void> {
  return invoke('download_game', { launcherApi, gameFolder });
}

export async function updateGame(launcherApi: string, gameFolder: string): Promise<void> {
  return invoke('update_game', { launcherApi, gameFolder });
}

export async function updateGamePatch(launcherApi: string, gameFolder: string): Promise<void> {
  return invoke('update_game_patch', { launcherApi, gameFolder });
}

export async function verifyGameFiles(launcherApi: string, gameFolder: string): Promise<VerifyResult> {
  return invoke<VerifyResult>('verify_game_files', { launcherApi, gameFolder });
}

export async function cancelDownload(): Promise<void> {
  return invoke('cancel_download');
}

export async function getLocalVersion(gameFolder: string): Promise<string | null> {
  return invoke<string | null>('get_local_version', { gameFolder });
}

export interface GameLauncherApiInfo {
  launcherApi?: string;
  launcherDownloadApi?: string;
  defaultFolder?: string;
  supported: boolean;
}

export async function getGameLauncherApi(gamePreset: string): Promise<GameLauncherApiInfo> {
  return invoke<GameLauncherApiInfo>('get_game_launcher_api', { gamePreset });
}

export async function getDefaultGameFolder(gameName: string): Promise<string> {
  return invoke<string>('get_default_game_folder', { gameName });
}

// ============================================================
// Tauri 辅助函数封装
// ============================================================

export function convertFileSrc(path: string): string {
  return tauriConvertFileSrc(path);
}

export async function joinPath(...parts: string[]): Promise<string> {
  // Tauri 的 join 只接受两个参数，需要依次拼接
  let result = parts[0];
  for (let i = 1; i < parts.length; i++) {
    result = await join(result, parts[i]);
  }
  return result;
}

export async function openFileDialog(options?: {
  directory?: boolean;
  multiple?: boolean;
  filters?: Array<{ name: string; extensions: string[] }>;
  title?: string;
}): Promise<string | null> {
  const result = await open({
    directory: options?.directory,
    multiple: options?.multiple,
    filters: options?.filters,
    title: options?.title,
  });
  if (typeof result === 'string') return result;
  if (result && typeof result === 'object' && 'length' in result && (result as any).length > 0) return (result as any)[0];
  return result as string | null;
}

export async function showMessage(
  msg: string,
  options?: { title?: string; kind?: string }
): Promise<void> {
  await message(msg, {
    title: options?.title,
    kind: (options?.kind as any) || 'info',
  });
}

export async function askConfirm(
  msg: string,
  options?: { title?: string; kind?: string; okLabel?: string; cancelLabel?: string }
): Promise<boolean> {
  return ask(msg, {
    title: options?.title,
    kind: (options?.kind as any) || 'info',
    okLabel: options?.okLabel,
    cancelLabel: options?.cancelLabel,
  });
}

export async function listenEvent(
  event: string,
  callback: (event: any) => void
): Promise<() => void> {
  return listen(event, callback);
}
