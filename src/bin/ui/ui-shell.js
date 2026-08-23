// ui-shell.js — shared runtime for all Physis UI pages.
// Include before page-specific scripts:
//   <script src="ui-shell.js"></script>
//   <script>/* page-specific code */</script>
//
// Everything lives inside this IIFE and is published on `window` at the end.
// Top level would be fatal: Console, Demo and Study still declare their own
// `const $`, `const S`, `let toastTimer`, `const save`… and two top-level
// lexical declarations of one name in the same realm is a SyntaxError that
// kills the ENTIRE page script — which is exactly what shipped when the
// extraction added the script tags without removing the page-local copies.
// A page's own declarations simply shadow these; pages that declare nothing
// get the shared implementation.
(function () {
'use strict';

const $ = id => document.getElementById(id);
const S = window.location.pathname.replace(/\/\w+.*$/, '/');

function esc(s) {
  if (s == null) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function apiError(e) {
  return '<span class="err">' + esc(String(e)) + '</span>';
}

let toastTimer = null;
function toast(msg, err) {
  const t = $('toast');
  if (!t) return;
  t.textContent = msg;
  t.className = 'toast show ' + (err ? 'err' : 'ok');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.className = 'toast', 4200);
}

function setStatus(m, e) {
  const s = $('status');
  if (!s) return;
  const now = new Date();
  const ts = now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  s.innerHTML = '<span class="muted">' + (e ? '⚠ ' : '') + m + ' · ' + ts + '</span>';
}

function typing(el) {
  const tag = (el || document.activeElement).tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}

// Join the mount prefix to an API path with exactly one slash between them.
//
// `S + p` looked right and was not: `S` is "/" at the site root and every
// caller passes a path starting with "/", so it produced "//api/v1/..." — a
// protocol-relative URL, which the browser resolves as the HOST "api". Requests
// left for a machine that does not exist (ERR_NAME_NOT_RESOLVED) instead of
// hitting this server. Console, Demo and Study each define their own getJSON
// and so never reached this; /agent uses the shared one and broke outright.
function apiUrl(p) {
  const base = S.replace(/\/+$/, '');
  const path = p.startsWith('/') ? p : '/' + p;
  return base + path;
}

async function getJSON(p) {
  const r = await fetch(apiUrl(p), { headers: hdr() });
  if (!r.ok) throw new Error(r.status + ' ' + (await r.text()).slice(0, 180));
  return r.json();
}

async function postJSON(p, body) {
  const r = await fetch(apiUrl(p), {
    method: 'POST',
    headers: { ...hdr(), 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw new Error(r.status + ' ' + (await r.text()).slice(0, 180));
  return r.json();
}

function hdr() {
  const h = {};
  const t = $('token');
  if (t && t.value.trim()) h['Authorization'] = 'Bearer ' + t.value.trim();
  return h;
}

function live() {
  const b = $('base');
  return b && b.value.trim().length > 0;
}

function save() {
  ['base', 'token'].forEach(k => {
    const el = $(k);
    if (el) localStorage.setItem('physis_' + k, el.value);
  });
}

function syncConnect() {
  const btn = $('connectBtn');
  if (!btn) return;
  btn.textContent = live() ? 'connected ▾' : 'Offline demo ▾';
}

// ── Health ─────────────────────────────────────────────────────────
async function loadHealth() {
  try {
    const h = await getJSON('/health');
    const e = await getJSON('/api/v1/embedder').catch(() => null);
    const txt = $('healthTxt');
    if (txt) txt.textContent = h.status + ' · ' + (e && e.provenance ? e.provenance : 'engine');
    const dot = $('health');
    if (dot) dot.querySelector('.dot').className = 'dot ' + (h.status === 'ok' ? 'ok' : 'bad');
  } catch (err) {
    const txt = $('healthTxt');
    if (txt) txt.textContent = 'offline';
    const dot = $('health');
    if (dot) dot.querySelector('.dot').className = 'dot bad';
  }
}

// ── Welcome overlay ───────────────────────────────────────────────
function initWelcome(storageKey) {
  const w = $('welcome');
  if (!w) return;
  if (!localStorage.getItem(storageKey)) {
    w.classList.add('show');
  }
  const dismiss = $('welcomeDismiss');
  if (dismiss) {
    dismiss.onclick = () => {
      localStorage.setItem(storageKey, '1');
      w.classList.remove('show');
    };
  }
}

// ── Help overlay ──────────────────────────────────────────────────
function initHelp() {
  function toggle() {
    const h = $('help');
    if (h) h.classList.toggle('show');
    const w = $('welcome');
    if (w) w.classList.remove('show');
  }
  const btn = $('btnHelp') || $('helpBtn');
  if (btn) btn.onclick = toggle;
  const close = $('helpClose');
  if (close) close.onclick = () => { const h = $('help'); if (h) h.classList.remove('show'); };
}

// ── Coach ─────────────────────────────────────────────────────────
function initCoach(storageKey) {
  const dismiss = $('coachDismiss');
  if (dismiss) {
    dismiss.onclick = () => {
      localStorage.setItem(storageKey, '1');
      const coach = $('coach');
      if (coach) coach.hidden = true;
    };
  }
  if (localStorage.getItem(storageKey)) {
    const coach = $('coach');
    if (coach) coach.hidden = true;
  }
}

// ── Product switcher ──────────────────────────────────────────────
// The four dashboards are one product served by one binary, but each was an
// island: no page linked to any other, so Agent in particular was reachable
// only by typing the URL. The switcher lives here rather than in four headers
// because those headers share no structure (Console has an <h1> + connect
// panel, Demo/Study a .brand block, Agent an <h1> + .sub), so four hand-written
// copies would drift apart on the first edit. Any page that loads this shell
// gets the nav, including ones not written yet.
const PRODUCT_PAGES = [
  { href: '/ui',     label: 'Console', hint: 'operations — plant, timeline, meaning' },
  { href: '/agent',  label: 'Agent',   hint: 'orchestration — tools, OpenCode, coherence gate' },
  { href: '/study',  label: 'Study',   hint: 'learning — sessions and the study log' },
  { href: '/demo',   label: 'Demo',    hint: 'the guided walkthrough' },
];

function initProductNav() {
  const header = document.querySelector('header');
  if (!header || document.getElementById('productNav')) return;

  // Compare on pathname only: a query string or trailing slash still means the
  // same page, and marking nothing as current is worse than marking one.
  const here = window.location.pathname.replace(/\/+$/, '') || '/';

  const nav = document.createElement('nav');
  nav.id = 'productNav';
  nav.setAttribute('aria-label', 'Physis apps');
  nav.innerHTML = PRODUCT_PAGES.map(pg => {
    const current = here === pg.href;
    return '<a href="' + pg.href + '"' +
      (current ? ' aria-current="page"' : '') +
      ' title="' + esc(pg.hint) + '">' + esc(pg.label) + '</a>';
  }).join('');
  header.appendChild(nav);

  // Styles ship with the switcher so a page needs no CSS of its own. Scoped to
  // #productNav so nothing here can reach a page's own header or nav rules.
  if (document.getElementById('productNavCss')) return;
  const css = document.createElement('style');
  css.id = 'productNavCss';
  css.textContent =
    // Console and Studio both carry a bare `nav {}` rule for their sidebar
    // (fixed width, column direction). Restate the properties that rule
    // could otherwise impose, so the switcher cannot inherit a sidebar's
    // shape on any page that styles `nav` generically.
    '#productNav{display:flex;flex-direction:row;width:auto;gap:2px;' +
      'margin-left:auto;align-items:center;position:relative;' +
      'padding:3px;border-radius:999px;border:1px solid rgba(135,157,219,.22);' +
      'background:rgba(10,14,30,.55)}' +
    '#productNav a{font:10px/1 ui-monospace,SFMono-Regular,Menlo,monospace;' +
      'letter-spacing:.1em;text-transform:uppercase;text-decoration:none;' +
      'color:#8997b8;padding:6px 11px;border-radius:999px;white-space:nowrap}' +
    '#productNav a:hover{color:#f1f5ff;background:rgba(255,255,255,.06)}' +
    '#productNav a[aria-current="page"]{color:#04121c;background:#68e7ff;font-weight:700}' +
    // Console/Agent already push a status pill to the right with margin-left:auto;
    // two auto margins would split the gap and unbalance the row, so the pill
    // keeps its position and the switcher sits beside it.
    '#productNav ~ #status,#productNav ~ .status{margin-left:10px}' +
    '@media(max-width:720px){#productNav{margin-left:0;flex-wrap:wrap}}';
  document.head.appendChild(css);
}

// ── Ask bar ───────────────────────────────────────────────────────
// One query box on every page. It searches the vector graph inline and can
// hand the same text to the full agent.
//
// Inline search first, agent second, deliberately: /api/v1/search/nodes embeds
// the query and ranks node labels by cosine with no LLM involved, so the box
// still works on an install with no provider configured — which is the default
// state. A box that needs an API key to do anything is a box that mostly does
// nothing.
function initAskBar() {
  const header = document.querySelector('header');
  if (!header || document.getElementById('askBar')) return;

  const wrap = document.createElement('div');
  wrap.id = 'askBar';
  wrap.innerHTML =
    '<input id="askInput" type="search" autocomplete="off" spellcheck="false" ' +
      'placeholder="Ask the graph…  (press /)" aria-label="Search the Physis graph">' +
    '<button id="askAgent" type="button" title="Open this question in the Agent">Agent ↗</button>';
  header.appendChild(wrap);

  const input = wrap.querySelector('#askInput');

  // The results panel is portalled to <body>, not nested in the bar. Nesting it
  // put it inside whatever stacking context an ancestor happened to create, and
  // dashboards raise their own panels (Console's `.tab` is z-index 1 on a
  // positioned element), so page content painted straight over the results —
  // a larger z-index does not help, because the comparison never reaches it.
  // position:fixed against the viewport also escapes any `overflow:hidden` on
  // the way up, which would otherwise clip the panel.
  const panel = document.createElement('div');
  panel.id = 'askPanel';
  panel.hidden = true;
  document.body.appendChild(panel);

  // Keep the panel under the input, recomputed on show/resize/scroll because a
  // fixed element does not move with its logical parent.
  function place() {
    const r = input.getBoundingClientRect();
    panel.style.top = (r.bottom + 8) + 'px';
    // Prefer right-aligning to the bar, but never off the left edge.
    const width = Math.min(520, window.innerWidth * 0.92);
    panel.style.left = Math.max(8, Math.min(r.right - width, window.innerWidth - width - 8)) + 'px';
    panel.style.width = width + 'px';
  }

  const show = (html) => { panel.innerHTML = html; panel.hidden = false; place(); };
  const hide = () => { panel.hidden = true; };
  window.addEventListener('resize', () => { if (!panel.hidden) place(); });
  window.addEventListener('scroll', () => { if (!panel.hidden) place(); }, true);

  async function search() {
    const q = input.value.trim();
    if (!q) { hide(); return; }
    show('<div class="ask-note">searching…</div>');
    try {
      const r = await postJSON('/api/v1/search/nodes', { query: q, top_k: 8 });
      const hits = r.results || [];
      if (!hits.length) {
        show('<div class="ask-note">Nothing in the graph matches yet. ' +
             'Scan a directory or register a node first.</div>');
        return;
      }
      show(
        hits.map(h =>
          '<div class="ask-hit">' +
            '<span class="ask-score">' + (h.score != null ? h.score.toFixed(3) : '—') + '</span>' +
            '<span class="ask-kind">' + esc(h.kind || 'node') + '</span>' +
            '<span class="ask-label">' + esc(h.label || '') + '</span>' +
          '</div>').join('') +
        // The response is token-packed, so a truncated result set is normal and
        // must be said — a short list otherwise reads as "that is all there is".
        '<div class="ask-note">' + hits.length + ' of a ' + r.budget +
          '-token budget' + (r.truncated ? ' · more matches were cut to fit' : '') + '</div>'
      );
    } catch (e) {
      show('<div class="ask-note">search failed: ' + esc(String(e)) + '</div>');
    }
  }

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); search(); }
    if (e.key === 'Escape') { hide(); input.blur(); }
  });
  // Clearing the field with the native search "x" should clear the results too.
  input.addEventListener('search', () => { if (!input.value.trim()) hide(); });

  wrap.querySelector('#askAgent').onclick = () => {
    const q = input.value.trim();
    // The Agent reads ?q= and prefills its prompt, so the question survives the
    // hop instead of having to be retyped.
    window.location.href = '/agent' + (q ? '?q=' + encodeURIComponent(q) : '');
  };

  document.addEventListener('click', (e) => {
    // The panel is no longer a descendant of the bar, so it needs its own
    // containment check or clicking a result would dismiss the results.
    if (!wrap.contains(e.target) && !panel.contains(e.target)) hide();
  });

  // "/" to focus, the convention users already have. Ctrl+K is left alone:
  // several pages bind it to their own main input via initShortcuts.
  document.addEventListener('keydown', (e) => {
    // Check the active element as well as the event target. They are the same
    // for a real keystroke, but a synthetic event dispatched on `document` has
    // a target with no tagName, and `typing()` would then wave it through and
    // yank focus out of whatever the user was writing in.
    const inField = typing(e.target) || typing(document.activeElement);
    if (e.key === '/' && !inField && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      input.focus();
      input.select();
    }
  });

  if (document.getElementById('askBarCss')) return;
  const css = document.createElement('style');
  css.id = 'askBarCss';
  css.textContent =
    '#askBar{position:relative;display:flex;gap:6px;align-items:center;margin-left:14px}' +
    '#askBar input{width:min(30vw,280px);font:12px/1 ui-sans-serif,system-ui,sans-serif;' +
      'padding:7px 11px;border-radius:999px;border:1px solid rgba(135,157,219,.28);' +
      'background:rgba(10,14,30,.6);color:#f1f5ff;outline:none}' +
    '#askBar input::placeholder{color:#8997b8}' +
    '#askBar input:focus{border-color:#68e7ff}' +
    '#askBar button{font:10px/1 ui-monospace,SFMono-Regular,Menlo,monospace;' +
      'letter-spacing:.1em;text-transform:uppercase;padding:7px 10px;border-radius:999px;' +
      'border:1px solid rgba(135,157,219,.28);background:rgba(10,14,30,.6);' +
      'color:#8997b8;cursor:pointer}' +
    '#askBar button:hover{color:#f1f5ff;border-color:#68e7ff}' +
    // Anchored to the bar and above page content: several dashboards stack
    // panels with their own z-index, and results behind a panel are results
    // the user never sees.
    '#askPanel{position:fixed;max-height:52vh;overflow:auto;z-index:2147483647;' +
      'padding:8px;border-radius:12px;' +
      'border:1px solid rgba(135,157,219,.28);background:#0b1025;' +
      'box-shadow:0 20px 60px rgba(0,0,0,.5)}' +
    '#askPanel .ask-hit{display:flex;gap:9px;align-items:baseline;padding:7px 8px;' +
      'border-radius:8px}' +
    '#askPanel .ask-hit:hover{background:rgba(255,255,255,.05)}' +
    '#askPanel .ask-score{font:11px ui-monospace,Menlo,monospace;color:#68e7ff;flex:none}' +
    '#askPanel .ask-kind{font:9px ui-monospace,Menlo,monospace;letter-spacing:.1em;' +
      'text-transform:uppercase;color:#8997b8;flex:none}' +
    '#askPanel .ask-label{font:12px/1.45 ui-sans-serif,system-ui,sans-serif;color:#f1f5ff;' +
      'overflow-wrap:anywhere}' +
    '#askPanel .ask-note{font:11px ui-monospace,Menlo,monospace;color:#8997b8;padding:6px 8px}' +
    '@media(max-width:720px){#askBar{margin-left:0;width:100%}' +
      '#askBar input{width:100%}}';
  document.head.appendChild(css);
}

// ── Keyboard shortcuts ────────────────────────────────────────────
function initShortcuts(bindings) {
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') {
      const w = $('welcome');
      const h = $('help');
      if (w) w.classList.remove('show');
      if (h) h.classList.remove('show');
      return;
    }
    if (e.key === '?' && !e.ctrlKey && !e.metaKey && !e.altKey && !typing(e.target)) {
      e.preventDefault();
      const h = $('help');
      if (h) h.classList.toggle('show');
      const w = $('welcome');
      if (w) w.classList.remove('show');
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
      e.preventDefault();
      if (bindings.focus) bindings.focus();
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      if (bindings.run) bindings.run();
    }
    if (!typing(e.target) && (e.key === 's' || e.key === 'S') && !e.ctrlKey && !e.metaKey) {
      if (bindings.seed) {
        e.preventDefault();
        bindings.seed();
      }
    }
  });
}

// ── Exports ───────────────────────────────────────────────────────
// Published as globals so a page can use whichever helpers it doesn't
// define itself. Assigned, never declared — see the note at the top.
Object.assign(window, {
  $, S, apiUrl, esc, apiError, toast, setStatus, typing, getJSON, postJSON, hdr, live, save,
  syncConnect, loadHealth, initWelcome, initHelp, initCoach, initShortcuts,
  initProductNav, initAskBar,
});

function mountShellChrome() { initProductNav(); initAskBar(); }

// Mount the shared chrome without each page opting in — an opt-in is exactly what
// leaves the next page an island again. Agent loads this shell from <head> and
// the others from the end of <body>, so both readiness states must be handled:
// waiting unconditionally would never fire for the pages already parsed.
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', mountShellChrome);
} else {
  mountShellChrome();
}
})();
