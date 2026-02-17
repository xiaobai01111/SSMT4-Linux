import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import { appSettings, settingsLoaded } from '../store'

const routes: RouteRecordRaw[] = [
  { path: '/', name: 'Home', component: () => import('../views/Home.vue') },
  { path: '/setup', name: 'Setup', component: () => import('../views/Setup.vue') },
  { path: '/games', name: 'GameLibrary', component: () => import('../views/GameLibrary.vue') },
  { path: '/websites', name: 'Websites', component: () => import('../views/Websites.vue') },
  { path: '/settings', name: 'Settings', component: () => import('../views/Settings.vue') },
  { path: '/documents', name: 'Documents', component: () => import('../views/Documents.vue') },
  { path: '/log-viewer', name: 'LogViewer', component: () => import('../views/LogViewer.vue') },
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
