import { createRouter, createWebHistory } from 'vue-router'
import Home from '../views/Home.vue'
import GameLibrary from '../views/GameLibrary.vue'
import Websites from '../views/Websites.vue'
import Settings from '../views/Settings.vue'
import Documents from '../views/Documents.vue'
import ModsManagement from '../views/ModsManagement.vue'
import Setup from '../views/Setup.vue'
import LogViewer from '../views/LogViewer.vue'
import { appSettings, settingsLoaded } from '../store'

const routes = [
  { path: '/', name: 'Home', component: Home },
  { path: '/setup', name: 'Setup', component: Setup },
  { path: '/games', name: 'GameLibrary', component: GameLibrary },
  { path: '/mods', name: 'ModsManagement', component: ModsManagement },
  { path: '/websites', name: 'Websites', component: Websites },
  { path: '/settings', name: 'Settings', component: Settings },
  { path: '/documents', name: 'Documents', component: Documents },
  { path: '/log-viewer', name: 'LogViewer', component: LogViewer },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

// 首次启动导航守卫：等待设置加载完成后，未完成初始化或未确认风险时跳转到向导页
router.beforeEach(async (to) => {
  // 日志查看器窗口不受初始化/风险确认限制
  if (to.name === 'LogViewer') return;
  await settingsLoaded;
  if ((!appSettings.initialized || !appSettings.tosRiskAcknowledged) && to.name !== 'Setup') {
    return { name: 'Setup' };
  }
})

export default router
