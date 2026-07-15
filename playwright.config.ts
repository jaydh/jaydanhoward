import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 60_000,
  // The self-hosted CI runner is modest, and this page is genuinely heavy
  // per tab now (WebGL2 3D globe rendering ~16k real satellites, real
  // client-side conjunction-event polling, ~10 Foster SSE connections) —
  // 4 parallel chromium instances doing all of that at once was enough to
  // crash the browser mid-run in the real CI environment ("Target page,
  // context or browser has been closed"). One worker is slower but robust.
  workers: 1,
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
