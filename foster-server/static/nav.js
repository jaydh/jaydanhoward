// Contact dropdown — whether it's open is per-visitor UI state, not
// shared data, so this is a plain client-side toggle rather than a
// Foster machine (same reasoning as theme/life/pathfinding/photography's
// lightbox/lighthouse's gate).
export function initNav() {
  const wrap = document.getElementById('contact-wrap');
  const toggle = document.getElementById('contact-toggle');
  const menu = document.getElementById('contact-menu');
  if (!wrap || !toggle || !menu) return;

  function setOpen(open) {
    menu.style.display = open ? '' : 'none';
  }

  toggle.addEventListener('click', (e) => {
    e.stopPropagation();
    setOpen(menu.style.display === 'none');
  });

  document.addEventListener('click', (e) => {
    if (!wrap.contains(e.target)) setOpen(false);
  });
}
