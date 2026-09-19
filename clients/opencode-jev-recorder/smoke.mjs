// Drive the recorder's hook without OpenCode and check what actually landed.
//
//   node clients/opencode-jev-recorder/smoke.mjs [workspace]
//
// A plugin that records nothing and a plugin that is not installed look
// identical from inside OpenCode, so this asks `physis system history` rather
// than the plugin.
import { execFile } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import Plugin from "./plugin.js";

const run = promisify(execFile);
const BIN = process.env.PHYSIS_BIN || "physis";
const directory =
  process.argv[2] || mkdtempSync(join(tmpdir(), "physis-jev-recorder-"));

const hooks = await Plugin({ directory });
const session = "smoke-session";

const evaluation = {
  metrics: {
    correctness: { applicable: true, score: 4.2, confidence: 0.6 },
    readability: { applicable: true, score: 6.5, confidence: 0.7 },
    security: { applicable: false },
  },
  priorities: [{ metric: "correctness", severity: "high", reason: "off-by-one" }],
  comparison: [
    { metric: "correctness", previousScore: 3.0, currentScore: 4.2, delta: 1.2, direction: "improved" },
  ],
};

await hooks["tool.execute.after"](
  { tool: "jev-review_jev_review", sessionID: session, args: { task: "smoke the recorder" } },
  { output: JSON.stringify(evaluation) },
);

// A non-JEV tool must not be recorded.
await hooks["tool.execute.after"](
  { tool: "bash", sessionID: session, args: { command: "cargo test" } },
  { output: "test result: ok" },
);

await hooks.dispose();

const { stdout } = await run(BIN, [
  "system",
  "--root",
  directory,
  "history",
  "jev_review",
]);
const lines = stdout.split("\n").filter((l) => l.startsWith("obs:"));
console.log(stdout.trim());
console.log(`\nworkspace ${directory}`);
console.log(`history rows matching "jev_review": ${lines.length} (expected 1)`);
console.log(
  `score line present: ${lines.some((l) => l.includes("correctness=4.2"))}`,
);
process.exit(lines.length === 1 ? 0 : 1);
