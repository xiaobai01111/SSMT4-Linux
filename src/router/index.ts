import { createRouter, createWebHistory } from 'vue-router'
import Home from '../views/Home.vue'
import GameLibrary from '../views/GameLibrary.vue'
import Websites from '../views/Websites.vue'
import Settings from '../views/Settings.vue'
import Documents from '../views/Documents.vue'
import ModsManagement from '../views/ModsManagement.vue'

const routes = [
  { path: '/', name: 'Home', component: Home },
  { path: '/games', name: 'GameLibrary', component: GameLibrary },
  { path: '/mods', name: 'ModsManagement', component: ModsManagement },
  { path: '/websites', name: 'Websites', component: Websites },
  { path: '/settings', name: 'Settings', component: Settings },
  { path: '/documents', name: 'Documents', component: Documents },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

export default router
