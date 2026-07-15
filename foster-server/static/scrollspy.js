// Nav scroll-spy — plain IntersectionObserver, no Foster involved at all.
//
// Scroll position changes many times a second. Foster's model is "fire a
// named event, wait for a server round trip, patch the DOM from the
// response" — appropriate for a button click, wrong for something that
// fires on every scroll tick. There's no machine here, not even a nested
// one: this is the case where the right answer is "don't model it in
// Foster," full stop, and just toggle a class directly.
export function initScrollSpy() {
  const links = document.querySelectorAll('#scroll-spy-links a[data-spy]');
  if (!links.length) return;

  const linkFor = {};
  links.forEach((a) => { linkFor[a.dataset.spy] = a; });

  const sections = Array.from(document.querySelectorAll('main[id]'))
    .filter((el) => el.id in linkFor);
  if (!sections.length) return;

  const setActive = (id) => {
    links.forEach((a) => a.classList.toggle('active', a.dataset.spy === id));
  };

  const observer = new IntersectionObserver(
    (entries) => {
      const visible = entries
        .filter((e) => e.isIntersecting)
        .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
      if (visible) setActive(visible.target.id);
    },
    { rootMargin: '-30% 0px -60% 0px', threshold: [0, 0.25, 0.5, 0.75, 1] }
  );

  sections.forEach((s) => observer.observe(s));
  setActive(sections[0].id);
}
