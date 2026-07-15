import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 60_000,
  use: {
    baseURL: process.env.BASE_URL ?? 'https://jaydanhoward.com',
    headless: true,
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    // Foster runs ~10 independent SSE connections (one per machine) plus
    // regular fetches; over plain HTTP/1.1 that blows past Chromium's
    // 6-connections-per-origin cap and POSTs (e.g. button clicks) hang
    // forever waiting for a free slot. CI fronts the app with a local
    // HTTP/2 reverse proxy (self-signed cert) to multiplex it over one
    // connection — see general.yml's integration-test job — hence this.
    ignoreHTTPSErrors: true,
  },
});
