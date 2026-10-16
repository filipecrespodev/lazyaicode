import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// Svelte para a casca e os agentes; React + Tailwind só para a visão "Git"
// (src/git), que importa os próprios módulos via "@/".
export default defineConfig({
  plugins: [svelte(), react({ include: /src\/git\/.*\.[jt]sx?$/ }), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src/git', import.meta.url)) } },
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  build: { target: 'es2022', chunkSizeWarningLimit: 3000 },
  test: { environment: 'node', include: ['src/**/*.test.ts'] },
})
