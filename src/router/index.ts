import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import { appSettings, settingsLoaded } from '../store'

// 懒加载工厂 + 首次导航后预加载所有路由组件
const lazyHome = () => import('../views/Home.vue')
const lazySetup = () => import('../views/Setup.vue')
const lazyGameLibrary = () => import('../views/GameLibrary.vue')
const lazyWebsites = () => import('../views/Websites.vue')
const lazySettings = () => import('../views/Settings.vue')
const lazyDocuments = () => import('../views/Documents.vue')
const lazyLogViewer = () => import('../views/LogViewer.vue')
const lazyGameLogViewer = () => import('../views/GameLogViewer.vue')

const routes: RouteRecordRaw[] = [
  { path: '/', name: 'Home', component: lazyHome },
  { path: '/setup', name: 'Setup', component: lazySetup },
  { path: '/games', name: 'GameLibrary', component: lazyGameLibrary },
  { path: '/websites', name: 'Websites', component: lazyWebsites },
  { path: '/settings', name: 'Settings', component: lazySettings },
  { path: '/documents', name: 'Documents', component: lazyDocuments },
  { path: '/log-viewer', name: 'LogViewer', component: lazyLogViewer },
  { path: '/game-log-viewer', name: 'GameLogViewer', component: lazyGameLogViewer },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

// 首次启动导航守卫：等待设置加载完成后，未完成初始化或未确认风险时跳转到向导页
router.beforeEach(async (to) => {
  // 日志查看器窗口不受初始化/风险确认限制
  if (to.name === 'LogViewer' || to.name === 'GameLogViewer') return;
  await settingsLoaded;
  if ((!appSettings.initialized || !appSettings.tosRiskAcknowledged) && to.name !== 'Setup') {
    return { name: 'Setup' };
  }
})

// 首次导航完成后，后台预加载所有路由组件（消除后续页面切换的编译延迟）
let prefetched = false
const idleCallback = typeof requestIdleCallback === 'function'
  ? requestIdleCallback
  : (cb: () => void) => setTimeout(cb, 200)

router.afterEach(() => {
  if (prefetched) return
  prefetched = true
  idleCallback(() => {
    lazyHome()
    lazySetup()
    lazyGameLibrary()
    lazyWebsites()
    lazySettings()
    lazyDocuments()
  })
})

export default router
