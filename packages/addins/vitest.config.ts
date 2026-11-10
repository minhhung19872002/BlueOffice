import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    testTimeout: 30_000,
    css: { modules: { classNameStrategy: 'non-scoped' } },
  },
});
