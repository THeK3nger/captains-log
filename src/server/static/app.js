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

  // Enter in the journal field with a half-typed name completes it with the
  // first suggestion instead of submitting (browsers only do that when the
  // suggestion was highlighted). An exact match, or a name matching nothing
  // (a new journal), submits as usual.
  document.addEventListener('keydown', function (event) {
    var input = event.target;
    if (event.key !== 'Enter' || event.isComposing || !input.matches || !input.matches('#entry-journal')) return;
    var typed = input.value.trim().toLowerCase();
    if (!typed) return;
    var names = Array.from(input.list ? input.list.options : []).map(function (o) { return o.value; });
    if (names.some(function (n) { return n.toLowerCase() === typed; })) return;
    var match = names.find(function (n) { return n.toLowerCase().startsWith(typed); }) ||
      names.find(function (n) { return n.toLowerCase().includes(typed); });
    if (!match) return;
    event.preventDefault();
    input.value = match;
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });

  // ---- Markdown editor -------------------------------------------------
  // Every edit goes through execCommand('insertText') so the browser's undo
  // stack (Cmd+Z) keeps working and `input` fires for the draft autosave.

  function replaceRange(ta, start, end, text, selStart, selEnd) {
    ta.focus();
    ta.setSelectionRange(start, end);
    var done = false;
    try { done = document.execCommand('insertText', false, text); } catch (e) { /* unsupported */ }
    if (!done || ta.value.slice(start, start + text.length) !== text) {
      ta.setRangeText(text, start, end, 'end');
      ta.dispatchEvent(new Event('input', { bubbles: true }));
    }
    ta.setSelectionRange(selStart, selEnd);
  }

  // Wraps the selection in `marker` (or removes it when already wrapped).
  // Surrounding spaces stay outside: "**word **" would not render as bold.
  function toggleWrap(ta, marker) {
    var v = ta.value, s = ta.selectionStart, e = ta.selectionEnd, m = marker.length;
    while (s < e && /\s/.test(v[s])) s++;
    while (e > s && /\s/.test(v[e - 1])) e--;
    var sel = v.slice(s, e);
    if (sel.length >= 2 * m && sel.startsWith(marker) && sel.endsWith(marker)) {
      var inner = sel.slice(m, sel.length - m);
      return replaceRange(ta, s, e, inner, s, s + inner.length);
    }
    if (v.slice(s - m, s) === marker && v.slice(e, e + m) === marker) {
      return replaceRange(ta, s - m, e + m, sel, s - m, e - m);
    }
    replaceRange(ta, s, e, marker + sel + marker, s + m, e + m);
  }

  function insertCode(ta) {
    var v = ta.value, s = ta.selectionStart, e = ta.selectionEnd;
    var sel = v.slice(s, e);
    if (sel.indexOf('\n') < 0) return toggleWrap(ta, '`');
    var before = s > 0 && v[s - 1] !== '\n' ? '\n' : '';
    var after = v[e] !== undefined && v[e] !== '\n' ? '\n' : '';
    var text = before + '```\n' + sel.replace(/\n$/, '') + '\n```' + after;
    var open = s + before.length + 3; // caret after ``` to type a language
    replaceRange(ta, s, e, text, open, open);
  }

  function insertLink(ta) {
    var v = ta.value, s = ta.selectionStart, e = ta.selectionEnd;
    var sel = v.slice(s, e).trim();
    var text;
    if (/^(https?:\/\/|mailto:)\S+$/i.test(sel)) {
      text = '[link text](' + sel + ')';
      return replaceRange(ta, s, e, text, s + 1, s + 10);
    }
    var label = sel || 'link text';
    text = '[' + label + '](url)';
    var url = s + label.length + 3;
    replaceRange(ta, s, e, text, sel ? url : s + 1, sel ? url + 3 : s + 1 + label.length);
  }

  var LINE_PREFIX = {
    bullets: { re: /^(\s*)[-*+] (\[[ xX]\] )?/, make: function () { return '- '; } },
    numbers: { re: /^(\s*)\d+[.)] /, make: function (i) { return (i + 1) + '. '; } },
    quote: { re: /^(\s*)> ?/, make: function () { return '> '; } },
    heading: { re: /^(\s*)#{1,6} /, make: function () { return '## '; } }
  };

  // Adds a prefix to every selected line, or removes it when all have one.
  function toggleLines(ta, kind) {
    var rule = LINE_PREFIX[kind];
    var v = ta.value;
    var start = v.lastIndexOf('\n', ta.selectionStart - 1) + 1;
    var endAt = ta.selectionEnd > ta.selectionStart && v[ta.selectionEnd - 1] === '\n'
      ? ta.selectionEnd - 1 : ta.selectionEnd;
    var end = v.indexOf('\n', endAt);
    if (end < 0) end = v.length;
    var lines = v.slice(start, end).split('\n');
    var filled = lines.filter(function (l) { return l.trim(); });
    var remove = filled.length > 0 && filled.every(function (l) { return rule.re.test(l); });
    var n = 0;
    var out = lines.map(function (line) {
      if (remove) return line.replace(rule.re, '$1');
      if (!line.trim() && lines.length > 1) return line;
      var bare = line;
      Object.keys(LINE_PREFIX).forEach(function (k) { bare = bare.replace(LINE_PREFIX[k].re, '$1'); });
      var indent = bare.match(/^\s*/)[0];
      return indent + rule.make(n++) + bare.slice(indent.length);
    }).join('\n');
    var caret = lines.length === 1 ? start + out.length : null;
    replaceRange(ta, start, end, out, caret === null ? start : caret, caret === null ? start + out.length : caret);
  }

  // Enter inside a list or quote starts the next item; Enter on an empty
  // item ends the list instead.
  function continueList(ta) {
    var v = ta.value, s = ta.selectionStart;
    if (s !== ta.selectionEnd) return false;
    var lineStart = v.lastIndexOf('\n', s - 1) + 1;
    var line = v.slice(lineStart, s);
    var m = line.match(/^(\s*)(?:([-*+]) (\[[ xX]\] )?|(\d+)([.)]) |(>) ?)/);
    if (!m) return false;
    if (!line.slice(m[0].length).trim() && v.slice(s, v.indexOf('\n', s) < 0 ? v.length : v.indexOf('\n', s)).trim() === '') {
      replaceRange(ta, lineStart, s, m[1], lineStart + m[1].length, lineStart + m[1].length);
      return true;
    }
    var next = m[2] ? m[2] + ' ' + (m[3] ? '[ ] ' : '')
      : m[4] ? (parseInt(m[4], 10) + 1) + m[5] + ' '
      : '> ';
    var text = '\n' + m[1] + next;
    replaceRange(ta, s, s, text, s + text.length, s + text.length);
    return true;
  }

  function editorParts(from) {
    var field = from.closest('.md-editor');
    return field && {
      field: field,
      textarea: field.querySelector('textarea'),
      preview: field.querySelector('.md-preview'),
      toggle: field.querySelector('.md-preview-toggle')
    };
  }

  function setPreview(parts, on) {
    parts.toggle.setAttribute('aria-pressed', on ? 'true' : 'false');
    parts.toggle.textContent = on ? 'WRITE' : 'PREVIEW';
    parts.field.classList.toggle('previewing', on);
    parts.field.querySelectorAll('.md-btn:not(.md-preview-toggle)').forEach(function (b) { b.disabled = on; });
    parts.textarea.hidden = on;
    parts.preview.hidden = !on;
    if (!on) { parts.textarea.focus(); return; }
    parts.preview.innerHTML = '<p class="md-preview-empty">Rendering…</p>';
    var request = (parts.field.previewRequest || 0) + 1;
    parts.field.previewRequest = request;
    fetch('/preview', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ content: parts.textarea.value })
    }).then(function (r) {
      if (!r.ok) throw new Error(r.status);
      return r.text();
    }).then(function (html) {
      if (parts.field.previewRequest === request) parts.preview.innerHTML = html;
    }).catch(function () {
      if (parts.field.previewRequest === request) {
        parts.preview.innerHTML = '<p class="md-preview-empty">Preview unavailable: the server did not answer.</p>';
      }
    });
  }

  function applyFormat(ta, action) {
    switch (action) {
      case 'bold': return toggleWrap(ta, '**');
      case 'italic': return toggleWrap(ta, '_');
      case 'strike': return toggleWrap(ta, '~~');
      case 'code': return insertCode(ta);
      case 'link': return insertLink(ta);
      default: if (LINE_PREFIX[action]) toggleLines(ta, action);
    }
  }

  document.addEventListener('click', function (event) {
    var button = event.target.closest('.md-btn');
    if (!button) return;
    var parts = editorParts(button);
    if (!parts) return;
    if (button.dataset.md === 'preview') setPreview(parts, parts.textarea.hidden !== true);
    else applyFormat(parts.textarea, button.dataset.md);
  });

  // Keep the textarea focused when a toolbar button is clicked, so the
  // selection it acts on is not lost.
  document.addEventListener('mousedown', function (event) {
    var button = event.target.closest('.md-btn:not(.md-preview-toggle)');
    if (button) event.preventDefault();
  });

  var SHIFT_SHORTCUTS = { Digit7: 'numbers', Digit8: 'bullets', Period: 'quote', KeyX: 'strike', KeyH: 'heading' };
  var SHORTCUTS = { b: 'bold', i: 'italic', k: 'link', e: 'code' };

  document.addEventListener('keydown', function (event) {
    if (event.isComposing) return;
    var parts = editorParts(event.target);
    if (!parts) return;
    var mod = event.metaKey || event.ctrlKey;
    if (mod && !event.altKey && event.key === 'Enter') {
      event.preventDefault();
      var form = event.target.closest('form');
      if (form && !form.querySelector('.btn-save').disabled) form.requestSubmit();
      return;
    }
    if (mod && event.shiftKey && !event.altKey && event.code === 'KeyP') {
      event.preventDefault();
      setPreview(parts, parts.textarea.hidden !== true);
      return;
    }
    if (event.target !== parts.textarea) return;
    if (!mod && !event.altKey && !event.shiftKey && event.key === 'Enter') {
      if (continueList(parts.textarea)) event.preventDefault();
      return;
    }
    if (!mod || event.altKey) return;
    var action = event.shiftKey ? SHIFT_SHORTCUTS[event.code] : SHORTCUTS[event.key.toLowerCase()];
    if (!action) return;
    event.preventDefault();
    applyFormat(parts.textarea, action);
  });

  // ⌘/Ctrl+Enter also saves from the other form fields.
  document.addEventListener('keydown', function (event) {
    if (event.isComposing || !(event.metaKey || event.ctrlKey) || event.key !== 'Enter') return;
    if (editorParts(event.target)) return;
    var form = event.target.closest && event.target.closest('#entry-detail .entry-form form');
    if (!form) return;
    event.preventDefault();
    if (!form.querySelector('.btn-save').disabled) form.requestSubmit();
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
