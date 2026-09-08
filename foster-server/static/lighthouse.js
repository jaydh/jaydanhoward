// Lighthouse "Load Report" gate — whether it's open is per-visitor UI
// state (just "did I click this button"), not shared data, so this is a
// plain client-side toggle rather than a Foster machine. Same reasoning
// as theme/life/pathfinding/photography's lightbox.
export function initLighthouse() {
  const widget = document.getElementById('lighthouse-widget');
  const gate = document.getElementById('lh-gate');
  const report = document.getElementById('lh-report');
  const btn = document.getElementById('lh-load-report');
  if (!widget || !gate || !report || !btn) return;

  function loadReport() {
    gate.style.display = 'none';
    report.style.display = '';
  }

  btn.addEventListener('click', loadReport);

  // Auto-load once the widget scrolls into view, same pattern as the
  // other widgets' autoplay-on-scroll — saves the click for visitors who
  // scroll to it anyway.
  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        loadReport();
        observer.disconnect();
      }
    }
  }, { threshold: 0.1 });
  observer.observe(widget);
}
