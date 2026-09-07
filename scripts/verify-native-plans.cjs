// Read-only verification of an actual installed Monitor's native Task, including zero-ACP plans.
// Supply a real task ID as argv[2]; private screenshots/reports remain under ignored .local/.
const {chromium}=require('@playwright/test');const fs=require('node:fs/promises');const path=require('node:path');const assert=require('node:assert/strict');
(async()=>{const taskId=process.argv[2];assert(/^tsk_[a-z0-9]+$/.test(taskId||''),'Provide a real local task ID');
 const snapshot=await (await fetch('http://127.0.0.1:43218/api/snapshot')).json();assert.equal(snapshot.version,'0.5.5');assert(!snapshot.stale);const task=snapshot.tasks.find(t=>t.id===taskId);assert(task,'Native task must exist in the real snapshot');
 const browser=await chromium.launch({headless:true});try{const page=await browser.newPage({viewport:{width:1360,height:900}});const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto('http://127.0.0.1:43218/');const item=page.locator('.native-plan-item').filter({has:page.locator(':scope > summary strong',{hasText:task.title})}).first();await item.waitFor();
 if(!await item.evaluate(e=>e.open))await item.locator(':scope > summary').click();
 await item.locator('.task-steps').waitFor();assert.equal(await item.locator('.task-steps > div').count(),task.steps.length);assert.equal(await item.locator('.task-steps > .completed').count(),task.steps.filter(s=>s.status==='completed').length);assert.equal(await item.locator(':scope > summary').getAttribute('disabled'),null);
 await item.scrollIntoViewIfNeeded();await page.screenshot({path:path.resolve('.local/native055-desktop.png')});
 await page.setViewportSize({width:420,height:860});await item.scrollIntoViewIfNeeded();await page.screenshot({path:path.resolve('.local/native055-mobile.png')});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+2),'No global mobile horizontal overflow');assert.deepEqual(errors,[]);
 const report={source:'installed Monitor / real native Task',version:snapshot.version,taskId,steps:task.steps.length,completed:task.steps.filter(s=>s.status==='completed').length,currentStep:task.steps.find(s=>s.status==='in_progress')?.id,updatedAt:task.updatedAt,nativePlanExpandableWithoutAcp:true,desktopAndMobile:true,pageErrors:errors};await fs.writeFile(path.resolve('.local/native055-'+Date.now()+'.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
