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

async function getJSON(p) {
  const r = await fetch(S + p, { headers: hdr() });
  if (!r.ok) throw new Error(r.status + ' ' + (await r.text()).slice(0, 180));
  return r.json();
}

async function postJSON(p, body) {
  const r = await fetch(S + p, {
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
  $, S, esc, apiError, toast, setStatus, typing, getJSON, postJSON, hdr, live, save,
  syncConnect, loadHealth, initWelcome, initHelp, initCoach, initShortcuts,
});
})();
