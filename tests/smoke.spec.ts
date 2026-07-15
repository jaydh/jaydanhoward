import { test, expect } from '@playwright/test';

// Foster's client (static/pkg/foster_client.js + .wasm) is the one piece of
// WASM the new site ships — same class of failure as the old Leptos
// hydration bundle (LinkError from a missing web-sys/JS shim binding).
test('WASM instantiates and page loads without fatal errors', async ({ page }) => {
  const fatal: string[] = [];

  page.on('pageerror', err => fatal.push(err.message));
  page.on('console', msg => {
    if (msg.type() === 'error') {
      const t = msg.text();
      if (t.includes('LinkError') || t.includes('WebAssembly') || t.includes('instantiate')) {
        fatal.push(t);
      }
    }
  });

  await page.goto('/');
  await page.waitForTimeout(10_000);

  if (fatal.length) console.log('Fatal errors:', fatal);
  expect(fatal, 'No WASM/fatal JS errors on page load').toHaveLength(0);
});

// Catches broken hand-rolled axum routes (500s) that would show blank
// panels to users. DB/Prometheus/CelesTrak-dependent endpoints are excluded
// in CI since none of those are available there — covered by testing
// against production instead.
test('API routes return non-500 on initial load', async ({ page }) => {
  const failures: string[] = [];

  const ciSkip = [
    // Requires CelesTrak network access (blocks the CI runner's IP with 403)
    '/api/conjunction', '/api/satellites',
  ];

  page.on('response', res => {
    const url = res.url();
    if (!url.includes('/api/') && !url.includes('/world-map.svg')) return;
    if (res.status() < 500) return;
    if (ciSkip.some(e => url.includes(e))) return;
    failures.push(`${res.status()} ${url}`);
  });

  await page.goto('/');
  await page.waitForTimeout(20_000);

  if (failures.length) console.log('500 errors:', failures);
  expect(failures, 'No unexpected server 500s').toHaveLength(0);
});
