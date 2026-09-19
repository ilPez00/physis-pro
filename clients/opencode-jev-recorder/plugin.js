// jev-recorder — persist every JEV evaluation into the Physis observation log.
//
// JEV returns a structured Evaluation (19 metrics with score/confidence/issues,
// priorities, and comparison deltas). That is a structured observation about the
// code, not a pass/fail verdict. This plugin forwards it to the same workspace
// store the rest of the harness writes to (`physis system remember`), so score
// history becomes queryable:
//
//   physis system history jev_review
//
// Install (physis-pro project config):
//   add "/home/gio/dev/physis-pro/clients/opencode-jev-recorder/plugin.js"
//   to the `plugin` array in .opencode/opencode.json, and ensure `physis` is on
//   PATH (or set PHYSIS_BIN).
//
// Configuration, all optional, read from the environment:
//   PHYSIS_BIN     path to the `physis` executable (default: physis)
//   PHYSIS_JEV_OFF set to 1 to disable recording without uninstalling
//
// What this does NOT do: it never blocks or modifies a tool call, never reads
// agent reasoning, and never converts a score into a pass/fail outcome. The
// recorded outcome stays `unverified` — JEV scored the change, it did not accept
// or reject it.

import { execFile } from "node:child_process";
import { promisify } from "node:util";

const run = promisify(execFile);

const BIN = process.env.PHYSIS_BIN || "physis";
const DISABLED = process.env.PHYSIS_JEV_OFF === "1";
const JEV_TOOL = /jev[_-]?review/i;

/** Keep the subject line readable; scores are the payload, not the prose. */
const SUMMARY_CHARS = 240;
const TEXT_CHARS = 1200;

function summarise(text, limit = SUMMARY_CHARS) {
  if (!text) return "";
  const oneLine = String(text).replace(/\s+/g, " ").trim();
  return oneLine.length > limit ? `${oneLine.slice(0, limit)}…` : oneLine;
}

function tryParse(value) {
  if (typeof value !== "string") return null;
  const start = value.indexOf("{");
  const end = value.lastIndexOf("}");
  if (start === -1 || end <= start) return null;
  try {
    const parsed = JSON.parse(value.slice(start, end + 1));
    return Array.isArray(parsed) ? parsed[0] ?? null : parsed;
  } catch {
    return null;
  }
}

/** Pull the Evaluation out of whatever shape the tool result arrives in. */
function extractEvaluation(output) {
  const candidates = [];
  if (typeof output === "string") candidates.push(output);
  if (output && typeof output === "object") {
    if (typeof output.output === "string") candidates.push(output.output);
    if (output.structuredContent)
      candidates.push(JSON.stringify(output.structuredContent));
    if (output.metadata) candidates.push(JSON.stringify(output.metadata));
    if (Array.isArray(output.content)) {
      for (const part of output.content) {
        if (part && typeof part.text === "string") candidates.push(part.text);
      }
    }
  }
  for (const candidate of candidates) {
    const parsed = tryParse(candidate);
    if (parsed && parsed.metrics) return parsed;
  }
  return null;
}

/** Compact, deterministic summary of one Evaluation. */
function describe(evaluation, task) {
  const metrics = evaluation.metrics || {};
  const scored = Object.entries(metrics)
    .filter(([, m]) => m && m.applicable && typeof m.score === "number")
    .map(([key, m]) => ({ key, score: m.score, confidence: m.confidence }));

  const scores = scored
    .sort((a, b) => a.key.localeCompare(b.key))
    .map((m) => `${m.key}=${m.score.toFixed(1)}`)
    .join(",");

  const weakest = scored
    .slice()
    .sort((a, b) => a.score - b.score)
    .slice(0, 3)
    .map((m) => `${m.key}:${m.score.toFixed(1)}`)
    .join(",");

  const priorities = (evaluation.priorities || [])
    .slice(0, 3)
    .map((p) => `${p.metric}/${p.severity}`)
    .join(",");

  const comparison = evaluation.comparison || [];
  const improved = comparison
    .filter((c) => c.direction === "improved")
    .map((c) => `${c.metric}:${c.previousScore}->${c.currentScore}`)
    .join(",");
  const regressed = comparison
    .filter((c) => c.direction === "regressed")
    .map((c) => `${c.metric}:${c.previousScore}->${c.currentScore}`)
    .join(",");

  const parts = [];
  if (task) parts.push(`task=${summarise(task, 160)}`);
  parts.push(`scores=${scores || "none"}`);
  if (weakest) parts.push(`weakest=${weakest}`);
  if (priorities) parts.push(`priorities=${priorities}`);
  if (improved) parts.push(`improved=${improved}`);
  if (regressed) parts.push(`regressed=${regressed}`);
  return summarise(parts.join(" "), TEXT_CHARS);
}

export const JevRecorder = async ({ directory }) => {
  const counts = { attempted: 0, recorded: 0, failed: 0, skipped: 0 };

  async function remember(text) {
    counts.attempted += 1;
    try {
      await run(
        BIN,
        ["system", "--root", directory, "remember", text, "--outcome", "unverified", "--actor", "opencode:jev_review"],
        { timeout: 10_000 },
      );
      counts.recorded += 1;
    } catch (e) {
      counts.failed += 1;
      if (counts.failed <= 3) {
        console.error(`[jev-recorder] remember failed: ${e.message}`);
      }
    }
  }

  return {
    "tool.execute.after": async ({ tool, sessionID, args }, output) => {
      if (DISABLED) return;
      if (!JEV_TOOL.test(String(tool || ""))) return;

      const evaluation = extractEvaluation(output);
      if (!evaluation) {
        counts.skipped += 1;
        return;
      }

      const task = args && (args.task || args.diff);
      await remember(
        `[opencode] jev_review session=${sessionID} ${describe(evaluation, task)}`,
      );
    },

    dispose: async () => {
      if (DISABLED || counts.attempted === 0) return;
      console.error(
        `[jev-recorder] ${counts.recorded}/${counts.attempted} evaluations recorded` +
          (counts.skipped ? `, ${counts.skipped} unparsed` : "") +
          (counts.failed ? `, ${counts.failed} failed` : ""),
      );
    },
  };
};

export default JevRecorder;
