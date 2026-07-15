// Real conjunction screening widget — polls a hand-rolled axum route
// (GET /api/conjunction) independently of Foster's SSE for the
// "conjunction" machine, which only tracks the button's idle/started
// label. The actual screening pass (Hoots filter + SGP4 + TCA, real TLE
// data) is a background job; see src/conjunction.rs.

export function initConjunction() {
  const button = document.getElementById('conjunction-start');
  const statusEl = document.getElementById('conjunction-status');
  const statsEl = document.getElementById('conjunction-stats');
  const eventsEl = document.getElementById('conjunction-events');
  if (!button || !statusEl || !statsEl || !eventsEl) return;

  let polling = null;

  async function poll() {
    const res = await fetch('/api/conjunction');
    const data = await res.json();
    statusEl.textContent = data.status;

    if (data.status === 'complete') {
      statsEl.textContent = `${data.events_found} events found across ${data.pairs_after_hoots} pairs (of ${data.total_pairs} total) in ${data.elapsed_ms}ms`;
      eventsEl.innerHTML = data.events
        .map((e) => `<li>${e.sat_a} vs ${e.sat_b} — ${e.miss_distance_km.toFixed(1)} km</li>`)
        .join('');
      if (polling) { clearInterval(polling); polling = null; }
    } else if (data.status === 'failed') {
      statsEl.textContent = data.error || 'failed';
      if (polling) { clearInterval(polling); polling = null; }
    }
  }

  button.addEventListener('click', async () => {
    statsEl.textContent = '';
    eventsEl.innerHTML = '';
    statusEl.textContent = 'running';
    await fetch('/api/conjunction/start', { method: 'POST' });
    if (polling) clearInterval(polling);
    polling = setInterval(poll, 3000);
  });

  poll();
}
