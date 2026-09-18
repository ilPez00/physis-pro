import { tool } from "@opencode-ai/plugin";
import { readFile, realpath } from "node:fs/promises";
import path from "node:path";
import { Runner, Store } from "../../clients/opencode-orchestration/router.mjs";
import { agents } from "../../clients/opencode-orchestration/roles.mjs";

export const Orchestration = async ({client, directory}) => {
  let policy;
  try { policy = JSON.parse(await readFile(path.join(directory,".opencode/orchestration.json"),"utf8")); }
  catch { console.error("[orchestrate] Policy unavailable; ordinary OpenCode unchanged"); return {}; }
  const runner = new Runner(client,policy,new Store(directory),directory);
  const answer = async fn => { try { return JSON.stringify(await fn()); } catch (e) { return JSON.stringify({status:"blocked",reason:e.message || "SDK error",validation:"not performed"}); } };
  function coordinator(context) { if (context.agent !== "orchestrator") throw Error("Only orchestrator may delegate/control runs (depth=1)"); }
  return {
    config: async config => { config.agent = {...config.agent,...agents(policy)}; },
    "tool.execute.before": async ({sessionID,tool: name}, output) => {
      // Child sessions keep native permissions. Additional fence checks common
      // editing tools; arbitrary shell remains approval-gated, not sandboxed.
      if (!["edit","write","apply_patch"].includes(name)) return;
      const child = await client.session.get({path:{id:sessionID},query:{directory}});
      if (!child.data?.parentID) return;
      let run;
      try { run = await runner.store.load(child.data.parentID); }
      catch (e) { if (e.code === "ENOENT") return; throw e; }
      if (run.pending?.child !== sessionID || run.pending.role !== "worker") throw Error("Not an active orchestration worker");
      if (name === "apply_patch") throw Error("Use edit/write for task-scoped edits; patch tool is not scope-checked");
      const node = run.nodes.find(n=>n.id === run.pending.task_id);
      async function canonical(file) {
        const absolute = path.resolve(directory,file);
        try { return await realpath(absolute); }
        catch (e) {
          if (e.code !== "ENOENT") throw e;
          return path.join(await realpath(path.dirname(absolute)),path.basename(absolute));
        }
      }
      const target = output.args.filePath;
      if (typeof target !== "string") throw Error("Missing edit target");
      const allowed = await Promise.all(node.writes.map(canonical));
      if (!allowed.includes(await canonical(target))) throw Error("Edit outside TaskNode.writes");
    },
    tool: {
      orchestrate_state: tool({
        description: "Start/status/resume/checkpoint task graph, route measured signals, record tested/reviewed acceptance. Native session ID is run ID. No source edits.",
        args:{action:tool.schema.enum(["start","status","plan","options","route","accept","finish","reconcile"]),input:tool.schema.string().max(24000).describe("JSON object; start {goal}; plan {nodes}; route {task_id,signals,evidence_refs}; accept {task_id}; options {astra,physis}; reconcile {child,outcome,evidence_refs}")},
        async execute({action,input},context) { return answer(async()=>{
          coordinator(context); const parsed = JSON.parse(input);
          if (action === "accept") await context.ask({permission:"orchestration_accept",patterns:[parsed.task_id || "task"],always:[],metadata:{warning:"Confirm native bash evidence (exact planned commands, metadata.exit 0) and actual task diff before accepting; human approval cannot substitute missing native evidence",input:parsed}});
          if (action === "reconcile" || (action === "options" && parsed.astra === true)) await context.ask({permission:"orchestration_override",patterns:[action],always:[],metadata:{action,input:parsed}});
          return runner.control(context.sessionID,action,parsed);
        }); }
      }),
      orchestrate_delegate: tool({
        description: "Bounded native OpenCode child session with configured role/model, one at a time. Independent context; no recursive delegation. Only explicit unavailable-model failures may fall back. Worker timeout requires human reconciliation.",
        args:{role:tool.schema.enum(["scout","planner","critic","worker","reviewer","architect","physis-inspector"]),task_id:tool.schema.string().max(64).optional(),packet:tool.schema.string().max(policy.limits.packet_chars)},
        async execute({role,task_id,packet},context) { return answer(async()=>{
          coordinator(context);
          if (role === "architect") await context.ask({permission:"orchestration_astra",patterns:[task_id || "decision"],always:[],metadata:{task_id,packet}});
          return runner.delegate(context.sessionID,role,task_id,packet,context.abort);
        }); }
      })
    }
  };
};
