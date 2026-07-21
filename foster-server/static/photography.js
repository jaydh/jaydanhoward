// Photography gallery thumbnails + lightbox. `fx-for` only binds text
// content per item (`fx-field`), there's no declarative way to bind an
// <img>'s `src` per list item — so thumbnails read each item's own
// `data-fx-item` JSON (the same attribute the Satellites section reads
// for its per-satellite data) and set `src`/`alt` by hand.
//
// Which photo is open in the lightbox is plain client-side state, not a
// Foster machine: it's per-visitor (two people looking at the gallery at
// once shouldn't see the same photo pop open in their lightbox), and
// Foster machines are one shared instance across every connected client —
// same reasoning as theme/life/pathfinding. The real shared data (the
// photo list itself) stays in the "photography" Foster machine; only
// "which one is currently open" moved out here.

export function initPhotography() {
  const root = document.querySelector('[fx-machine="photography"]');
  const grid = document.querySelector('[fx-for="photos"]');
  const lightbox = document.getElementById('photo-lightbox');
  const lightboxImg = document.getElementById('photo-lightbox-img');
  if (!root || !grid || !lightbox || !lightboxImg) return;

  let photos = [];
  let viewingIndex = -1;

  function fillThumbnails() {
    const items = [];
    for (const img of grid.querySelectorAll('img[data-fx-item]')) {
      const item = JSON.parse(img.getAttribute('data-fx-item'));
      items.push(item);
      if (!img.dataset.filled) {
        img.src = item.thumb_url || item.medium_url;
        img.alt = item.name;
        img.dataset.filled = '1';
        img.addEventListener('click', () => open(items.indexOf(item)));
      }
    }
    photos = items;
  }

  function open(index) {
    if (index < 0 || index >= photos.length) return;
    viewingIndex = index;
    lightboxImg.src = photos[index].medium_url;
    lightboxImg.alt = photos[index].name;
    // .lightbox's CSS default is display:none (so it can never get stuck
    // visible before this script runs — see index.html's fx-if comment);
    // clearing the inline style would just fall back to that same
    // display:none, so this has to set the visible value explicitly.
    lightbox.style.display = 'flex';
  }

  function close() {
    viewingIndex = -1;
    lightbox.style.display = 'none';
  }

  function step(delta) {
    if (viewingIndex < 0 || photos.length === 0) return;
    open((viewingIndex + delta + photos.length) % photos.length);
  }

  document.getElementById('photo-prev').addEventListener('click', () => step(-1));
  document.getElementById('photo-next').addEventListener('click', () => step(1));
  document.getElementById('photo-close').addEventListener('click', close);

  fillThumbnails();
  // fx-for re-renders its children whenever the snapshot changes (not just
  // this specific list) — a MutationObserver on the grid catches that.
  const observer = new MutationObserver(fillThumbnails);
  observer.observe(grid, { childList: true, subtree: true });
}
