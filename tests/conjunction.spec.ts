import { test, expect } from '@playwright/test';

test('conjunction events do not flash after loading completes', async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on('console', msg => {
    if (msg.type() === 'error') consoleErrors.push(msg.text());
  });

  await page.goto('/');

  // Scroll to the satellites section so it's in view
  await page.locator('#satellites').scrollIntoViewIfNeeded();

  // Wait up to 45s for the conjunction table to appear (needs screening to complete)
  const tableBody = page.locator('#satellites tbody');
  await tableBody.waitFor({ state: 'visible', timeout: 45_000 });

  // Give it a couple more seconds to stabilise
  await page.waitForTimeout(2_000);

  const initialRows = await tableBody.locator('tr').count();
  console.log(`Stable row count: ${initialRows}`);
  expect(initialRows).toBeGreaterThan(0);

  // Sample the row count every 500 ms for 15 seconds and detect drops to zero
  let zeroCount = 0;
  let totalSamples = 0;
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    await page.waitForTimeout(500);
    const count = await tableBody.locator('tr').count();
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
