// Dark-mode toggle — plain client-side state (localStorage + system
// preference), not a Foster machine. Foster's state is server-owned and
// shared by every connected client; theme is a per-visitor preference, so
// modeling it as a Foster machine would make one visitor's toggle flip
// dark mode for everyone. The FOUC-prevention script in index.html's
// <head> already set the initial `dark` class on <html> before this runs;
// this just wires up the toggle button, ported verbatim from the real
// src/components/nav.rs::ThemeToggle.
export function initTheme() {
  const html = document.documentElement;
  const btn = document.getElementById('theme-toggle');
  const moonIcon = document.getElementById('theme-icon-moon');
  const sunIcon = document.getElementById('theme-icon-sun');
  if (!btn || !moonIcon || !sunIcon) return;

  function syncIcons() {
    const isDark = html.classList.contains('dark');
    moonIcon.style.display = isDark ? 'none' : '';
    sunIcon.style.display = isDark ? '' : 'none';
  }

  btn.addEventListener('click', () => {
    const newDark = !html.classList.contains('dark');
    html.classList.toggle('dark', newDark);
    localStorage.setItem('theme', newDark ? 'dark' : 'light');
    syncIcons();
  });

  syncIcons();
}
