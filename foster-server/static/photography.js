// Photography lightbox. Thumbnails are plain Foster now (fx-for +
// fx-bind-attr="src=item:thumb_url"); what's left here is which photo is
// open, which is per-visitor state with prev/next index arithmetic that a
// local Foster machine (pass/merge only) can't express yet.

export function initPhotography() {
  const grid = document.querySelector('[fx-for="photos"]');
  const lightbox = document.getElementById('photo-lightbox');
  const lightboxImg = document.getElementById('photo-lightbox-img');
  if (!grid || !lightbox || !lightboxImg) return;

  let viewingIndex = -1;

  // fx-for re-renders the tiles on every snapshot, so read the current list
  // from the DOM (each tile carries its item as data-fx-item) when needed.
  const photos = () =>
    [...grid.querySelectorAll('[data-fx-item]')].map((t) => JSON.parse(t.getAttribute('data-fx-item')));

  function open(index) {
    const list = photos();
    if (index < 0 || index >= list.length) return;
    viewingIndex = index;
    lightboxImg.src = list[index].medium_url;
    lightboxImg.alt = list[index].name;
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
    const n = photos().length;
    if (viewingIndex < 0 || n === 0) return;
    open((viewingIndex + delta + n) % n);
  }

  // Delegated, so it survives fx-for replacing the tiles.
  grid.addEventListener('click', (e) => {
    const tile = e.target.closest('[data-fx-item]');
    if (tile) open([...grid.querySelectorAll('[data-fx-item]')].indexOf(tile));
  });
  document.getElementById('photo-prev').addEventListener('click', () => step(-1));
  document.getElementById('photo-next').addEventListener('click', () => step(1));
  document.getElementById('photo-close').addEventListener('click', close);
}
