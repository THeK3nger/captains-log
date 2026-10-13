// HTMX owns browser history. Derive selection from the content that actually
// arrived, so failed or superseded requests never highlight the wrong entry.
(function () {
  if (window.captainsLogInitialized) return;
  window.captainsLogInitialized = true;

  function syncSelection() {
    var current = document.querySelector('#entry-detail [data-entry-id]');
    var id = current && current.dataset.entryId;
    document.querySelectorAll('.entry-item').forEach(function (item) {
      var active = item.dataset.entryId === id;
      item.classList.toggle('active', active);
      if (active) item.setAttribute('aria-current', 'page');
      else item.removeAttribute('aria-current');
    });
  }

  document.addEventListener('htmx:afterSwap', function (event) {
    if (event.detail.target.id !== 'entry-detail') return;
    event.detail.target.scrollTop = 0;
    syncSelection();
  });
  document.addEventListener('htmx:historyRestore', syncSelection);
  syncSelection();
})();
