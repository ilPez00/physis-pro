import { mkdir, readFile, writeFile, rename, rm } from "node:fs/promises";
import path from "node:path";
import { createHash, randomUUID } from "node:crypto";

export const revision = node => createHash("sha256").update(JSON.stringify(node)).digest("hex");

export async function hashFiles(root, files, limit = 32) {
  const hashes = {};
  const bounded = (files || []).slice(0, limit);
  for (const file of bounded) {
    const absolute = path.resolve(root, file);
    const canonical = path.relative(root, absolute);
    if (!canonical || canonical.startsWith("..") || path.isAbsolute(canonical)) throw Error(`Out of scope: ${file}`);
    try {
      const content = await readFile(absolute);
      hashes[file] = createHash("sha256").update(content).digest("hex");
    } catch (e) {
      if (e.code === "ENOENT") hashes[file] = null;
      else throw e;
    }
  }
  return hashes;
}

export function nativeTestEvidence(messages, node) {
  const rows = [];
  for (const message of messages || []) {
    for (const part of message.parts || []) {
      if (part?.type !== "tool" || part.tool !== "bash" || part.state?.status !== "completed") continue;
      const command = part.state.input?.command;
      if (typeof command !== "string" || !node.acceptance_tests.includes(command)) continue;
      const exit = part.state.metadata?.exit;
      if (!Number.isInteger(exit)) throw Error(`Native result without exit code: ${command}`);
      rows.push({command, exit, evidence_ref: `${part.sessionID}/${part.callID}`, evidence_kind: "native"});
    }
  }
  if (!rows.length) throw Error("No completed native bash results match planned acceptance commands; missing evidence cannot be accepted silently");
  const missing = node.acceptance_tests.filter(command => !rows.some(row => row.command === command));
  if (missing.length) throw Error(`Missing native results: ${missing.join(", ")}`);
  for (const command of node.acceptance_tests) {
    const latest = rows.filter(row => row.command === command).at(-1);
    if (latest.exit !== 0) throw Error(`Native acceptance failed: ${command} latest exit ${latest.exit}`);
  }
  return rows;
}

export function risk(signals, policy) {
  let sum = 0, weight = 0;
  const available = {}, missing = [];
  for (const [key, w] of Object.entries(policy.risk.weights)) {
    const v = signals[key];
    if (v == null) { missing.push(key); continue; }
    if (!Number.isFinite(v) || v < 0 || v > 1) throw Error(`Invalid signal: ${key}`);
    available[key] = v; sum += v * w; weight += w;
  }
  return {score: weight ? sum / weight : null, signals: available, missing};
}
export function validatePlan(nodes, limit) {
  if (!Array.isArray(nodes) || !nodes.length || nodes.length > limit) throw Error("Plan size outside limit");
  const ids = new Set(nodes.map(n => n.id));
  if (ids.size !== nodes.length) throw Error("Duplicate task ID");
  const visited = new Set(), active = new Set();
  for (const n of nodes) {
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(n.id) || !n.objective) throw Error("Invalid task");
    for (const k of ["reads", "writes", "depends_on", "produces", "invariants", "acceptance_tests", "assumptions"])
      if (!Array.isArray(n[k]) || n[k].some(v => typeof v !== "string")) throw Error(`Invalid ${k}`);
    if (!n.acceptance_tests.length || !Number.isFinite(n.uncertainty) || n.uncertainty < 0 || n.uncertainty > 1) throw Error("Missing acceptance or uncertainty");
    if (n.depends_on.some(id => !ids.has(id))) throw Error("Missing dependency");
  }
  function visit(id) {
    if (active.has(id)) throw Error("Dependency cycle");
    if (visited.has(id)) return;
    active.add(id); nodes.find(n => n.id === id).depends_on.forEach(visit);
    active.delete(id); visited.add(id);
  }
  ids.forEach(visit);
}
export function unavailable(error) {
  const parts = [];
  for (let e = error, depth = 0; e && depth < 4; e = e.cause, depth++) {
    parts.push(e.message, e.data?.message, e.code, e.name);
  }
  const text = parts.filter(Boolean).join(" ");
  // A candidate is skipped only when the provider refused it before any tool
  // use. That covers an unknown model AND a provider that is temporarily
  // unwilling to serve it — rate limit, quota, overload, or an unpaid plan.
  // Treating a 429 as a hard provider error used to leave the run pending for
  // a human even though the next configured model was callable.
  return /unsupported_model|model_not_found|model[^\n]{0,120}(not available|not found|does not exist)|rate[_ ]?limit|\b429\b|quota|usage limit|too many requests|overloaded|payment required/i.test(text);
}
export function candidates(role, policy, catalog, used = {}) {
  let list = policy.models[role] || [];
  if (policy.profile === "A" && role !== "architect") list = policy.models.planner;
  if (policy.profile === "B" && role !== "architect") list = policy.models.worker;
  const forbidden = new Set(policy.models.architect);
  list = list.filter(m => catalog.has(m) && (role === "architect" || !forbidden.has(m)));
  const prior = role === "critic" ? used.planner : role === "reviewer" ? used.worker : null;
  const family = policy.families[prior];
  // Unknown family never counts as demonstrated independence.
  if (family && policy.profile !== "A") list = [...list].sort((a,b) => Number(policy.families[b] && policy.families[b] !== family) - Number(policy.families[a] && policy.families[a] !== family));
  return [...new Set(list)];
}
export function route(node, signals, state, policy) {
  const measured = risk(signals, policy);
  const high = measured.score !== null && measured.score >= policy.risk.threshold;
  const unresolved = (signals.critic_disagreement ?? 0) >= 0.7 || (signals.normalized_failures ?? 0) >= 1;
  const structural = node.writes.length > 1 || unresolved || (signals.structural_coupling ?? 0) >= 0.6;
  return {...measured, unresolved, action: unresolved || high ? (state.astra && policy.profile === "E" ? "architect" : "human") : "worker",
    inspect: state.physis && ["D", "E"].includes(policy.profile) && structural};
}

// Compact checkpoints reference OpenCode child sessions; native sessions retain
// messages/tool results/tokens. This is not another conversation/event transport.
export class Store {
  constructor(root) { this.dir = path.join(root, ".physis", "opencode-orchestration"); }
  file(id) { if (!/^[\w-]{1,160}$/.test(id)) throw Error("Invalid session ID"); return path.join(this.dir, `${id}.json`); }
  async load(id) { return JSON.parse(await readFile(this.file(id), "utf8")); }
  async save(id, data) {
    await mkdir(this.dir, {recursive: true, mode: 0o700});
    const f = this.file(id), tmp = `${f}.${randomUUID()}.tmp`;
    await writeFile(tmp, JSON.stringify(data, null, 2), {mode: 0o600}); await rename(tmp, f);
  }
  async lock(id, fn) {
    await mkdir(this.dir, {recursive:true, mode:0o700});
    const lock = `${this.file(id)}.lock`;
    try { await mkdir(lock); } catch { throw Error("Run busy or interrupted lock: inspect child session before removing lock"); }
    try { return await fn(); } finally { await rm(lock, {recursive:true, force:true}); }
  }
}
function data(result) { if (result.error) throw result.error; if (!result.data) throw Error("Empty SDK response"); return result.data; }
export class Runner {
  constructor(client, policy, store, directory) { Object.assign(this, {client, policy, store, directory}); }
  async control(id, action, input = {}) {
    return this.store.lock(id, async () => {
      if (action === "start") {
        let exists = false;
        try { await this.store.load(id); exists = true; } catch (e) { if (e.code !== "ENOENT") throw e; }
        if (exists) throw Error("Existing run: use status/resume, never overwrite");
        const s = {version:1, run_id:id, goal:input.goal, profile:this.policy.profile, nodes:[], accepted:[], events:[], used:{}, counts:{}, astra:this.policy.astra, physis:this.policy.physis, pending:null, outcome:"in_progress"};
        await this.store.save(id,s); return s;
      }
      const s = await this.store.load(id);
      if (action === "status") return s;
      if (s.profile !== this.policy.profile) throw Error("Profile changed; start new session for ablation");
      if (action === "reconcile") {
        const orphan = s.pending && ["creating","created_unknown"].includes(s.pending.phase) && !s.pending.child;
        if (orphan && input.child === null && input.evidence_refs?.length && input.outcome === "aborted") { s.events.push({type:"reconcile",at:Date.now(),...input,evidence_kind:"human_reported"}); s.pending = null; await this.store.save(id,s); return s; }
        if (!s.pending || input.child !== s.pending.child || !input.evidence_refs?.length || !["aborted","completed","failed"].includes(input.outcome)) throw Error("Reconciliation requires pending child, outcome and evidence");
        const child = data(await this.client.session.get({path:{id:input.child},query:{directory:this.directory}}));
        if (child.parentID !== id) throw Error("Child belongs to another run");
        data(await this.client.session.abort({path:{id:input.child},query:{directory:this.directory}}));
        s.events.push({type:"reconcile",at:Date.now(),...input,evidence_kind:"human_reported"}); s.pending = null;
        await this.store.save(id,s); return s;
      }
      if (s.pending) throw Error(`Interrupted delegation ${s.pending.child || "not started"}: inspect native session and working tree; do not replay. Human must resolve checkpoint.`);
      if (action === "plan") { validatePlan(input.nodes,this.policy.limits.nodes); for (const id of s.accepted) { if (JSON.stringify(s.nodes.find(n=>n.id === id)) !== JSON.stringify(input.nodes.find(n=>n.id === id))) throw Error("Accepted tasks are immutable"); } s.nodes = input.nodes; s.events.push({type:"plan",at:Date.now(),nodes:input.nodes.map(n=>({id:n.id,revision:revision(n)}))}); }
      else if (action === "options") {
        // Disabling is always allowed; enabling Astra requires separate permission in plugin.
        if (typeof input.astra === "boolean") s.astra = input.astra;
        if (typeof input.physis === "boolean") s.physis = input.physis;
      } else if (action === "route") {
        const node = s.nodes.find(n => n.id === input.task_id); if (!node) throw Error("Unknown task");
        const decision = route(node,input.signals || {},s,this.policy);
        s.events.push({type:"route", task_id:node.id, revision:revision(node), at:Date.now(), ...decision, evidence_refs:input.evidence_refs || []});
      } else if (action === "accept") {
        const node = s.nodes.find(n => n.id === input.task_id); if (!node) throw Error("Unknown task");
        const last = s.events.findLast(e => e.task_id === node.id && e.revision === revision(node) && e.role === "reviewer" && e.type === "result");
        const worker = s.events.findLast(e => e.task_id === node.id && e.revision === revision(node) && e.role === "worker" && e.type === "result");
        if (!worker || !last || last.at < worker.at || last.verdict !== "accept" || !last.child) throw Error("Acceptance requires current reviewer verdict accept");
        if (node.depends_on.some(d => !s.accepted.includes(d))) throw Error("Dependencies not accepted");
        if (input.allow_missing_evidence) throw Error("Missing native evidence cannot be accepted silently");
        const workerChild = data(await this.client.session.get({path:{id:worker.child},query:{directory:this.directory}}));
        if (workerChild.parentID !== id) throw Error("Worker child belongs to another run");
        const messages = data(await this.client.session.messages({path:{id:worker.child},query:{directory:this.directory}}));
        const tests = nativeTestEvidence(messages, node);
        const current = await hashFiles(this.directory, [...node.reads, ...node.writes]);
        if (last.file_hashes && JSON.stringify(last.file_hashes) !== JSON.stringify(current)) throw Error("Task files changed after review; reviewer.inspect required before acceptance");
        if (!s.accepted.includes(node.id)) s.accepted.push(node.id);
        s.events.push({type:"accept", at:Date.now(), task_id:node.id, tests, reviewer_ref:last.child, evidence_kind:"native"});
      } else if (action === "finish") {
        if (!s.nodes.length || s.accepted.length !== s.nodes.length) throw Error("Unaccepted tasks remain"); s.outcome = "success";
      } else throw Error("Unknown action");
      await this.store.save(id,s); return s;
    });
  }
  async delegate(id, role, taskID, packet, abortSignal) {
    return this.store.lock(id, async () => {
      const s = await this.store.load(id), p = this.policy;
      if (s.pending) throw Error("Interrupted delegation: inspect child and diff, no automatic replay");
      if (s.outcome !== "in_progress") throw Error("Run finished");
      if (!p.models[role] || role === "orchestrator") throw Error("Unknown worker role");
      if (packet.length > p.limits.packet_chars) throw Error("Packet over budget");
      if ((s.counts.total || 0) >= p.limits.delegations) throw Error("Delegation limit: human required");
      if (role === "critic" && ["A","B"].includes(s.profile)) throw Error("Critic disabled by ablation");
      if (role === "physis-inspector" && (!s.physis || !["D","E"].includes(s.profile))) throw Error("Physis disabled");
      const node = s.nodes.find(n => n.id === taskID);
      if (["worker","reviewer","architect"].includes(role) && !node) throw Error("Task required");
      if (role === "worker" && node.depends_on.some(d => !s.accepted.includes(d))) throw Error("Dependency not accepted");
      if (role === "worker") {
        if (s.accepted.includes(taskID)) throw Error("Task already accepted");
        const decision = s.events.findLast(e=>e.type === "route" && e.task_id === taskID && e.revision === revision(node));
        if (!decision || decision.action !== "worker") throw Error("Route requires resolution before worker");
        if (["C","D","E"].includes(s.profile) && !s.events.some(e=>e.role === "critic" && ["result","unavailable"].includes(e.type))) throw Error("Critic attempt required before worker");
      }
      const key = `${taskID || "plan"}:${role}`;
      const cap = role === "worker" ? 1 + p.limits.worker_repairs : role === "critic" ? 1 + p.limits.critic_repairs : role === "architect" ? p.limits.astra_calls : 3;
      if ((s.counts[key] || 0) >= cap) throw Error("Repair/depth limit: human required");
      if (role === "architect") {
        const decision = s.events.findLast(e => e.type === "route" && e.task_id === taskID && e.revision === revision(node));
        if (!s.astra || s.profile !== "E" || decision?.action !== "architect" || (s.counts.architect || 0) >= p.limits.astra_calls) throw Error("Astra disabled, not routed, or exhausted");
      }
      const providers = data(await this.client.provider.list({query:{directory:this.directory}}));
      const connected = new Set(providers.connected || []);
      const catalog = new Set(providers.all.filter(pr => !Array.isArray(providers.connected) || connected.has(pr.id)).flatMap(pr => Object.keys(pr.models).map(m => `${pr.id}/${m}`)));
      const models = candidates(role,p,catalog,s.used);
      s.counts.total = (s.counts.total || 0) + 1; s.counts[key] = (s.counts[key] || 0) + 1;
      if (role === "architect") s.counts.architect = (s.counts.architect || 0) + 1;
      for (const model of models) {
        const start = Date.now();
        // Persist before starting; crash can never cause silent worker replay.
        s.pending = {role, task_id:taskID, model, at:start, phase:"creating"}; await this.store.save(id,s);
        let child;
        try { child = data(await this.client.session.create({query:{directory:this.directory},body:{parentID:id,title:`orch:${role}:${taskID || "plan"}`}})).id; }
        catch (e) { s.pending.phase = "created_unknown"; await this.store.save(id,s); throw Error(`Child creation failed ambiguous: ${e.message} — human reconcile required`); }
        s.pending.child = child; s.pending.phase = "prompting"; await this.store.save(id,s);
        const slash = model.indexOf("/");
        const controller = new AbortController();
        const cancel = () => controller.abort();
        abortSignal?.addEventListener("abort",cancel,{once:true});
        if (abortSignal?.aborted) cancel();
        const timer = setTimeout(cancel,p.limits.timeout_ms);
        let response;
        try {
          response = data(await this.client.session.prompt({path:{id:child},query:{directory:this.directory},signal:controller.signal,
            body:{agent:`orch-${role}`,model:{providerID:model.slice(0,slash),modelID:model.slice(slash+1)},parts:[{type:"text",text:`OBJECTIVE: ${s.goal}\nTASK: ${JSON.stringify(node || null)}\nPACKET:\n${packet}`} ]}}));
          if (response.info?.error) throw response.info.error;
        } catch (e) {
          // Fall back only if provider explicitly rejected model BEFORE tool use.
          let messages;
          try { messages = data(await this.client.session.messages({path:{id:child},query:{directory:this.directory}})); } catch { messages = null; }
          const clean = Array.isArray(messages) && !messages.some(m => m.parts?.some(part => part.type === "tool"));
          const retry = unavailable(e) && clean && !controller.signal.aborted && role !== "architect";
          s.events.push({type:"error",role,task_id:taskID,model,child,at:start,latency_ms:Date.now()-start,code:unavailable(e)?"model_unavailable":"provider_or_execution_error",retry});
          if (!retry) {
            // Ambiguous failure remains pending until human reconciles actual state.
            if (clean && unavailable(e)) s.pending = null;
            await this.store.save(id,s);
            return {status:"human",child,reason:s.events.at(-1).code, pending:s.pending};
          }
          s.pending = null; await this.store.save(id,s); continue;
        } finally {
          clearTimeout(timer); abortSignal?.removeEventListener("abort",cancel);
          if (controller.signal.aborted) await this.client.session.abort({path:{id:child},query:{directory:this.directory}}).catch(()=>{});
        }
        if (!response.parts.some(part=>part.type === "text" && part.text.trim())) {
          s.events.push({type:"error",role,task_id:taskID,child,model,at:start,code:"empty_output"});
          await this.store.save(id,s); return {status:"human",child,reason:"Empty output: inspect child and reconcile; validation not performed"};
        }
        s.pending = null; s.used[role] = model;
        let reviewVerdict, missingValidation;
        if (role === "reviewer") {
          const m = response.parts.filter(part=>part.type === "text").map(part=>part.text).join("\n").match(/\{[\s\S]*\}/);
          try {
            const parsed = JSON.parse(m[0]);
            if (!Array.isArray(parsed.findings)) throw Error("reviewer JSON missing findings");
            if (!["accept","repair","human"].includes(parsed.verdict)) throw Error("reviewer JSON missing verdict");
            reviewVerdict = parsed.verdict; missingValidation = parsed.missing_validation ?? [];
          } catch (e) { reviewVerdict = "repair"; missingValidation = [e.message]; }
        }
        let fileHashes = null;
        if (["worker","reviewer"].includes(role) && node) fileHashes = await hashFiles(this.directory, [...node.reads, ...node.writes]);
        const prior = role === "critic" ? s.used.planner : role === "reviewer" ? s.used.worker : null;
        const independent = prior ? Boolean(p.families[prior] && p.families[model] && p.families[prior] !== p.families[model]) : null;
        const event = {type:"result",role,task_id:taskID,revision:node ? revision(node) : null,file_hashes:fileHashes,model,child,at:start,latency_ms:Date.now()-start,verdict:reviewVerdict ?? null,missing_validation:missingValidation ?? null,tokens:response.info?.tokens ?? null,cost:response.info?.cost ?? null,independent};
        s.events.push(event);
        let warning;
        try { await this.store.save(id,s); } catch { warning = "Checkpoint write failed: inspect child session; do not repeat execution"; }
        return {status:"completed",...event,warning,text:response.parts.filter(part=>part.type === "text").map(part=>part.text).join("\n").slice(0,p.limits.packet_chars)};
      }
      s.pending = null; s.events.push({type:"unavailable",role,task_id:taskID,at:Date.now()}); await this.store.save(id,s);
      return {status: role === "worker" || role === "architect" ? "human" : "unavailable",role,reason:"No configured callable model; validation not performed"};
    });
  }
}
