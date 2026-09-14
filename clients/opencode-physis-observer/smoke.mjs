// Drive the plugin's hooks without OpenCode and check what actually landed.
//
//   node clients/opencode-physis-observer/smoke.mjs [workspace]
//
// Prints the count it attempted and the count `physis system history` can see;
// a plugin that records nothing and one that is not installed look identical
// from inside OpenCode, so this asks the log rather than the plugin.
import { execFile } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import Plugin from "./plugin.js";

const run = promisify(execFile);
const BIN = process.env.PHYSIS_BIN || "physis";
const directory =
  process.argv[2] || mkdtempSync(join(tmpdir(), "physis-observer-"));
writeFileSync(join(directory, "pump.rs"), "// calibration\n");

const hooks = await Plugin({ directory });
const session = "smoke-session";

await hooks["tool.execute.before"]({ tool: "bash", sessionID: session, callID: "1" });
await hooks["tool.execute.after"](
  { tool: "bash", sessionID: session, callID: "1", args: { command: "cargo test" } },
  { title: "cargo test", output: "test result: ok. 14 passed" },
);
await hooks["tool.execute.before"]({ tool: "edit", sessionID: session, callID: "2" });
await hooks["tool.execute.after"](
  { tool: "edit", sessionID: session, callID: "2", args: { filePath: "pump.rs" } },
  { title: "edit pump.rs", output: "1 change" },
);
await hooks["chat.message"](
  { sessionID: session, agent: "worker" },
  { message: { parts: [{ text: "Calibrate the pump and run the suite" }] } },
);
await hooks.dispose();

const { stdout } = await run(BIN, ["system", "--root", directory, "history", "opencode"]);
const lines = stdout.split("\n").filter((l) => l.startsWith("obs:"));
console.log(stdout.trim());
console.log(`\nworkspace ${directory}`);
console.log(`history rows matching "opencode": ${lines.length} (expected 3)`);
process.exit(lines.length === 3 ? 0 : 1);
