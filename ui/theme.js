/* Тема применяется до отрисовки, чтобы не было мигания. */
(function () {
  var root = document.documentElement;
  var mq = window.matchMedia('(prefers-color-scheme: light)');
  var mode = 'system';
  function get(k, d) { try { return localStorage.getItem(k) || d; } catch (e) { return d; } }
  function put(k, v) { try { localStorage.setItem(k, v); } catch (e) { /* без хранилища тоже работает */ } }
  function paint() { root.dataset.theme = mode === 'system' ? (mq.matches ? 'light' : 'dark') : mode; }
  window.PJTheme = {
    current: function () { return root.dataset.theme; },
    set: function (m, animate) {
      if (m !== 'dark' && m !== 'light') m = 'system';
      if (animate) { root.classList.add('theming'); setTimeout(function () { root.classList.remove('theming'); }, 320); }
      mode = m; put('pj-theme', m); paint();
    },
  };
  mode = get('pj-theme', 'system');
  paint();
  if (mq.addEventListener) mq.addEventListener('change', function () { if (mode === 'system') paint(); });
})();
