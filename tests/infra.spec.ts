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

// Foster's /pkg (wasm-pack output) has no content-hash versioning yet,
// unlike the real Leptos site's ?v={hash} query-string scheme — see
// site_middleware.rs::cache_control's doc comment. Caching it immutably
// without a busting mechanism would pin browsers to a stale WASM/JS pair
// after a deploy, so this only asserts the safer fallback is in effect,
// not the (currently inapplicable) immutable rule.
test('cache: WASM asset does not use an unsafe immutable TTL without cache-busting', async ({ page }) => {
  // This test has failed on every CI run since the Foster migration
  // (never reproduces locally, in isolation or under the full suite) —
  // the failure mode is a bare 30s timeout with zero '.wasm' response
  // ever observed. These listeners exist to turn the next CI failure into
  // real evidence (every request + its outcome, plus JS/page errors)
  // instead of another blind guess-and-check cycle.
  const allRequests: string[] = [];
  const failedRequests: string[] = [];
  const pageErrors: string[] = [];
  const consoleErrors: string[] = [];

  page.on('request', req => allRequests.push(req.url()));
  page.on('requestfailed', req => failedRequests.push(`${req.url()} — ${req.failure()?.errorText}`));
  page.on('pageerror', err => pageErrors.push(err.message));
  page.on('console', msg => {
    if (msg.type() === 'error') consoleErrors.push(msg.text());
  });

  const wasmResponsePromise = page.waitForResponse(
    res => new URL(res.url()).pathname.endsWith('.wasm'),
    { timeout: 30_000 },
  );

  await page.goto('/');

  let res;
  try {
    res = await wasmResponsePromise;
  } catch (err) {
    console.log('--- WASM response never observed. Diagnostics: ---');
    console.log('All requests:', JSON.stringify(allRequests, null, 2));
    console.log('Failed requests:', JSON.stringify(failedRequests, null, 2));
    console.log('Page errors:', JSON.stringify(pageErrors, null, 2));
    console.log('Console errors:', JSON.stringify(consoleErrors, null, 2));
    throw err;
  }
  const wasmCacheControl = res.headers()['cache-control'] ?? null;

  console.log(`WASM: ${res.url()}  →  ${wasmCacheControl}`);
  expect(wasmCacheControl).not.toContain('immutable');
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
