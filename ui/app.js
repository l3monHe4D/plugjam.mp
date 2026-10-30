'use strict';
/* plugjam.mp — интерфейс. Без фреймворков.
   Экономия ресурсов: список виртуализирован (в DOM только видимые строки), таймеры спят,
   пока окно скрыто, опрос деталей — раз в 5 с и только для выбранного сервера.
   Все строки от серверов вставляются через textContent (никакого innerHTML). */

const { invoke, Channel } = window.__TAURI__.core;
const TAURI = window.__TAURI__;
// Заставка играет сама (CSS); убираем её из DOM, когда анимация закончилась.
setTimeout(() => document.getElementById('splash')?.remove(), 1800);
const $ = (s) => document.querySelector(s);
const el = (tag, cls, text) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
};

const ROW_H = 30;
const coll = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });
const svg = (d) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
const ICONS = {
  play: svg('<path d="M7 5l12 7-12 7z" fill="currentColor"/>'),
  star: svg('<path d="M12 3.5l2.6 5.4 5.9.8-4.3 4.1 1 5.9L12 16.9l-5.2 2.8 1-5.9-4.3-4.1 5.9-.8z"/>'),
  plus: svg('<path d="M12 5v14M5 12h14"/>'),
  refresh: svg('<path d="M20 11a8 8 0 10-2.3 5.7M20 4.5V11h-6.5"/>'),
  sliders: svg('<path d="M4 7h9M17 7h3M4 17h3M11 17h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/>'),
  crown: svg('<path d="M4 8l4 4 4-6 4 6 4-4-2 10H6z"/>'),
  sun: svg('<circle cx="12" cy="12" r="4"/><path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6L7 7M17 17l1.4 1.4M18.4 5.6L17 7M7 17l-1.4 1.4"/>'),
  moon: svg('<path d="M20 14.5A8 8 0 019.5 4a8 8 0 1010.5 10.5z"/>'),
  chevron: svg('<path d="M6 9l6 6 6-6"/>'),
  lock: svg('<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 018 0v3"/>'),
};

// ───────────────────────── состояние ─────────────────────────
const S = {
  store: { nickname: '', gtaPath: '', sort: 'name', folders: [], favorites: [], theme: 'system', appIconEffective: 'classic',
           premium: false, premiumUntil: null, maxFolders: 3, nicknames: [], serverNicks: {}, userId: '' },
  tab: 'favorites',
  folder: null,            // null = «Все серверы»
  live: new Map(),         // address -> свежие данные из UDP-опроса
  internet: [],            // список из индекса
  inetMap: new Map(),
  pinged: new Set(),       // адреса из Internet, у которых уже узнали пинг
  internetAt: 0,
  favAt: 0,
  inetError: '',
  loadingInet: false,
  selected: null,
  rows: [],
  idx: new Map(),          // address -> позиция в rows
  rowsVer: 0,
  pingHist: [],
  f: { search: '', mode: '', lang: '', notFull: false, notEmpty: false, noPass: false },
};

const blank = (address, hostname = '') => ({
  address, hostname, players: null, maxPlayers: null, ping: null,
  mode: '', language: '', password: false, online: null, version: '',
});

function lookup(a) {
  const base = S.live.get(a) || S.inetMap.get(a);
  const fav = S.store.favorites.find((x) => x.address === a);
  if (base) return { ...base, hostname: base.hostname || fav?.name || a };
  return blank(a, fav?.name || a);
}
const isFav = (a) => S.store.favorites.some((f) => f.address === a);

function normAddr(s) {
  s = s.trim().replace(/^samp:\/\//i, '').replace(/\/+$/, '').toLowerCase();
  return /:\d+$/.test(s) ? s : s + ':7777';
}

// ───────────────────────── уведомления и диалоги ─────────────────────────
let toastT = 0;
function toast(msg, bad = false) {
  const t = $('#toast');
  t.textContent = msg;
  t.classList.toggle('bad', bad);
  t.classList.add('show');
  clearTimeout(toastT);
  toastT = setTimeout(() => t.classList.remove('show'), bad ? 4200 : 2200);
}

function ask({ title, text, fields = [], ok = 'OK' }) {
  return new Promise((resolve) => {
    const d = $('#dlg');
    d.replaceChildren();
    d.classList.remove('wide');
    const form = el('form');
    form.append(el('h2', '', title));
    if (text) form.append(el('p', '', text));
    const inputs = {};
    for (const f of fields) {
      const l = el('label');
      l.append(el('span', '', f.label));
      const i = el('input');
      i.type = f.type || 'text';
      i.value = f.value || '';
      i.placeholder = f.placeholder || '';
      i.spellcheck = false;
      i.autocomplete = 'off';
      l.append(i);
      form.append(l);
      inputs[f.id] = i;
    }
    const row = el('div', 'actions');
    const cancel = el('button', 'btn', 'Отмена');
    cancel.type = 'button';
    const okb = el('button', 'btn primary', ok);
    okb.type = 'submit';
    row.append(cancel, okb);
    form.append(row);
    d.append(form);
    let done = false;
    const finish = (v) => {
      if (done) return;
      done = true;
      d.close();
      resolve(v);
    };
    cancel.onclick = () => finish(null);
    d.oncancel = (e) => { e.preventDefault(); finish(null); };
    form.onsubmit = (e) => {
      e.preventDefault();
      const v = {};
      for (const k in inputs) v[k] = inputs[k].value.trim();
      finish(v);
    };
    d.showModal();
    const first = Object.values(inputs)[0];
    if (first) { first.focus(); first.select(); }
  });
}

// Диалог с произвольным содержимым: build(close, redraw) возвращает массив узлов; redraw() перерисовывает.
function modal(title, build, { wide = false, sub = '' } = {}) {
  return new Promise((resolve) => {
    const d = $('#dlg');
    d.replaceChildren();
    d.classList.toggle('wide', wide);
    const box = el('div');
    const h = el('h2', '', title);
    if (sub) h.append(el('small', '', sub));
    let done = false;
    const close = (v) => { if (done) return; done = true; d.close(); resolve(v); };
    const render = () => box.replaceChildren(h, ...build(close, render));
    render();
    d.append(box);
    d.oncancel = (e) => { e.preventDefault(); close(null); };
    d.showModal();
    document.activeElement?.blur?.(); // без «залипшей» рамки фокуса на первом элементе
  });
}
const btn = (label, fn, cls = '') => { const b = el('button', 'btn ' + cls, label); b.type = 'button'; b.onclick = fn; return b; };
const section = (title, ...nodes) => { const s = el('div', 'sec'); if (title) s.append(el('h4', '', title)); s.append(...nodes); return s; };
const fmtDate = (sec) => new Date(sec * 1000).toLocaleDateString('ru-RU');
const copyText = (t, msg) => navigator.clipboard?.writeText(t).then(() => toast(msg || 'Скопировано')).catch(() => toast('Не удалось скопировать', true));

// ───────────────────────── контекстное меню ─────────────────────────
function hideMenu() { $('#menu').hidden = true; }
function showMenu(x, y, items) {
  const m = $('#menu');
  m.replaceChildren();
  for (const it of items) {
    if (it === '-') { m.append(el('hr')); continue; }
    const b = el('button', 'mi' + (it.danger ? ' danger' : ''), it.label);
    b.onclick = () => { hideMenu(); it.fn(); };
    m.append(b);
  }
  m.hidden = false;
  m.style.left = Math.max(4, Math.min(x, innerWidth - m.offsetWidth - 6)) + 'px';
  m.style.top = Math.max(4, Math.min(y, innerHeight - m.offsetHeight - 6)) + 'px';
  m.classList.remove('in');
  void m.offsetWidth;
  m.classList.add('in');
}
document.addEventListener('pointerdown', (e) => { if (!e.target.closest('#menu')) hideMenu(); }, true);
window.addEventListener('blur', hideMenu);

// ───────────────────────── строки списка ─────────────────────────
function passes(r) {
  const f = S.f;
  if (f.search && !(r.hostname.toLowerCase().includes(f.search) || r.address.includes(f.search))) return false;
  if (f.mode && !(r.mode || '').toLowerCase().includes(f.mode)) return false;
  if (f.lang && !(r.language || '').toLowerCase().includes(f.lang)) return false;
  if (f.notFull && r.players != null && r.maxPlayers > 0 && r.players >= r.maxPlayers) return false;
  if (f.notEmpty && !(r.players > 0)) return false;
  if (f.noPass && r.password) return false;
  return true;
}

const nameKey = (r) => (r.hostname || r.address).replace(/^[^\p{L}\p{N}]+/u, '').toLowerCase();

function sortRows(rows) {
  const byName = S.store.sort !== 'online';
  const k = rows.map((r) => ({ r, n: nameKey(r), p: r.online === false ? -1 : (r.players ?? -1) }));
  k.sort(byName ? (a, b) => coll.compare(a.n, b.n) : (a, b) => b.p - a.p || coll.compare(a.n, b.n));
  return k.map((x) => x.r);
}

function currentRows() {
  let list;
  if (S.tab === 'favorites') {
    const favs = S.folder == null ? S.store.favorites : S.store.favorites.filter((f) => f.folder === S.folder);
    list = favs.map((f) => lookup(f.address));
  } else {
    list = S.internet.map((s) => S.live.get(s.address) || s);
  }
  return sortRows(list.filter(passes));
}

function pingClass(p) { return p == null ? '' : p < 60 ? 'ping-good' : p < 120 ? 'ping-mid' : 'ping-bad'; }

function rowEl(r, i) {
  const d = el('div', 'row' + (i % 2 ? ' alt' : '') + (r.online === false ? ' off' : '') + (r.address === S.selected ? ' sel' : ''));
  d.dataset.addr = r.address;
  d.draggable = S.tab === 'favorites';
  const dot = el('span', 'dot' + (r.online === false ? ' off' : r.password ? ' pass' : r.online == null ? ' unk' : ''));
  dot.title = r.online === false ? 'Не отвечает' : r.password ? 'Нужен пароль' : 'Открытый сервер';
  const name = el('span', '', r.hostname || r.address);
  name.title = r.hostname || r.address;
  const pl = el('span', 'pl', r.players == null || r.online === false ? '—' : `${r.players} / ${r.maxPlayers}`);
  if (r.players != null && r.maxPlayers > 0) pl.style.setProperty('--fill', Math.min(100, Math.round((r.players / r.maxPlayers) * 100)));
  const ping = el('span', pingClass(r.ping), r.ping == null ? '—' : String(r.ping));
  const mode = el('span', 'dim', r.mode || '');
  mode.title = r.mode || '';
  d.append(dot, name, pl, ping, mode, el('span', 'dim', r.language || ''));
  return d;
}

let drawnKey = '';
function drawWindow(force) {
  const box = $('#vlist');
  const n = S.rows.length;
  const start = Math.max(0, Math.floor(box.scrollTop / ROW_H) - 6);
  const end = Math.min(n, Math.ceil((box.scrollTop + box.clientHeight) / ROW_H) + 6);
  const key = `${start}:${end}:${S.rowsVer}`;
  if (!force && key === drawnKey) return;
  drawnKey = key;
  const frag = document.createDocumentFragment();
  for (let i = start; i < end; i++) frag.append(rowEl(S.rows[i], i));
  const win = $('#vwin');
  win.style.top = start * ROW_H + 'px';
  win.replaceChildren(frag);
}

function playEnter() {
  const w = $('#vwin');
  w.classList.remove('enter');
  void w.offsetWidth;
  w.classList.add('enter');
}

function refreshRows() {
  S.rows = currentRows();
  S.idx = new Map(S.rows.map((r, i) => [r.address, i]));
  S.rowsVer++;
  $('#vspacer').style.height = S.rows.length * ROW_H + 'px';
  drawWindow(true);
  updateEmpty();
  updateStatus();
  schedulePing();
}

function patchRow(a) {
  const i = S.idx.get(a);
  if (i == null) return;
  S.rows[i] = S.tab === 'favorites' ? lookup(a) : (S.live.get(a) || S.inetMap.get(a) || S.rows[i]);
}

let patchQ = new Set(), patchRaf = 0;
function schedulePatch(a) {
  patchQ.add(a);
  if (patchRaf) return;
  patchRaf = requestAnimationFrame(() => {
    patchRaf = 0;
    for (const x of patchQ) patchRow(x);
    patchQ = new Set();
    if (S.tab !== 'top') { S.rowsVer++; drawWindow(true); updateStatus(); }
  });
}

function updateEmpty() {
  const e = $('#empty');
  let msg = '';
  if (!S.rows.length) {
    if (S.tab === 'favorites') {
      msg = S.store.favorites.length
        ? 'В этой папке пока нет серверов.\nПеретащите сюда сервер из списка.'
        : 'Здесь пока пусто.\nДобавьте сервер кнопкой «+» или нажмите ★ на сервере во вкладке Internet.';
    } else {
      msg = S.loadingInet ? 'Загрузка списка серверов…' : (S.inetError || (S.internet.length ? 'Ничего не найдено — измените фильтр' : ''));
    }
  }
  e.textContent = msg;
  e.hidden = !msg;
}

function updateStatus() {
  const rows = S.tab === 'top' ? topList() : S.rows;
  let pl = 0;
  for (const r of rows) pl += r.players || 0;
  const t = S.tab === 'favorites' ? S.favAt : S.internetAt;
  $('#status').textContent = `Серверов: ${rows.length} · игроков: ${pl}` + (t ? ` · обновлено ${new Date(t).toLocaleTimeString('ru-RU')}` : '');
}

function markSelected() {
  for (const n of $('#vwin').children) n.classList.toggle('sel', n.dataset.addr === S.selected);
  for (const n of $('#top').children) n.classList.toggle('sel', n.dataset.addr === S.selected);
}

// ───────────────────────── Top 10 ─────────────────────────
// Берём только правдоподобные записи (игроков не больше слотов), чтобы накрученные счётчики не попадали в рейтинг.
function topList() {
  return S.internet
    .filter((s) => s.maxPlayers > 0 && s.players <= s.maxPlayers)
    .sort((a, b) => b.players - a.players)
    .slice(0, 10);
}

function renderTop() {
  const box = $('#top');
  box.classList.remove('show');
  const list = topList();
  if (!list.length) {
    box.replaceChildren(el('div', 'hint', S.loadingInet ? 'Загрузка рейтинга…' : (S.inetError || 'Нет данных')));
    updateStatus();
    return;
  }
  const max = Math.max(1, list[0].players);
  box.replaceChildren(...list.map((s, i) => {
    const c = el('div', 'tcard' + (s.address === S.selected ? ' sel' : ''));
    c.dataset.addr = s.address;
    c.style.setProperty('--i', i);
    c.style.setProperty('--p', (s.players / max).toFixed(3));
    const count = el('div', 'tcount');
    count.append(el('b', '', String(s.players)), el('small', '', `/ ${s.maxPlayers}`));
    const meter = el('div', 'meter');
    meter.append(el('i'));
    c.append(
      el('div', 'rank', String(i + 1)),
      el('div', 'tname', s.hostname || s.address),
      count,
      el('div', 'tmeta', [s.mode, s.language, s.address].filter(Boolean).join(' · ')),
      meter,
    );
    return c;
  }));
  requestAnimationFrame(() => requestAnimationFrame(() => box.classList.add('show')));
  updateStatus();
}

// ───────────────────────── сеть ─────────────────────────
let busy = 0;
function setBusy(d) {
  busy += d;
  $('#loadbar').classList.toggle('on', busy > 0);
}

function queryBatch(addrs, onInfo) {
  return new Promise((resolve, reject) => {
    if (!addrs.length) return resolve();
    let left = addrs.length;
    const ch = new Channel();
    ch.onmessage = (info) => { onInfo(info); if (--left === 0) resolve(); };
    invoke('query_servers', { addresses: addrs, onResult: ch }).then(() => setTimeout(resolve, 300), reject);
  });
}

function applyResult(info) {
  const a = info.address;
  if (info.online) S.live.set(a, info);
  else {
    const o = S.live.get(a) || S.inetMap.get(a) || {};
    S.live.set(a, { ...o, address: a, online: false, ping: null, players: 0 });
  }
}

let favBusy = false;
async function refreshFavorites() {
  if (favBusy) return;
  const addrs = S.store.favorites.map((f) => f.address);
  S.favAt = Date.now();
  if (!addrs.length) { updateStatus(); return; }
  favBusy = true;
  setBusy(1);
  try {
    for (let i = 0; i < addrs.length; i += 300) {
      await queryBatch(addrs.slice(i, i + 300), (info) => { applyResult(info); schedulePatch(info.address); });
    }
  } catch (e) {
    toast(String(e), true);
  } finally {
    setBusy(-1);
    favBusy = false;
  }
  const names = [];
  for (const f of S.store.favorites) {
    const l = S.live.get(f.address);
    if (l && l.online && l.hostname && l.hostname !== f.name) { names.push([f.address, l.hostname]); f.name = l.hostname; }
  }
  if (names.length) invoke('set_names', { items: names }).catch(() => {});
  if (S.tab === 'favorites') refreshRows();
}

async function loadInternet(force) {
  if (!force && S.internet.length && Date.now() - S.internetAt < 60000) return;
  S.loadingInet = true;
  S.inetError = '';
  if (S.tab === 'internet') updateEmpty();
  setBusy(1);
  try {
    const list = await invoke('fetch_internet', { force });
    S.internet = list;
    S.inetMap = new Map(list.map((s) => [s.address, s]));
    S.internetAt = Date.now();
    S.pinged.clear();
    for (const k of [...S.live.keys()]) if (!isFav(k)) S.live.delete(k);
  } catch (e) {
    S.inetError = String(e);
    toast(String(e), true);
  } finally {
    S.loadingInet = false;
    setBusy(-1);
  }
  afterInternet();
}

function afterInternet() {
  if (S.tab === 'top') renderTop();
  else if (S.tab === 'internet') refreshRows();
}

// Пинг для видимых строк Internet: тянем только то, что на экране, и только после остановки прокрутки.
let pingT = 0;
function schedulePing() {
  clearTimeout(pingT);
  if (S.tab !== 'internet') return;
  pingT = setTimeout(pingVisible, 300);
}
async function pingVisible() {
  const b = $('#vlist');
  const s = Math.floor(b.scrollTop / ROW_H);
  const e = Math.ceil((b.scrollTop + b.clientHeight) / ROW_H);
  const need = S.rows.slice(s, e).map((r) => r.address).filter((a) => !S.pinged.has(a) && !S.live.has(a)).slice(0, 60);
  if (!need.length) return;
  need.forEach((a) => S.pinged.add(a));
  try {
    await queryBatch(need, (info) => { applyResult(info); schedulePatch(info.address); });
  } catch { /* не критично */ }
}

// ───────────────────────── панель выбранного сервера ─────────────────────────
let detTimer = 0, detSeq = 0, plSig = '';

function showBasic(r) {
  $('#iName').textContent = r ? (r.hostname || r.address) : 'Выберите сервер в списке';
  $('#iAddr').textContent = r ? r.address : '—';
  $('#iPlayers').textContent = r && r.players != null && r.online !== false ? `${r.players} / ${r.maxPlayers}` : '—';
  $('#iPing').textContent = r && r.ping != null ? `${r.ping} мс` : '—';
  $('#iMode').textContent = (r && r.mode) || '—';
  $('#iLang').textContent = (r && r.language) || '—';
}

function clearDetails(msg = 'Загрузка…') {
  plSig = '';
  $('#players').replaceChildren(el('div', 'hint', msg));
  $('#rules').replaceChildren(el('div', 'hint', msg));
  $('#pcount').textContent = '';
}

function drawGraph() {
  const N = 40, step = 200 / (N - 1);
  const h = S.pingHist;
  const max = Math.max(100, ...h.filter((v) => v != null));
  const off = N - h.length;
  const pts = [];
  h.forEach((v, i) => { if (v != null) pts.push(`${((off + i) * step).toFixed(1)},${(56 - (v / max) * 52).toFixed(1)}`); });
  if (pts.length === 1) pts.push(`${(Math.min(200, (off + h.length - 1) * step + step)).toFixed(1)},${pts[0].split(',')[1]}`);
  $('#gline').setAttribute('points', pts.join(' '));
  const last = h.length ? h[h.length - 1] : null;
  $('#gping').textContent = last != null ? `${last} мс` : (h.length ? 'нет ответа' : '');
}

function renderDetails(d) {
  const sig = JSON.stringify([d.players, d.rules]);
  if (sig === plSig) return;
  plSig = sig;
  const pl = [...d.players].sort((a, b) => b.score - a.score || coll.compare(a.name, b.name));
  $('#pcount').textContent = d.info.online ? `${pl.length} из ${d.info.players}` : '';
  $('#players').replaceChildren(...(pl.length
    ? pl.map((p) => { const r = el('div', 'pr'); r.append(el('span', '', p.name), el('span', '', String(p.score))); return r; })
    : [el('div', 'hint', d.info.online ? (d.info.players ? 'Сервер не отдаёт список игроков' : 'Игроков нет') : 'Нет ответа')]));
  $('#rules').replaceChildren(...(d.rules.length
    ? d.rules.map((x) => { const r = el('div', 'rr'); r.append(el('span', '', x.name), el('span', '', x.value)); return r; })
    : [el('div', 'hint', d.info.online ? 'Нет данных' : 'Нет ответа')]));
}

async function loadDetails(addr) {
  const seq = ++detSeq;
  try {
    const d = await invoke('server_details', { address: addr });
    if (seq !== detSeq || S.selected !== addr) return;
    applyResult(d.info);
    S.pingHist.push(d.info.online ? d.info.ping : null);
    if (S.pingHist.length > 40) S.pingHist.shift();
    drawGraph();
    showBasic(lookup(addr));
    showCover(addr);
    renderDetails(d);
    schedulePatch(addr);
  } catch (e) {
    if (seq === detSeq) clearDetails(String(e));
  }
}

function startPolling(addr) {
  clearInterval(detTimer);
  loadDetails(addr);
  detTimer = setInterval(() => { if (!document.hidden && S.selected === addr) loadDetails(addr); }, 5000);
}

function updateFavBtn() {
  const b = $('#bFav');
  b.classList.toggle('on', !!S.selected && isFav(S.selected));
  b.disabled = !S.selected;
  $('#bConnect').disabled = !S.selected;
}

// ───────────────────────── обложка проекта (эксперимент) ─────────────────────────
S.covers = new Map();   // hostname -> {dataUrl, source} | null
let coverSeq = 0, coverHost = null;

function paintCover(c, title) {
  const box = $('#cover');
  box.classList.remove('loading', 'show');
  if (!c) { box.hidden = true; return; }
  const img = $('#coverImg');
  img.onload = () => { void box.offsetWidth; box.classList.add('show'); };
  img.src = c.dataUrl;
  $('#coverName').textContent = title;
  $('#coverTag').textContent = c.source === 'vk-cover' ? 'VK · обложка' : 'VK';
  box.hidden = false;
}

async function showCover(addr) {
  const host = (lookup(addr).hostname || '').trim();
  if (host === coverHost) return;
  coverHost = host;
  const seq = ++coverSeq;
  const box = $('#cover');
  if (S.covers.has(host)) return paintCover(S.covers.get(host), host);
  box.classList.remove('show');
  // индикатор загрузки — только если ответ задерживается (сеть), мгновенный «нет обложки» не мигает
  const t = setTimeout(() => { if (seq === coverSeq) { box.hidden = false; box.classList.add('loading'); $('#coverImg').removeAttribute('src'); $('#coverName').textContent = ''; } }, 150);
  let c = null;
  try { c = await invoke('get_cover', { hostname: host }); } catch { c = null; }
  clearTimeout(t);
  S.covers.set(host, c);
  if (seq === coverSeq) paintCover(c, host);
}

function select(addr) {
  if (S.selected === addr) return;
  S.selected = addr;
  markSelected();
  S.pingHist = [];
  drawGraph();
  showBasic(lookup(addr));
  clearDetails();
  updateFavBtn();
  showCover(addr);
  startPolling(addr);
}

// ───────────────────────── действия ─────────────────────────
async function mutate(cmd, args, animate = false) {
  try { S.store = await invoke(cmd, args); applyAppearance(animate); return true; }
  catch (e) { toast(String(e), true); return false; }
}

// Тема, набор иконок, кнопка Plus — всё, что зависит от состояния хранилища.
function applyAppearance(animate = false) {
  const st = S.store;
  if (window.PJTheme) PJTheme.set(st.theme, animate);
  const dark = window.PJTheme ? PJTheme.current() === 'dark' : true;
  $('#bTheme').innerHTML = ICONS[dark ? 'moon' : 'sun'];
  $('#bTheme').title = dark ? 'Сейчас тёмная тема — включить светлую' : 'Сейчас светлая тема — включить тёмную';
  const plus = $('#bPlus');
  plus.classList.toggle('active', !!st.premium);
  plus.title = st.premium ? `Plus активен до ${fmtDate(st.premiumUntil)}` : 'Подписка Plus';
}

async function toggleTheme() {
  const next = window.PJTheme && PJTheme.current() === 'dark' ? 'light' : 'dark';
  await mutate('set_theme', { theme: next }, true);
}

function afterStoreChange() {
  renderSide();
  updateFavBtn();
  if (S.tab === 'favorites') refreshRows();
}

async function addFav(a, folder = null, silent = false) {
  const r = lookup(a);
  if (!(await mutate('add_favorite', { address: a, name: r.hostname && r.hostname !== a ? r.hostname : '', folder }))) return;
  afterStoreChange();
  if (!silent) toast('Добавлено в Favorites');
  if (!S.live.has(a)) queryBatch([a], (i) => { applyResult(i); schedulePatch(a); }).catch(() => {});
}

async function removeFav(a) {
  if (!(await mutate('remove_favorite', { address: a }))) return;
  afterStoreChange();
  toast('Убрано из Favorites');
}

async function moveFav(a, folder) {
  if (!(await mutate('move_favorite', { address: a, folder }))) return;
  afterStoreChange();
}

async function toggleFav(a = S.selected) {
  if (!a) return;
  if (isFav(a)) await removeFav(a); else await addFav(a, S.tab === 'favorites' ? S.folder : null);
}

async function connect(a = S.selected) {
  if (!a) { toast('Сначала выберите сервер'); return; }
  const r = lookup(a);
  let password = null;
  if (r.password) {
    const v = await ask({ title: 'Пароль сервера', fields: [{ id: 'p', label: r.hostname || a, type: 'password' }], ok: 'Играть' });
    if (!v) return;
    password = v.p || null;
  }
  try {
    await invoke('connect', { address: a, password });
    toast('Запускаю игру…');
  } catch (e) {
    toast(String(e), true);
    if (/папк|gta_sa|библиотек/i.test(String(e))) openSettings();
  }
}

async function gameCheck() {
  if (!S.store.gtaPath) return null;
  try { return await invoke('check_game', { path: S.store.gtaPath }); } catch { return null; }
}

async function pickFolder() {
  try {
    const dir = await TAURI.dialog.open({ directory: true, multiple: false, title: 'Папка GTA San Andreas', defaultPath: S.store.gtaPath || undefined });
    return typeof dir === 'string' ? dir : null;
  } catch (e) { toast(String(e), true); return null; }
}

async function openSettings() {
  let check = await gameCheck();
  await modal('Настройки', (close, redraw) => {
    // — папка игры —
    const path = el('input');
    path.readOnly = true;
    path.value = S.store.gtaPath || '';
    path.placeholder = 'Папка с gta_sa.exe не выбрана';
    path.title = path.value;
    const browse = btn('Обзор…', async () => {
      const dir = await pickFolder();
      if (!dir) return;
      if (await mutate('save_settings', { nickname: S.store.nickname, gtaPath: dir })) { check = await gameCheck(); redraw(); }
    }, 'primary');
    const chips = el('div', 'chips');
    if (!S.store.gtaPath) chips.append(el('span', 'chip bad', 'Папка не выбрана'));
    else if (check) {
      chips.append(el('span', 'chip ' + (check.exe ? 'ok' : 'bad'), check.exe ? 'gta_sa.exe найден' : 'gta_sa.exe не найден'));
      chips.append(el('span', 'chip ' + (check.clientDll ? 'ok' : 'bad'), check.clientDll ? `Клиент: ${check.clientDll}` : 'Нет plugjam.dll / samp.dll'));
      if (check.exe && check.exeV100 === false) chips.append(el('span', 'chip warn', 'gta_sa.exe не версии 1.0 US'));
      if (check.bitnessOk === false) chips.append(el('span', 'chip bad', 'Нужна 32-битная сборка лаунчера'));
    }
    const auto = el('button', 'link', 'Найти автоматически');
    auto.type = 'button';
    auto.onclick = async () => {
      const found = await invoke('detect_game').catch(() => null);
      if (!found) return toast('Игра не найдена — нажмите «Обзор»', true);
      if (await mutate('save_settings', { nickname: S.store.nickname, gtaPath: found })) { check = await gameCheck(); redraw(); toast('Папка найдена'); }
    };
    const game = section('Игра', Object.assign(el('div', 'line'), {}), chips, Object.assign(el('p', 'note'), {}));
    game.children[1].append(path, browse);
    game.lastChild.append(auto, document.createTextNode(' · если клиент установлен в папку с игрой, путь выбирается сам'));

    // — тема —
    const seg = el('div', 'seg in');
    for (const [v, label] of [['system', 'Системная'], ['dark', 'Тёмная'], ['light', 'Светлая']]) {
      const b = el('button', S.store.theme === v ? 'on' : '', label);
      b.type = 'button';
      b.onclick = async () => { if (await mutate('set_theme', { theme: v }, true)) redraw(); };
      seg.append(b);
    }

    // — иконка лаунчера —
    const grid = el('div', 'appicons');
    for (const [k, label] of [['classic', 'Классика'], ['sunset', 'Закат'], ['ocean', 'Океан'], ['violet', 'Фиолет'], ['mono', 'Моно']]) {
      const locked = k !== 'classic' && !S.store.premium;
      const b = el('button', 'appicon' + (S.store.appIconEffective === k ? ' on' : '') + (locked ? ' locked' : ''));
      b.type = 'button';
      const img = el('img');
      img.src = `appicons/${k}.png`;
      img.alt = '';
      b.append(img, el('span', '', label));
      if (locked) { const l = el('span', 'lock'); l.innerHTML = ICONS.lock; b.append(l); }
      b.onclick = async () => {
        if (locked) { close(); openPlus('icons'); return; }
        if (S.store.appIconEffective === k) return;
        if (await mutate('set_app_icon', { icon: k })) { redraw(); toast('Иконка окна изменена'); }
      };
      grid.append(b);
    }
    const iconsSec = section(null, grid, el('p', 'note', 'Меняется иконка окна и панели задач. Иконка файла .exe и ярлыков остаётся прежней.'));
    iconsSec.prepend(Object.assign(el('h4'), { innerHTML: 'Иконка лаунчера' + (S.store.premium ? '' : '<span class="lockbadge">смена — Plus</span>') }));

    // — ID —
    const id = el('div', 'kv');
    const code = el('code', 'mono', S.store.userId || '—');
    id.append(el('span', '', 'Ваш ID'), code, btn('Копировать', () => copyText(S.store.userId, 'ID скопирован'), 'sm'));

    const actions = el('div', 'actions');
    actions.append(btn('Закрыть', () => close(), 'primary'));
    return [game, section('Тема', seg), iconsSec, section('Идентификатор', id), actions];
  }, { wide: true });
}

// ───────────────────────── подписка Plus ─────────────────────────
const UPSELL = {
  folders: 'Бесплатно доступно 3 папки. С Plus — до 10.',
  nicks: 'Список сохранённых ников — функция Plus.',
  icons: 'Смена иконки лаунчера — функция Plus.',
};

async function openPlus(reason) {
  const info = await invoke('get_donate_info').catch(() => null);
  await modal('Подписка Plus', (close, redraw) => {
    const st = S.store;
    const state = el('div', 'state' + (st.premium ? '' : ' free'), st.premium ? `Plus активен до ${fmtDate(st.premiumUntil)}` : 'Бесплатная версия');
    const out = [state];
    if (reason && !st.premium) out.push(el('p', '', UPSELL[reason] || ''));
    const perks = el('ul', 'perks');
    for (const t of ['До 10 папок вместо 3', 'Сохранённые ники: общий список и свой ник для каждого сервера', 'Иконка лаунчера на выбор']) perks.append(el('li', '', t));
    const price = el('div', 'price');
    price.append(el('b', '', `${info ? info.price : '0.99'} USDT`), el('span', '', `/ ${info ? info.periodDays : 30} дней`));
    out.push(perks, price);

    const row = (label, value, onCopy) => {
      const r = el('div', 'kv');
      r.append(el('span', '', label), el('code', 'mono', value), btn('Копировать', onCopy, 'sm'));
      return r;
    };
    const wayTon = el('div', 'way');
    wayTon.append(el('h4', '', 'Способ 1 · USDT в сети TON'));
    if (info) {
      wayTon.append(
        row('Кошелёк', info.wallet, () => copyText(info.wallet, 'Адрес скопирован')),
        row('Сумма', `${info.price} USDT`, () => copyText(info.price, 'Сумма скопирована')),
        row('Комментарий', info.comment, () => copyText(info.comment, 'Комментарий скопирован')),
      );
      const warn = el('p', 'note');
      warn.innerHTML = '<b>Важно:</b> в комментарии укажите ваш ID — по нему выдаётся ключ. Другие токены и сети не принимаются, платёж будет потерян.';
      wayTon.append(warn);
    }
    const handle = info?.telegramHandle || '@irishatemskey';
    const tgUrl = info?.telegramUrl || 'https://t.me/irishatemskey';
    const wayTg = el('div', 'way');
    wayTg.append(el('h4', '', 'Способ 2 · Telegram'));
    wayTg.append(el('p', 'note', `Напишите разработчику ${handle} и отправьте свой ID (${st.userId}) — он подскажет, как оформить подписку, и пришлёт ключ.`));
    const tgLine = el('div', 'line');
    tgLine.append(btn('Написать в Telegram', () => {
      copyText(st.userId, 'ID скопирован — вставьте его в чат');
      invoke('open_url', { url: tgUrl }).catch((e) => toast(String(e), true));
    }, 'primary'), btn('Копировать ID', () => copyText(st.userId, 'ID скопирован')));
    wayTg.append(tgLine);
    out.push(wayTon, wayTg);

    const key = el('input');
    key.placeholder = 'PJ1.…';
    key.spellcheck = false;
    key.autocomplete = 'off';
    const act = btn('Активировать', async () => {
      if (!key.value.trim()) return;
      if (await mutate('activate_license', { key: key.value.trim() })) { toast(`Plus активен до ${fmtDate(S.store.premiumUntil)}`); redraw(); renderSide(); }
    }, 'primary');
    const line = el('div', 'line');
    line.append(key, act);
    const how = el('p', 'note', 'Ключ выдаётся на 30 дней по вашему ID. Продление — новая оплата и новый ключ.');
    out.push(section('Ключ', line, how));
    const actions = el('div', 'actions');
    actions.append(btn('Закрыть', () => close()));
    out.push(actions);
    return out;
  }, { wide: true, sub: 'символическая, 0,99 $ в месяц' });
}

// ───────────────────────── сохранённые ники (Plus) ─────────────────────────
function serverTitle(addr) {
  const f = S.store.favorites.find((x) => x.address === addr);
  return (S.live.get(addr)?.hostname || f?.name || addr);
}

async function useNick(n) {
  if (await mutate('save_settings', { nickname: n, gtaPath: S.store.gtaPath })) $('#nick').value = S.store.nickname;
}

async function openNicks() {
  if (!S.store.premium) return openPlus('nicks');
  await modal('Мои ники', (close, redraw) => {
    const list = el('div', 'nlist');
    if (!S.store.nicknames.length) list.append(el('div', 'hint', 'Список пуст. Сохраните текущий ник или добавьте новый.'));
    for (const n of S.store.nicknames) {
      const r = el('div', 'nrow' + (n === S.store.nickname ? ' cur' : ''));
      r.append(el('span', '', n));
      if (n !== S.store.nickname) r.append(btn('Выбрать', async () => { await useNick(n); redraw(); }, 'sm'));
      const x = el('button', 'x', '×');
      x.type = 'button';
      x.title = 'Удалить из списка';
      x.onclick = async () => { if (await mutate('remove_saved_nick', { nick: n })) redraw(); };
      r.append(x);
      list.append(r);
    }
    const add = el('input');
    add.placeholder = 'Новый ник';
    add.maxLength = 20;
    add.spellcheck = false;
    const save = async (n) => { if (n && await mutate('add_saved_nick', { nick: n })) { add.value = ''; redraw(); } };
    add.onkeydown = (e) => { if (e.key === 'Enter') { e.preventDefault(); save(add.value.trim()); } };
    const line = el('div', 'line');
    line.append(add, btn('Добавить', () => save(add.value.trim())), btn('Сохранить текущий', () => save(S.store.nickname)));

    const sn = el('div', 'nlist');
    const entries = Object.entries(S.store.serverNicks);
    if (!entries.length) sn.append(el('div', 'hint', 'Правый клик по серверу → «Ник для сервера».'));
    for (const [addr, n] of entries) {
      const r = el('div', 'nrow');
      const t = el('span', '', n);
      t.append(document.createTextNode(' '), el('small', '', '· ' + serverTitle(addr)));
      const x = el('button', 'x', '×');
      x.type = 'button';
      x.onclick = async () => { if (await mutate('set_server_nick', { address: addr, nick: null })) redraw(); };
      r.append(t, x);
      sn.append(r);
    }
    const actions = el('div', 'actions');
    actions.append(btn('Закрыть', () => close(), 'primary'));
    return [section('Общий список', list, line), section('Ники для серверов', sn), actions];
  }, { wide: true });
}

async function serverNickDialog(a) {
  if (!S.store.premium) return openPlus('nicks');
  const cur = S.store.serverNicks[a] || '';
  const v = await ask({ title: 'Ник для сервера', text: serverTitle(a), fields: [{ id: 'n', label: 'Ник для этого сервера (пусто — общий ник)', value: cur }], ok: 'Сохранить' });
  if (v && await mutate('set_server_nick', { address: a, nick: v.n || null })) toast(v.n ? 'Ник для сервера сохранён' : 'Будет использоваться общий ник');
}

async function addByAddress() {
  const v = await ask({ title: 'Добавить сервер', fields: [{ id: 'a', label: 'Адрес сервера', placeholder: '185.71.66.21:7777' }], ok: 'Добавить' });
  if (!v || !v.a) return;
  const a = normAddr(v.a);
  await addFav(a, S.tab === 'favorites' ? S.folder : null);
  if (S.tab !== 'favorites') await setTab('favorites');
  select(a);
}

async function newFolder() {
  if (S.store.folders.length >= S.store.maxFolders) {
    if (!S.store.premium) return openPlus('folders');
    return toast(`Достигнут предел: ${S.store.maxFolders} папок`, true);
  }
  const v = await ask({ title: 'Новая папка', fields: [{ id: 'n', label: 'Название (например, название проекта)' }], ok: 'Создать' });
  if (!v || !v.n) return;
  const before = new Set(S.store.folders.map((f) => f.id));
  if (await mutate('create_folder', { name: v.n })) {
    const created = S.store.folders.find((f) => !before.has(f.id));
    if (created) S.folder = created.id;
    afterStoreChange();
  }
}

async function renameFolder(id) {
  const f = S.store.folders.find((x) => x.id === id);
  const v = await ask({ title: 'Переименовать папку', fields: [{ id: 'n', label: 'Название', value: f?.name || '' }], ok: 'Сохранить' });
  if (v && v.n && await mutate('rename_folder', { id, name: v.n })) afterStoreChange();
}

async function deleteFolder(id) {
  if (await mutate('delete_folder', { id })) {
    if (S.folder === id) S.folder = null;
    afterStoreChange();
    toast('Папка удалена, серверы остались в избранном');
  }
}

function serverMenu(x, y, a) {
  const items = [{ label: 'Подключиться', fn: () => connect(a) }];
  const sn = S.store.premium ? S.store.serverNicks[a] : null;
  items.push({ label: sn ? `Ник для сервера: ${sn}…` : S.store.premium ? 'Ник для сервера…' : 'Ник для сервера (Plus)…', fn: () => serverNickDialog(a) });
  items.push({ label: isFav(a) ? 'Убрать из избранного' : 'В избранное', fn: () => toggleFav(a) });
  const cur = S.store.favorites.find((f) => f.address === a)?.folder ?? null;
  const targets = S.store.folders.filter((f) => f.id !== cur);
  if (targets.length || (isFav(a) && cur != null)) items.push('-');
  for (const f of targets) items.push({ label: `В папку «${f.name}»`, fn: () => (isFav(a) ? moveFav(a, f.id) : addFav(a, f.id)) });
  if (isFav(a) && cur != null) items.push({ label: 'Вынуть из папки', fn: () => moveFav(a, null) });
  items.push('-', { label: 'Копировать адрес', fn: () => navigator.clipboard?.writeText(a).then(() => toast('Адрес скопирован')).catch(() => {}) });
  showMenu(x, y, items);
}

// ───────────────────────── боковая панель папок ─────────────────────────
function renderSide() {
  const box = $('#folders');
  const mk = (id, label, count) => {
    const b = el('button', 'fold' + (S.folder === id ? ' on' : ''));
    b.dataset.id = id == null ? '' : String(id);
    b.append(el('span', 'fn', label), el('span', 'cnt', String(count)));
    return b;
  };
  box.replaceChildren(
    mk(null, 'Все серверы', S.store.favorites.length),
    ...S.store.folders.map((f) => mk(f.id, f.name, S.store.favorites.filter((x) => x.folder === f.id).length)),
  );
  $('.side-h').textContent = `Папки · ${S.store.folders.length}/${S.store.maxFolders}`;
  $('#side').classList.toggle('collapsed', S.tab !== 'favorites');
}

// ───────────────────────── вкладки ─────────────────────────
function moveInk() {
  const b = document.querySelector('#tabs [data-tab].on');
  const ink = $('#ink');
  if (!b) return;
  ink.style.width = b.offsetWidth - 20 + 'px';
  ink.style.transform = `translateX(${b.offsetLeft + 10}px)`;
}

async function setTab(t) {
  S.tab = t;
  $('#app').dataset.tab = t;
  document.querySelectorAll('#tabs [data-tab]').forEach((b) => b.classList.toggle('on', b.dataset.tab === t));
  moveInk();
  $('#side').classList.toggle('collapsed', t !== 'favorites');
  $('#tablewrap').hidden = t === 'top';
  $('#top').hidden = t !== 'top';
  if (t === 'top') {
    renderTop();
    await loadInternet(false);
    if (S.tab === 'top') renderTop();
  } else if (t === 'internet') {
    refreshRows();
    playEnter();
    await loadInternet(false);
  } else {
    refreshRows();
    playEnter();
    if (Date.now() - S.favAt > 20000) refreshFavorites();
  }
}

async function refresh() {
  if (S.tab === 'favorites') await refreshFavorites();
  else await loadInternet(true);
  if (S.tab === 'favorites') refreshRows();
}

// ───────────────────────── события ─────────────────────────
function addrFromEvent(e) {
  const n = e.target.closest('.row, .tcard');
  return n ? n.dataset.addr : null;
}

function bind() {
  document.querySelectorAll('[data-i]').forEach((b) => b.insertAdjacentHTML('afterbegin', ICONS[b.dataset.i]));

  $('#bConnect').onclick = () => connect();
  $('#bFav').onclick = () => toggleFav();
  $('#bAdd').onclick = addByAddress;
  $('#bRefresh').onclick = refresh;
  $('#bSettings').onclick = openSettings;
  $('#bFolder').onclick = newFolder;
  $('#bPlus').onclick = () => openPlus();
  $('#bTheme').onclick = toggleTheme;
  $('#bNicks').onclick = openNicks;

  $('#tabs').addEventListener('click', (e) => {
    const b = e.target.closest('[data-tab]');
    if (b && b.dataset.tab !== S.tab) setTab(b.dataset.tab);
  });

  const nick = $('#nick');
  nick.value = S.store.nickname;
  nick.addEventListener('change', async () => {
    await mutate('save_settings', { nickname: nick.value, gtaPath: S.store.gtaPath });
    nick.value = S.store.nickname;
  });

  const sortBtns = document.querySelectorAll('#sort [data-sort]');
  const markSort = () => sortBtns.forEach((b) => b.classList.toggle('on', b.dataset.sort === S.store.sort));
  markSort();
  $('#sort').addEventListener('click', async (e) => {
    const b = e.target.closest('[data-sort]');
    if (!b || b.dataset.sort === S.store.sort) return;
    if (await mutate('set_sort', { sort: b.dataset.sort })) { markSort(); refreshRows(); playEnter(); }
  });

  let ft = 0;
  const applyFilters = () => {
    clearTimeout(ft);
    ft = setTimeout(() => {
      S.f.search = $('#search').value.trim().toLowerCase();
      S.f.mode = $('#fMode').value.trim().toLowerCase();
      S.f.lang = $('#fLang').value.trim().toLowerCase();
      S.f.notFull = $('#fFull').checked;
      S.f.notEmpty = $('#fEmpty').checked;
      S.f.noPass = $('#fPass').checked;
      if (S.tab !== 'top') { $('#vlist').scrollTop = 0; refreshRows(); }
    }, 120);
  };
  for (const id of ['#search', '#fMode', '#fLang', '#fFull', '#fEmpty', '#fPass']) {
    $(id).addEventListener('input', applyFilters);
    $(id).addEventListener('change', applyFilters);
  }

  // список
  let raf = 0;
  $('#vlist').addEventListener('scroll', () => {
    if (raf) return;
    raf = requestAnimationFrame(() => { raf = 0; drawWindow(false); schedulePing(); });
  }, { passive: true });
  for (const root of [$('#vwin'), $('#top')]) {
    root.addEventListener('click', (e) => { const a = addrFromEvent(e); if (a) select(a); });
    root.addEventListener('dblclick', (e) => { const a = addrFromEvent(e); if (a) { select(a); connect(a); } });
    root.addEventListener('contextmenu', (e) => {
      const a = addrFromEvent(e);
      if (!a) return;
      e.preventDefault();
      select(a);
      serverMenu(e.clientX, e.clientY, a);
    });
  }
  $('#vwin').addEventListener('dragstart', (e) => {
    const a = addrFromEvent(e);
    if (!a) return e.preventDefault();
    e.dataTransfer.setData('text/plain', a);
    e.dataTransfer.effectAllowed = 'move';
  });

  // папки
  const fo = $('#folders');
  const folderId = (e) => { const b = e.target.closest('.fold'); return b ? (b.dataset.id === '' ? null : Number(b.dataset.id)) : undefined; };
  fo.addEventListener('click', (e) => {
    const id = folderId(e);
    if (id === undefined) return;
    S.folder = id;
    renderSide();
    refreshRows();
    playEnter();
  });
  fo.addEventListener('contextmenu', (e) => {
    const id = folderId(e);
    if (id == null) return;
    e.preventDefault();
    showMenu(e.clientX, e.clientY, [
      { label: 'Переименовать', fn: () => renameFolder(id) },
      { label: 'Удалить папку', danger: true, fn: () => deleteFolder(id) },
    ]);
  });
  fo.addEventListener('dragover', (e) => {
    const b = e.target.closest('.fold');
    if (!b) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    b.classList.add('drop');
  });
  fo.addEventListener('dragleave', (e) => { e.target.closest?.('.fold')?.classList.remove('drop'); });
  fo.addEventListener('drop', (e) => {
    e.preventDefault();
    fo.querySelectorAll('.drop').forEach((n) => n.classList.remove('drop'));
    const id = folderId(e);
    const a = e.dataTransfer.getData('text/plain');
    if (id !== undefined && a) moveFav(a, id);
  });

  // клавиатура
  document.addEventListener('keydown', (e) => {
    if (e.key === 'F5') { e.preventDefault(); refresh(); return; }
    if (e.key === 'Escape') { hideMenu(); return; }
    const t = e.target;
    if (t.closest?.('input, dialog')) return;
    if (e.key === 'Enter' && !t.closest?.('button')) connect();
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); moveSel(e.key === 'ArrowDown' ? 1 : -1); }
    else if (e.key === 'Delete' && S.tab === 'favorites' && S.selected && isFav(S.selected)) removeFav(S.selected);
  });

  window.addEventListener('resize', () => { moveInk(); drawWindow(true); });
  document.addEventListener('visibilitychange', () => { if (!document.hidden && S.selected) loadDetails(S.selected); });

  // фоновое обновление, пока окно видно
  setInterval(() => {
    if (document.hidden) return;
    const now = Date.now();
    if (S.tab === 'favorites') { if (now - S.favAt > 30000) refreshFavorites(); }
    else if (now - S.internetAt > 120000) loadInternet(true);
  }, 10000);
}

function moveSel(d) {
  if (S.tab === 'top') {
    const list = topList();
    if (!list.length) return;
    const i = Math.max(0, Math.min(list.length - 1, list.findIndex((s) => s.address === S.selected) + d));
    select(list[i].address);
    return;
  }
  if (!S.rows.length) return;
  const cur = S.selected ? (S.idx.get(S.selected) ?? -1) : -1;
  const i = Math.max(0, Math.min(S.rows.length - 1, cur + d));
  select(S.rows[i].address);
  const b = $('#vlist');
  const top = i * ROW_H, bot = top + ROW_H;
  if (top < b.scrollTop) b.scrollTop = top;
  else if (bot > b.scrollTop + b.clientHeight) b.scrollTop = bot - b.clientHeight;
}

// ───────────────────────── старт ─────────────────────────
(async function init() {
  try { S.store = await invoke('get_state'); } catch (e) { toast(String(e), true); }
  bind();
  applyAppearance(false);
  renderSide();
  updateFavBtn();
  drawGraph();
  clearDetails('Выберите сервер');
  await setTab('favorites');
  if (document.fonts?.ready) document.fonts.ready.then(moveInk);
  // Первый запуск: игру найти не удалось — после заставки сразу предлагаем выбрать папку.
  if (!S.store.gtaPath) setTimeout(() => { toast('Папка игры не найдена — выберите её'); openSettings(); }, 1900);
})();
