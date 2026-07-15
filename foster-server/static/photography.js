// Photography gallery thumbnails — `fx-for` only binds text content per
// item (`fx-field`), there's no declarative way to bind an <img>'s `src`
// per list item. So this reads each item's own `data-fx-item` JSON (the
// same attribute the Satellites section reads for its per-satellite data)
// and sets `src`/`alt` by hand. The click-to-view behavior itself needs no
// help — `fx-on="click->view"` on the template element is cloned per item
// by Foster already, and foster-client automatically merges that item's
// own JSON (including its "index" field) into the transition payload.

export function initPhotography() {
  const root = document.querySelector('[fx-machine="photography"]');
  const grid = document.querySelector('[fx-for="photos"]');
  if (!root || !grid) return;

  function fillThumbnails() {
    for (const img of grid.querySelectorAll('img[data-fx-item]')) {
      if (img.dataset.filled) continue;
      const item = JSON.parse(img.getAttribute('data-fx-item'));
      img.src = item.thumb_url || item.medium_url;
      img.alt = item.name;
      img.dataset.filled = '1';
    }
  }

  fillThumbnails();
  // fx-for re-renders its children whenever the snapshot changes (not just
  // this specific list) — a MutationObserver on the grid catches that.
  const observer = new MutationObserver(fillThumbnails);
  observer.observe(grid, { childList: true, subtree: true });
}
