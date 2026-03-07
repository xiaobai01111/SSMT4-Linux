import { createRouter, createWebHistory, type RouteRecordName, type RouteRecordRaw } from 'vue-router'
import { appSettings, settingsLoaded, settingsReady } from '../store'

type PreloadableRouteName =
  | 'Home'
  | 'Setup'
  | 'GameLibrary'
  | 'Websites'
  | 'Settings'
  | 'Documents'
  | 'LogViewer'
  | 'GameLogViewer'

// 懒加载工厂
const lazyHome = () => import('../views/Home.vue')
const lazySetup = () => import('../views/Setup.vue')
const lazyGameLibrary = () => import('../views/GameLibrary.vue')
const lazyWebsites = () => import('../views/Websites.vue')
const lazySettings = () => import('../views/Settings.vue')
const lazyDocuments = () => import('../views/Documents.vue')
const lazyLogViewer = () => import('../views/LogViewer.vue')
const lazyGameLogViewer = () => import('../views/GameLogViewer.vue')

const routeComponentLoaders: Record<PreloadableRouteName, () => Promise<unknown>> = {
  Home: lazyHome,
  Setup: lazySetup,
  GameLibrary: lazyGameLibrary,
  Websites: lazyWebsites,
  Settings: lazySettings,
  Documents: lazyDocuments,
  LogViewer: lazyLogViewer,
  GameLogViewer: lazyGameLogViewer,
}

const preloadedRoutes = new Set<PreloadableRouteName>()

export const preloadRouteView = (routeName: PreloadableRouteName) => {
  if (preloadedRoutes.has(routeName)) return

  preloadedRoutes.add(routeName)
  void routeComponentLoaders[routeName]().catch(() => {
    preloadedRoutes.delete(routeName)
  })
}

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

const isSetupExemptRoute = (name: RouteRecordName | null | undefined) => {
  return name === 'LogViewer' || name === 'GameLogViewer'
}

const needsSetup = () => {
  return !appSettings.initialized || !appSettings.tosRiskAcknowledged
}

let deferredSetupCheckRegistered = false

const registerDeferredSetupCheck = () => {
  if (deferredSetupCheckRegistered) return

  deferredSetupCheckRegistered = true
  void settingsLoaded.then(() => {
    deferredSetupCheckRegistered = false

    const currentRouteName = router.currentRoute.value.name
    if (isSetupExemptRoute(currentRouteName) || currentRouteName === 'Setup') return
    if (!needsSetup()) return

    void router.replace({ name: 'Setup' })
  })
}

// 首次启动导航守卫：设置未就绪时先放行首路由，加载完成后再按需纠偏到向导页。
router.beforeEach((to) => {
  if (isSetupExemptRoute(to.name)) return true

  if (!settingsReady.value) {
    registerDeferredSetupCheck()
    return true
  }

  if (needsSetup() && to.name !== 'Setup') {
    return { name: 'Setup' }
  }

  return true
})

export default router
