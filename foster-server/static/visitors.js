// Visitor world-map dots — same data-fx-item pattern as satellites.js and
// photography.js (fx-for only binds text content per item, not arbitrary
// per-item positioning), overlaid on the real world-map SVG served at
// GET /world-map.svg (ported from the real site's routes/world_map.rs —
// real Natural Earth land geometry, fetched once at startup).

export function initVisitors() {
  const container = document.getElementById('visitor-map-points');
  const pointsSrc = document.querySelector('[fx-for="points"]');
  if (!container || !pointsSrc) return;

  function render() {
    container.innerHTML = '';
    for (const el of pointsSrc.querySelectorAll('[data-fx-item]')) {
      const { lat, lon } = JSON.parse(el.getAttribute('data-fx-item'));
      const x = ((lon + 180) / 360) * 100;
      const y = ((90 - lat) / 180) * 100;
      const dot = document.createElement('div');
      dot.className = 'visitor-dot';
      dot.style.left = `${x}%`;
      dot.style.top = `${y}%`;
      container.appendChild(dot);
    }
  }

  render();
  const observer = new MutationObserver(render);
  observer.observe(pointsSrc, { childList: true, subtree: true });
}
