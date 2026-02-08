import { defineConfig } from 'vitest/config'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { svelteTesting } from '@testing-library/svelte/vite'

export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        ws: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./vitest-setup.ts'],
  },
})
