// organic.js — a living force-directed renderer on a plain 2D canvas.
//
// Self-hosted on purpose. Every other graph on this product loads cytoscape
// from unpkg, which means the graph is blank on the air-gapped installs this
// engine is sold into — the exact deployments whose whole pitch is that
// nothing leaves the machine. This file ships inside the binary.
//
// "Organic" is not decoration here. The layout is a physical simulation, so
// what you see is the graph settling under its own forces: dense
// neighbourhoods contract, weakly-tied nodes drift out, and a cluster that
// gains a member visibly re-balances. Breathing amplitude is tied to
// coherence, edge sway to weight — the motion carries the same facts as the
// static picture, which is why it is worth spending frames on.
//
//   const g = Organic.create(canvas, { onSelect, onHover });
//   g.setData({ nodes, edges, communities });
//
// Node:    { id, label, score, asserted, domain, mode, community, degree }
// Edge:    { source, target, weight, kind }
(function () {
'use strict';

// Cell colours match the design system's DOMAIN palette (physis.css) so a node
// means the same thing here as in every table and chip elsewhere.
const DOMAIN_COLORS = {
  HEAL:      [168, 230, 99],
  CONSTRUCT: [255, 196, 107],
  FABRICATE: [95, 242, 214],
  BOND:      [157, 140, 255],
  STUDY:     [255, 143, 208],
};
const NEUTRAL = [126, 147, 184];

// Distinct hues for community hulls. Deliberately low-saturation: the hull is
// context, and a hull that outshouts its nodes hides the graph it is grouping.
const COMMUNITY_HUES = [188, 265, 40, 330, 150, 210, 20, 290, 100, 350];

function rgba(c, a) { return 'rgba(' + c[0] + ',' + c[1] + ',' + c[2] + ',' + a + ')'; }

function colorOf(node) {
  return DOMAIN_COLORS[node.domain] || NEUTRAL;
}

// Deterministic pseudo-random in [0,1) from a string. Layout seeding must not
// use Math.random: an unchanged graph would land somewhere new on every
// refresh, and "did the graph change?" is the first question the picture is
// asked.
function hashUnit(str, salt) {
  let h = 2166136261 ^ (salt || 0);
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return ((h >>> 0) % 100000) / 100000;
}

function create(canvas, opts) {
  opts = opts || {};
  const ctx = canvas.getContext('2d');

  const S = {
    nodes: [], edges: [], communities: [],
    byId: new Map(),
    view: { x: 0, y: 0, k: 1 },
    hover: null, selected: null, dragging: null, panning: null,
    filter: null,          // (node) => boolean; non-matching nodes fade out
    alpha: 1,              // simulation temperature
    t: 0,                  // frame counter, drives breathing and pulses
    raf: null,
    running: false,
    width: 0, height: 0,
  };

  // ── Sizing ───────────────────────────────────────────────────────────────
  function resize() {
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    S.width = rect.width;
    S.height = rect.height;
    canvas.width = Math.max(1, Math.round(rect.width * dpr));
    canvas.height = Math.max(1, Math.round(rect.height * dpr));
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  // ── Data ─────────────────────────────────────────────────────────────────
  function setData(graph) {
    const prev = S.byId;
    S.nodes = (graph.nodes || []).map((n, i) => {
      const old = prev.get(n.id);
      // Carry position across updates: re-seeding on every poll would make the
      // graph explode and re-settle each time, which reads as "everything
      // changed" when nothing did.
      const seedA = hashUnit(n.id, 1) * Math.PI * 2;
      const seedR = 40 + hashUnit(n.id, 2) * 180;
      return {
        ...n,
        x: old ? old.x : Math.cos(seedA) * seedR,
        y: old ? old.y : Math.sin(seedA) * seedR,
        vx: 0, vy: 0,
        phase: hashUnit(n.id, 3) * Math.PI * 2,
        r: 4 + Math.min(9, Math.sqrt((n.degree || 0) + 1) * 2.4 + (n.score || 0) * 3),
        idx: i,
        fixed: old ? old.fixed : false,
      };
    });
    S.byId = new Map(S.nodes.map(n => [n.id, n]));
    S.edges = (graph.edges || []).map(e => ({
      ...e,
      a: S.byId.get(e.source),
      b: S.byId.get(e.target),
    })).filter(e => e.a && e.b);
    S.communities = graph.communities || [];
    if (S.selected && !S.byId.has(S.selected)) S.selected = null;
    S.alpha = 1;
    start();
  }

  // ── Simulation ───────────────────────────────────────────────────────────
  // Plain O(n²) repulsion. At the few hundred nodes this view draws that is
  // ~40k cheap ops per frame — a quadtree would be faster and would also be a
  // second spatial index to keep correct for no visible gain at this size.
  function step() {
    const n = S.nodes.length;
    if (!n) return;
    // Tuned so a small graph still fills a large canvas at ~1x zoom. Scaling
    // the *layout* is right and scaling the *view* is wrong: zooming a compact
    // cluster magnifies the blobs until they hide the edges, which are the
    // structure the picture exists to show.
    const REPULSION = 2600;
    const SPRING = 0.012;
    const GRAVITY = 0.0009;
    const DAMPING = 0.86;

    for (let i = 0; i < n; i++) {
      const a = S.nodes[i];
      for (let j = i + 1; j < n; j++) {
        const b = S.nodes[j];
        let dx = a.x - b.x, dy = a.y - b.y;
        let d2 = dx * dx + dy * dy;
        if (d2 < 0.01) {
          // Exactly coincident nodes have no direction to separate along;
          // nudge deterministically rather than dividing by zero.
          dx = (hashUnit(a.id, 7) - 0.5) * 0.1;
          dy = (hashUnit(b.id, 7) - 0.5) * 0.1;
          d2 = dx * dx + dy * dy + 0.01;
        }
        if (d2 > 260000) continue; // far enough to ignore
        const f = REPULSION / d2;
        const d = Math.sqrt(d2);
        const fx = (dx / d) * f, fy = (dy / d) * f;
        a.vx += fx; a.vy += fy;
        b.vx -= fx; b.vy -= fy;
      }
      // Toward the origin, so a disconnected component cannot drift to
      // infinity and take the viewport with it.
      a.vx -= a.x * GRAVITY;
      a.vy -= a.y * GRAVITY;
    }

    for (const e of S.edges) {
      const dx = e.b.x - e.a.x, dy = e.b.y - e.a.y;
      const d = Math.sqrt(dx * dx + dy * dy) || 1;
      // Stronger links sit closer: rest length shrinks with weight, so the
      // distance between two nodes on screen *means* something.
      const rest = 190 - 90 * Math.min(1, e.weight || 0.5);
      const f = (d - rest) * SPRING * (e.kind === 'holarchy' ? 1.4 : 1);
      const fx = (dx / d) * f, fy = (dy / d) * f;
      e.a.vx += fx; e.a.vy += fy;
      e.b.vx -= fx; e.b.vy -= fy;
    }

    let motion = 0;
    for (const a of S.nodes) {
      if (a.fixed || a === S.dragging) { a.vx = 0; a.vy = 0; continue; }
      a.vx *= DAMPING; a.vy *= DAMPING;
      const step = S.alpha;
      a.x += a.vx * step;
      a.y += a.vy * step;
      motion += Math.abs(a.vx) + Math.abs(a.vy);
    }
    // Cool down, but never to zero: a graph that stops entirely looks dead,
    // and the residual motion is what makes the breathing read as alive.
    S.alpha = Math.max(0.08, S.alpha * 0.985);
    return motion / n;
  }

  // ── Drawing ──────────────────────────────────────────────────────────────
  function toScreen(p) {
    return {
      x: (p.x * S.view.k) + S.view.x + S.width / 2,
      y: (p.y * S.view.k) + S.view.y + S.height / 2,
    };
  }
  function toWorld(sx, sy) {
    return {
      x: (sx - S.width / 2 - S.view.x) / S.view.k,
      y: (sy - S.height / 2 - S.view.y) / S.view.k,
    };
  }

  function visible(node) {
    return !S.filter || S.filter(node);
  }

  function neighbours(id) {
    const set = new Set();
    for (const e of S.edges) {
      if (e.a.id === id) set.add(e.b.id);
      if (e.b.id === id) set.add(e.a.id);
    }
    return set;
  }

  function drawHulls() {
    // One soft blob per community: the centroid and spread of its members,
    // drawn as a radial gradient rather than a convex hull. A hull's straight
    // edges assert a boundary the clustering does not actually have.
    const groups = new Map();
    for (const n of S.nodes) {
      if (!visible(n)) continue;
      if (!groups.has(n.community)) groups.set(n.community, []);
      groups.get(n.community).push(n);
    }
    for (const [cid, members] of groups) {
      if (members.length < 3) continue;
      let cx = 0, cy = 0;
      for (const m of members) { cx += m.x; cy += m.y; }
      cx /= members.length; cy /= members.length;
      let spread = 0;
      for (const m of members) {
        spread = Math.max(spread, Math.hypot(m.x - cx, m.y - cy));
      }
      const c = toScreen({ x: cx, y: cy });
      const r = (spread + 40) * S.view.k;
      const hue = COMMUNITY_HUES[cid % COMMUNITY_HUES.length];
      const grad = ctx.createRadialGradient(c.x, c.y, r * 0.15, c.x, c.y, r);
      grad.addColorStop(0, 'hsla(' + hue + ',70%,60%,0.10)');
      grad.addColorStop(1, 'hsla(' + hue + ',70%,60%,0)');
      ctx.fillStyle = grad;
      ctx.beginPath();
      ctx.arc(c.x, c.y, r, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  function drawEdges() {
    const hi = S.hover || S.selected;
    const near = hi ? neighbours(hi) : null;
    for (const e of S.edges) {
      if (!visible(e.a) || !visible(e.b)) continue;
      const p = toScreen(e.a), q = toScreen(e.b);
      const active = hi && (e.a.id === hi || e.b.id === hi);
      // Sway: the control point drifts perpendicular to the edge on a slow
      // sine, so links look like tissue rather than wire. Amplitude scales
      // with weight — a weak link is looser.
      const mx = (p.x + q.x) / 2, my = (p.y + q.y) / 2;
      const dx = q.x - p.x, dy = q.y - p.y;
      const len = Math.hypot(dx, dy) || 1;
      const amp = (1 - Math.min(1, e.weight || 0.5)) * 14 + 4;
      const sway = Math.sin(S.t * 0.01 + (e.a.idx + e.b.idx) * 0.6) * amp;
      const cx = mx + (-dy / len) * sway;
      const cy = my + (dx / len) * sway;

      const base = e.kind === 'holarchy' ? [95, 242, 214] : [157, 140, 255];
      let alpha = (e.kind === 'holarchy' ? 0.46 : 0.26) * (0.5 + 0.5 * (e.weight || 0.5));
      if (hi) alpha = active ? Math.min(0.9, alpha * 3) : alpha * 0.25;
      ctx.strokeStyle = rgba(base, alpha);
      ctx.lineWidth = (active ? 1.8 : 1) * Math.max(0.5, S.view.k);
      ctx.beginPath();
      ctx.moveTo(p.x, p.y);
      ctx.quadraticCurveTo(cx, cy, q.x, q.y);
      ctx.stroke();

      // Recorded structure carries a travelling pulse; resemblance does not.
      // The distinction is the point: one is a claim the system made, the
      // other is a shape the vectors happen to have.
      if (e.kind === 'holarchy') {
        const t = ((S.t * 0.004) + (e.a.idx * 0.13)) % 1;
        const u = 1 - t;
        const px = u * u * p.x + 2 * u * t * cx + t * t * q.x;
        const py = u * u * p.y + 2 * u * t * cy + t * t * q.y;
        ctx.fillStyle = rgba(base, hi && !active ? 0.15 : 0.65);
        ctx.beginPath();
        ctx.arc(px, py, 1.7 * Math.max(0.6, S.view.k), 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  function drawNodes() {
    const hi = S.hover || S.selected;
    const near = hi ? neighbours(hi) : null;
    for (const n of S.nodes) {
      const p = toScreen(n);
      const shown = visible(n);
      const dim = (hi && n.id !== hi && !near.has(n.id)) || !shown;
      // Breathing: amplitude from coherence, so a well-supported node pulses
      // visibly and a weak one barely moves. Same fact as the radius, read
      // through a different channel.
      const breathe = 1 + Math.sin(S.t * 0.03 + n.phase) * (0.04 + 0.10 * (n.score || 0));
      // Radius grows sub-linearly with zoom. Scaling it by `k` directly turned
      // a zoomed-in graph into a field of overlapping discs where the edges —
      // the actual structure — were hidden behind the nodes.
      const r = n.r * breathe * Math.pow(S.view.k, 0.55);
      const c = colorOf(n);

      // Glow
      const glow = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, r * 3.4);
      glow.addColorStop(0, rgba(c, dim ? 0.08 : 0.30));
      glow.addColorStop(1, rgba(c, 0));
      ctx.fillStyle = glow;
      ctx.beginPath();
      ctx.arc(p.x, p.y, r * 3.4, 0, Math.PI * 2);
      ctx.fill();

      // Core
      ctx.fillStyle = rgba(c, dim ? 0.25 : 0.95);
      ctx.beginPath();
      ctx.arc(p.x, p.y, r, 0, Math.PI * 2);
      ctx.fill();

      // A judged node wears its verdict as a ring: green success, rose
      // failure. Unjudged nodes wear nothing — absence of a verdict is not a
      // neutral verdict, and drawing one would invent evidence.
      if (n.asserted != null && !dim) {
        ctx.strokeStyle = n.asserted >= 0.34 ? 'rgba(168,230,99,.95)'
                        : n.asserted <= -0.34 ? 'rgba(255,109,143,.95)'
                        : 'rgba(126,147,184,.7)';
        ctx.lineWidth = 1.6;
        ctx.beginPath();
        ctx.arc(p.x, p.y, r + 2.6, 0, Math.PI * 2);
        ctx.stroke();
      }
      // A pinned cell is a human decision; mark it so the picture distinguishes
      // "the classifier thinks" from "a person said".
      if (n.pinned && !dim) {
        ctx.strokeStyle = 'rgba(255,196,107,.9)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.arc(p.x, p.y, r + 5.5, 0, Math.PI * 2);
        ctx.stroke();
      }
      if (n.id === S.selected) {
        ctx.strokeStyle = 'rgba(95,242,214,.95)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(p.x, p.y, r + 8, 0, Math.PI * 2);
        ctx.stroke();
      }
    }
  }

  function drawLabels() {
    // Labels only where they can be read: above a zoom floor, for nodes that
    // earned the space (degree or selection). A canvas of overlapping text is
    // strictly less informative than no text.
    const hi = S.hover || S.selected;
    ctx.font = '11px ui-sans-serif, system-ui, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    const placed = [];
    const candidates = S.nodes
      .filter(n => visible(n))
      .sort((a, b) => (b.degree - a.degree) || (b.score - a.score));
    for (const n of candidates) {
      const important = n.id === hi || n.id === S.selected;
      if (!important && S.view.k < 0.75) continue;
      if (!important && n.degree < 1 && S.view.k < 1.4) continue;
      const p = toScreen(n);
      if (p.x < -60 || p.y < -20 || p.x > S.width + 60 || p.y > S.height + 20) continue;
      const text = n.label.length > 34 ? n.label.slice(0, 33) + '…' : n.label;
      const w = ctx.measureText(text).width;
      const box = { x: p.x - w / 2, y: p.y + n.r * S.view.k + 4, w, h: 13 };
      // Greedy overlap rejection: the strongest node keeps its label.
      let clash = false;
      for (const b of placed) {
        if (box.x < b.x + b.w && box.x + box.w > b.x && box.y < b.y + b.h && box.y + box.h > b.y) {
          clash = true; break;
        }
      }
      if (clash && !important) continue;
      placed.push({ x: box.x, y: box.y, w: box.w, h: box.h });
      ctx.fillStyle = important ? 'rgba(232,241,255,.98)' : 'rgba(232,241,255,.62)';
      ctx.fillText(text, p.x, box.y);
    }
  }

  function draw() {
    ctx.clearRect(0, 0, S.width, S.height);
    drawHulls();
    drawEdges();
    drawNodes();
    drawLabels();
  }

  function frame() {
    S.t++;
    step();
    draw();
    S.raf = requestAnimationFrame(frame);
  }

  function start() {
    if (S.running) return;
    S.running = true;
    S.raf = requestAnimationFrame(frame);
  }

  function stop() {
    S.running = false;
    if (S.raf) cancelAnimationFrame(S.raf);
    S.raf = null;
  }

  // ── Interaction ──────────────────────────────────────────────────────────
  function pick(sx, sy) {
    const w = toWorld(sx, sy);
    let best = null, bestD = Infinity;
    for (const n of S.nodes) {
      if (!visible(n)) continue;
      const d = Math.hypot(n.x - w.x, n.y - w.y);
      const hitR = Math.max(10, n.r + 6);
      if (d < hitR && d < bestD) { best = n; bestD = d; }
    }
    return best;
  }

  function relative(e) {
    const rect = canvas.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  }

  canvas.addEventListener('mousedown', e => {
    const p = relative(e);
    const n = pick(p.x, p.y);
    if (n) {
      S.dragging = n;
      n.fixed = true;
      S.alpha = Math.max(S.alpha, 0.5);
    } else {
      S.panning = { x: p.x - S.view.x, y: p.y - S.view.y };
    }
  });

  window.addEventListener('mousemove', e => {
    const p = relative(e);
    if (S.dragging) {
      const w = toWorld(p.x, p.y);
      S.dragging.x = w.x; S.dragging.y = w.y;
      S.dragging.vx = 0; S.dragging.vy = 0;
      return;
    }
    if (S.panning) {
      S.view.x = p.x - S.panning.x;
      S.view.y = p.y - S.panning.y;
      return;
    }
    if (p.x < 0 || p.y < 0 || p.x > S.width || p.y > S.height) return;
    const n = pick(p.x, p.y);
    const id = n ? n.id : null;
    if (id !== S.hover) {
      S.hover = id;
      canvas.style.cursor = id ? 'pointer' : 'grab';
      if (opts.onHover) opts.onHover(n, p);
    }
  });

  window.addEventListener('mouseup', e => {
    if (S.dragging) {
      // Released nodes rejoin the simulation. Keeping them pinned would let a
      // few drags freeze the layout into something the forces never chose.
      S.dragging.fixed = false;
      S.dragging = null;
      S.alpha = Math.max(S.alpha, 0.4);
      return;
    }
    if (S.panning) { S.panning = null; return; }
  });

  canvas.addEventListener('click', e => {
    const p = relative(e);
    const n = pick(p.x, p.y);
    S.selected = n ? n.id : null;
    if (opts.onSelect) opts.onSelect(n);
  });

  canvas.addEventListener('wheel', e => {
    e.preventDefault();
    const p = relative(e);
    const before = toWorld(p.x, p.y);
    const factor = Math.exp(-e.deltaY * 0.0015);
    S.view.k = Math.min(4, Math.max(0.15, S.view.k * factor));
    const after = toWorld(p.x, p.y);
    // Keep the point under the cursor fixed while zooming — otherwise the
    // graph slides away from whatever you were looking at.
    S.view.x += (after.x - before.x) * S.view.k;
    S.view.y += (after.y - before.y) * S.view.k;
  }, { passive: false });

  window.addEventListener('resize', resize);
  resize();

  return {
    setData,
    resize,
    stop,
    start,
    get selected() { return S.selected; },
    select(id) {
      S.selected = id;
      const n = S.byId.get(id);
      if (opts.onSelect) opts.onSelect(n || null);
    },
    /** Center the view on one node without changing zoom. */
    focus(id) {
      const n = S.byId.get(id);
      if (!n) return;
      S.view.x = -n.x * S.view.k;
      S.view.y = -n.y * S.view.k;
    },
    /** Fit every visible node into the viewport. */
    fit() {
      const shown = S.nodes.filter(visible);
      if (!shown.length) return;
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const n of shown) {
        minX = Math.min(minX, n.x); maxX = Math.max(maxX, n.x);
        minY = Math.min(minY, n.y); maxY = Math.max(maxY, n.y);
      }
      const w = Math.max(1, maxX - minX), h = Math.max(1, maxY - minY);
      // Cap the fit zoom: a three-node graph would otherwise fill the viewport
      // at 3x, where the picture says nothing that a list would not.
      S.view.k = Math.min(1.5, Math.max(0.15, Math.min(S.width / (w + 140), S.height / (h + 140))));
      S.view.x = -((minX + maxX) / 2) * S.view.k;
      S.view.y = -((minY + maxY) / 2) * S.view.k;
    },
    /** Dim everything the predicate rejects; null clears the filter. */
    setFilter(fn) { S.filter = fn; S.alpha = Math.max(S.alpha, 0.3); },
    node(id) { return S.byId.get(id); },
    stats() {
      return {
        nodes: S.nodes.length,
        edges: S.edges.length,
        visible: S.nodes.filter(visible).length,
        zoom: S.view.k,
      };
    },
  };
}

window.Organic = { create };
})();
