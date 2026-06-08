import { test, expect } from '@playwright/test';

const SECTIONS = ['about', 'cluster', 'satellites', 'life', 'path', 'photography'];

test('all section IDs present in DOM', async ({ page }) => {
  await page.goto('/');
  await page.waitForTimeout(5_000);

  for (const id of SECTIONS) {
    await expect(page.locator(`#${id}`), `#${id} should be in DOM`).toBeAttached();
  }
});

test('Game of Life canvas has non-zero dimensions', async ({ page }) => {
  await page.goto('/');
  await page.locator('#life').scrollIntoViewIfNeeded();
  await page.waitForTimeout(3_000);

  const canvas = page.locator('#life canvas').first();
  await expect(canvas).toBeVisible();

  const size = await canvas.evaluate(el => ({
    w: (el as HTMLCanvasElement).clientWidth,
    h: (el as HTMLCanvasElement).clientHeight,
  }));
  console.log(`Life canvas: ${size.w}×${size.h}`);
  expect(size.w, 'canvas width > 0').toBeGreaterThan(0);
  expect(size.h, 'canvas height > 0').toBeGreaterThan(0);
});

test('pathfinding canvases have non-zero dimensions', async ({ page }) => {
  await page.goto('/');
  await page.locator('#path').scrollIntoViewIfNeeded();
  await page.waitForTimeout(3_000);

  const canvases = page.locator('#path canvas');
  const count = await canvases.count();
  console.log(`Pathfinding canvases found: ${count}`);
  expect(count, 'at least one pathfinding canvas').toBeGreaterThan(0);

  const size = await canvases.first().evaluate(el => ({
    w: (el as HTMLCanvasElement).clientWidth,
    h: (el as HTMLCanvasElement).clientHeight,
  }));
  console.log(`First pathfinding canvas: ${size.w}×${size.h}`);
  expect(size.w, 'canvas width > 0').toBeGreaterThan(0);
  expect(size.h, 'canvas height > 0').toBeGreaterThan(0);
});
