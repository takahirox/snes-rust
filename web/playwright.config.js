import { defineConfig } from '@playwright/test';
const port = Number(process.env.SNES_TEST_PORT ?? 8080);
export default defineConfig({
  testDir: './tests', timeout: 30000, workers: 1,
  use: { baseURL: `http://127.0.0.1:${port}`, browserName: 'chromium' },
  webServer: { command: `python3 -m http.server ${port} --bind 127.0.0.1 --directory dist`, url: `http://127.0.0.1:${port}`, reuseExistingServer: false, timeout: 10000 },
});
