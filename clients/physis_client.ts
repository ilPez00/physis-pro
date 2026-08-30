// physis_client.ts — thin Physis HTTP client (Phase 19, B4).
//
// Zero dependencies, plain fetch. Mirrors the MCP tool surface so an agent
// harness can call the same operations over HTTP: classify, route, extract,
// remember/recall (via /search/nodes), ask, and instance settings. The server
// is the contract; this file is deliberately ~150 lines so it stays auditable
// against the wire format it wraps.
//
//   const physis = new PhysisClient({ baseUrl: 'http://localhost:19877', token, user });
//   const r = await physis.route("CNC press seized bearing");
//   if (r.routes.length) await physis.extract(text, r.routes[0].schema);

export interface PhysisClientOptions {
  /** Base URL of the running physis-pro-web instance. */
  baseUrl: string;
  /** Bearer token for protected endpoints. */
  token?: string;
  /** Tenant id sent as `X-Physis-User`. */
  user?: string;
  /** Request timeout ms (default 30_000). */
  timeoutMs?: number;
}

export class PhysisClient {
  private base: string;
  private token?: string;
  private user?: string;
  private timeoutMs: number;

  constructor(opts: PhysisClientOptions) {
    this.base = opts.baseUrl.replace(/\/+$/, '');
    this.token = opts.token;
    this.user = opts.user;
    this.timeoutMs = opts.timeoutMs ?? 30_000;
  }

  private async req<T = any>(method: string, path: string, body?: unknown): Promise<T> {
    const headers: Record<string, string> = {};
    if (this.token) headers['Authorization'] = `Bearer ${this.token}`;
    if (this.user) headers['X-Physis-User'] = this.user;
    let payload: BodyInit | undefined;
    if (body !== undefined) {
      headers['Content-Type'] = 'application/json';
      payload = JSON.stringify(body);
    }
    const ctrl = new AbortController();
    const t = setTimeout(() => ctrl.abort(), this.timeoutMs);
    try {
      const res = await fetch(this.base + path, {
        method, headers, body: payload, signal: ctrl.signal,
      });
      const text = await res.text();
      if (!res.ok) {
        throw new Error(`${res.status} ${res.statusText}: ${text.slice(0, 200)}`);
      }
      return text ? JSON.parse(text) : undefined as T;
    } finally {
      clearTimeout(t);
    }
  }

  // ── health / settings ─────────────────────────────────────────
  health() { return this.req('GET', '/health'); }
  embedder() { return this.req('GET', '/api/v1/embedder'); }
  settings() { return this.req('GET', '/api/v1/settings'); }

  // ── semantics ─────────────────────────────────────────────────
  classify(text: string) { return this.req('POST', '/api/v1/classify', { text }); }
  ask(question: string) { return this.req('POST', '/api/v1/ask', { question }); }

  // ── routing (Phase 19 B1) ─────────────────────────────────────
  routes() { return this.req('GET', '/api/v1/routes'); }
  saveRoute(route: Record<string, unknown>) {
    return this.req('POST', '/api/v1/routes', { route });
  }
  deleteRoute(name: string) {
    return this.req('DELETE', `/api/v1/routes/${encodeURIComponent(name)}`);
  }
  /** Classify text and return the routes bound to the winning cell. */
  route(text: string) { return this.req('POST', '/api/v1/routes/match', { text }); }

  // ── structured output (Phase 19 B2) ───────────────────────────
  /** Schema-constrained slot extraction over the provider cascade. */
  extract(text: string, schema: Record<string, unknown>) {
    return this.req('POST', '/api/v1/extract', { text, schema });
  }

  // ── memory ────────────────────────────────────────────────────
  /** Semantic recall over coherence + goal labels, budget-bounded. */
  recall(query: string, tokenBudget = 600, topK = 12) {
    return this.req('POST', '/api/v1/search/nodes', {
      query,
      token_budget: tokenBudget,
      top_k: topK,
    });
  }
}