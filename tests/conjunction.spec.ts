import { test, expect } from '@playwright/test';

test('conjunction events do not flash after loading completes', async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on('console', msg => {
    if (msg.type() === 'error') consoleErrors.push(msg.text());
  });

  await page.goto('/');
  // Conjunction screening is nested inside the Satellites section (matching
  // the real site's src/components/app.rs layout), not its own top-level id.
  await page.locator('[fx-machine="conjunction"]').scrollIntoViewIfNeeded();
  await page.click('#conjunction-start').catch(() => {});

  // Give the server 15 s to start a screening. If CelesTrak is unreachable (CI with
  // no egress to external hosts), screening never completes — skip rather than
  // fail, since there is nothing to flash-test in that case.
  // Status comes from the shared "conjunction" Foster machine (fx-text="status").
  const status = page.locator('[fx-machine="conjunction"] [fx-text="status"]');
  let screeningActive = false;
  const startDeadline = Date.now() + 15_000;
  while (Date.now() < startDeadline && !screeningActive) {
    await page.waitForTimeout(500);
    screeningActive = /running|complete/.test((await status.textContent()) ?? '');
  }
  if (!screeningActive) {
    test.skip(true, 'Conjunction screening did not start — CelesTrak unreachable in this environment');
  }

  // Rendered fx-for rows carry data-fx-item (the hidden template <li> doesn't).
  const events = page.locator('[fx-machine="conjunction"] li[data-fx-item]');
  await expect(async () => {
    expect(await events.count()).toBeGreaterThan(0);
  }).toPass({ timeout: 30_000 });

  await page.waitForTimeout(2_000);

  const initialRows = await events.count();
  console.log(`Stable row count: ${initialRows}`);
  expect(initialRows).toBeGreaterThan(0);

  // Sample the row count every 500 ms for 15 seconds and detect drops to zero
  let zeroCount = 0;
  let totalSamples = 0;
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    await page.waitForTimeout(500);
    const count = await events.count();
    totalSamples++;
    if (count === 0) {
      zeroCount++;
      console.log(`⚠ t=${((15_000 - (deadline - Date.now())) / 1000).toFixed(1)}s — rows dropped to 0`);
    }
  }

  console.log(`Zero-row samples: ${zeroCount}/${totalSamples}`);
  if (consoleErrors.length) console.log('JS errors:', consoleErrors);

  expect(zeroCount, 'rows should never drop to 0 after initial load').toBe(0);
});
