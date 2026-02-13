<script setup lang="ts">
import { ref } from 'vue';
import { useRouter } from 'vue-router';
import { appSettings } from '../store';
import { openFileDialog } from '../api';

const router = useRouter();
const step = ref(1);
const selectedDir = ref('');

const selectDir = async () => {
  const selected = await openFileDialog({
    directory: true,
    multiple: false,
    title: '选择 SSMT4 数据存储目录'
  });
  if (selected && typeof selected === 'string') {
    selectedDir.value = selected;
  }
};

const useDefault = () => {
  selectedDir.value = '';
  finish();
};

const finish = () => {
  appSettings.dataDir = selectedDir.value;
  appSettings.initialized = true;
  router.replace('/');
};
</script>

<template>
  <div class="setup-page">
    <div class="setup-card">
      <!-- Step 1: 欢迎 -->
      <template v-if="step === 1">
        <div class="setup-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 2L2 7l10 5 10-5-10-5z"/>
            <path d="M2 17l10 5 10-5"/>
            <path d="M2 12l10 5 10-5"/>
          </svg>
        </div>
        <h1 class="setup-title">欢迎使用 SSMT4</h1>
        <p class="setup-desc">3DMigoto Toolbox for Linux</p>
        <p class="setup-hint">首次使用需要进行简单配置，只需一步即可完成。</p>
        <button class="setup-btn primary" @click="step = 2">
          开始配置
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="9 18 15 12 9 6"/>
          </svg>
        </button>
      </template>

      <!-- Step 2: 选择数据目录 -->
      <template v-if="step === 2">
        <h2 class="setup-title">选择数据目录</h2>
        <p class="setup-desc">
          SSMT4 需要一个目录来存放所有数据，包括：
        </p>
        <ul class="setup-list">
          <li>游戏配置文件</li>
          <li>游戏下载与安装</li>
          <li>Wine 前缀</li>
          <li>Mod 缓存与工具</li>
          <li>日志文件</li>
        </ul>

        <div class="dir-section">
          <div class="dir-current" v-if="selectedDir">
            <span class="dir-label">已选择：</span>
            <span class="dir-path">{{ selectedDir }}</span>
          </div>
          <div class="dir-actions">
            <button class="setup-btn primary" @click="selectDir">
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
                fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
              选择目录
            </button>
            <button class="setup-btn secondary" @click="useDefault">
              使用默认路径
            </button>
          </div>
          <p class="setup-hint default-hint">
            默认路径：~/.config/ssmt4 + ~/.local/share/ssmt4（分散在系统目录中）
          </p>
        </div>

        <button
          class="setup-btn primary finish-btn"
          @click="finish"
          v-if="selectedDir"
        >
          完成配置，进入 SSMT4
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
        </button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.setup-page {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  padding: 24px;
}
.setup-card {
  max-width: 520px;
  width: 100%;
  background: rgba(20, 20, 20, 0.85);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  padding: 48px 40px;
  backdrop-filter: blur(20px);
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
  text-align: center;
}
.setup-icon {
  color: #F7CE46;
  margin-bottom: 20px;
}
.setup-title {
  font-size: 24px;
  font-weight: 700;
  color: #fff;
  margin: 0 0 8px 0;
}
.setup-desc {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.5);
  margin: 0 0 16px 0;
  line-height: 1.6;
}
.setup-hint {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.35);
  margin: 16px 0;
  line-height: 1.5;
}
.setup-list {
  text-align: left;
  padding-left: 20px;
  margin: 12px 0 20px 0;
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
  line-height: 2;
}
.setup-list li::marker {
  color: #F7CE46;
}
.dir-section {
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 10px;
  padding: 20px;
  margin: 16px 0;
}
.dir-current {
  margin-bottom: 12px;
  padding: 10px 14px;
  background: rgba(247, 206, 70, 0.08);
  border: 1px solid rgba(247, 206, 70, 0.2);
  border-radius: 6px;
  text-align: left;
}
.dir-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  display: block;
  margin-bottom: 4px;
}
.dir-path {
  font-size: 13px;
  color: #F7CE46;
  word-break: break-all;
}
.dir-actions {
  display: flex;
  gap: 10px;
  justify-content: center;
}
.default-hint {
  font-size: 11px;
  margin: 12px 0 0 0;
}
.setup-btn {
  padding: 10px 24px;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  transition: all 0.2s;
}
.setup-btn.primary {
  background: rgba(247, 206, 70, 0.2);
  color: #F7CE46;
  border: 1px solid rgba(247, 206, 70, 0.3);
}
.setup-btn.primary:hover {
  background: rgba(247, 206, 70, 0.3);
}
.setup-btn.secondary {
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.1);
}
.setup-btn.secondary:hover {
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
}
.finish-btn {
  margin-top: 20px;
  padding: 14px 32px;
  font-size: 15px;
}
</style>
