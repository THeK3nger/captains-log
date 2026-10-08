// Keep the detail pane in sync with browser back/forward navigation.
window.addEventListener('popstate', function () {
  var m = location.pathname.match(/^\/entry\/(\d+)$/);
  var detail = document.getElementById('entry-detail');
  document.querySelectorAll('.entry-item').forEach(function (n) { n.classList.remove('active'); });
  if (m) {
    var el = document.querySelector('[hx-get="/entry/' + m[1] + '"]');
    if (el) el.classList.add('active');
    fetch('/entry/' + m[1], { headers: { 'HX-Request': 'true' } })
      .then(function (r) { return r.text(); })
      .then(function (html) { detail.innerHTML = html; });
  } else {
    detail.innerHTML = document.getElementById('placeholder-tpl').innerHTML;
  }
});
