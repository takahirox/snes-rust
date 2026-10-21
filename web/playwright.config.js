import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests', timeout: 30000, workers: 1,
  use: { baseURL: 'http://127.0.0.1:8080', browserName: 'chromium' },
  webServer: { command: 'python3 -m http.server 8080 --bind 127.0.0.1 --directory dist', url: 'http://127.0.0.1:8080', reuseExistingServer: false, timeout: 10000 },
});
