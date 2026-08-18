// Physis Studio — front-end runtime. Mirrors the Physis Console shell
// (header chips, nav tabs, status pill, toast, ? overlay) so both UIs behave
// the same way; everything below talks to the physis-core studio API.

const $ = id => document.getElementById(id);
const state = { selectedCell: null, editingName: null, tab: 'classify' };

function esc(s) {
  if (s == null) return '';
  return String(s)
    .replace(/&/g, '&amp;').replace(/</g, '&lt;')
    .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// Cell keys are DOMAIN\0MODE on the wire; never show the NUL to a human.
const CELL_SEP = '\u0000';
const cellLabel = key => String(key || '').split(CELL_SEP).join(' × ');

async function api(path, body) {
  const res = await fetch(path, {
    method: body ? 'POST' : 'GET',
    headers: body ? { 'Content-Type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw new Error(res.status + ' ' + (await res.text()).slice(0, 160));
  return res.json();
}

let toastTimer;
function toast(msg, isErr) {
  const t = $('toast');
  t.textContent = msg;
  t.className = 'toast show ' + (isErr ? 'err' : 'ok');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.className = 'toast'; }, 3200);
}

function setStatus(msg, isErr) {
  const ts = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  $('status').innerHTML = '<span class="' + (isErr ? 'err' : 'muted') + '">' + (isErr ? '⚠ ' : '') + esc(msg) + ' · ' + ts + '</span>';
}

// Any handler can be wrapped: failures land in the status pill and a toast
// instead of dying silently in the console.
function guard(label, fn) {
  return async (...args) => {
    try {
      await fn(...args);
    } catch (e) {
      setStatus(label + ' failed: ' + e.message, true);
      toast(label + ' failed', true);
    }
  };
}

function typing() {
  const tag = document.activeElement && document.activeElement.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}

/* ── tabs ─────────────────────────────────────────────────────── */
const TABS = ['classify', 'grid', 'ontology', 'corpus', 'discover', 'quality'];

function showTab(name) {
  state.tab = name;
  document.querySelectorAll('nav button[data-tab]').forEach(b => {
    b.classList.toggle('active', b.dataset.tab === name);
  });
  TABS.forEach(t => $('tab-' + t).classList.toggle('active', t === name));
  // Tabs are lazy: the grid and corpus calls are the expensive ones, so they
  // only run when their tab is actually shown.
  if (name === 'grid') loadGrid();
  if (name === 'ontology') loadOntology();
  if (name === 'corpus') { loadSnapshot(); loadNodes(); }
  if (name === 'quality') loadQuality();
}
document.querySelectorAll('nav button[data-tab]').forEach(b => {
  b.onclick = () => showTab(b.dataset.tab);
});

/* ── header / health ──────────────────────────────────────────── */
async function loadHealth() {
  try {
    const h = await api('/api/health');
    const emb = $('chEmbedder');
    emb.innerHTML = 'embedder <b>' + esc(h.embedder) + '</b>';
    emb.className = 'chip' + (h.semantic ? ' good' : '');
    emb.title = (h.semantic ? 'semantic ONNX model' : 'deterministic fallback — pass --model for real semantics')
      + ' · ' + h.dimension + 'd';
    $('chEntries').innerHTML = 'entries <b>' + h.entries + '</b>' + (h.custom ? ' <span class="muted">+' + h.custom + '</span>' : '');
    $('chCells').innerHTML = 'cells <b>' + h.cells + '</b>';
    $('chNodes').innerHTML = 'corpus <b>' + h.nodes + '</b>';
    $('chCoherence').innerHTML = 'coherence <b>' + h.coherence_index.toFixed(2) + '</b>';
    $('chFailures').innerHTML = 'failures <b>' + h.failures + '</b>';
    $('helpPaths').innerHTML = 'ontology edits, corpus nodes and quality history are JSON files under <code>'
      + esc(h.data_dir) + '</code>';
    setStatus('engine ready · ' + h.cells + ' populated cells');
  } catch (e) {
    setStatus('engine unreachable: ' + e.message, true);
  }
}

/* ── classify ─────────────────────────────────────────────────── */
const SAMPLES = [
  'first layer adhesion failed on the nozzle after the filament change',
  'invoice reconciliation complete for March, two line items disputed',
  'monitor vibration on the press during startup and log the peaks',
  'drafted the onboarding guide for the new maintenance technician',
  'the pump seal was replaced during the planned shutdown window',
];

const doClassify = guard('classify', async () => {
  const text = $('classifyText').value.trim();
  if (!text) { toast('enter some text first', true); return; }
  const r = await api('/api/classify', { text });
  const best = r.best_entry;
  $('classifyMeta').textContent = r.query_embedding_dim + 'd vector'
    + (best ? ' · nearest entry ' + best.similarity.toFixed(3) + ' (' + best.domain + '×' + best.mode + ')' : '');

  const wrap = $('results');
  wrap.innerHTML = '';
  if (!r.results.length) {
    wrap.innerHTML = '<div class="muted">No populated cells scored — add ontology entries first.</div>';
  }
  const top = r.results.length ? r.results[0].adjusted_score : 1;
  r.results.forEach((res, i) => {
    const pct = Math.max(2, Math.round((res.adjusted_score / (top || 1)) * 100));
    const moved = Math.abs(res.adjusted_score - res.raw_score) > 0.001;
    const div = document.createElement('div');
    div.className = 'res' + (i === 0 ? ' top' : '');
    div.innerHTML =
      '<div class="head">'
      + '<span class="cell">' + esc(res.domain) + ' × ' + esc(res.mode) + '</span>'
      + (moved ? '<span class="tag" title="raw score before quality weighting">raw ' + res.raw_score.toFixed(3) + '</span>' : '')
      + '<span class="score">' + res.adjusted_score.toFixed(3) + '</span>'
      + '<button class="small good" title="right — boost this cell">✓</button>'
      + '<button class="small bad" title="wrong — penalize this cell">✕</button>'
      + '</div>'
      + '<div class="bar"><i style="width:' + pct + '%"></i></div>'
      + '<div class="meta">'
      + res.entries.map(e => '<span class="tag click" data-entry="' + esc(e) + '">' + esc(e) + '</span>').join('')
      + res.facets.map(f => '<span class="tag facet">' + esc(f) + '</span>').join('')
      + '</div>';
    const [good, bad] = div.querySelectorAll('button');
    const key = res.domain + CELL_SEP + res.mode;
    good.onclick = guard('boost', async () => {
      await api('/api/quality/pass', { cell: key });
      toast('boosted ' + res.domain + ' × ' + res.mode);
      loadHealth(); doClassify();
    });
    bad.onclick = guard('penalize', async () => {
      await api('/api/quality/fail', { feedback: 'wrong cell for: ' + text.slice(0, 120) });
      toast('penalized — re-scoring');
      loadHealth(); doClassify();
    });
    div.querySelectorAll('[data-entry]').forEach(t => {
      t.onclick = () => { showTab('ontology'); openEdit(t.dataset.entry); };
    });
    wrap.appendChild(div);
  });

  const rec = $('recall');
  rec.innerHTML = r.recall.length ? '' : '<div class="muted">No corpus nodes yet — scan a directory in the Corpus tab.</div>';
  r.recall.forEach(h => rec.appendChild(nodeItem(h, true)));
  setStatus('classified ' + text.length + ' chars against ' + r.results.length + ' cells');
});

$('btnClassify').onclick = doClassify;
$('btnClear').onclick = () => { $('classifyText').value = ''; $('results').innerHTML = ''; $('classifyMeta').textContent = ''; };
$('btnSample').onclick = loadSample;
function loadSample() {
  $('classifyText').value = SAMPLES[Math.floor(Math.random() * SAMPLES.length)];
  showTab('classify');
  doClassify();
}

/* ── grid ─────────────────────────────────────────────────────── */
let gridCache = null;

// The axes come from the ontology, not from a fixed 5×14 block: any domain or
// mode a config, an edited entry or a promoted proposal introduced gets its own
// row/column, flagged as an extra.
const loadGrid = guard('grid', async () => {
  const g = await api('/api/grid');
  gridCache = g;
  const axisLabel = a => '<span class="' + (a.extra ? 'extra' : '') + '" title="'
    + esc(a.name) + ' · ' + a.total + ' entries' + (a.extra ? ' · beyond the built-in axes' : '') + '">'
    + esc(a.name) + (a.extra ? ' ✦' : '') + '</span>';

  let html = '<table class="grid"><tr><th></th>' + g.modes.map(m => '<th>' + axisLabel(m) + '</th>').join('') + '</tr>';
  g.domains.forEach((d, di) => {
    html += '<tr><td class="rowlabel">' + axisLabel(d) + '</td>';
    g.modes.forEach((m, mi) => {
      const cell = g.cells[di * g.modes.length + mi];
      html += '<td class="cell" style="background:' + colorFor(cell.count) + ';color:' + (cell.count > 12 ? '#06121c' : '#f1f5ff')
        + '" data-domain="' + esc(d.name) + '" data-mode="' + esc(m.name) + '">' + cell.count + '</td>';
    });
    html += '</tr>';
  });
  $('gridwrap').innerHTML = html + '</table>';
  $('gridwrap').querySelectorAll('td.cell').forEach(td => {
    td.onclick = () => selectCell(td.dataset.domain, td.dataset.mode);
  });
  const extras = g.domains.filter(a => a.extra).length + g.modes.filter(a => a.extra).length;
  $('gridMeta').textContent = g.domains.length + ' domains × ' + g.modes.length + ' modes · '
    + g.cells.filter(c => c.count).length + ' populated'
    + (extras ? ' · ' + extras + ' axis(es) ✦ beyond the built-in grid' : '');
  if (state.selectedCell) selectCell(state.selectedCell.domain, state.selectedCell.mode);
});

function colorFor(n) {
  if (n === 0) return '#101a33';
  if (n <= 2) return '#153055';
  if (n <= 6) return '#1d5285';
  if (n <= 12) return '#2f86c7';
  return '#68e7ff';
}

function selectCell(d, m) {
  state.selectedCell = { domain: d, mode: m };
  $('gridwrap').querySelectorAll('td.cell').forEach(td => {
    td.classList.toggle('sel', td.dataset.domain === d && td.dataset.mode === m);
  });
  const cell = gridCache && gridCache.cells.find(c => c.domain === d && c.mode === m);
  $('cellTitle').textContent = d + ' × ' + m + ' · ' + (cell ? cell.entries.length : 0) + ' entries';
  const box = $('cellentries');
  if (!cell || !cell.entries.length) {
    box.innerHTML = '<div class="muted">Empty cell — a blind spot. Nothing here can ever be classified as '
      + esc(d) + ' × ' + esc(m) + '.</div>';
    return;
  }
  box.innerHTML = cell.entries.map(e => '<span class="tag click" data-entry="' + esc(e) + '">' + esc(e) + '</span>').join(' ');
  box.querySelectorAll('[data-entry]').forEach(t => { t.onclick = () => openEdit(t.dataset.entry); });
}

$('btnAdd').onclick = () => {
  const c = state.selectedCell || { domain: 'STUDY', mode: 'WORK' };
  openEdit(null, c.domain, c.mode);
};
$('btnNew').onclick = () => openEdit(null);

/* ── ontology browser ─────────────────────────────────────────── */
const loadOntology = guard('ontology', async () => {
  const o = await api('/api/ontology');
  $('ontMeta').textContent = o.entry_count + ' entries · ' + o.custom_count + ' edited here';
  const q = $('search').value.toLowerCase();
  const wrap = $('cats');
  wrap.innerHTML = '';
  let shown = 0;
  for (const cat of o.categories) {
    const entries = cat.entries.filter(e => !q
      || e.name.toLowerCase().includes(q)
      || (e.domain + '×' + e.mode).toLowerCase().includes(q)
      || e.hints.some(h => h.toLowerCase().includes(q)));
    if (!entries.length) continue;
    shown += entries.length;
    const div = document.createElement('div');
    div.className = 'cat';
    div.innerHTML = '<div class="name">' + esc(cat.name) + ' · ' + entries.length + '</div>';
    for (const e of entries) {
      const row = document.createElement('div');
      row.className = 'entryrow';
      row.innerHTML = '<span>' + esc(e.name) + (e.custom ? ' <span class="tag">edited</span>' : '') + '</span>'
        + '<span class="tag">' + esc(e.domain) + '×' + esc(e.mode) + '</span>';
      row.title = e.hints.join(', ');
      row.onclick = () => openEdit(e.name);
      div.appendChild(row);
    }
    wrap.appendChild(div);
  }
  if (!shown) wrap.innerHTML = '<div class="muted">No entries match “' + esc(q) + '”.</div>';
});
$('search').addEventListener('input', loadOntology);

/* ── corpus ───────────────────────────────────────────────────── */
function tile(v, l) {
  return '<div class="tile"><div class="v">' + v + '</div><div class="l">' + l + '</div></div>';
}

function renderSnapshot(s) {
  $('snapTiles').innerHTML =
    tile(s.total_nodes, 'nodes')
    + tile(s.coherence_index.toFixed(3), 'coherence index')
    + tile(s.high_coherence + ' / ' + s.mid_coherence + ' / ' + s.low_coherence, 'high / mid / low')
    + tile(s.asserted_success + ' / ' + s.asserted_inert + ' / ' + s.asserted_failure, 'worked / inert / failed')
    + tile(s.asserted_index === null || s.asserted_index === undefined ? '—' : s.asserted_index.toFixed(2), 'asserted index')
    + tile(s.dream_cycle_count, 'dreams');
}

const loadSnapshot = guard('snapshot', async () => renderSnapshot(await api('/api/core/snapshot')));

// One node row, reused by recall, search and the judge queue.
function nodeItem(n, compact) {
  const div = document.createElement('div');
  const cls = n.asserted === null || n.asserted === undefined ? 'warn' : (n.asserted > 0 ? 'good' : (n.asserted < 0 ? 'bad' : ''));
  div.className = 'item ' + cls;
  div.innerHTML = '<div class="l1">'
    + '<span class="s">' + n.score.toFixed(3) + '</span>'
    + '<span class="tag">density ' + n.coherence.toFixed(2) + '</span>'
    + (n.asserted === null || n.asserted === undefined
      ? '<span class="tag">unjudged</span>'
      : '<span class="tag">' + (n.asserted > 0 ? 'worked' : n.asserted < 0 ? 'failed' : 'inert') + '</span>')
    + '</div><div class="txt">' + esc(n.label) + '</div>';
  if (!compact) {
    const row = document.createElement('div');
    row.className = 'row tight';
    row.style.marginTop = '7px';
    [['success', '✓ worked', 'good'], ['inert', '– inert', ''], ['failure', '✕ failed', 'bad']].forEach(([v, label, kind]) => {
      const b = document.createElement('button');
      b.className = 'small ' + kind;
      b.textContent = label;
      b.onclick = guard('assert', async () => {
        const r = await api('/api/core/assert', { id: n.id, verdict: v });
        renderSnapshot(r.snapshot);
        toast('recorded: ' + label.slice(2));
        loadNodes(); loadHealth();
      });
      row.appendChild(b);
    });
    div.appendChild(row);
  }
  return div;
}

const loadNodes = guard('nodes', async () => {
  const r = await api('/api/core/nodes?filter=' + encodeURIComponent($('nodeFilter').value) + '&max=30');
  const box = $('nodeList');
  box.innerHTML = r.nodes.length ? '' : '<div class="muted">Nothing in this queue.</div>';
  r.nodes.forEach(n => box.appendChild(nodeItem(n, false)));
});
$('nodeFilter').onchange = loadNodes;
$('btnReloadNodes').onclick = loadNodes;

$('btnScan').onclick = guard('scan', async () => {
  const dir = $('scandir').value.trim();
  if (!dir) { toast('enter a directory path', true); return; }
  const btn = $('btnScan');
  btn.disabled = true; btn.textContent = 'Scanning…';
  try {
    const r = await api('/api/core/scan', { dir });
    $('scanStats').textContent = r.files + ' text files · ' + r.registered + ' passages registered'
      + (r.skipped ? ' · ' + r.skipped + ' file(s) skipped (empty or unreadable)' : '');
    renderSnapshot(r.snapshot);
    loadNodes(); loadHealth();
    toast('scanned ' + r.registered + ' documents');
  } finally {
    btn.disabled = false; btn.textContent = 'Scan';
  }
});

const corpusSearch = guard('search', async () => {
  const query = $('corpusQuery').value.trim();
  if (!query) { toast('enter a query', true); return; }
  const r = await api('/api/core/search', { query, max: 12 });
  const box = $('corpusHits');
  box.innerHTML = r.hits.length ? '' : '<div class="muted">No labelled nodes yet — scan a directory first.</div>';
  r.hits.forEach(h => box.appendChild(nodeItem(h, true)));
});
$('btnCorpusSearch').onclick = corpusSearch;
$('corpusQuery').addEventListener('keydown', e => { if (e.key === 'Enter') corpusSearch(); });

$('btnDream').onclick = guard('dream', async () => {
  const r = await api('/api/core/dream', {});
  renderSnapshot(r.snapshot);
  const box = $('dreamOut');
  if (!r.dreams.length) {
    box.innerHTML = '<div class="muted">Nothing to replay — every weak or failed node has already been dreamt.</div>';
  } else {
    box.innerHTML = '<div class="muted" style="margin-bottom:6px">' + r.dreams.length + ' replay(s)</div>'
      + r.dreams.map(d => '<div class="item ' + (d.prevented_failure ? 'bad' : 'good') + '">'
        + '<div class="l1"><span class="s">' + d.outcome.toFixed(1) + '</span>'
        + '<span class="tag">' + (d.prevented_failure ? 'prevented failure' : 'held up') + '</span>'
        + '<span class="tag">Δ ' + d.coherence_delta.toFixed(2) + '</span></div></div>').join('');
  }
  toast(r.dreams.length + ' node(s) replayed');
  loadHealth();
});

/* ── discover ─────────────────────────────────────────────────── */
$('btnIngest').onclick = guard('gap scan', async () => {
  const dir = $('ingestdir').value.trim();
  if (!dir) { toast('enter a directory path', true); return; }
  const btn = $('btnIngest');
  btn.disabled = true; btn.textContent = 'Scanning…';
  try {
    const r = await api('/api/ingest', { dir });
    const rep = r.report;
    $('ingestTiles').innerHTML =
      tile(r.files, 'files')
      + tile(r.lines, 'lines')
      + tile(rep.covered + ' / ' + rep.total, 'covered')
      + tile(rep.unmapped, 'unmapped')
      + tile(rep.threshold_used.toFixed(2) + (rep.auto_retuned ? '*' : ''), 'threshold' + (rep.auto_retuned ? ' (retuned)' : ''))
      + tile(rep.sim_mean.toFixed(2), 'mean similarity');
    const box = $('proposals');
    box.innerHTML = '';
    if (!rep.proposals.length) {
      box.innerHTML = '<div class="muted">No gaps — this corpus is already covered by the ontology.</div>';
    }
    rep.proposals.forEach(p => {
      const d = document.createElement('div');
      d.className = 'item warn';
      d.innerHTML = '<div class="l1"><b>' + esc(p.name) + '</b>'
        + '<span class="tag">' + esc(p.domain) + '×' + esc(p.mode) + '</span>'
        + '<span class="tag">' + p.count + ' texts</span>'
        + '<span class="tag">cov ' + p.coverage.toFixed(2) + '</span></div>'
        + '<div class="txt">hints: ' + esc(p.hints.join(', ')) + '</div>'
        + '<div class="txt">' + esc(p.samples.slice(0, 2).join(' | ')) + '</div>';
      const b = document.createElement('button');
      b.className = 'small';
      b.style.marginTop = '7px';
      b.textContent = 'Promote to entry';
      b.onclick = guard('promote', async () => {
        await api('/api/ingest/promote', {
          name: p.name, category: 'Discovered', domain: p.domain, mode: p.mode,
          hints: p.hints, samples: p.samples,
        });
        toast('promoted ' + p.name + ' — classifier rebuilt');
        d.remove();
        loadHealth();
      });
      d.appendChild(b);
      box.appendChild(d);
    });
    setStatus('gap scan: ' + rep.unmapped + ' unmapped of ' + rep.total);
  } finally {
    btn.disabled = false; btn.textContent = 'Find gaps';
  }
});

/* ── quality ──────────────────────────────────────────────────── */
const loadQuality = guard('quality', async () => {
  const q = await api('/api/quality');
  const weights = $('weights');
  const rows = [];
  for (const [cell, p] of Object.entries(q.penalties)) rows.push([cell, -p]);
  for (const [cell, b] of Object.entries(q.boosts)) rows.push([cell, b]);
  rows.sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]));
  weights.innerHTML = rows.length ? '' : '<div class="muted">No cell has been judged yet.</div>';
  rows.forEach(([cell, w]) => {
    const div = document.createElement('div');
    div.className = 'item ' + (w < 0 ? 'bad' : 'good');
    div.innerHTML = '<div class="l1"><span class="s">' + (w > 0 ? '+' : '') + w.toFixed(2) + '</span>'
      + '<span class="cell">' + esc(cellLabel(cell)) + '</span></div>';
    const b = document.createElement('button');
    b.className = 'small good';
    b.textContent = '✓ it was right';
    b.style.marginTop = '7px';
    b.onclick = guard('boost', async () => {
      await api('/api/quality/pass', { cell });
      toast('boosted ' + cellLabel(cell));
      loadQuality(); loadHealth();
    });
    div.appendChild(b);
    weights.appendChild(div);
  });

  const list = $('qlist');
  list.innerHTML = q.failures.length ? '' : '<div class="muted">No failures recorded.</div>';
  q.failures.forEach(f => {
    const div = document.createElement('div');
    div.className = 'item bad';
    div.innerHTML = '<div class="l1"><span class="cell">' + esc(cellLabel(f.top_cell)) + '</span>'
      + '<span class="tag">sev ' + f.severity.toFixed(1) + '</span>'
      + '<span class="tag">' + esc(f.timestamp.slice(5, 16).replace('T', ' ')) + '</span></div>'
      + '<div class="txt">' + esc(f.feedback) + '</div>';
    list.appendChild(div);
  });
});

$('btnFail').onclick = guard('record failure', async () => {
  const text = $('failtext').value.trim();
  if (!text) { toast('describe what went wrong', true); return; }
  const r = await api('/api/quality/fail', { feedback: text });
  $('failtext').value = '';
  toast('recorded → ' + cellLabel(r.top_cell));
  loadQuality(); loadHealth();
});

/* ── entry editor ─────────────────────────────────────────────── */
// Collect the facet controls into a Facets object. Empty selections are
// OMITTED rather than sent as "": the Rust side parses each facet as an enum,
// and "" is not a variant — sending it would 422 the whole entry.
function facetsFromForm() {
  const f = {};
  const put = (key, id) => { const v = $(id).value.trim(); if (v) f[key] = v; };
  put('lifecycle', 'mLifecycle'); put('agency', 'mAgency');
  put('scale', 'mScale'); put('abstraction', 'mAbstraction');
  put('sub_domain', 'mSubDomain'); put('sub_mode', 'mSubMode');
  return f;
}

// Fallback axes for the very first paint, before /api/grid has answered. The
// live lists come from the ontology — see loadGrid — and the editor takes free
// text, so a genuinely new domain or mode can be introduced right here.
const DOMAINS = ['HEAL', 'CONSTRUCT', 'FABRICATE', 'BOND', 'STUDY'];
const MODES = ['LIFT', 'REST', 'WALK', 'WORK', 'CREATE', 'LEARN', 'DESTROY', 'SENSE', 'GUIDE', 'PLAY', 'BRAINSTORM', 'MAINTAIN', 'MOVE', 'PLAN'];

function axisNames(kind) {
  const fallback = kind === 'domains' ? DOMAINS : MODES;
  if (!gridCache) return fallback;
  return gridCache[kind].map(a => a.name);
}

// Keep the editor's suggestions honest even if the Grid tab was never opened.
function ensureAxes() {
  if (!gridCache) loadGrid();
}

function datalist(id, values) {
  return '<datalist id="' + id + '">' + values.map(v => '<option value="' + esc(v) + '">').join('') + '</datalist>';
}

function openEdit(name, domain, mode) {
  ensureAxes();
  state.editingName = name;
  const d = domain || 'STUDY';
  const m = mode || 'WORK';
  $('modal').innerHTML = `<h3>${name ? 'Edit entry' : 'New entry'}</h3>
    <p class="muted" style="margin:0 0 10px;font-size:12px">Hints are what the entry is embedded from — write the words a real document would use.</p>
    <label>Name</label><input type="text" id="mName" value="${esc(name || '')}" placeholder="e.g. invoice line check">
    <label>Category</label><input type="text" id="mCat" placeholder="e.g. Finance">
    ${datalist('dlDomains', axisNames('domains'))}${datalist('dlModes', axisNames('modes'))}
    <div class="g2">
      <div><label>Domain</label><input type="text" id="mDomain" list="dlDomains" value="${esc(d)}" placeholder="HEAL · CONSTRUCT · or a new one"></div>
      <div><label>Mode</label><input type="text" id="mMode" list="dlModes" value="${esc(m)}" placeholder="WORK · MAINTAIN · or a new one"></div>
    </div>
    <p class="muted" style="margin:0;font-size:11px">Type a domain or mode that doesn't exist yet and the grid grows a row or column for it.</p>
    <div class="g2">
      <div><label>Axis kind</label><input type="text" id="mAxisKind" placeholder="e.g. epistemic"></div>
      <div><label>Axis name</label><input type="text" id="mAxisName" placeholder="e.g. formal"></div>
    </div>
    <div class="g2">
      <div><label>Unit</label><input type="text" id="mUnit" placeholder="e.g. checks"></div>
      <div><label>Hints (comma separated)</label><input type="text" id="mHints" placeholder="reconcile, line item, payment"></div>
    </div>

    <label style="margin-top:14px">Facets · the dimensions the 2-D grid cannot hold</label>
    <!-- option values are the serde variant names of physis-core's facet
         enums; the label may read differently (Self → SelfActor) but the
         value must match or the entry fails to deserialize. -->
    <div class="g2">
      <div><label>Lifecycle</label><select id="mLifecycle">
        <option value="">—</option>
        <option value="Design">Design</option>
        <option value="Build">Build</option>
        <option value="Operate">Operate</option>
        <option value="Retire">Retire</option>
      </select></div>
      <div><label>Agency</label><select id="mAgency">
        <option value="">—</option>
        <option value="SelfActor">Self</option>
        <option value="Other">Other</option>
        <option value="Automated">Automated</option>
        <option value="Collective">Collective</option>
      </select></div>
    </div>
    <div class="g2">
      <div><label>Scale</label><select id="mScale">
        <option value="">—</option>
        <option value="Personal">Personal</option>
        <option value="Interpersonal">Interpersonal</option>
        <option value="Organizational">Organizational</option>
        <option value="Civil">Civil</option>
      </select></div>
      <div><label>Abstraction</label><select id="mAbstraction">
        <option value="">—</option>
        <option value="Concrete">Concrete</option>
        <option value="Abstract">Abstract</option>
      </select></div>
    </div>
    <div class="g2">
      <div><label>Sub-domain</label><input type="text" id="mSubDomain" placeholder="e.g. Repair"></div>
      <div><label>Sub-mode</label><input type="text" id="mSubMode" placeholder="e.g. Preventive"></div>
    </div>

    <div class="row" style="margin-top:16px">
      <button class="primary" id="mSave">Save</button>
      ${name ? '<button class="bad" id="mDel">Delete</button>' : ''}
      <button class="ghost" id="mCancel">Cancel</button>
    </div>`;
  $('modalBg').classList.add('show');
  $('mName').focus();

  if (name) {
    // Prefill from the ontology, facets included — a save writes the whole
    // DomainDef, so an unfilled control would silently blank a stored facet.
    api('/api/ontology').then(o => {
      for (const cat of o.categories) {
        for (const e of cat.entries) {
          if (e.name !== name) continue;
          $('mCat').value = e.category || '';
          $('mDomain').value = e.domain;
          $('mMode').value = e.mode;
          $('mAxisKind').value = e.axis_kind;
          $('mAxisName').value = e.axis_name;
          $('mUnit').value = e.unit;
          $('mHints').value = e.hints.join(', ');
          const f = e.facets || {};
          $('mLifecycle').value = f.lifecycle || '';
          $('mAgency').value = f.agency || '';
          $('mScale').value = f.scale || '';
          $('mAbstraction').value = f.abstraction || '';
          $('mSubDomain').value = f.sub_domain || '';
          $('mSubMode').value = f.sub_mode || '';
        }
      }
    });
  }

  const close = () => { $('modalBg').classList.remove('show'); state.editingName = null; };
  $('mCancel').onclick = close;
  $('mSave').onclick = guard('save', async () => {
    // Every upsert re-embeds the whole ontology, which takes about a second —
    // without this the modal just sits there looking ignored.
    const btn = $('mSave');
    btn.disabled = true;
    btn.textContent = 'Rebuilding classifier…';
    try {
      await api('/api/ontology/upsert', {
        name: $('mName').value.trim(), category: $('mCat').value.trim(),
        // Axes are case-sensitive cell keys; normalizing here stops "exchange"
        // and "EXCHANGE" becoming two rows of the same grid.
        domain: $('mDomain').value.trim().toUpperCase(), mode: $('mMode').value.trim().toUpperCase(),
        axis_kind: $('mAxisKind').value, axis_name: $('mAxisName').value,
        unit: $('mUnit').value, hints: $('mHints').value,
        facets: facetsFromForm(),
      });
    } finally {
      btn.disabled = false;
      btn.textContent = 'Save';
    }
    close();
    toast('saved — classifier rebuilt');
    refreshAll();
  });
  if (name) $('mDel').onclick = guard('delete', async () => {
    await api('/api/ontology/delete', { name });
    close();
    toast('deleted — classifier rebuilt');
    refreshAll();
  });
}
$('modalBg').addEventListener('click', e => {
  if (e.target === $('modalBg')) { $('modalBg').classList.remove('show'); state.editingName = null; }
});

/* ── help + shortcuts ─────────────────────────────────────────── */
$('btnHelp').onclick = () => $('help').classList.toggle('show');
$('helpClose').onclick = () => $('help').classList.remove('show');
$('help').addEventListener('click', e => { if (e.target === $('help')) $('help').classList.remove('show'); });

document.addEventListener('keydown', e => {
  if (e.key === 'Escape') {
    $('help').classList.remove('show');
    $('modalBg').classList.remove('show');
    return;
  }
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') { e.preventDefault(); doClassify(); return; }
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    const focusTarget = {
      classify: 'classifyText', ontology: 'search', corpus: 'corpusQuery',
      discover: 'ingestdir', quality: 'failtext',
    }[state.tab];
    if (focusTarget) $(focusTarget).focus();
    return;
  }
  if (typing() || e.ctrlKey || e.metaKey || e.altKey) return;
  if (e.key === '?') { e.preventDefault(); $('help').classList.toggle('show'); return; }
  if (e.key === 's' || e.key === 'S') { e.preventDefault(); loadSample(); return; }
  const n = parseInt(e.key, 10);
  if (n >= 1 && n <= TABS.length) showTab(TABS[n - 1]);
});

/* ── boot ─────────────────────────────────────────────────────── */
function refreshAll() {
  loadHealth();
  if (state.tab === 'grid') loadGrid();
  if (state.tab === 'ontology') loadOntology();
  if (state.tab === 'quality') loadQuality();
}
loadHealth();
