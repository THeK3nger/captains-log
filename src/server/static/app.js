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

  // A form re-rendered after a failed save carries the user's text as its
  // defaults, so it counts as unsaved whenever anything was typed.
  function hasUnsavedInput(form) {
    if (!form || form.dataset.saved) return false;
    var fields = Array.from(form.querySelectorAll('input:not([type="hidden"]), textarea'));
    if (form.querySelector('.form-error')) {
      return fields.some(function (field) { return field.name !== 'journal' && field.name !== 'timestamp' && field.value.trim(); });
    }
    return fields.some(function (field) { return field.value !== field.defaultValue; });
  }

  var DISCARD_PROMPT = 'Discard this entry? Your unsaved changes will be lost.';

  // Drafts live in localStorage so back/forward, reloads and closed tabs
  // never lose text: one per form, keyed by the entry being edited.
  // Storage can be missing or blocked; drafts are a safety net, not a must.
  function draftKey(entryForm) {
    return 'captainslog:draft:' + (entryForm.dataset.entryId ? 'edit:' + entryForm.dataset.entryId : 'new');
  }

  function openEntryForm() {
    return document.querySelector('#entry-detail .entry-form');
  }

  function saveDraft(entryForm) {
    try {
      if (!hasUnsavedInput(entryForm)) {
        localStorage.removeItem(draftKey(entryForm));
        return;
      }
      var fields = {};
      entryForm.querySelectorAll('input:not([type="hidden"]), textarea').forEach(function (field) {
        fields[field.name] = field.value;
      });
      localStorage.setItem(draftKey(entryForm), JSON.stringify({ savedAt: Date.now(), fields: fields }));
    } catch (e) { /* no storage: nothing to keep */ }
  }

  function clearDraft(entryForm) {
    if (!entryForm) return;
    try { localStorage.removeItem(draftKey(entryForm)); } catch (e) { /* ignore */ }
  }

  function restoreDraft() {
    var entryForm = openEntryForm();
    if (!entryForm || entryForm.dataset.draftChecked) return;
    entryForm.dataset.draftChecked = 'true';
    var draft;
    try { draft = JSON.parse(localStorage.getItem(draftKey(entryForm))); } catch (e) { return; }
    if (!draft || !draft.fields) return;
    entryForm.querySelectorAll('input:not([type="hidden"]), textarea').forEach(function (field) {
      if (Object.prototype.hasOwnProperty.call(draft.fields, field.name)) field.value = draft.fields[field.name];
    });
    var notice = document.createElement('div');
    notice.className = 'draft-notice';
    notice.setAttribute('role', 'status');
    notice.textContent = 'Unsaved draft restored (' + new Date(draft.savedAt).toLocaleString() + ').';
    entryForm.querySelector('form').before(notice);
  }

  document.addEventListener('input', function (event) {
    var entryForm = event.target.closest('#entry-detail .entry-form');
    if (entryForm) saveDraft(entryForm);
  });

  // CANCEL (and Esc) is an explicit discard.
  document.addEventListener('click', function (event) {
    if (event.target.closest('#entry-detail .btn-cancel')) clearDraft(openEntryForm());
  }, true);

  // The server announces a successful save just before redirecting to the
  // entry; a failed one re-renders the form instead and keeps the draft.
  document.addEventListener('entry-saved', function (event) {
    var entryForm = event.target.closest('#entry-detail .entry-form');
    if (!entryForm) return;
    clearDraft(entryForm);
    entryForm.dataset.saved = 'true';
  });

  function openFormIsDirty() {
    return hasUnsavedInput(document.querySelector('#entry-detail .entry-form'));
  }

  // Opening another entry or a new form would replace the one being written.
  // The form's own SAVE and CANCEL are left alone: those are explicit.
  document.addEventListener('htmx:confirm', function (event) {
    var target = event.detail.target;
    if (!target || target.id !== 'entry-detail') return;
    if (event.detail.elt.closest('#entry-detail .entry-form form, #entry-detail .btn-cancel')) return;
    if (!openFormIsDirty()) return;
    event.preventDefault();
    if (!window.confirm(DISCARD_PROMPT)) return;
    clearDraft(openEntryForm());
    event.detail.issueRequest(true);
  });

  // Journal, tag and search links reload the whole page.
  window.addEventListener('beforeunload', function (event) {
    if (!openFormIsDirty()) return;
    event.preventDefault();
    event.returnValue = '';
  });

  document.addEventListener('htmx:afterSwap', function (event) {
    if (event.detail.target.id !== 'entry-detail') return;
    event.detail.target.scrollTop = 0;
    syncSelection();
    restoreDraft();
    var titleInput = event.detail.target.querySelector('.entry-form input[name="title"]');
    var focusTarget = titleInput || event.detail.target.querySelector('.ed-title');
    if (focusTarget) focusTarget.focus({ preventScroll: true });
  });
  document.addEventListener('htmx:historyRestore', function () {
    syncSelection();
    restoreDraft();
  });

  document.addEventListener('keydown', function (event) {
    if (event.isComposing || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    var target = event.target;
    var editing = target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])');
    var cancel = document.querySelector('#entry-detail .btn-cancel');
    if (event.key === 'Escape' && cancel) {
      event.preventDefault();
      if (hasUnsavedInput(cancel.closest('.entry-form')) &&
          !window.confirm(DISCARD_PROMPT)) return;
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
  restoreDraft();
})();
