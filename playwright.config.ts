import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 60_000,
  use: {
    baseURL: process.env.BASE_URL ?? 'https://jaydanhoward.com',
    headless: true,
  },
});
