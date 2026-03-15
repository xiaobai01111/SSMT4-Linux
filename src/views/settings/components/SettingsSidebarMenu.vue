<script setup lang="ts">
import { useI18n } from 'vue-i18n';

const props = defineProps<{
  activeMenu: string;
  guideMenu: string;
}>();

const emit = defineEmits<{
  (event: 'select', value: string): void;
}>();

const { t, te } = useI18n();

const tr = (key: string, fallback: string) => {
  return te(key) ? String(t(key)) : fallback;
};

const handleSelect = (value: string) => {
  emit('select', value);
};
</script>

<template>
  <div class="sidebar-menu-container" data-onboarding="settings-menu">
    <ul class="menu-list">
      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'basic' }" @click="handleSelect('basic')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
          <span class="menu-text">{{ t('settings.basicsettings') }}</span>
        </a>
      </li>

      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'appearance' }" @click="handleSelect('appearance')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2.69l5.66 5.66a8 8 0 1 1-11.31 0z"/></svg>
          <span class="menu-text">{{ t('settings.appearance') }}</span>
        </a>
      </li>

      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'display' }" @click="handleSelect('display')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
          <span class="menu-text">{{ t('settings.page_display') }}</span>
        </a>
      </li>

      <li class="menu-group-title">{{ tr('settings.sidebar.systemMaintenance', '系统维护') }}</li>

      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'version' }" @click="handleSelect('version')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 6v6l4 2"/><circle cx="12" cy="12" r="9"/></svg>
          <span class="menu-text">{{ tr('settings.version_check_title', '版本检查') }}</span>
        </a>
      </li>

      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'resource' }" @click="handleSelect('resource')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 14a8 8 0 0 1 8-8h8"/><path d="M20 10V6h-4"/><path d="M20 10a8 8 0 0 1-8 8H4"/><path d="M4 14v4h4"/></svg>
          <span class="menu-text">{{ tr('settings.resource.title', '资源更新') }}</span>
        </a>
      </li>

      <li class="menu-group-title">{{ tr('settings.sidebar.runtimeManagement', '核心环境管理') }}</li>

      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'proton' }" @click="handleSelect('proton')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 16V8a2 2 0 0 0-1-1.73l-6-3.46a2 2 0 0 0-2 0L5 6.27A2 2 0 0 0 4 8v8a2 2 0 0 0 1 1.73l6 3.46a2 2 0 0 0 2 0l6-3.46A2 2 0 0 0 20 16z"></path><polyline points="7.5 4.21 12 6.81 16.5 4.21"></polyline><polyline points="7.5 19.79 7.5 14.6 3 12"></polyline><polyline points="21 12 16.5 14.6 16.5 19.79"></polyline><polyline points="12 22.08 12 16.9 7.5 14.3"></polyline><polyline points="12 16.9 16.5 14.3"></polyline><polyline points="12 6.81 12 12"></polyline></svg>
          <span class="menu-text">{{ tr('settings.proton_manage_title', 'Proton 管理') }}</span>
          <span v-if="props.guideMenu === 'proton'" class="guide-dot"></span>
        </a>
      </li>

      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'dxvk' }" @click="handleSelect('dxvk')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 20h20"/><path d="M5 20V8l7-5 7 5v12"/><path d="M9 20v-4h6v4"/><path d="M9 12h6"/><path d="M9 16h6"/></svg>
          <span class="menu-text">{{ tr('settings.dxvk_manage_title', 'DXVK 管理') }}</span>
          <span v-if="props.guideMenu === 'dxvk'" class="guide-dot"></span>
        </a>
      </li>

      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'vkd3d' }" @click="handleSelect('vkd3d')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 20h16"/><path d="M6 20V8l6-4 6 4v12"/><path d="M9 12h6"/><path d="M9 16h6"/></svg>
          <span class="menu-text">{{ tr('settings.vkd3d_manage_title', 'VKD3D 管理') }}</span>
        </a>
      </li>

      <li>
        <a class="menu-link" :class="{ 'active': activeMenu === 'migoto' }" @click="handleSelect('migoto')">
          <svg class="menu-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>
          <span class="menu-text">{{ tr('settings.migoto_manage_title', '3DMIGOTO 管理') }}</span>
        </a>
      </li>

    </ul>
  </div>
</template>

<style scoped>
/* 侧边栏主容器 - 增加固定宽度和防压缩 */
.sidebar-menu-container {
  width: 100%;
  min-width: 0;
  flex-shrink: 0;
  height: 100%;
  overflow-y: auto;
  background-color: transparent;
  padding-right: 0;
}

/* 隐藏外置滚动条但保留滚动能力（更美观） */
.sidebar-menu-container::-webkit-scrollbar {
  width: 4px;
}
.sidebar-menu-container::-webkit-scrollbar-thumb {
  background: var(--el-border-color-darker);
  border-radius: 4px;
}

/* 菜单列表容器 */
.menu-list {
  list-style: none;
  padding: 18px 12px;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 菜单项基础样式 */
.menu-link {
  display: flex;
  align-items: center;
  padding: 10px 14px;
  border: 1px solid transparent;
  border-radius: 14px;
  cursor: pointer;
  color: var(--el-text-color-regular);
  text-decoration: none;
  background: rgba(255, 255, 255, 0.02);
  transition:
    background-color 0.2s,
    color 0.2s,
    border-color 0.2s,
    box-shadow 0.2s,
    transform 0.2s;
  user-select: none;
}

/* 悬停状态 */
.menu-link:hover {
  background-color: rgba(255, 255, 255, 0.05);
  border-color: rgba(255, 255, 255, 0.08);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
  color: var(--el-text-color-primary);
  transform: translateX(2px);
}

/* 激活状态（自动读取 Element Plus 主题色） */
.menu-link.active {
  background:
    linear-gradient(90deg, rgba(var(--el-color-primary-rgb), 0.16), rgba(var(--el-color-primary-rgb), 0.06));
  border-color: rgba(var(--el-color-primary-rgb), 0.28);
  box-shadow:
    inset 0 0 0 1px rgba(var(--el-color-primary-rgb), 0.12),
    0 10px 20px rgba(0, 0, 0, 0.16);
  color: var(--el-color-primary);
}

/* 图标控制（彻底解决图标变大问题） */
.menu-icon {
  width: 20px;
  height: 20px;
  margin-right: 12px;
  opacity: 0.7;
  flex-shrink: 0;
  transition: opacity 0.2s, color 0.2s;
}

.menu-link:hover .menu-icon {
  opacity: 1;
}

.menu-link.active .menu-icon {
  opacity: 1;
  color: var(--el-color-primary);
}

/* 文本截断与对齐 */
.menu-text {
  font-size: 14px;
  font-weight: 500;
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.menu-link.active .menu-text {
  font-weight: 600;
}

/* 分组标题（如“系统维护”、“核心环境管理”） */
.menu-group-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 1px;
  margin: 18px 0 8px 0;
  padding: 14px 12px 0 12px;
  opacity: 0.78;
  user-select: none;
  position: relative;
}

.menu-group-title::before {
  content: '';
  position: absolute;
  top: 0;
  left: 12px;
  right: 12px;
  height: 1px;
  background: linear-gradient(90deg, rgba(255, 255, 255, 0.14), rgba(255, 255, 255, 0.03));
}

/* 呼吸提示小红点 */
.guide-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--el-color-primary);
  margin-left: auto;
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0% { box-shadow: 0 0 0 0 rgba(var(--el-color-primary-rgb), 0.7); }
  70% { box-shadow: 0 0 0 6px rgba(var(--el-color-primary-rgb), 0); }
  100% { box-shadow: 0 0 0 0 rgba(var(--el-color-primary-rgb), 0); }
}
</style>
