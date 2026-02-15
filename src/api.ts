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
  dataDir: string;
  initialized: boolean;
  tosRiskAcknowledged: boolean;
  snowbreakSourcePolicy: 'official_first' | 'community_first';
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

export type RuntimeEnv = 'wine' | 'steam' | 'linux';
export type GameBackgroundType = 'Image';

export interface GameInfoMeta {
  displayName: string;
  gamePreset: string;
}

export interface GameInfoRuntime {
  runtimeEnv: RuntimeEnv;
}

export interface GameInfoAssets {
  backgroundType: GameBackgroundType;
  iconFile?: string | null;
  backgroundFile?: string | null;
}

export interface GameInfoConfigV2 {
  schemaVersion: number;
  gameName: string;
  meta: GameInfoMeta;
  runtime: GameInfoRuntime;
  assets: GameInfoAssets;
  readOnly: boolean;
  warningCode?: string | null;
}

export interface PresetCatalogItem {
  id: string;
  label: string;
  displayNameEn: string;
  legacyIds: string[];
  defaultFolder: string;
  supportedDownload: boolean;
  supportedProtection: boolean;
  supported3dmigoto: boolean;
}

export interface GameInfoMetaPatch {
  displayName?: string;
  gamePreset?: string;
}

export interface GameInfoRuntimePatch {
  runtimeEnv?: RuntimeEnv;
}

export interface GameInfoAssetsPatch {
  backgroundType?: GameBackgroundType;
  iconFile?: string;
  backgroundFile?: string;
}

export interface ValidateNameResult {
  valid: boolean;
  code: string;
  message: string;
}

export interface MigrateResult {
  success: boolean;
  migrated: boolean;
  code: string;
  message: string;
}

export interface RenamePair {
  from: string;
  to: string;
}

export interface GameKeyMigrationStatus {
  needed: boolean;
  done: boolean;
  reason: string;
}

export interface GameKeyMigrationPreview {
  needed: boolean;
  dbRenames: RenamePair[];
  gameDirRenames: RenamePair[];
  prefixDirRenames: RenamePair[];
  configFilesToUpdate: number;
  conflicts: string[];
}

export interface GameKeyMigrationResult {
  success: boolean;
  migrated: boolean;
  message: string;
  backupDir?: string | null;
  conflicts: string[];
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
  sandbox_enabled: boolean;
  sandbox_isolate_home: boolean;
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

// DXVK 版本管理
export interface DxvkLocalVersion {
  version: string;
  extracted: boolean;
  path: string;
}

export interface DxvkRemoteVersion {
  version: string;
  tag_name: string;
  download_url: string;
  file_size: number;
  published_at: string;
  is_local: boolean;
}

export interface DxvkInstalledStatus {
  installed: boolean;
  version: string | null;
  dlls_found: string[];
}

export interface RemoteWineVersion {
  tag: string;
  version: string;
  variant: string;
  download_url: string;
  file_size: number;
  published_at: string;
  installed: boolean;
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

export interface GpuDevice {
  pci_addr: string;
  name: string;
  driver: string;
  index: number;
}

export interface DisplayInfo {
  server: string;
  wayland_compositor: string | null;
  gpu_driver: string | null;
  vulkan_available: boolean;
  vulkan_version: string | null;
  ime_configured: boolean;
  gamepad_detected: boolean;
  gpus: GpuDevice[];
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

export async function runResourceExecutable(resourceName: string, args: string[]): Promise<string> {
  return invoke<string>('run_resource_executable', {
    resourceName,
    filename: resourceName,
    args,
  });
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

export async function listGamePresetsForInfo(): Promise<PresetCatalogItem[]> {
  return invoke<PresetCatalogItem[]>('list_game_presets_for_info');
}

export async function loadGameInfoV2(gameName: string): Promise<GameInfoConfigV2> {
  return invoke<GameInfoConfigV2>('load_game_info_v2', { gameName });
}

export async function saveGameInfoMeta(gameName: string, patch: GameInfoMetaPatch): Promise<void> {
  return invoke('save_game_info_meta', { gameName, patch });
}

export async function saveGameInfoRuntime(gameName: string, patch: GameInfoRuntimePatch): Promise<void> {
  return invoke('save_game_info_runtime', { gameName, patch });
}

export async function saveGameInfoAssets(gameName: string, patch: GameInfoAssetsPatch): Promise<void> {
  return invoke('save_game_info_assets', { gameName, patch });
}

export async function validateGameConfigName(
  name: string,
  currentGameName?: string | null,
): Promise<ValidateNameResult> {
  return invoke<ValidateNameResult>('validate_game_config_name', {
    name,
    currentGameName: currentGameName ?? null,
  });
}

export async function migrateGameConfigToV2(gameName: string): Promise<MigrateResult> {
  return invoke<MigrateResult>('migrate_game_config_to_v2', { gameName });
}

export async function getGameKeyMigrationStatus(): Promise<GameKeyMigrationStatus> {
  return invoke<GameKeyMigrationStatus>('get_game_key_migration_status');
}

export async function previewGameKeyMigration(): Promise<GameKeyMigrationPreview> {
  return invoke<GameKeyMigrationPreview>('preview_game_key_migration');
}

export async function executeGameKeyMigration(): Promise<GameKeyMigrationResult> {
  return invoke<GameKeyMigrationResult>('execute_game_key_migration');
}

export async function createNewConfig(newName: string, config: GameConfig): Promise<void> {
  return invoke('create_new_config', { newName, config });
}

export async function deleteGameConfigFolder(gameName: string): Promise<void> {
  return invoke('delete_game_config_folder', { gameName });
}

// 游戏配置模板
export interface GameTemplateInfo {
  name: string;
  gameId: string;
  displayName: string;
  iconPath: string;
  hasIcon: boolean;
  alreadyExists: boolean;
}

export async function getGameTemplatesDir(): Promise<string> {
  return invoke<string>('get_game_templates_dir');
}

export async function listGameTemplates(): Promise<GameTemplateInfo[]> {
  return invoke<GameTemplateInfo[]>('list_game_templates');
}

export async function importGameTemplate(templateName: string, overwrite: boolean = false): Promise<void> {
  return invoke('import_game_template', { templateName, overwrite });
}

export async function setGameBackground(
  gameName: string, filePath: string, bgType: string
): Promise<void> {
  return invoke('set_game_background', { gameName, filePath, bgType });
}

export async function setGameIcon(gameName: string, filePath: string): Promise<void> {
  return invoke('set_game_icon', { gameName, filePath });
}

export async function resetGameIcon(gameName: string): Promise<void> {
  return invoke('reset_game_icon', { gameName });
}

export async function resetGameBackground(gameName: string): Promise<void> {
  return invoke('reset_game_background', { gameName });
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

export async function getSymlinkStatus(gamePath: string): Promise<boolean> {
  return invoke<boolean>('get_symlink_status', { gamePath });
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
  const raw = await invoke<ArchivePreview | string[]>('preview_mod_archive', { path });

  // Backward compatibility: older backend may return string[] file list.
  if (Array.isArray(raw)) {
    const files = raw.filter((item): item is string => typeof item === 'string');
    const rootDirs: string[] = [];
    const seen = new Set<string>();
    let hasIni = false;

    for (const file of files) {
      const normalized = file.replace(/\\/g, '/').replace(/^\.?\//, '').replace(/^\/+/, '');
      if (!normalized) continue;

      if (normalized.toLowerCase().endsWith('.ini')) {
        hasIni = true;
      }

      const idx = normalized.indexOf('/');
      if (idx > 0) {
        const root = normalized.slice(0, idx);
        if (!seen.has(root)) {
          seen.add(root);
          rootDirs.push(root);
        }
      }
    }

    const ext = path.split('.').pop()?.toLowerCase() || 'unknown';
    return {
      root_dirs: rootDirs,
      file_count: files.length,
      has_ini: hasIni,
      format: ext,
    };
  }

  return raw;
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

export async function fetchRemoteProton(): Promise<RemoteWineVersion[]> {
  return invoke<RemoteWineVersion[]>('fetch_remote_proton');
}

export async function downloadProton(downloadUrl: string, tag: string, variant: string): Promise<string> {
  return invoke<string>('download_proton', { downloadUrl, tag, variant });
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

// DXVK 版本管理
export async function scanLocalDxvk(): Promise<DxvkLocalVersion[]> {
  return invoke<DxvkLocalVersion[]>('scan_local_dxvk');
}

export async function detectDxvkStatus(gameId: string): Promise<DxvkInstalledStatus> {
  return invoke<DxvkInstalledStatus>('detect_dxvk_status', { gameId });
}

export async function fetchDxvkVersions(): Promise<DxvkRemoteVersion[]> {
  return invoke<DxvkRemoteVersion[]>('fetch_dxvk_versions');
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
// 遥测防护 Commands
// ============================================================

export async function checkTelemetryStatus(gamePreset: string): Promise<any> {
  return invoke('check_telemetry_status', { gamePreset });
}

export async function checkGameProtectionStatus(gamePreset: string, gamePath?: string): Promise<any> {
  return invoke('check_game_protection_status', { gamePreset, gamePath: gamePath || null });
}

export async function disableTelemetry(gamePreset: string): Promise<any> {
  return invoke('disable_telemetry', { gamePreset });
}

export async function restoreTelemetry(gamePreset: string): Promise<any> {
  return invoke('restore_telemetry', { gamePreset });
}

export async function removeTelemetryFiles(gamePreset: string, gamePath: string): Promise<any> {
  return invoke('remove_telemetry_files', { gamePreset, gamePath });
}

export async function applyGameProtection(gamePreset: string, gamePath: string): Promise<any> {
  return invoke('apply_game_protection', { gamePreset, gamePath });
}

export async function getGameProtectionInfo(gamePreset: string): Promise<any> {
  return invoke('get_game_protection_info', { gamePreset });
}

// ============================================================
// Game Download Commands
// ============================================================

export async function getLauncherInfo(launcherApi: string): Promise<any> {
  return invoke('get_launcher_info', { launcherApi });
}

export async function getGameState(launcherApi: string, gameFolder: string, bizPrefix?: string): Promise<GameState> {
  return invoke<GameState>('get_game_state', { launcherApi, gameFolder, bizPrefix: bizPrefix || null });
}

export async function downloadGame(launcherApi: string, gameFolder: string, languages?: string[], bizPrefix?: string): Promise<void> {
  return invoke('download_game', { launcherApi, gameFolder, languages: languages || null, bizPrefix: bizPrefix || null });
}

export async function updateGame(launcherApi: string, gameFolder: string, languages?: string[], bizPrefix?: string): Promise<void> {
  return invoke('update_game', { launcherApi, gameFolder, languages: languages || null, bizPrefix: bizPrefix || null });
}

export async function updateGamePatch(launcherApi: string, gameFolder: string): Promise<void> {
  return invoke('update_game_patch', { launcherApi, gameFolder });
}

export async function verifyGameFiles(launcherApi: string, gameFolder: string, bizPrefix?: string): Promise<VerifyResult> {
  return invoke<VerifyResult>('verify_game_files', { launcherApi, gameFolder, bizPrefix: bizPrefix || null });
}

export async function cancelDownload(gameFolder?: string): Promise<void> {
  return invoke('cancel_download', { gameFolder: gameFolder || null });
}

export async function getLocalVersion(gameFolder: string): Promise<string | null> {
  return invoke<string | null>('get_local_version', { gameFolder });
}

export interface GameLauncherApiInfo {
  launcherApi?: string;
  launcherDownloadApi?: string;
  defaultFolder?: string;
  servers?: Array<{ id: string; label: string; launcherApi: string; bizPrefix?: string }>;
  audioLanguages?: Array<{ code: string; label: string }>;
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

export function convertFileSrc(path: string, protocol?: string): string {
  return tauriConvertFileSrc(path, protocol);
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

// ============================================================
// Jadeite 反作弊补丁
// ============================================================

export interface JadeiteStatus {
  installed: boolean;
  localVersion: string | null;
  patchDir: string;
}

export async function getJadeiteStatus(gameName: string): Promise<JadeiteStatus> {
  return invoke<JadeiteStatus>('get_jadeite_status', { gameName });
}

export async function installJadeite(gameName: string): Promise<string> {
  return invoke<string>('install_jadeite', { gameName });
}

// ============================================================
// 日志查看器
// ============================================================

export async function getLogDir(): Promise<string> {
  return invoke<string>('get_log_dir');
}

export async function readLogFile(maxLines?: number): Promise<string> {
  return invoke<string>('read_log_file', { maxLines: maxLines ?? null });
}

export async function openLogWindow(): Promise<void> {
  return invoke('open_log_window');
}
