// Real Chromium extension injection against explicitly synthetic ChatGPT response fixtures.
// This is not acceptance of the owner's authenticated ChatGPT transport.
const {chromium}=require('@playwright/test');const fs=require('node:fs/promises');const path=require('node:path');const assert=require('node:assert/strict');
const cid='11111111-1111-4111-8111-111111111111',other='22222222-2222-4222-8222-222222222222';
function mapping(id){const suffix=id===cid?'one':'two';return {conversation_id:id,title:'Capture fixture',current_node:'final',mapping:{user:{message:{id:'user-'+suffix,author:{role:'user'},content:{parts:['继续']}}},call:{parent:'user',message:{id:'call-'+suffix,author:{role:'assistant'},recipient:'api_tool.call_tool',content:{text:JSON.stringify({path:'/AgentDock/task_manage',arguments:{action:'create',project:'C:\\dev\\fixture',title:'Fixture plan'}})}}},result:{parent:'call',message:{id:'result-'+suffix,author:{role:'tool',name:'api_tool.call_tool'},status:'finished_successfully',content:{parts:[JSON.stringify({action:'create',task_id:'tsk_fixture'+suffix})]}}},final:{parent:'result',message:{id:'final-'+suffix,author:{role:'assistant'},recipient:'all',end_turn:true,status:'finished_successfully',content:{parts:['完成']}}}}};}
(async()=>{const extension=path.resolve('extension');const profile=path.resolve('.local/capture-profile-'+Date.now());await fs.mkdir(profile,{recursive:true});let context;
 try{
  context=await chromium.launchPersistentContext(profile,{channel:'chromium',headless:true,args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`]});
  const worker=context.serviceWorkers()[0]||await context.waitForEvent('serviceworker');
  await worker.evaluate(async()=>{globalThis.__fixtureEvents=[];globalThis.fetch=async(url,options)=>{globalThis.__fixtureEvents.push({url,payload:JSON.parse(options.body)});return new Response(JSON.stringify({ok:true}),{headers:{'Content-Type':'application/json'}});};await chrome.storage.local.set({pairingKey:'a'.repeat(64)});});
  let authReads=0;
  await context.route('https://chatgpt.com/**',async route=>{const req=route.request(),url=new URL(req.url());if(url.pathname==='/api/auth/session'){authReads++;return route.fulfill({json:{accessToken:'fixture-secret-never-exported'}});}
   if(url.pathname.startsWith('/backend-api/conversation/')){if(req.headers().authorization!=='Bearer fixture-secret-never-exported')return route.fulfill({status:401,json:{error:'auth required'}});return route.fulfill({json:mapping(url.pathname.split('/').pop())});}
   const suffix=url.pathname.endsWith(other)?'two':'one';return route.fulfill({contentType:'text/html',body:`<html><head><title>Fixture</title></head><body><main><div data-message-author-role="user" data-message-id="user-${suffix}">继续</div></main></body></html>`});});
  const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto('https://chatgpt.com/c/'+cid);
  await page.waitForFunction(()=>window.__agentMonitorObserver?.version==='0.5.5');
  async function until(predicate){for(let i=0;i<100;i++){const events=await worker.evaluate(()=>globalThis.__fixtureEvents);if(predicate(events))return events;await new Promise(r=>setTimeout(r,100));}throw new Error('Fixture event acceptance timeout');}
  let events=await until(rows=>rows.some(x=>x.url.endsWith('/v1/tool')&&x.payload.taskId==='tsk_fixtureone')&&rows.some(x=>x.url.endsWith('/v1/request')&&x.payload.phase==='completed'));
  assert(authReads>0,'Mapping 401 recovered through same-origin session');assert(!JSON.stringify(events).includes('fixture-secret-never-exported'));
  // Reinjection into an already-open tab must not throw or require a page reload.
  await worker.evaluate(async()=>{for(const tab of await chrome.tabs.query({url:'https://chatgpt.com/*'}))await inject(tab.id);});
  await page.evaluate(()=>{const n=document.querySelector('[data-message-id]');n.setAttribute('data-message-id','repeat-exact');n.appendChild(document.createTextNode(''));});
  // Observer runs periodic scan too; identical text is not a dedup key.
  events=await until(rows=>rows.some(x=>x.url.endsWith('/v1/request')&&x.payload.userMessageId==='repeat-exact'));
  const second=await context.newPage();second.on('pageerror',e=>errors.push(e.message));await second.goto('https://chatgpt.com/c/'+other);
  events=await until(rows=>rows.some(x=>x.url.endsWith('/v1/tool')&&x.payload.taskId==='tsk_fixturetwo'));
  for(const row of events.filter(x=>x.url.endsWith('/v1/tool')))assert.equal(row.payload.conversationUrl,'https://chatgpt.com/c/'+(row.payload.userMessageId==='user-one'?cid:other));
  assert.deepEqual(errors,[]);const report={mode:'synthetic transport / real Chromium extension',extensionVersion:'0.5.5',authenticatedMappingRetry:true,taskCreateCaptured:true,recipientAllCompletion:true,repeatedTextNewId:true,existingTabReinjection:true,concurrentTabIsolation:true,credentialsNotExported:true,pageErrors:errors};
  await fs.writeFile(path.resolve('.local/capture055.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
 }finally{await context?.close();await fs.rm(profile,{recursive:true,force:true}).catch(()=>{});}
})().catch(e=>{console.error(e);process.exitCode=1;});
