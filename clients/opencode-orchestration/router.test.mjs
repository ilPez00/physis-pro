import test from "node:test";
import assert from "node:assert/strict";
import {readFile, writeFile, mkdtemp, rm} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {risk,route,candidates,validatePlan,Runner,Store,nativeTestEvidence as evidenceOf,hashFiles} from "./router.mjs";
const policy = JSON.parse(await readFile(new URL("../../.opencode/orchestration.json",import.meta.url)));
const node = (id="T1",writes=["a.js"],deps=[]) => ({id,objective:"fixture",reads:[],writes,depends_on:deps,produces:writes,invariants:["valid"],acceptance_tests:["node --test"],assumptions:[],uncertainty:0.1,parallelizable:false});
function setup(t, behavior = async(o)=>({data:{info:{tokens:{input:10,output:2}},parts:[{type:"text",text:o.body.agent === "orch-reviewer" ? JSON.stringify({verdict:"accept",findings:[],missing_validation:[]}) : "checked"}]}})) {
  const records = new Map();
  const store = {load:async id=>{if(!records.has(id))throw Object.assign(Error("missing"),{code:"ENOENT"});return structuredClone(records.get(id));},save:async(id,s)=>records.set(id,structuredClone(s)),lock:async(id,fn)=>fn()};
  const p = structuredClone(policy), calls=[];
  const transcripts = new Map();
  const extra = [];
  const catalogProviders = [...new Set(Object.keys(p.families).map(k=>k.slice(0,k.indexOf("/"))))].map(id=>({id,models:Object.fromEntries(Object.keys(p.families).filter(k=>k.startsWith(id+"/")).map(k=>[k.slice(id.length+1),{}]))}));
  const client = {provider:{list:async()=>({data:{all:catalogProviders}})},session:{create:async o=>{const id=`child_${calls.length}`;transcripts.set(id,[]);return {data:{id,parentID:o.body.parentID}};},prompt:async o=>{calls.push(o);const result=await behavior(o,calls.length);const texts=(result.data?.parts||[]).filter(part=>part?.type==="text").map(part=>part.text).join("\n");transcripts.set(o.path.id,[...(transcripts.get(o.path.id)||[]),{info:{role:"assistant"},parts:[{type:"text",text:texts}]}]);return result;},messages:async o=>({data:[...(transcripts.get(o.path.id)||[]),...extra]}),get:async()=>({data:{parentID:"run"}}),abort:async()=>({data:true})}};
  return {runner:new Runner(client,p,store,"/tmp"),store,p,calls,client,transcripts,extra};
}
async function ready(f) {
  await f.runner.control("run","start",{goal:"safe fixture"});
  await f.runner.control("run","plan",{nodes:[node()]});
  await f.runner.delegate("run","critic",null,"inspect");
  await f.runner.control("run","route",{task_id:"T1",signals:{planner_uncertainty:0.1}});
}
test("risk normalizes only available metrics and rejects invented invalid values",()=>{
  assert.equal(risk({},policy).score,null);
  assert.equal(risk({planner_uncertainty:0.4,semantic_delta:null},policy).score,0.4);
  assert.throws(()=>risk({semantic_delta:NaN},policy));
});
test("isolated task skips Physis; cross-module suggests inspection",()=>{
  assert.equal(route(node(),{},policy,policy).inspect,false);
  assert.equal(route(node("T1",["a","b"]),{},policy,policy).inspect,true);
  assert.equal(route(node("T1",["a","b"]),{},{...policy,physis:false},policy).inspect,false);
});
test("disagreement/high risk escalates; Astra disabled routes human",()=>{
  assert.equal(route(node(),{critic_disagreement:0.9},policy,policy).action,"architect");
  assert.equal(route(node(),{critic_disagreement:0.9},{...policy,astra:false},policy).action,"human");
});
test("graph detects missing dependencies and cycles",()=>{
  validatePlan([node()],8);
  assert.throws(()=>validatePlan([node("T1",[],["T2"])],8));
  assert.throws(()=>validatePlan([node("T1",[],["T2"]),node("T2",[],["T1"])],8));
});
test("independent model family preferred; Astra never worker fallback",()=>{
  const p=structuredClone(policy);p.models.critic.reverse();p.models.worker.push(p.models.architect[0]);
  const catalog=new Set(Object.keys(p.families));
  assert.equal(candidates("critic",p,catalog,{planner:p.models.critic[0]})[0],p.models.critic[1]);
  assert.ok(!candidates("worker",p,catalog).includes(p.models.architect[0]));
});
test("simple worker + independent review + tests acceptance",async t=>{
  const f=setup(t); await ready(f);
  const work=await f.runner.delegate("run","worker","T1","implement");
  f.extra.push({info:{role:"assistant"},parts:[{type:"tool",tool:"bash",sessionID:work.child,messageID:"m",callID:"c",state:{status:"completed",input:{command:"node --test"},output:"pass",title:"t",metadata:{exit:0},time:{start:1,end:2}}}]});
  await assert.rejects(()=>f.runner.control("run","accept",{task_id:"T1"}));
  const review=await f.runner.delegate("run","reviewer","T1","review"); assert.equal(review.independent,true);
  const accepted=await f.runner.control("run","accept",{task_id:"T1"});
  assert.equal(accepted.events.at(-1).evidence_kind,"native");
  assert.equal((await f.runner.control("run","finish")).outcome,"success");
});
test("failed test -> bounded worker repair; no endless loop",async t=>{
  const f=setup(t);await ready(f);
  for(let i=0;i<3;i++) await f.runner.delegate("run","worker","T1","repair local bug");
  await assert.rejects(()=>f.runner.delegate("run","worker","T1","again"),/limit/);
  await f.runner.delegate("run","reviewer","T1","final repair review");
  f.extra.push({info:{role:"assistant"},parts:[{type:"tool",tool:"bash",sessionID:"child",messageID:"m",callID:"c",state:{status:"completed",input:{command:"node --test"},output:"fail",title:"t",metadata:{exit:1},time:{start:1,end:2}}}]});
  await assert.rejects(()=>f.runner.control("run","accept",{task_id:"T1"}),/Native acceptance failed/);
  f.extra.push({info:{role:"assistant"},parts:[{type:"tool",tool:"bash",sessionID:"child",messageID:"m",callID:"c2",state:{status:"completed",input:{command:"node --test"},output:"pass",title:"t",metadata:{exit:0},time:{start:2,end:3}}}]});
  const accepted=await f.runner.control("run","accept",{task_id:"T1"});
  assert.equal(accepted.events.at(-1).tests.at(-1).exit,0);
});
test("explicit unavailable worker falls back with zero tool side effects",async t=>{
  const f=setup(t,async(o,n)=> n===2?{data:{info:{error:{message:"unsupported_model"}},parts:[]}}:{data:{info:{},parts:[{type:"text",text:"ok"}]}});await ready(f);
  const result=await f.runner.delegate("run","worker","T1","implement");
  assert.equal(result.status,"completed");assert.equal(result.model,policy.models.worker[1]);
});
test("rate-limited candidate falls back before any tool use",async t=>{
  const f=setup(t,async(o,n)=> n===2?{data:{info:{error:{message:"Command Code API error 429: You've reached your weekly usage limit. RATE_LIMITED: rate_limit_error"}},parts:[]}}:{data:{info:{},parts:[{type:"text",text:"ok"}]}});await ready(f);
  const result=await f.runner.delegate("run","worker","T1","implement");
  assert.equal(result.status,"completed");assert.equal(result.model,policy.models.worker[1]);
});
test("unavailable worker with tool side effects never silently retries",async t=>{
  const f=setup(t,async()=>({data:{info:{error:{message:"unsupported_model"}},parts:[]}}));
  await f.runner.control("run","start",{goal:"x"});
  f.client.session.messages=async()=>({data:[{parts:[{type:"tool"}]}]});
  const r=await f.runner.delegate("run","scout",null,"x");assert.equal(r.status,"human");
  assert.ok((await f.store.load("run")).pending);
});
test("Physis unavailable explicitly degrades; useful evidence retained",async t=>{
  const f=setup(t);await f.runner.control("run","start",{goal:"x"});
  f.p.models["physis-inspector"]=["missing/nope"];
  assert.equal((await f.runner.delegate("run","physis-inspector",null,"x")).status,"unavailable");
  f.p.models["physis-inspector"]=policy.models["physis-inspector"];
  assert.equal((await f.runner.delegate("run","physis-inspector",null,"x")).text,"checked");
});
test("Astra disabled never called, unavailable Astra goes human",async t=>{
  const f=setup(t);await ready(f);
  await f.runner.control("run","options",{astra:false});
  await f.runner.control("run","route",{task_id:"T1",signals:{critic_disagreement:1}});
  await assert.rejects(()=>f.runner.delegate("run","architect","T1","question"),/Astra/);
  await f.runner.control("run","options",{astra:true});
  await f.runner.control("run","route",{task_id:"T1",signals:{critic_disagreement:1}});
  f.client.session.prompt=async()=>({data:{info:{error:{message:"unsupported_model"}},parts:[]}});
  assert.equal((await f.runner.delegate("run","architect","T1","question")).status,"human");
});
test("resume interrupted run requires human reconciliation; checkpoint survives restart",async t=>{
  const root=await mkdtemp(path.join(os.tmpdir(),"orch-test-"));t.after(()=>rm(root,{recursive:true,force:true}));
  const f=setup(t);f.runner.store=new Store(root);await ready(f);
  const state=await f.runner.store.load("run");state.pending={child:"child_1"};await f.runner.store.save("run",state);
  const next=new Runner(f.client,f.p,new Store(root),root);
  assert.ok((await next.control("run","status")).pending);
  await assert.rejects(()=>next.delegate("run","worker","T1","again"),/Interrupted/);
  await next.control("run","reconcile",{child:"child_1",outcome:"aborted",evidence_refs:["git diff inspected"]});
  assert.equal((await next.control("run","status")).pending,null);
});
test("profiles A-E keep ablations explicit",()=>{
  for(const profile of ["A","B","C","D","E"]){
    const p={...policy,profile}; const r=route(node("T1",["a","b"]),{critic_disagreement:1},p,p);
    assert.equal(r.inspect,["D","E"].includes(profile));assert.equal(r.action,profile==="E"?"architect":"human");
  }
});
test("store serializes run and refuses concurrent access",async t=>{
  const root=await mkdtemp(path.join(os.tmpdir(),"orch-lock-"));t.after(()=>rm(root,{recursive:true,force:true}));
  const s=new Store(root);await s.lock("run",async()=>{await assert.rejects(()=>s.lock("run",async()=>{}),/busy/);});
  assert.throws(()=>s.file("../../escape"));
});
test("actual reviewer rejection cannot be overridden by coordinator claim",async t=>{
  const f=setup(t);await ready(f);await f.runner.delegate("run","worker","T1","x");
  f.client.session.prompt=async()=>({data:{info:{},parts:[{type:"text",text:JSON.stringify({verdict:"repair",findings:[],missing_validation:[]})}]}});
  await f.runner.delegate("run","reviewer","T1","x");
  await assert.rejects(()=>f.runner.control("run","accept",{task_id:"T1"}),/reviewer verdict/);
});
test("plan amendment invalidates old route and review",async t=>{
  const f=setup(t);await ready(f);await f.runner.delegate("run","worker","T1","x");await f.runner.delegate("run","reviewer","T1","x");
  await f.runner.control("run","plan",{nodes:[{...node(),objective:"different requirement"}]});
  await assert.rejects(()=>f.runner.delegate("run","worker","T1","x"),/Route/);
  await assert.rejects(()=>f.runner.control("run","accept",{task_id:"T1"}),/reviewer verdict/);
});
test("child creation failure can be reconciled only with evidence",async t=>{
  const f=setup(t);await ready(f);f.client.session.create=async()=>{throw Error("transport")};
  await assert.rejects(()=>f.runner.delegate("run","worker","T1","x"),/creation failed/);
  await assert.rejects(()=>f.runner.control("run","reconcile",{child:null,outcome:"aborted"}));
  await f.runner.control("run","reconcile",{child:null,outcome:"aborted",evidence_refs:["native children inspected; none started"]});
  assert.equal((await f.runner.control("run","status")).pending,null);
});
test("explicit disconnected catalog is not treated as callable",async t=>{
  const f=setup(t);await f.runner.control("run","start",{goal:"x"});
  const original=f.client.provider.list;f.client.provider.list=async()=>{const r=await original();r.data.connected=[];return r;};
  assert.equal((await f.runner.delegate("run","scout",null,"x")).status,"unavailable");assert.equal(f.calls.length,0);
});
test("native evidence parser honors exact command and nonzero exit",()=>{
  const part=(command,status,metadata)=>({info:{role:"a"},parts:[{type:"tool",tool:"bash",sessionID:"s",messageID:"m",callID:"c",state:{status,input:{command},output:"ok",title:"t",metadata,time:{start:0,end:1}}}]});
  const rows=evidenceOf([part("node --test","completed",{exit:0})],node());assert.equal(rows[0].evidence_kind,"native");
  assert.throws(()=>evidenceOf([part("npm test","completed",{exit:0})],node()),/No completed native bash/);
  assert.throws(()=>evidenceOf([part("node --test","error",{exit:0})],node()),/No completed native bash/);
  assert.throws(()=>evidenceOf([part("node --test","completed",{})],node()),/without exit code/);
});
test("post-review file drift blocks acceptance",async t=>{
  const root=await mkdtemp(path.join(os.tmpdir(),"orch-drift-"));t.after(()=>rm(root,{recursive:true,force:true}));
  const f=setup(t);f.runner.directory=root;f.runner.store=new Store(root);
  const task={...node(),reads:[],writes:["drift.txt"]};
  await f.runner.control("run","start",{goal:"drift"});
  await f.runner.control("run","plan",{nodes:[task]});
  await f.runner.delegate("run","critic",null,"x");
  await f.runner.control("run","route",{task_id:"T1",signals:{planner_uncertainty:0.1}});
  await writeFile(path.join(root,"drift.txt"),"edited");
  const work=await f.runner.delegate("run","worker","T1","x");
  f.extra.push({info:{role:"assistant"},parts:[{type:"tool",tool:"bash",sessionID:work.child,messageID:"m",callID:"c",state:{status:"completed",input:{command:"node --test"},output:"ok",title:"t",metadata:{exit:0},time:{start:0,end:1}}}]});
  await f.runner.delegate("run","reviewer","T1","x");
  await writeFile(path.join(root,"drift.txt"),"changed after review");
  await assert.rejects(()=>f.runner.control("run","accept",{task_id:"T1"}),/changed after review/);
  assert.equal((await f.runner.control("run","status")).accepted.length,0);
});
test("worker child from another run cannot feed acceptance",async t=>{
  const f=setup(t);await ready(f);
  const work=await f.runner.delegate("run","worker","T1","x");
  f.extra.push({info:{role:"assistant"},parts:[{type:"tool",tool:"bash",sessionID:work.child,messageID:"m",callID:"c",state:{status:"completed",input:{command:"node --test"},output:"ok",title:"t",metadata:{exit:0},time:{start:0,end:1}}}]});
  await f.runner.delegate("run","reviewer","T1","x");
  f.client.session.get=async()=>({data:{parentID:"other-run"}});
  await assert.rejects(()=>f.runner.control("run","accept",{task_id:"T1"}),/another run/);
});
test("hash scope rejects absolute escape",async()=>{
  await assert.rejects(hashFiles("/tmp",["/etc/hostname"]),/Out of scope/);
});
