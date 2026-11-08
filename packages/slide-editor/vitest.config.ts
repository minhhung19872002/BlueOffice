import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    css: { modules: { classNameStrategy: 'non-scoped' } },
    // The UI tests wait on async engine round trips; leave room when the
    // machine is busy (parallel workspace runs).
    testTimeout: 20_000,
    setupFiles: ['./test/setup.ts'],
  },
});
