// Lighthouse "Load Report" gate — whether it's open is per-visitor UI
// state (just "did I click this button"), not shared data, so this is a
// plain client-side toggle rather than a Foster machine. Same reasoning
// as theme/life/pathfinding/photography's lightbox.
export function initLighthouse() {
  const gate = document.getElementById('lh-gate');
  const report = document.getElementById('lh-report');
  const btn = document.getElementById('lh-load-report');
  if (!gate || !report || !btn) return;

  btn.addEventListener('click', () => {
    gate.style.display = 'none';
    report.style.display = '';
  });
}
