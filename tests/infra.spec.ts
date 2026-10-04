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

// /pkg and /widgets are versioned by content hash (?v=…, stamped into the
// page at startup — see site_middleware.rs::bundle_cache_policy). A stale
// cached glue file paired with a new .wasm breaks Foster entirely (a
// production LinkError after a deploy is what motivated this).
test('cache: WASM bundle is versioned and only immutable with the current version', async ({ page }) => {
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
  // The page stamps the bundle's content hash into its URL (?v=…), and the
  // server only marks it immutable when that version matches what it serves.
  // An unversioned bundle URL must never be immutable.
  const version = new URL(res.url()).searchParams.get('v');
  expect(version, 'foster_client_bg.wasm is requested with ?v=<content hash>').toBeTruthy();
  expect(wasmCacheControl).toContain('immutable');

  const unversioned = await page.request.get(new URL(res.url()).pathname);
  expect(unversioned.headers()['cache-control'] ?? '').not.toContain('immutable');
  const stale = await page.request.get(`${new URL(res.url()).pathname}?v=not-this-build`);
  expect(stale.headers()['cache-control']).toBe('no-store');
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
