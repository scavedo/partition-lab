// Partition Lab UI. All simulation logic lives in Rust (src/); this file only
// draws the state the engine returns and forwards clicks to it.

const IDS = ['A', 'B', 'C'];
const POS = [[200, 58], [82, 214], [318, 214]];
const EDGES = [[0, 1], [1, 2], [0, 2]];
const reduceMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
const $ = (id) => document.getElementById(id);

let engine;          // creates labs
let lab;             // the current WasmLab
let view;            // last view from the engine
let logItems = [];
let t0 = Date.now();

async function loadEngine() {
  // Tests can supply their own engine; the site loads the WebAssembly build.
  if (window.__labEngine) return window.__labEngine;
  const wasm = await import('./pkg/partition_lab.js');
  await wasm.default();
  return { create: (mode) => new wasm.WasmLab(mode) };
}

async function call(method, ...args) {
  return JSON.parse(await lab[method](...args));
}

// ---------- actions ----------
async function perform(method, device, ...args) {
  const out = await call(method, ...args);
  out.events.forEach((e) => log(e.text, e.kind));
  out.transfers.forEach(([a, b], k) => setTimeout(() => packet(a, b), k * 60));
  if (device !== null && out.message) flash(device, out.message, out.ok);
  await refresh();
}

async function setMode(mode) {
  lab = await engine.create(mode);
  document.body.dataset.mode = mode;
  $('mode-ap').setAttribute('aria-pressed', String(mode === 'AP'));
  $('mode-cp').setAttribute('aria-pressed', String(mode === 'CP'));
  $('tryap').hidden = mode !== 'AP';
  $('trycp').hidden = mode !== 'CP';
  $('modehint').textContent = mode === 'AP'
    ? 'Every device always accepts writes. Copies may differ during a split and merge when they reconnect.'
    : 'A write needs a majority (2 of 3). A cut-off device refuses writes, so copies never conflict.';
  logItems = [];
  t0 = Date.now();
  buildCards();
  log(mode === 'AP'
    ? 'Availability mode. All three devices are connected and agree: 6 items sold.'
    : 'Consistency mode. All three devices are connected at version 7: 6 items sold.');
  await refresh();
}

// ---------- log and flash ----------
function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
}

function log(text, kind = 'info') {
  const s = Math.round((Date.now() - t0) / 1000);
  const t = `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
  logItems.unshift({ t, text, kind });
  logItems = logItems.slice(0, 60);
  $('log').innerHTML = logItems
    .map((l) => `<li class="${l.kind === 'refused' ? 'bad' : l.kind}"><span class="t">${l.t}</span><span class="m">${esc(l.text)}</span></li>`)
    .join('');
}

function flash(i, msg, good) {
  const el = $(`flash-${IDS[i]}`);
  el.textContent = msg;
  el.className = `flash${good ? ' good' : ''}`;
  clearTimeout(el._t);
  el._t = setTimeout(() => { el.textContent = ''; }, 3200);
}

// ---------- device cards ----------
function buildCards() {
  $('devices').innerHTML = IDS.map((id, i) => `
    <article class="device" id="dev-${id}" aria-label="Device ${id}">
      <header><h3>Device ${id}</h3><span class="pill" id="pill-${id}"></span></header>
      <div class="field">
        <div class="label">Cabin note <span class="kind ap-only">last write wins</span><span class="kind cp-only">needs the edit lock</span></div>
        <div class="value" id="note-${id}"></div>
        <div class="mono muted" id="note-meta-${id}"></div>
        <form class="row" data-dev="${i}">
          <input id="note-input-${id}" name="note-${id}" aria-label="New note for device ${id}" placeholder="New note" maxlength="60" autocomplete="off">
          <button type="submit" class="btn">Save note</button>
        </form>
      </div>
      <div class="field">
        <div class="label">Items sold <span class="kind ap-only">PN-counter</span><span class="kind cp-only">replicated number</span></div>
        <div class="value big" id="sold-${id}"></div>
        <div class="mono muted" id="sold-meta-${id}"></div>
        <div class="row">
          <button type="button" class="btn" data-act="sell" data-dev="${i}">Sell 1</button>
          <button type="button" class="btn" data-act="refund" data-dev="${i}">Refund 1</button>
        </div>
      </div>
      <div class="field ap-only">
        <div class="label">Same sales, as one plain number <span class="kind">last write wins</span></div>
        <div class="value" id="naive-${id}"></div>
      </div>
      <div class="field cp-only">
        <div class="label">Edit lock <span class="kind">mutex</span></div>
        <div class="value" id="lock-${id}"></div>
        <div class="row">
          <button type="button" class="btn" data-act="take" data-dev="${i}">Take lock</button>
          <button type="button" class="btn" data-act="release" data-dev="${i}">Release</button>
          <button type="button" class="btn" data-act="expire" data-dev="${i}">Expire lease</button>
        </div>
      </div>
      <div class="flash" id="flash-${id}" role="status"></div>
    </article>`).join('');

  $('devices').querySelectorAll('form').forEach((f) => f.addEventListener('submit', (e) => {
    e.preventDefault();
    const i = Number(f.dataset.dev);
    const input = f.querySelector('input');
    const text = input.value;
    input.value = '';
    perform('set_note', i, i, text);
  }));
  $('devices').querySelectorAll('[data-act]').forEach((b) => b.addEventListener('click', () => {
    const i = Number(b.dataset.dev);
    const act = b.dataset.act;
    if (act === 'sell') perform('sell', i, i, 1);
    if (act === 'refund') perform('sell', i, i, -1);
    if (act === 'take') perform('take_lock', i, i);
    if (act === 'release') perform('release_lock', i, i);
    if (act === 'expire') perform('expire_lease', i, i);
  }));
}

// ---------- drawing ----------
const isUp = (a, b) => view.links[EDGES.findIndex(([x, y]) => (x === a && y === b) || (x === b && y === a))];

async function refresh() {
  view = JSON.parse(await lab.view());
  const alone = (i) => view.groups.find((g) => g.includes(i)).length === 1;
  const names = (ids) => ids.map((n) => IDS[n]).join(', ');

  const st = $('netstatus');
  if (view.mode === 'AP') {
    st.textContent = view.copies_agree ? 'Copies agree' : 'Copies differ: they merge on reconnect';
  } else {
    const maj = view.groups.find((g) => g.length >= 2);
    st.textContent = maj
      ? `Majority: ${names(maj)}${view.groups.length > 1 ? ' · others refuse writes' : ''}`
      : 'No majority: every write is refused';
  }
  st.className = `status${view.copies_agree && view.groups.length === 1 ? '' : ' warn'}`;

  $('linkbtns').innerHTML = EDGES.map(([a, b], k) =>
    `<button type="button" class="btn" aria-pressed="${view.links[k]}" data-l="${a}${b}">${IDS[a]}–${IDS[b]} ${view.links[k] ? 'connected' : 'cut'}</button>`).join('');
  $('linkbtns').querySelectorAll('[data-l]').forEach((b) => b.addEventListener('click', () => {
    const [a, c] = b.dataset.l.split('').map(Number);
    perform('toggle_link', null, a, c);
  }));

  view.devices.forEach((d, i) => {
    const id = IDS[i];
    $(`dev-${id}`).classList.toggle('alone', alone(i));
    if (d.kind === 'AP') {
      $(`pill-${id}`).textContent = d.peers.length ? `Syncing with ${names(d.peers)}` : 'Cut off · still writable';
      $(`note-${id}`).textContent = d.note || '—';
      $(`note-meta-${id}`).textContent = d.stamp ? `stamp (${d.stamp.counter}, ${IDS[d.stamp.device]})` : 'no writes yet';
      $(`sold-${id}`).textContent = d.sold;
      $(`sold-meta-${id}`).textContent =
        `+ ${d.inc.map((n, k) => `${IDS[k]}:${n}`).join(' ')}   − ${d.dec.map((n, k) => `${IDS[k]}:${n}`).join(' ')}`;
      const lost = d.sold - d.naive;
      $(`naive-${id}`).innerHTML = `${d.naive} ${lost === 0
        ? '<span class="agree">· matches</span>'
        : `<span class="loss">· ${Math.abs(lost)} ${Math.abs(lost) === 1 ? 'update' : 'updates'} lost</span>`}`;
    } else {
      $(`pill-${id}`).textContent = d.majority ? `Majority with ${names(d.peers)}` : 'Minority · writes refused';
      $(`note-${id}`).textContent = d.note || '—';
      $(`note-meta-${id}`).innerHTML = `version ${d.version}${d.majority ? '' : ' <span class="stale">· may be stale</span>'}`;
      $(`sold-${id}`).textContent = d.sold;
      $(`sold-meta-${id}`).textContent = d.majority ? 'confirmed by a majority' : 'can’t confirm with a majority';
      let lockText = d.lock === null ? 'Free' : d.lock === i ? 'Held by this device' : `Held by ${IDS[d.lock]}`;
      if (d.lock !== null && !d.lock_reachable) lockText += ' (unreachable)';
      if (d.lock === i && !d.majority) lockText += ', but cut off: can’t write';
      $(`lock-${id}`).textContent = lockText;
    }
  });
  drawTopo(alone);
}

function drawTopo(alone) {
  let h = '';
  EDGES.forEach(([a, b], k) => {
    const [x1, y1] = POS[a];
    const [x2, y2] = POS[b];
    const up = view.links[k];
    h += `<line class="edge${up ? '' : ' cut'}" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}"/>`;
    if (!up) h += `<text class="cutmark" x="${(x1 + x2) / 2}" y="${(y1 + y2) / 2 - 8}">cut</text>`;
    h += `<line class="edge-hit" data-edge="${a}${b}" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}"><title>${up ? 'Cut' : 'Restore'} the ${IDS[a]}–${IDS[b]} link</title></line>`;
  });
  POS.forEach(([x, y], i) => {
    const d = view.devices[i];
    const sub = d.kind === 'CP' ? `v${d.version}` : `${d.sold} sold`;
    h += `<g class="node${alone(i) ? ' alone' : ''}"><circle class="ring" cx="${x}" cy="${y}" r="30"/><text x="${x}" y="${y}">${IDS[i]}</text><text class="sub" x="${x}" y="${y + 47}">${sub}</text></g>`;
  });
  const main = $('topo-main');
  main.innerHTML = h;
  main.querySelectorAll('[data-edge]').forEach((el) => el.addEventListener('click', () => {
    const [a, b] = el.dataset.edge.split('').map(Number);
    perform('toggle_link', null, a, b);
  }));
}

function packet(a, b) {
  if (reduceMotion || !isUp(a, b)) return;
  const c = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
  c.setAttribute('r', '6');
  c.setAttribute('class', 'packet');
  $('packets').appendChild(c);
  const [x1, y1] = POS[a];
  const [x2, y2] = POS[b];
  const start = performance.now();
  const step = (now) => {
    const p = Math.min(1, (now - start) / 520);
    const e = p < 0.5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
    c.setAttribute('cx', x1 + (x2 - x1) * e);
    c.setAttribute('cy', y1 + (y2 - y1) * e);
    if (p < 1) requestAnimationFrame(step); else c.remove();
  };
  requestAnimationFrame(step);
}

// ---------- start ----------
async function start() {
  try {
    engine = await loadEngine();
  } catch (err) {
    console.error(err);
    $('loaderr').hidden = false;
    $('netstatus').textContent = 'Engine not loaded';
    return;
  }
  $('mode-ap').addEventListener('click', () => { if (view.mode !== 'AP') setMode('AP'); });
  $('mode-cp').addEventListener('click', () => { if (view.mode !== 'CP') setMode('CP'); });
  $('reset').addEventListener('click', () => setMode(view.mode));
  document.querySelectorAll('[data-preset]').forEach((b) => b.addEventListener('click', () => {
    const p = b.dataset.preset;
    if (p === 'heal') perform('heal', null);
    else perform('isolate', null, Number(p));
  }));

  // On GitHub Pages, link to the repository this page was built from.
  const host = location.hostname;
  if (host.endsWith('.github.io')) {
    const user = host.replace('.github.io', '');
    const repo = location.pathname.split('/').filter(Boolean)[0] || `${user}.github.io`;
    const url = `https://github.com/${user}/${repo}`;
    $('source').insertAdjacentHTML('beforeend', ` <a href="${url}">Read the code on GitHub</a>.`);
  }

  await setMode('AP');
}

start();
