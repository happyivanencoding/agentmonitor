// Developer-only native WebView harness, injected only by --qa-native in debug builds.
// Tests the actual embedded frontend and Rust IPC. No mock data replaces collector snapshots.
(async()=>{
 const sleep=ms=>new Promise(r=>setTimeout(r,ms));
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const report=payload=>invoke('plugin:event|emit',{event:'agent-monitor:qa',payload});
 const errors=[];window.addEventListener('error',e=>errors.push(e.message));
 const assert=(value,message)=>{if(!value)throw new Error(message)};
 const click=(selector,label)=>{const button=[...document.querySelectorAll(selector)].find(b=>b.textContent.trim()===label||b.getAttribute('aria-label')===label);assert(button,`Missing control: ${label}`);button.click();};
 const checkpoint=async step=>{await sleep(450);await report({status:'running',step,at:Date.now()});await sleep(1300)};
 let temporaryEntity=null;
 try{
   for(let i=0;i<60&&!document.querySelector('.agent-row');i++)await sleep(1000);
   assert(document.querySelector('.agent-row'),`No real agent rows. URL: ${location.href}; title: ${document.title}`);
   const snapshot=await invoke('get_snapshot');assert(!snapshot.stale,'Collector is stale');assert(snapshot.agents.length>0,'No actual agents');assert(snapshot.sources.codex.readOnly,'Source must be read-only');
   const exact=snapshot.agents.find(a=>a.source==='AgentDock ACP'&&a.model&&a.tokensUsed>0&&a.turn?.source==='rollout');assert(exact,'Missing real ACP rollout fallback');
   const firstTime=snapshot.generatedAt;
   await checkpoint('overview-dark');
   click('button','切换亮色');await checkpoint('overview-light');click('button','切换暗色');
   document.querySelector('.agent-main').click();await sleep(300);assert(document.querySelector('.detail-panel'),'Detail panel did not open');await checkpoint('agent-details');
   click('.detail-tabs button','关系与证据');await sleep(200);assert(document.querySelector('.ids'),'Evidence not rendered');await checkpoint('evidence');
   click('.detail-tabs button','时间线');await checkpoint('agent-timeline');click('button','关闭详情');
   const overflow=[];
   for(const [label,name]of [['时间线','timeline'],['系统进程','processes'],['用量与历史','usage'],['AgentDock 任务','tasks'],['连接与设置','settings']]){
     const button=[...document.querySelectorAll('.sidebar nav button')].find(b=>b.querySelector('span')?.textContent===label);assert(button,`Missing nav ${label}`);button.click();await checkpoint(name);
     if(document.documentElement.scrollWidth>innerWidth+2)overflow.push(label);
   }
   const setup=await invoke('get_setup_info');assert(typeof setup.autostart==='boolean','Startup state missing');
   let invalidRejected=false;try{await invoke('bind_agent',{input:{entityId:exact.acpSessions[0].id,conversationUrl:'https://example.com/c/invalid',title:''}})}catch{invalidRejected=true}assert(invalidRejected,'Invalid URL accepted');
   const entity=exact.acpSessions[0].id;let bindingRoundTrip=false,conflictRejected=false;
   if(!snapshot.bindings.some(b=>b.entityId===entity)){
     temporaryEntity=entity;
     await invoke('bind_agent',{input:{entityId:entity,conversationUrl:'https://chatgpt.com/c/00000000-0000-4000-8000-000000000001',title:'[Temporary native QA — removed immediately]'}});
     await sleep(6000);const bound=await invoke('get_snapshot');assert(bound.bindings.some(b=>b.entityId===entity),'Binding not visible after refresh');
     try{await invoke('bind_agent',{input:{entityId:entity,conversationUrl:'https://chatgpt.com/c/00000000-0000-4000-8000-000000000002',title:'must not overwrite'}})}catch{conflictRejected=true}
     assert(conflictRejected,'Conflicting association was not rejected');bindingRoundTrip=true;
     await invoke('unbind_agent',{entityId:entity});temporaryEntity=null;
   }
   click('.sidebar nav button','工作总览');await sleep(4500);const after=await invoke('get_snapshot');assert(after.generatedAt>firstTime,'Snapshot did not refresh');assert(!after.bindings.some(b=>b.title?.startsWith('[Temporary native QA')),'QA binding left behind');
   assert(overflow.length===0,`Global horizontal overflow: ${overflow.join(', ')}`);assert(errors.length===0,`Frontend errors: ${errors.join('; ')}`);
   await checkpoint('overview-final');
   await report({status:'passed',at:Date.now(),url:location.href,agentCount:snapshot.agents.length,acpCount:snapshot.sources.acp.count,threadCount:snapshot.sources.codex.threadCount,collectionMs:snapshot.collectionMs,exactLockMappings:snapshot.sources.locks.mappedThreads,agentdockHealthy:snapshot.sources.agentdock.healthy,realAcpFallback:true,invalidUrlRejected:invalidRejected,bindingRoundTrip,conflictRejected,refreshAdvanced:true,autostart:setup.autostart,consoleErrors:errors,overflow,screenshots:11});
 }catch(error){await report({status:'failed',at:Date.now(),error:String(error.stack||error),url:location.href,consoleErrors:errors});}
 finally{if(temporaryEntity){try{await invoke('unbind_agent',{entityId:temporaryEntity})}catch{await report({status:'failed',error:'QA cleanup failed; remove the temporary native QA binding from the desktop evidence panel.'})}}}
})();
