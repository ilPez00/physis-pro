// physis-observer — record what a swarm of agents actually did, in one log.
//
// OpenCode runs several agents against the same working tree (with
// opencode-orchestrator: Commander, Planner, Worker, Reviewer). Each keeps its
// own transcript, so "has anyone already tried this?" is answerable only by
// reading transcripts — which is the expensive way to ask, and impossible
// across harnesses. This plugin appends every tool call and every finished turn
// to the workspace's Physis observation log through the supported write path
// (`physis system remember`), so the question becomes `physis system history`.
//
// Install:
//   ln -s ~/dev/physis-pro/clients/opencode-physis-observer \
//         ~/.config/opencode/plugins/physis-observer
//   # then add "physis-observer" to opencode.json `plugin` if it is not
//   # auto-discovered, and make sure `physis` is on PATH.
//
// Configuration, all optional, read from the environment:
//   PHYSIS_BIN           path to the `physis` executable (default: physis)
//   PHYSIS_OBSERVE_TOOLS comma-separated allowlist of tools to record
//                        (default: every tool)
//   PHYSIS_OBSERVE_OFF   set to 1 to disable recording without uninstalling
//
// What this does NOT do: it does not read agent reasoning, it does not judge
// outcomes, and it never blocks or modifies a tool call. A recorded outcome is
// the tool's own exit condition, not a claim that the agent's goal was met.

import { execFile } from "node:child_process";
import { promisify } from "node:util";

const run = promisify(execFile);

const BIN = process.env.PHYSIS_BIN || "physis";
const DISABLED = process.env.PHYSIS_OBSERVE_OFF === "1";
const ALLOW = (process.env.PHYSIS_OBSERVE_TOOLS || "")
  .split(",")
  .map((t) => t.trim())
  .filter(Boolean);

/** Tool output is unbounded; a log entry is a pointer, not a copy. */
const SUMMARY_CHARS = 240;

function summarise(text) {
  if (!text) return "";
  const oneLine = String(text).replace(/\s+/g, " ").trim();
  return oneLine.length > SUMMARY_CHARS
    ? `${oneLine.slice(0, SUMMARY_CHARS)}…`
    : oneLine;
}

export const PhysisObserver = async ({ directory }) => {
  // Counters, not confidence: `physis system history` shows what landed, and
  // these show what was attempted, so a silent failure is visible as a gap.
  const counts = { attempted: 0, recorded: 0, failed: 0 };
  const started = new Map();

  async function remember(text, outcome, actor) {
    counts.attempted += 1;
    try {
      await run(
        BIN,
        [
          "system",
          "--root",
          directory,
          "remember",
          text,
          "--outcome",
          outcome,
          "--actor",
          actor,
        ],
        { timeout: 10_000 },
      );
      counts.recorded += 1;
    } catch (e) {
      counts.failed += 1;
      // Never throw: an observability plugin that can break the agent loop is
      // worse than one that misses records. Report on dispose instead.
      if (counts.failed <= 3) {
        console.error(`[physis-observer] remember failed: ${e.message}`);
      }
    }
  }

  return {
    "tool.execute.before": async ({ tool, sessionID, callID }) => {
      if (DISABLED) return;
      started.set(callID, { at: Date.now(), tool, sessionID });
    },

    "tool.execute.after": async ({ tool, sessionID, callID, args }, output) => {
      if (DISABLED) return;
      if (ALLOW.length && !ALLOW.includes(tool)) return;
      const start = started.get(callID);
      started.delete(callID);
      const ms = start ? Date.now() - start.at : -1;
      // The subject line carries the fields a later query filters on; the
      // summary is what a human reads in `history`.
      const target =
        args && (args.filePath || args.path || args.command || args.pattern);
      await remember(
        `[opencode] tool=${tool} session=${sessionID} ms=${ms}` +
          (target ? ` target=${summarise(target)}` : "") +
          ` — ${summarise(output && (output.title || output.output))}`,
        "unverified",
        `opencode:${tool}`,
      );
    },

    "chat.message": async ({ sessionID, agent }, { message }) => {
      if (DISABLED) return;
      const text = summarise(
        message && message.parts
          ? message.parts.map((p) => p.text || "").join(" ")
          : "",
      );
      if (!text) return;
      await remember(
        `[opencode] turn agent=${agent || "?"} session=${sessionID} — ${text}`,
        "unverified",
        `opencode:${agent || "agent"}`,
      );
    },

    dispose: async () => {
      if (DISABLED || counts.attempted === 0) return;
      console.error(
        `[physis-observer] ${counts.recorded}/${counts.attempted} observations recorded` +
          (counts.failed ? `, ${counts.failed} failed` : ""),
      );
    },
  };
};

export default PhysisObserver;
