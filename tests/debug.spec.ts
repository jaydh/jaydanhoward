import { test } from '@playwright/test';

test('debug conjunction API responses', async ({ page }) => {
  const responses: { t: number; url: string; body: string }[] = [];
  const t0 = Date.now();

  page.on('response', async res => {
    if (res.url().includes('conjunction_status')) {
      try {
        const body = await res.text();
        responses.push({ t: Date.now() - t0, url: res.url(), body });
      } catch {}
    }
  });

  await page.goto('/');
  await page.locator('#satellites').scrollIntoViewIfNeeded();
  await page.waitForTimeout(30_000);

  console.log('\n=== conjunction_status API responses ===');
  for (const r of responses) {
    console.log(`t=${(r.t / 1000).toFixed(1)}s: ${r.body.trim()}`);
  }
});
