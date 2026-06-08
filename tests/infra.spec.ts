import { test, expect } from '@playwright/test';

test('/health_check returns 200', async ({ request }) => {
  const res = await request.get('/health_check');
  expect(res.status()).toBe(200);
});

test('/robots.txt returns 200 with User-agent directive', async ({ request }) => {
  const res = await request.get('/robots.txt');
  expect(res.status()).toBe(200);
  const body = await res.text();
  expect(body).toContain('User-agent');
});

test('cache: HTML root gets max-age=0 must-revalidate', async ({ request }) => {
  const res = await request.get('/');
  const cc = res.headers()['cache-control'];
  console.log(`/ Cache-Control: ${cc}`);
  expect(cc).toContain('max-age=0');
  expect(cc).toContain('must-revalidate');
});

test('cache: WASM asset gets 1-hour TTL', async ({ page }) => {
  let wasmCacheControl: string | null = null;
  let wasmUrl: string | null = null;

  page.on('response', res => {
    if (!wasmUrl && res.url().endsWith('.wasm')) {
      wasmUrl = res.url();
      wasmCacheControl = res.headers()['cache-control'] ?? null;
    }
  });

  await page.goto('/');
  await page.waitForTimeout(8_000);

  console.log(`WASM: ${wasmUrl}  →  ${wasmCacheControl}`);
  expect(wasmUrl, '.wasm file should be requested on page load').toBeTruthy();
  expect(wasmCacheControl).toContain('max-age=3600');
});

test('cache: hashed JS gets immutable 1-year TTL', async ({ page }) => {
  const hasHashSegment = (url: string) =>
    url.split('/').some(seg => seg.length >= 8 && /^[0-9a-f]+$/i.test(seg));

  let hashedJsUrl: string | null = null;
  let hashedJsCc: string | null = null;

  page.on('response', res => {
    const url = res.url();
    if (!hashedJsUrl && url.endsWith('.js') && hasHashSegment(url)) {
      hashedJsUrl = url;
      hashedJsCc = res.headers()['cache-control'] ?? null;
    }
  });

  await page.goto('/');
  await page.waitForTimeout(8_000);

  console.log(`Hashed JS: ${hashedJsUrl}  →  ${hashedJsCc}`);
  if (!hashedJsUrl) {
    test.skip(true, 'no hashed JS URLs requested — build may not hash JS filenames');
    return;
  }
  expect(hashedJsCc).toContain('immutable');
});
