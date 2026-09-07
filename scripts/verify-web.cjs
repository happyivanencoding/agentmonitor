// Real-browser acceptance against the running Windows collector, not mocked snapshots.
// Optional AGENT_MONITOR_TEST_URL is usable with privately supplied Access service credentials.
const {chromium}=require('@playwright/test');
const assert=require('node:assert/strict');const fs=require('node:fs/promises');const path=require('node:path');const http=require('node:http');
const base=process.env.AGENT_MONITOR_TEST_URL||'http://127.0.0.1:43218';
const out=path.resolve('.local/web-qa');
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn,message,timeout=25000){const end=Date.now()+timeout;while(Date.now()<end){if(await fn())return;await delay(500);}throw new Error(message)}
async function main(){
 await fs.mkdir(out,{recursive:true});const browser=await chromium.launch({headless:true});
 const context=await browser.newContext({viewport:{width:1440,height:1000}});const page=await context.newPage();const errors=[];
 page.on('pageerror',e=>errors.push(e.message));const report={base,tests:[],screenshots:[],errors};let boundEntity=null;
 const snap=async()=>{const r=await context.request.get(base+'/api/snapshot');assert.equal(r.status(),200);return r.json()};
 const post=(route,value)=>context.request.post(base+route,{headers:{Origin:base,'X-Agent-Monitor':'1'},data:value});
 const screenshot=async name=>{await page.screenshot({path:path.join(out,name+'.png'),animations:'disabled'});report.screenshots.push(name)};
 const fit=async label=>{const size=await page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth}));assert.ok(size.scroll<=size.width+1,`${label}: global overflow ${JSON.stringify(size)}`)};
 try{
  await page.goto(base,{waitUntil:'networkidle'});
  await page.locator('.agent-main').first().waitFor({timeout:30000});
  const initial=await snap();assert.ok(initial.agents.length>0);assert.ok(initial.sources.codex.threadCount>0);
  report.realAgents=initial.agents.length;report.acpRecords=initial.sources.acp.count;report.snapshotBytes=Buffer.byteLength(JSON.stringify(initial));
  report.tests.push('Real Windows collector data loads in an ordinary browser');
  await fit('desktop');await screenshot('desktop-overview-dark');
  await page.getByRole('button',{name:'切换亮色',exact:true}).click();await screenshot('desktop-overview-light');await page.getByRole('button',{name:'切换暗色',exact:true}).click();
  for(const name of ['时间线','系统进程','用量与历史','AgentDock 任务','连接与设置']){
   await page.locator('.sidebar').getByRole('button',{name,exact:true}).click();await delay(300);await fit('desktop '+name);
  }
  await page.getByRole('button',{name:'安装 / 添加到主屏幕',exact:true}).click();
  assert.ok((await page.locator('.install-card').innerText()).includes('PWA'));
  assert.equal(await page.getByRole('button',{name:'复制配对密钥',exact:true}).count(),0);
  report.tests.push('All web pages work; installation guidance and native-only settings separation');
  await screenshot('desktop-remote-settings');
  for(const width of [360,390,412]){
   await page.setViewportSize({width,height:844});
   for(const name of ['工作总览','时间线','系统进程','用量与历史','AgentDock 任务','连接与设置']){
    await page.locator('.mobile-nav').getByRole('button',{name,exact:true}).click();await delay(150);await fit(`${width} ${name}`);
   }
  }
  report.tests.push('Six pages fit 360, 390 and 412 px mobile viewports');
  await page.setViewportSize({width:390,height:844});await page.locator('.mobile-nav').getByRole('button',{name:'工作总览',exact:true}).click();await screenshot('mobile-overview');
  const candidate=initial.agents.find(a=>!a.binding&&a.acpSessions.length&&a.tokensUsed>0&&a.turn);
  assert.ok(candidate,'Need a real unbound ACP sample');
  await page.getByRole('button',{name:'全部记录',exact:true}).click();await page.getByRole('textbox',{name:'搜索工作'}).fill(candidate.id);
  await page.locator('.agent-main').first().click();await page.locator('.detail-panel').waitFor();await fit('mobile detail');
  assert.ok((await page.locator('.detail-panel').innerText()).includes(candidate.model));await screenshot('mobile-agent-detail');
  await page.locator('.detail-tabs').getByRole('button',{name:'关系与证据',exact:true}).click();await screenshot('mobile-evidence');
  await page.locator('.detail-tabs').getByRole('button',{name:'时间线',exact:true}).click();await page.locator('.detail-timeline').waitFor({timeout:15000});
  await page.getByRole('button',{name:'全部元数据',exact:true}).click();await screenshot('mobile-agent-timeline');
  await page.locator('.detail-tabs').getByRole('button',{name:'当前工作',exact:true}).click();await page.getByRole('button',{name:'绑定对话',exact:true}).click();
  await page.getByLabel('ChatGPT 对话 URL').fill('https://chatgpt.com/c/00000000-0000-4000-8000-000000000002');
  await page.getByLabel('对话标题（可选；未知时留空）').fill('QA · temporary web binding');
  boundEntity=candidate.acpSessions[0].id;
  await page.getByRole('button',{name:'保存明确绑定',exact:true}).click();
  await until(async()=>{const s=await snap();return s.agents.some(a=>a.id===candidate.id&&a.binding?.title==='QA · temporary web binding')},'Web binding did not reach the shared collector');
  const conflict=await post('/api/bind',{entityId:boundEntity,conversationUrl:'https://chatgpt.com/c/00000000-0000-4000-8000-000000000003',title:'must not replace'});assert.equal(conflict.status(),400);
  await post('/api/unbind',{entityId:boundEntity});
  await until(async()=>!(await snap()).agents.find(a=>a.id===candidate.id)?.binding,'Temporary QA binding was not removed');boundEntity=null;
  report.tests.push('Actual mobile dialog creates a shared binding; conflicting reassignment rejected; cleanup verified');
  await page.getByRole('button',{name:'关闭详情',exact:true}).click();await page.getByRole('textbox',{name:'搜索工作'}).fill('');
  const old=(await snap()).generatedAt;await until(async()=>(await snap()).generatedAt>old,'Collector did not produce a fresh snapshot');report.tests.push('Real collector keeps advancing');
  await until(()=>page.evaluate(()=>!!navigator.serviceWorker.controller),'Service worker did not become active');
  const manifest=await (await context.request.get(base+'/manifest.webmanifest')).json();assert.equal(manifest.display,'standalone');assert.equal(manifest.icons.length,2);
  const cached=await page.evaluate(async()=>{const entries=[];for(const key of await caches.keys()){for(const r of await(await caches.open(key)).keys())entries.push(new URL(r.url).pathname);}return entries;});assert.ok(!cached.some(p=>p.startsWith('/api/')||p.startsWith('/cdn-cgi/')));report.cachedPaths=cached;
  await context.setOffline(true);await until(()=>page.getByRole('alert').count(),'Offline alert missing');assert.ok((await page.locator('.live-label').innerText()).includes('OFFLINE'));await screenshot('mobile-offline-state');
  await page.reload({waitUntil:'domcontentloaded'});assert.ok((await page.locator('body').innerText()).includes('暂时连接不到控制室'));await screenshot('mobile-offline-page');
  await context.setOffline(false);await page.goto(base);await page.locator('.agent-main').first().waitFor({timeout:30000});report.tests.push('Offline explicitly invalidates LIVE; reload shows no cached data; reconnection recovers');
  await page.route('**/api/snapshot*',r=>r.fulfill({status:401,contentType:'application/json',body:JSON.stringify({error:'simulated expired access session'})}));
  await until(()=>page.getByRole('button',{name:'重新连接 / 登录',exact:true}).count(),'Session expiration recovery action missing');await page.unroute('**/api/snapshot*');
  await until(async()=>await page.locator('.live-label').innerText()==='LIVE','Auth recovery did not return to real data');report.tests.push('Injected session-expiry failure is visible and recovers to real data');
  await page.locator('.mobile-nav').getByRole('button',{name:'连接与设置',exact:true}).click();await screenshot('mobile-install-settings');
  assert.deepEqual(errors,[]);report.status='passed';
 }finally{
  if(boundEntity)await post('/api/unbind',{entityId:boundEntity});
  report.finishedAt=new Date().toISOString();await fs.writeFile(path.join(out,'report.json'),JSON.stringify(report,null,2));await browser.close();
 }
 console.log(JSON.stringify(report,null,2));
}
main().catch(e=>{console.error(e.message);process.exitCode=1});
