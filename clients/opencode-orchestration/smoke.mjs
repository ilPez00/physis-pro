// Deterministic adapter smoke, NOT a live-model integration claim.
// SDK double executes real isolated IO with explicit mock permission allowlist.
import {mkdtemp, readFile, writeFile, rm} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import assert from "node:assert/strict";
import {Runner, Store, route} from "./router.mjs";
const policy=JSON.parse(await readFile(new URL("../../.opencode/orchestration.json",import.meta.url)));
const root=await mkdtemp(path.join(os.tmpdir(),"orchestrate-smoke-"));
const target=path.join(root,"fixture.json");
const permissions={worker:{write:[target],read:[target]},reviewer:{write:[],read:[target]}};
const task={id:"T1",objective:"write and validate a JSON fixture",reads:[],writes:[target],depends_on:[],produces:[target],invariants:["ok === true"],acceptance_tests:["JSON.parse fixture and assert ok"],assumptions:[],uncertainty:0,parallelizable:false};
const native=new Map();let sequence=0;
function permit(role,action,file){assert.ok(permissions[role]?.[action].includes(file),`DENIED ${role} ${action} ${file}`);}
const transcripts=new Map();
const client={provider:{list:async()=>({data:{all:[{id:"commandcode",models:Object.fromEntries(Object.keys(policy.families).map(m=>[m.slice(12),{}]))}]}})},session:{
  create:async()=>{const id=`mock_child_${++sequence}`;transcripts.set(id,[]);return {data:{id}};},
  prompt:async({path:{id},body})=>{
    const role=body.agent.replace("orch-","");let result;
    if(role==="worker"){
      permit(role,"write",target);await writeFile(target,JSON.stringify({ok:true}));
      permit(role,"read",target);assert.equal(JSON.parse(await readFile(target,"utf8")).ok,true);
      result={tests:[{command:task.acceptance_tests[0],exit:0,evidence_ref:`${id}/assert`}]};
    }else if(role==="reviewer"){
      permit(role,"read",target);assert.equal(JSON.parse(await readFile(target,"utf8")).ok,true);
      assert.throws(()=>permit(role,"write",target),/DENIED/);
      result={verdict:"accept",findings:[],missing_validation:[]};
    }else result={findings:[]};
    native.set(id,result);
    transcripts.set(id,[...(transcripts.get(id)||[]),{info:{role:"assistant"},parts:[{type:"text",text:JSON.stringify(result)}]}]);
    if(role==="worker") transcripts.get(id).push({info:{role:"assistant"},parts:[{type:"tool",tool:"bash",sessionID:id,messageID:`${id}/m`,callID:`${id}/bash`,state:{status:"completed",input:{command:task.acceptance_tests[0]},output:"fixture ok",title:"acceptance",metadata:{exit:0},time:{start:1,end:2}}}]});
    return {data:{info:{tokens:null,cost:null},parts:[{type:"text",text:JSON.stringify(result)}]}};
  }, messages:async o=>({data:transcripts.get(o.path.id)||[]}),get:async o=>({data:{id:o.path.id,parentID:"smoke"}}),abort:async()=>({data:true})
}};
try{
  const runner=new Runner(client,policy,new Store(root),root);
  await runner.control("smoke","start",{goal:task.objective});
  await runner.control("smoke","options",{astra:false,physis:false});
  await runner.control("smoke","plan",{nodes:[task]});
  await runner.delegate("smoke","critic","T1","Inspect isolated fixture contract");
  const decision=route(task,{planner_uncertainty:0},{astra:false,physis:false},policy);
  assert.equal(decision.action,"worker");assert.equal(decision.inspect,false);
  await runner.control("smoke","route",{task_id:"T1",signals:{planner_uncertainty:0},evidence_refs:["fixture-contract"]});
  const work=await runner.delegate("smoke","worker","T1","Write fixture and assert JSON");
  const review=await runner.delegate("smoke","reviewer","T1","Independently read fixture and assert JSON");
  assert.equal(review.independent,true);
  assert.equal(native.get(review.child).verdict,"accept");
  await runner.control("smoke","accept",{task_id:"T1"});
  const checkpoint=await runner.control("smoke","finish");
  assert.equal(checkpoint.outcome,"success");
  console.log(JSON.stringify({kind:"SDK-double, real temporary IO, mocked permissions",checkpoint},null,2));
}finally{await rm(root,{recursive:true,force:true});}
