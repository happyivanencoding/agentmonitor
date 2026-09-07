// Integration verification against this application's real Tauri window, not a mocked web UI.
// Run: node scripts/verify-live.cjs [WebView2 CDP port]
// All screenshots and private snapshots stay under ignored .local/.
const {chromium}=require('@playwright/test');
const fs=require('node:fs/promises');const path=require('node:path');const assert=require('node:assert/strict');
(async()=>{
 const dir=path.resolve('.local/ui');await fs.mkdir(dir,{recursive:true});
 const endpoint=`http://127.0.0.1:${process.argv[2]||9337}`;
 let ready=false;for(let i=0;i<60;i++){try{const r=await fetch(endpoint+'/json/version',{signal:AbortSignal.timeout(1000)});if(r.ok){ready=true;break}}catch{}await new Promise(r=>setTimeout(r,1000));}
 if(!ready)throw new Error('Developer CDP endpoint did not become ready');
 const browser=await chromium.connectOverCDP(endpoint,{timeout:15000});
 const pages=browser.contexts().flatMap(c=>c.pages());const page=pages.find(p=>/localhost:1420|tauri\.localhost|tauri:\/\//.test(p.url()))||pages[0];
 if(!page)throw new Error('No Agent Monitor WebView');
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__,{timeout:15000});
 await page.waitForFunction(()=>document.querySelector('h1')?.textContent?.includes('工作总览'),{timeout:45000});
 await page.waitForFunction(()=>document.querySelectorAll('.agent-row').length>0,{timeout:60000});
 const invoke=(cmd,args={})=>page.evaluate(([cmd,args])=>window.__TAURI_INTERNALS__.invoke(cmd,args),[cmd,args]);
 const snapshot=await invoke('get_snapshot');assert(!snapshot.stale,'Collector must not be stale');assert(snapshot.agents.length>0);assert(snapshot.sources.codex.readOnly===true);assert(snapshot.sources.acp.count>0);
 const exact=snapshot.agents.find(a=>a.source==='AgentDock ACP'&&a.tokensUsed>0&&a.model&&a.turn?.source==='rollout');assert(exact,'Real ACP model/tokens and rollout fallback must exist');
 const firstUsage=snapshot.analyticsToday.observedTokens;
 await page.screenshot({path:path.join(dir,'overview-dark.png')});
 await page.getByRole('button',{name:'切换亮色',exact:true}).click();await page.screenshot({path:path.join(dir,'overview-light.png')});
 await page.getByRole('button',{name:'切换暗色',exact:true}).click();
 await page.locator('.agent-main').first().click();await page.locator('.detail-panel').waitFor();await page.screenshot({path:path.join(dir,'agent-details.png')});
 await page.locator('.detail-tabs').getByRole('button',{name:'关系与证据',exact:true}).click();assert(await page.getByText('Codex thread ID',{exact:true}).count()>0);
 await page.locator('.detail-tabs').getByRole('button',{name:'时间线',exact:true}).click();await page.waitForTimeout(1500);await page.screenshot({path:path.join(dir,'agent-timeline.png')});
 await page.getByRole('button',{name:'关闭详情',exact:true}).click();
 for(const [label,file]of [['时间线','timeline'],['系统进程','processes'],['用量与历史','usage'],['AgentDock 任务','tasks'],['连接与设置','settings']]){
  await page.locator('.sidebar nav').getByRole('button',{name:label,exact:false}).click();await page.waitForTimeout(400);await page.screenshot({path:path.join(dir,`${file}.png`)});
  const horizontal=await page.evaluate(()=>document.documentElement.scrollWidth>window.innerWidth+2);assert(!horizontal,`No global overflow on ${label}`);
 }
 const setup=await invoke('get_setup_info');assert(typeof setup.autostart==='boolean');assert(setup.extensionDir.endsWith('extension'));
 // Invalid URL must be rejected before any mutation.
 let rejected=false;try{await invoke('bind_agent',{input:{entityId:exact.acpSessions[0].id,conversationUrl:'https://example.com/c/not-real',title:'invalid test'}})}catch{rejected=true}assert(rejected);
 // Round-trip explicit association, only if this entity is currently unbound. Remove in finally.
 const entity=exact.acpSessions[0].id,existing=snapshot.bindings.find(b=>b.entityId===entity);let roundTrip=false;
 if(!existing){try{
   await invoke('bind_agent',{input:{entityId:entity,conversationUrl:'https://chatgpt.com/c/00000000-0000-4000-8000-000000000001',title:'[Temporary QA binding — removed immediately]'}});
   await page.waitForTimeout(6000);const bound=await invoke('get_snapshot');assert(bound.bindings.some(b=>b.entityId===entity));
   let conflict=false;try{await invoke('bind_agent',{input:{entityId:entity,conversationUrl:'https://chatgpt.com/c/00000000-0000-4000-8000-000000000002',title:'must not overwrite'}})}catch{conflict=true}assert(conflict);roundTrip=true;
 }finally{await invoke('unbind_agent',{entityId:entity});}}
 await page.locator('.sidebar nav').getByRole('button',{name:'工作总览',exact:true}).click();
 await page.waitForTimeout(4000);const later=await invoke('get_snapshot');assert(later.generatedAt>snapshot.generatedAt,'Live collector must continue publishing');
 assert(!later.bindings.some(b=>b.title?.startsWith('[Temporary QA')),'No QA attribution left behind');
 const report={testedAt:new Date().toISOString(),uiUrl:page.url(),agentCount:snapshot.agents.length,acpCount:snapshot.sources.acp.count,codexThreadCount:snapshot.sources.codex.threadCount,collectionMs:snapshot.collectionMs,exactLockMappings:snapshot.sources.locks.mappedThreads,agentdockHealthy:snapshot.sources.agentdock.healthy,realAcpFallbackVerified:true,observedTokensBefore:firstUsage,observedTokensAfter:later.analyticsToday.observedTokens,explicitBindingRoundTrip:roundTrip,invalidUrlRejected:true,refreshAdvanced:true,autostart:setup.autostart,consoleErrors:errors,screenshotCount:9};
 await fs.writeFile(path.resolve('.local/live-verification.json'),JSON.stringify(report,null,2));assert.deepEqual(errors,[],'No frontend runtime errors');console.log(JSON.stringify(report,null,2));await browser.close();
})().catch(e=>{console.error(e);process.exitCode=1;});
