import { defineVitestConfig } from '@nuxt/test-utils/config'

// Unit tests of pure server-side logic (text normalization, duration parsing,
// filter composition, hash helpers) run in a plain node environment. Component
// tests can opt into the 'nuxt' environment per-file via:
//   // @vitest-environment nuxt
export default defineVitestConfig({
  test: {
    environment: 'node',
    include: ['test/**/*.{test,spec}.ts'],
    setupFiles: ['./test/setup.ts'],
  },
})
