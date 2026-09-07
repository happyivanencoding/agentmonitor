// Observe a genuine Task checkpoint; this script never writes a Task or a Monitor record.
// Usage: node scripts/verify-progress-latency.cjs <task-id> <step-id>
// Start it before completing the named step through the normal AgentDock task tool.
const assert=require('node:assert/strict');const fs=require('node:fs/promises');const path=require('node:path');
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
(async()=>{
 const [taskId,stepId]=process.argv.slice(2);assert(/^tsk_[a-z0-9]+$/.test(taskId||'')&&stepId,'Provide a real Task and step ID');
 const get=async()=>{const r=await fetch('http://127.0.0.1:43218/api/snapshot',{signal:AbortSignal.timeout(10000)});assert(r.ok);return r.json();};
 const first=await get();assert.equal(first.version,'0.5.5');const before=first.tasks.find(t=>t.id===taskId);assert(before);assert.notEqual(before.steps.find(s=>s.id===stepId)?.status,'completed');
 console.log('Watching the actual Task for the next step completion.');let proof;
 for(const deadline=Date.now()+120000;Date.now()<deadline;){
  const s=await get(),task=s.tasks.find(t=>t.id===taskId);
  if(task?.steps.find(x=>x.id===stepId)?.status==='completed'){
   proof={version:s.version,taskId,stepId,taskUpdatedAt:task.updatedAt,firstObservedAt:Date.now(),observedAfterMs:Date.now()-task.updatedAt,taskProgressAt:s.taskProgressAt,collectionMs:s.collectionMs,fullCollectorSampleAt:s.generatedAt};
   assert(proof.observedAfterMs>=0&&proof.observedAfterMs<5000,'Native checkpoint should be visible in under five seconds');break;
  }await sleep(150);
 }
 assert(proof,'No real step completion observed');console.log(JSON.stringify(proof,null,2));
 // A slow full collection must not restore the pre-checkpoint Task state.
 const initial=(await get()).collectionCompletedAt||0;let preserved=false;
 for(const deadline=Date.now()+120000;Date.now()<deadline;){
  const s=await get(),task=s.tasks.find(t=>t.id===taskId);assert(task.updatedAt>=proof.taskUpdatedAt);assert.equal(task.steps.find(x=>x.id===stepId)?.status,'completed');
  if((s.collectionCompletedAt||0)>initial){preserved=true;break;}await sleep(500);
 }
 assert(preserved,'Did not observe a full collector publication during the verification window');proof.preservedAcrossSlowCollection=true;
 await fs.mkdir(path.resolve('.local'),{recursive:true});await fs.writeFile(path.resolve('.local/progress-latency055.json'),JSON.stringify(proof,null,2));console.log('Real checkpoint remained current after the full collector published.');
})().catch(e=>{console.error(e);process.exitCode=1;});
