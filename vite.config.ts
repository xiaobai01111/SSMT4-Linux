import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'
import { resolve } from "node:path";


const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(({ command }) => ({
  base: command === "build" ? "./" : "/",
  plugins: [
    vue(),
    AutoImport({
      resolvers: [ElementPlusResolver({ importStyle: 'css' })],
    }),
    Components({
      resolvers: [ElementPlusResolver({ importStyle: 'css' })],
    }),
  ],
  esbuild:
    command === "build"
      ? {
          drop: ["debugger"],
          pure: ["console.log", "console.debug", "console.info"],
        }
      : undefined,
  build:
    command === "build"
      ? {
          cssCodeSplit: true,
          rollupOptions: {
            input: {
              main: resolve(__dirname, "index.html"),
            },
            output: {
              manualChunks(id) {
                if (!id.includes('node_modules')) return undefined

                if (
                  id.includes('element-plus') ||
                  id.includes('@element-plus') ||
                  id.includes('async-validator') ||
                  id.includes('@ctrl/tinycolor')
                ) {
                  return 'vendor-element-plus'
                }

                if (
                  id.includes('/vue/') ||
                  id.includes('@vue') ||
                  id.includes('vue-router') ||
                  id.includes('vue-i18n')
                ) {
                  return 'vendor-vue'
                }

                if (id.includes('@tauri-apps')) {
                  return 'vendor-tauri'
                }

                return 'vendor-misc'
              },
            },
          },
        }
      : undefined,

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
