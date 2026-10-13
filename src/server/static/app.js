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
      if (active) {
        item.setAttribute('aria-current', 'page');
        if (item.getClientRects().length) item.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      }
      else item.removeAttribute('aria-current');
    });
  }

  document.addEventListener('htmx:afterSwap', function (event) {
    if (event.detail.target.id !== 'entry-detail') return;
    event.detail.target.scrollTop = 0;
    syncSelection();
    var titleInput = event.detail.target.querySelector('.entry-form input[name="title"]');
    var focusTarget = titleInput || event.detail.target.querySelector('.ed-title');
    if (focusTarget) focusTarget.focus({ preventScroll: true });
  });
  document.addEventListener('htmx:historyRestore', syncSelection);

  document.addEventListener('keydown', function (event) {
    if (event.isComposing || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    var target = event.target;
    var editing = target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])');
    var cancel = document.querySelector('#entry-detail .btn-cancel');
    if (event.key === 'Escape' && cancel) {
      event.preventDefault();
      cancel.click();
      return;
    }
    if (editing) return;
    // Keep an open form intact until it is saved or cancelled.
    if (cancel) return;
    if (event.key === 'n') {
      event.preventDefault();
      document.querySelector('[data-action="new-entry"]').click();
      return;
    }
    if (event.key !== 'j' && event.key !== 'k') return;
    var items = Array.from(document.querySelectorAll('.entry-item'));
    if (!items.length) return;
    var index = items.findIndex(function (item) { return item.classList.contains('active'); });
    var next = index < 0 ? (event.key === 'j' ? 0 : items.length - 1)
      : Math.max(0, Math.min(items.length - 1, index + (event.key === 'j' ? 1 : -1)));
    if (next === index) return;
    event.preventDefault();
    items[next].click();
  });
  syncSelection();
})();
