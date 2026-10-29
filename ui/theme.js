/* Тема и набор иконок применяются до отрисовки, чтобы не было мигания. */
(function () {
  var root = document.documentElement;
  var mq = window.matchMedia('(prefers-color-scheme: light)');
  var mode = 'system';
  var PACKS = {
    classic: { sw: 2, markR: '3px', markRot: '45deg', foldR: '2px 3px 3px 3px', foldH: '9px' },
    bold: { sw: 2.8, markR: '2px', markRot: '45deg', foldR: '1px 4px 2px 2px', foldH: '11px' },
    thin: { sw: 1.5, markR: '50%', markRot: '0deg', foldR: '5px', foldH: '9px' },
  };
  function get(k, d) { try { return localStorage.getItem(k) || d; } catch (e) { return d; } }
  function put(k, v) { try { localStorage.setItem(k, v); } catch (e) { /* без хранилища тоже работает */ } }
  function paint() { root.dataset.theme = mode === 'system' ? (mq.matches ? 'light' : 'dark') : mode; }
  var PJTheme = {
    PACKS: PACKS,
    mode: function () { return mode; },
    current: function () { return root.dataset.theme; },
    set: function (m, animate) {
      if (m !== 'dark' && m !== 'light') m = 'system';
      if (animate) { root.classList.add('theming'); setTimeout(function () { root.classList.remove('theming'); }, 320); }
      mode = m; put('pj-theme', m); paint();
    },
    pack: function (name) {
      var k = PACKS[name] ? name : 'classic';
      var p = PACKS[k], st = root.style;
      st.setProperty('--ico-sw', p.sw); st.setProperty('--mark-r', p.markR);
      st.setProperty('--mark-rot', p.markRot); st.setProperty('--fold-r', p.foldR); st.setProperty('--fold-h', p.foldH);
      root.dataset.icons = k; put('pj-icons', k);
    },
  };
  window.PJTheme = PJTheme;
  mode = get('pj-theme', 'system');
  paint();
  PJTheme.pack(get('pj-icons', 'classic'));
  var onChange = function () { if (mode === 'system') paint(); };
  if (mq.addEventListener) mq.addEventListener('change', onChange);
})();
