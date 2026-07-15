// Historical CPU/mem/disk/network sparkline — real 24h Prometheus range
// data (see src/cluster.rs::fetch_historical_metrics), read off fx-for's
// data-fx-item attribute since it's structured (multiple time series),
// not a single scalar fx-text/fx-bind-attr can carry — same pattern
// satellites.js/photography.js use for their own structured data.

export function initCluster() {
  const canvas = document.getElementById('cluster-sparkline');
  const container = document.querySelector('[fx-for="historical_series_list"]');
  if (!canvas || !container) return;
  const ctx2d = canvas.getContext('2d');

  const SERIES = [
    { key: 'cpu', color: '#3b82f6' },
    { key: 'memory', color: '#a78bfa' },
    { key: 'disk', color: '#f59e0b' },
  ];

  function draw() {
    const el = container.querySelector('[data-fx-item]');
    if (!el) return;
    const data = JSON.parse(el.getAttribute('data-fx-item'));

    ctx2d.clearRect(0, 0, canvas.width, canvas.height);
    ctx2d.fillStyle = getComputedStyle(document.body).getPropertyValue('--surface') || '#fff';
    ctx2d.fillRect(0, 0, canvas.width, canvas.height);

    for (const { key, color } of SERIES) {
      const series = data[key] || [];
      if (series.length < 2) continue;
      const max = Math.max(...series, 1);
      ctx2d.beginPath();
      ctx2d.strokeStyle = color;
      ctx2d.lineWidth = 1.5;
      series.forEach((v, i) => {
        const x = (i / (series.length - 1)) * canvas.width;
        const y = canvas.height - (v / max) * (canvas.height - 10) - 5;
        if (i === 0) ctx2d.moveTo(x, y);
        else ctx2d.lineTo(x, y);
      });
      ctx2d.stroke();
    }
  }

  draw();
  const observer = new MutationObserver(draw);
  observer.observe(container, { childList: true, subtree: true });
}
