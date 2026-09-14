// Verify Codex's real MCP client without starting a model turn or paid inference.
// Run from this checkout: node scripts/physis-codex-smoke.mjs
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const root = fileURLToPath(new URL("../", import.meta.url));
const child = spawn("codex", ["app-server", "--stdio"], {
  cwd: root, stdio: ["pipe", "pipe", "pipe"],
});
const pending = new Map();
let seq = 0;
let diagnostics = "";
child.stderr.on("data", (chunk) => { diagnostics = (diagnostics + chunk).slice(-4000); });
const lines = createInterface({ input: child.stdout });
lines.on("line", (line) => {
  let message;
  try { message = JSON.parse(line); } catch { return; }
  const p = pending.get(message.id);
  if (!p) return;
  pending.delete(message.id);
  clearTimeout(p.timer);
  if (message.error) p.reject(new Error(JSON.stringify(message.error)));
  else p.resolve(message.result);
});
child.on("error", failPending);
child.on("exit", () => failPending(new Error(`Codex exited: ${diagnostics}`)));
function failPending(error) {
  for (const p of pending.values()) { clearTimeout(p.timer); p.reject(error); }
  pending.clear();
}
function request(method, params) {
  const id = ++seq;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`${method} timed out: ${diagnostics}`)); }, 45000);
    pending.set(id, { resolve, reject, timer });
    child.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
  });
}
try {
  await request("initialize", { clientInfo: { name: "physis_wiring_check", version: "1" }, capabilities: { experimentalApi: true } });
  child.stdin.write(`${JSON.stringify({ method: "initialized", params: {} })}\n`);
  const started = await request("thread/start", { cwd: root, ephemeral: true, approvalPolicy: "never", sandbox: "read-only" });
  const threadId = started.thread.id;
  const status = await request("mcpServerStatus/list", { threadId });
  const server = status.data.find((entry) => entry.name === "physis_system");
  assert.ok(server, "Codex did not load physis_system");
  const toolCount = Object.keys(server.tools).length;
  assert.equal(toolCount, 10, "Codex did not discover all workspace tools");
  const response = await request("mcpServer/tool/call", {
    threadId, server: "physis_system", tool: "physis_history",
    arguments: { query: "PHYSIS_WIRING_20260914" },
  });
  assert.ok(!response.isError, JSON.stringify(response));
  const result = JSON.parse(response.content.find((part) => part.type === "text").text);
  assert.equal(result.log, `${root}.physis/system/observations.jsonl`);
  assert.ok(result.records.some((r) => JSON.parse(r.body).actor === "opencode"), "Codex did not read the OpenCode smoke record");
  console.log(JSON.stringify({ client: "codex", server: "physis_system", tools: toolCount, log: result.log, observations: result.records.map((r) => `obs:${r.seq}`), model_turns: 0 }));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  failPending(new Error("smoke ended"));
  child.stdin.end();
  const timeout = setTimeout(() => child.kill("SIGTERM"), 1000);
  timeout.unref();
}
