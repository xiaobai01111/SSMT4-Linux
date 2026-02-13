<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import {
  getGameState,
  downloadGame as apiDownloadGame,
  updateGame as apiUpdateGame,
  verifyGameFiles as apiVerifyGameFiles,
  cancelDownload as apiCancelDownload,
  getDefaultGameFolder,
  listenEvent,
  openFileDialog,
  showMessage,
  loadGameConfig,
  saveGameConfig,
  type GameState,
  type DownloadProgress,
} from '../api';

const props = defineProps<{
  modelValue: boolean;
  gameName: string;
  displayName: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
  (e: 'gameConfigured'): void;
}>();

const gameState = ref<GameState | null>(null);
const progress = ref<DownloadProgress | null>(null);
const isChecking = ref(false);
const isDownloading = ref(false);
const isVerifying = ref(false);
const launcherApi = ref('');
const gameFolder = ref('');
const error = ref('');
const gamePreset = ref('');
const isSupported = ref(false);
const statusMsg = ref('');

// 已知游戏的 launcher API 配置（前端硬编码，无需后端命令）
const KNOWN_LAUNCHER_APIS: Record<string, { launcherApi: string; defaultFolder: string }> = {
  'WWMI': {
    launcherApi: 'https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/game/G152/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/index.json',
    defaultFolder: 'Wuthering Waves Game',
  },
  'WuWa': {
    launcherApi: 'https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/game/G152/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/index.json',
    defaultFolder: 'Wuthering Waves Game',
  },
};

const close = () => {
  if (isDownloading.value) return; // 下载中不可关闭
  emit('update:modelValue', false);
};

// 打开时自动加载：检测是否支持自动下载 + 读取配置
const loadState = async () => {
  if (!props.gameName) return;
  error.value = '';
  statusMsg.value = '正在加载配置...';

  // 1. 先尝试从 Config.json 读取 GamePreset，失败则直接用 gameName
  let preset = props.gameName;
  let savedFolder = '';
  try {
    const config = await loadGameConfig(props.gameName);
    preset = (config as any).GamePreset || config.basic?.gamePreset || props.gameName;
    // 恢复之前保存的下载配置（保存在 config.other 下）
    const other = config.other || {};
    launcherApi.value = other.launcherApi || '';
    savedFolder = other.gameFolder || '';
  } catch (e) {
    console.warn('[GameDownload] loadGameConfig 失败，使用 gameName 作为 preset:', e);
  }
  gamePreset.value = preset;
  console.log('[GameDownload] gameName =', props.gameName, ', preset =', preset);

  // 2. 检测是否支持自动下载：先用 preset 匹配，再用 gameName 匹配
  const knownApi = KNOWN_LAUNCHER_APIS[preset] || KNOWN_LAUNCHER_APIS[props.gameName];
  isSupported.value = !!knownApi;
  console.log('[GameDownload] isSupported =', isSupported.value, ', knownApi =', knownApi);

  if (!knownApi) {
    statusMsg.value = '';
    return;
  }

  // 3. 自动填充 launcher API（不覆盖已保存的值）
  if (!launcherApi.value) {
    launcherApi.value = knownApi.launcherApi;
  }

  // 4. 始终从后端获取最新默认目录（跟随 dataDir 变化）
  try {
    const baseDir = await getDefaultGameFolder(props.gameName);
    const defaultFolder = baseDir + '/' + knownApi.defaultFolder;
    // 仅当用户没有手动设置过自定义目录时，使用默认路径
    // 判断依据：savedFolder 为空，或 savedFolder 是旧的默认路径格式
    if (!savedFolder || savedFolder.includes('/.local/share/ssmt4/') || savedFolder === defaultFolder) {
      gameFolder.value = defaultFolder;
    } else {
      gameFolder.value = savedFolder;
    }
  } catch (e) {
    console.warn('[GameDownload] getDefaultGameFolder failed:', e);
    gameFolder.value = savedFolder || '';
  }

  statusMsg.value = '';
  console.log('[GameDownload] launcherApi =', launcherApi.value);
  console.log('[GameDownload] gameFolder =', gameFolder.value);

  // 5. 自动检查游戏状态
  try {
    await checkState();
  } catch (e) {
    console.warn('[GameDownload] checkState 失败:', e);
  }
};

const checkState = async () => {
  if (!launcherApi.value || !gameFolder.value) {
    error.value = '请先配置启动器 API 和游戏安装目录';
    return;
  }
  isChecking.value = true;
  error.value = '';
  statusMsg.value = '正在检查游戏状态...';
  try {
    gameState.value = await getGameState(launcherApi.value, gameFolder.value);
    statusMsg.value = '';
  } catch (e: any) {
    error.value = `检查状态失败: ${e}`;
    statusMsg.value = '';
  } finally {
    isChecking.value = false;
  }
};

const startDownload = async () => {
  if (!launcherApi.value || !gameFolder.value) return;
  isDownloading.value = true;
  error.value = '';
  progress.value = null;

  // 监听下载进度事件
  const unlisten = await listenEvent('game-download-progress', (event: any) => {
    progress.value = event.payload;
  });

  try {
    if (gameState.value?.state === 'needupdate') {
      await apiUpdateGame(launcherApi.value, gameFolder.value);
    } else {
      await apiDownloadGame(launcherApi.value, gameFolder.value);
    }
    // 下载完成后保存配置
    await saveDownloadConfig();
    await showMessage('下载完成！正在校验文件...', { title: '成功', kind: 'info' });
    // 自动校验
    await startVerify();
    // 刷新状态
    await checkState();
  } catch (e: any) {
    if (!String(e).includes('cancelled')) {
      error.value = `下载失败: ${e}`;
    }
  } finally {
    isDownloading.value = false;
    unlisten();
  }
};

const startVerify = async () => {
  if (!launcherApi.value || !gameFolder.value) return;
  isVerifying.value = true;
  progress.value = null;

  const unlisten = await listenEvent('game-verify-progress', (event: any) => {
    progress.value = event.payload;
  });

  try {
    const result = await apiVerifyGameFiles(launcherApi.value, gameFolder.value);
    if (result.failed.length > 0) {
      error.value = `校验完成，但有 ${result.failed.length} 个文件仍然异常`;
    } else {
      await showMessage(
        `校验完成！共 ${result.total_files} 个文件，${result.verified_ok} 个正常，${result.redownloaded} 个已重新下载。`,
        { title: '校验结果', kind: 'info' }
      );
    }
  } catch (e: any) {
    if (!String(e).includes('cancelled')) {
      error.value = `校验失败: ${e}`;
    }
  } finally {
    isVerifying.value = false;
    unlisten();
  }
};

const cancelDownload = async () => {
  try { await apiCancelDownload(); } catch (e) { console.error(e); }
};

const selectGameExe = async () => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: '可执行文件', extensions: ['exe', 'sh', 'AppImage', 'desktop', '*'] }],
      title: '选择游戏可执行文件'
    });
    if (selected && typeof selected === 'string') {
      const config = await loadGameConfig(props.gameName);
      config.other = config.other || {};
      config.other.gamePath = selected;
      await saveGameConfig(props.gameName, config);
      await showMessage('已设置游戏路径', { title: '成功', kind: 'info' });
      emit('gameConfigured');
      close();
    }
  } catch (e: any) {
    error.value = `选择文件失败: ${e}`;
  }
};

const selectGameFolder = async () => {
  try {
    const selected = await openFileDialog({ directory: true, title: '选择游戏安装目录' });
    if (selected && typeof selected === 'string') {
      gameFolder.value = selected;
      // 自动保存用户选择的目录
      await saveDownloadConfig();
      // 用新目录重新检查游戏状态
      await checkState();
    }
  } catch (e) { console.error(e); }
};

const saveDownloadConfig = async () => {
  try {
    const config = await loadGameConfig(props.gameName);
    config.other = config.other || {};
    config.other.launcherApi = launcherApi.value;
    config.other.gameFolder = gameFolder.value;
    await saveGameConfig(props.gameName, config);
  } catch (e) { console.error(e); }
};

// === 格式化辅助 ===
const formatSize = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
};
const formatEta = (seconds: number) => {
  if (seconds <= 0) return '--:--';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  return `${m}:${s.toString().padStart(2, '0')}`;
};

const progressPercent = computed(() => {
  if (!progress.value || progress.value.total_size === 0) return 0;
  return Math.round((progress.value.finished_size / progress.value.total_size) * 100);
});

const stateLabel = computed(() => {
  if (!gameState.value) return '';
  switch (gameState.value.state) {
    case 'startgame': return '✓ 已是最新版本';
    case 'needinstall': return '需要下载安装';
    case 'needupdate':
      return `需要更新 (${gameState.value.local_version} → ${gameState.value.remote_version})`;
    case 'networkerror': return '⚠ 网络错误，请检查网络连接';
    default: return String(gameState.value.state);
  }
});
const stateClass = computed(() => {
  if (!gameState.value) return '';
  switch (gameState.value.state) {
    case 'startgame': return 'state-ok';
    case 'needinstall': return 'state-install';
    case 'needupdate': return 'state-update';
    case 'networkerror': return 'state-error';
    default: return '';
  }
});

const canDownload = computed(() => {
  if (!gameState.value) return false;
  return gameState.value.state === 'needinstall' || gameState.value.state === 'needupdate';
});

const isWorking = computed(() => isDownloading.value || isVerifying.value);
const workingPhase = computed(() => {
  if (isVerifying.value) return '校验中';
  if (isDownloading.value) return '下载中';
  return '';
});

watch(() => props.modelValue, (val) => {
  if (val) {
    error.value = '';
    progress.value = null;
    gameState.value = null;
    statusMsg.value = '';
    loadState();
  }
}, { immediate: true });
</script>

<template>
  <transition name="modal-fade">
    <div v-if="modelValue" class="dl-overlay" @click.self="close">
      <div class="dl-window">
        <!-- 标题栏 -->
        <div class="dl-header">
          <span class="dl-title">下载 / 安装游戏</span>
          <div class="dl-close" @click="close" v-if="!isWorking">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
              fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </div>
        </div>

        <div class="dl-body">
          <!-- 游戏名 -->
          <div class="game-info">
            <span class="game-name">{{ displayName || gameName }}</span>
            <span v-if="isSupported" class="badge supported">支持自动下载</span>
            <span v-else class="badge manual">手动安装</span>
          </div>

          <!-- 加载提示 -->
          <div v-if="statusMsg" class="status-msg">{{ statusMsg }}</div>

          <!-- ========== 支持自动下载的游戏 (鸣潮) ========== -->
          <template v-if="isSupported">
            <!-- 游戏状态卡片 -->
            <div v-if="gameState" class="state-card" :class="stateClass">
              <div class="state-label">{{ stateLabel }}</div>
              <div class="state-versions" v-if="gameState.remote_version">
                <span v-if="gameState.local_version">本地: {{ gameState.local_version }}</span>
                <span>最新: {{ gameState.remote_version }}</span>
              </div>
            </div>

            <!-- 安装目录（始终显示，下载前必须确认） -->
            <div v-if="!isWorking" class="install-dir-section">
              <label class="install-dir-label">安装目录</label>
              <div class="install-dir-row">
                <input v-model="gameFolder" type="text" class="dl-input" placeholder="选择游戏安装目录..." />
                <button class="icon-btn" @click="selectGameFolder" title="选择目录">
                  <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
                    fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                  </svg>
                </button>
              </div>
              <p class="install-dir-hint">游戏文件将下载到此目录，请确保有足够磁盘空间（约 30GB+）</p>
            </div>

            <!-- 下载/更新按钮 -->
            <div v-if="!isWorking" class="main-actions">
              <button
                v-if="canDownload"
                class="action-btn primary large"
                @click="startDownload"
                :disabled="!gameFolder"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
                  fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                  <polyline points="7 10 12 15 17 10"/>
                  <line x1="12" y1="15" x2="12" y2="3"/>
                </svg>
                {{ gameState?.state === 'needupdate' ? '开始更新' : '开始下载' }}
              </button>
              <button class="action-btn" @click="startVerify" v-if="gameState?.state === 'startgame'">
                校验游戏文件
              </button>
              <button class="action-btn" @click="checkState" :disabled="isChecking">
                {{ isChecking ? '检查中...' : '刷新状态' }}
              </button>
            </div>

            <!-- 进度条 -->
            <div v-if="isWorking && progress" class="progress-section">
              <div class="progress-phase">{{ workingPhase }}</div>
              <div class="progress-bar-track">
                <div class="progress-bar-fill" :style="{ width: progressPercent + '%' }"></div>
              </div>
              <div class="progress-info">
                <span>{{ progressPercent }}%</span>
                <span>{{ formatSize(progress.finished_size) }} / {{ formatSize(progress.total_size) }}</span>
              </div>
              <div class="progress-detail">
                <span class="progress-file">{{ progress.current_file }}</span>
                <span>{{ formatSize(progress.speed_bps) }}/s · 剩余 {{ formatEta(progress.eta_seconds) }}</span>
              </div>
              <div class="progress-counts">
                文件: {{ progress.finished_count }} / {{ progress.total_count }}
              </div>
              <button class="action-btn danger" @click="cancelDownload">取消</button>
            </div>

            <!-- 高级配置（折叠） -->
            <details class="config-details">
              <summary>高级配置</summary>
              <div class="config-content">
                <div class="field">
                  <label>启动器 API</label>
                  <input v-model="launcherApi" type="text" class="dl-input" />
                </div>
                <button class="action-btn sm" @click="saveDownloadConfig">保存配置</button>
              </div>
            </details>

            <div class="divider"><span>或者</span></div>
          </template>

          <!-- ========== 手动指定路径 ========== -->
          <div class="section">
            <div class="section-title" v-if="!isSupported">此游戏暂不支持自动下载</div>
            <p class="hint">如果游戏已安装（通过 Steam、Lutris 或手动安装），选择可执行文件即可。</p>
            <button class="action-btn primary" @click="selectGameExe">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
                fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
              选择游戏可执行文件
            </button>
          </div>

          <!-- 错误提示 -->
          <div v-if="error" class="error-msg">{{ error }}</div>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.dl-overlay {
  position: fixed; inset: 0;
  background: rgba(0,0,0,0.6); backdrop-filter: blur(4px);
  z-index: 2000; display: flex; align-items: center; justify-content: center;
}
.dl-window {
  width: 580px; max-height: 82vh;
  background: rgba(30,30,30,0.97); border: 1px solid rgba(255,255,255,0.1);
  border-radius: 12px; box-shadow: 0 8px 32px rgba(0,0,0,0.6);
  display: flex; flex-direction: column; animation: slideUp 0.25s ease-out;
}
@keyframes slideUp {
  from { opacity:0; transform:translateY(16px); }
  to { opacity:1; transform:translateY(0); }
}
.dl-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 18px 24px; border-bottom: 1px solid rgba(255,255,255,0.06);
}
.dl-title { font-size:16px; font-weight:600; color:#fff; }
.dl-close {
  width:28px; height:28px; display:flex; align-items:center; justify-content:center;
  border-radius:4px; cursor:pointer; color:rgba(255,255,255,0.5); transition:all 0.2s;
}
.dl-close:hover { background:rgba(255,255,255,0.1); color:#fff; }
.dl-body { padding:24px; overflow-y:auto; flex:1; }

/* 游戏信息 */
.game-info { margin-bottom:20px; display:flex; align-items:center; gap:10px; }
.game-name { font-size:18px; font-weight:600; color:#F7CE46; }
.badge {
  font-size:11px; padding:2px 8px; border-radius:4px; white-space:nowrap;
}
.badge.supported { background:rgba(103,194,58,0.15); color:#67c23a; border:1px solid rgba(103,194,58,0.3); }
.badge.manual { background:rgba(255,255,255,0.06); color:rgba(255,255,255,0.4); }

.status-msg {
  text-align:center; color:rgba(255,255,255,0.5); font-size:13px;
  padding:16px 0; animation: pulse 1.5s ease-in-out infinite;
}
@keyframes pulse { 0%,100% { opacity:0.5; } 50% { opacity:1; } }

/* 状态卡片 */
.state-card {
  background:rgba(0,0,0,0.25); border:1px solid rgba(255,255,255,0.06);
  border-radius:8px; padding:14px 16px; margin-bottom:16px;
}
.state-card.state-ok { border-color:rgba(103,194,58,0.3); }
.state-card.state-install { border-color:rgba(247,206,70,0.3); }
.state-card.state-update { border-color:rgba(0,122,204,0.3); }
.state-card.state-error { border-color:rgba(232,17,35,0.3); }
.state-label { font-size:15px; font-weight:500; color:rgba(255,255,255,0.9); }
.state-versions {
  margin-top:6px; font-size:12px; color:rgba(255,255,255,0.45);
  display:flex; gap:16px;
}

/* 安装目录 */
.install-dir-section { margin-bottom:16px; }
.install-dir-label {
  display:block; font-size:13px; font-weight:500;
  color:rgba(255,255,255,0.7); margin-bottom:6px;
}
.install-dir-row { display:flex; gap:6px; }
.install-dir-row .dl-input { flex:1; }
.install-dir-row .icon-btn {
  display:flex; align-items:center; justify-content:center;
  color:rgba(255,255,255,0.6);
}
.install-dir-row .icon-btn:hover { color:#fff; }
.install-dir-hint {
  font-size:11px; color:rgba(255,255,255,0.35); margin-top:6px; line-height:1.4;
}

/* 主要操作按钮 */
.main-actions { display:flex; gap:8px; margin-bottom:16px; flex-wrap:wrap; }
.action-btn {
  padding:8px 16px; border:none; border-radius:6px; font-size:13px; cursor:pointer;
  color:#fff; background:rgba(255,255,255,0.1); transition:all 0.2s;
  display:inline-flex; align-items:center; gap:6px;
}
.action-btn:hover { background:rgba(255,255,255,0.18); }
.action-btn:disabled { opacity:0.5; cursor:not-allowed; }
.action-btn.primary {
  background:rgba(247,206,70,0.2); color:#F7CE46; border:1px solid rgba(247,206,70,0.3);
}
.action-btn.primary:hover { background:rgba(247,206,70,0.3); }
.action-btn.primary.large { padding:12px 24px; font-size:15px; font-weight:600; }
.action-btn.danger { background:rgba(232,17,35,0.2); color:#ff6b6b; margin-top:8px; }
.action-btn.danger:hover { background:rgba(232,17,35,0.3); }
.action-btn.sm { padding:6px 12px; font-size:12px; margin-top:8px; }

/* 进度条 */
.progress-section {
  background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06);
  border-radius:8px; padding:16px; margin-bottom:16px;
}
.progress-phase {
  font-size:13px; font-weight:600; color:rgba(255,255,255,0.8); margin-bottom:10px;
}
.progress-bar-track {
  width:100%; height:10px; background:rgba(255,255,255,0.08); border-radius:5px; overflow:hidden;
}
.progress-bar-fill {
  height:100%; background:linear-gradient(90deg, #F7CE46, #f0a030); border-radius:5px;
  transition: width 0.3s ease;
}
.progress-info {
  display:flex; justify-content:space-between; font-size:13px;
  color:rgba(255,255,255,0.7); margin-top:8px;
}
.progress-detail {
  display:flex; justify-content:space-between; font-size:11px;
  color:rgba(255,255,255,0.4); margin-top:4px;
}
.progress-file {
  max-width:280px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
}
.progress-counts {
  font-size:11px; color:rgba(255,255,255,0.35); margin-top:4px;
}

/* 折叠配置 */
.config-details {
  margin:16px 0;
}
.config-details summary {
  font-size:12px; color:rgba(255,255,255,0.4); cursor:pointer;
  padding:6px 0; user-select:none;
}
.config-details summary:hover { color:rgba(255,255,255,0.7); }
.config-content {
  margin-top:10px; padding:12px; background:rgba(0,0,0,0.15);
  border:1px solid rgba(255,255,255,0.04); border-radius:6px;
}
.field { margin-bottom:10px; }
.field label { display:block; font-size:12px; color:rgba(255,255,255,0.5); margin-bottom:4px; }
.dl-input {
  width:100%; box-sizing:border-box; background:rgba(0,0,0,0.3);
  border:1px solid rgba(255,255,255,0.1); border-radius:4px;
  padding:8px 10px; color:#fff; font-size:13px; outline:none;
}
.dl-input:focus { border-color:#F7CE46; }
.input-row { display:flex; gap:6px; }
.input-row .dl-input { flex:1; }
.icon-btn {
  background:rgba(255,255,255,0.08); border:1px solid rgba(255,255,255,0.1);
  border-radius:4px; padding:6px 10px; cursor:pointer; font-size:14px;
}
.icon-btn:hover { background:rgba(255,255,255,0.15); }

/* 分隔线 */
.divider {
  display:flex; align-items:center; gap:12px; margin:20px 0;
  color:rgba(255,255,255,0.3); font-size:12px;
}
.divider::before, .divider::after {
  content:''; flex:1; height:1px; background:rgba(255,255,255,0.08);
}

/* 手动安装区 */
.section { margin-bottom:16px; }
.section-title {
  font-size:14px; font-weight:600; color:rgba(255,255,255,0.8); margin-bottom:8px;
}
.hint { font-size:12px; color:rgba(255,255,255,0.4); margin-bottom:12px; line-height:1.5; }

/* 错误 */
.error-msg {
  background:rgba(232,17,35,0.12); border:1px solid rgba(232,17,35,0.25);
  border-radius:6px; padding:10px 14px; color:#ff6b6b; font-size:13px; margin-top:12px;
}

/* 过渡 */
.modal-fade-enter-active, .modal-fade-leave-active { transition:opacity 0.25s; }
.modal-fade-enter-from, .modal-fade-leave-to { opacity:0; }
</style>
