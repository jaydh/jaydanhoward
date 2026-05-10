import { test, expect } from '@playwright/test';

// The most important test: catches missing web-sys feature bindings (LinkError)
// and any other fatal JS errors that break page load.
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
  // Allow time for WASM to download, compile, and hydrate
  await page.waitForTimeout(10_000);

  if (fatal.length) console.log('Fatal errors:', fatal);
  expect(fatal, 'No WASM/fatal JS errors on page load').toHaveLength(0);
});

// Catches broken server functions (500s) that would show blank panels to users.
// DB-dependent and Prometheus-backed endpoints are excluded in CI since neither
// is available — those are covered by testing against production.
test('server functions return non-500 on initial load', async ({ page }) => {
  const failures: string[] = [];

  const ciSkip = [
    // Requires Postgres
    'get_network_insights', 'get_claude_audit_log', 'get_spike_config',
    'get_visitor_stats', 'save_spike_config',
    // Requires Prometheus
    'get_top_network_pods', 'get_node_metrics', 'get_cluster_metrics',
    'get_network_insights_chart', 'get_gitops_status',
  ];

  page.on('response', res => {
    const url = res.url();
    if (!url.includes('/api/')) return;
    if (res.status() < 500) return;
    if (ciSkip.some(e => url.includes(e))) return;
    failures.push(`${res.status()} ${url}`);
  });

  await page.goto('/');
  await page.waitForTimeout(20_000);

  if (failures.length) console.log('500 errors:', failures);
  expect(failures, 'No unexpected server function 500s').toHaveLength(0);
});

// Catches regressions in the conjunction screening pipeline.
test('conjunction table populates within 60s', async ({ page }) => {
  await page.goto('/');
  await page.locator('#satellites').scrollIntoViewIfNeeded();

  const tableBody = page.locator('#satellites tbody');
  await tableBody.waitFor({ state: 'visible', timeout: 60_000 });

  const rows = await tableBody.locator('tr').count();
  console.log(`Conjunction rows: ${rows}`);
  expect(rows).toBeGreaterThan(0);
});
