const ENDPOINT='http://127.0.0.1:43217';
const ACP=/^acps_[a-z0-9]{8,64}$/i;
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
async function state(patch){await chrome.storage.local.set({status:{...(await chrome.storage.local.get('status')).status,...patch}});}
async function post(route,payload){const {pairingKey}=await chrome.storage.local.get('pairingKey');if(!/^[a-f0-9]{64}$/.test(pairingKey||''))throw new Error('请先在桌面应用中复制配对密钥。');const response=await fetch(ENDPOINT+route,{method:'POST',headers:{'Content-Type':'application/json','Authorization':'Bearer '+pairingKey},body:JSON.stringify(payload),signal:AbortSignal.timeout(5000)});const result=await response.json();if(!response.ok){const e=new Error(result.error||`HTTP ${response.status}`);e.httpStatus=response.status;throw e;}return result;}
async function heartbeatIfPaired(extra={}){const {pairingKey}=await chrome.storage.local.get('pairingKey');if(!/^[a-f0-9]{64}$/.test(pairingKey||''))return false;await post('/v1/heartbeat',{extensionVersion:chrome.runtime.getManifest().version,...extra});await state({lastHeartbeatAt:Date.now(),error:null});return true;}
function validate(p){if(!p||(!ACP.test(p.entityId||'')&&!UUID.test(p.entityId||'')))throw new Error('请输入真实 ACP ID 或 Codex thread ID。');const u=new URL(p.conversationUrl);if(u.protocol!=='https:'||u.hostname!=='chatgpt.com'||!u.pathname.match(/(?:^|\/)c\/[0-9a-f-]{36}$/i))throw new Error('当前页不是 ChatGPT conversation。');const userMessageId=String(p.userMessageId||'').slice(0,120),userMessageText=String(p.userMessageText||'').slice(0,2000);return {entityId:p.entityId,conversationUrl:u.href,title:String(p.title||'').slice(0,250),userMessageId,userMessageText};}
function conversationUrl(raw){const u=new URL(raw);if(u.protocol!=='https:'||u.hostname!=='chatgpt.com'||!u.pathname.match(/(?:^|\/)c\/[0-9a-f-]{36}$/i))throw new Error('当前页不是 ChatGPT conversation。');return u.href;}
function validateRequest(p){if(!p||!['started','completed'].includes(p.phase))throw new Error('请求事件格式不正确。');const userMessageId=String(p.userMessageId||'').slice(0,160);if(!userMessageId)throw new Error('缺少用户消息 ID。');return {conversationUrl:conversationUrl(p.conversationUrl),conversationTitle:String(p.conversationTitle||'').slice(0,250),userMessageId,userMessageText:String(p.userMessageText||'').slice(0,4000),phase:p.phase};}
function validateTool(p){if(!p||!['started','completed'].includes(p.phase)||!/^AgentDock\./i.test(p.toolName||''))throw new Error('工具事件格式不正确。');const invocationId=String(p.invocationId||'').slice(0,160),userMessageId=String(p.userMessageId||'').slice(0,160);if(!invocationId||!userMessageId)throw new Error('工具事件缺少 ID。');return {conversationUrl:conversationUrl(p.conversationUrl),conversationTitle:String(p.conversationTitle||'').slice(0,250),userMessageId,userMessageText:String(p.userMessageText||'').slice(0,4000),invocationId,toolName:String(p.toolName).slice(0,160),action:String(p.action||'').slice(0,80)||null,argsSummary:String(p.argsSummary||'').slice(0,1200)||null,projectHint:String(p.projectHint||'').slice(0,500)||null,entityId:String(p.entityId||'').slice(0,100)||null,taskId:String(p.taskId||'').slice(0,100)||null,phase:p.phase,ok:typeof p.ok==='boolean'?p.ok:null};}

// Persist sanitized attribution events before acknowledging a page. Monitor may be restarting.
let queueLock=Promise.resolve(),flushing=false;
function locked(fn){const work=queueLock.then(fn);queueLock=work.catch(()=>{});return work;}
async function enqueue(route,payload){
  const key=[route,payload.conversationUrl,payload.userMessageId||'',payload.invocationId||payload.entityId||'',payload.phase||''].join('|');
  await locked(async()=>{const {outbox=[]}=await chrome.storage.local.get('outbox');if(outbox.some(e=>e.key===key))return;if(outbox.length>=3000)throw new Error('本地事件队列已满；请检查 Monitor 是否运行。');outbox.push({key,route,payload});await chrome.storage.local.set({outbox});});
  void flush();return {ok:true,queued:true};
}
async function flush(){
  if(flushing)return;flushing=true;let more=false;
  try{for(let i=0;i<40;i++){
    const {outbox=[]}=await chrome.storage.local.get('outbox'),entry=outbox[0];if(!entry)break;
    try{await post(entry.route,entry.payload);}catch(e){await state({error:String(e.message).slice(0,250),pendingEvents:outbox.length});
      if([400,403,409,413,415].includes(e.httpStatus)){
        await locked(async()=>{const {outbox:q=[],rejected=[]}=await chrome.storage.local.get(['outbox','rejected']);await chrome.storage.local.set({outbox:q.filter(x=>x.key!==entry.key),rejected:[...rejected,{route:entry.route,at:Date.now(),error:String(e.message).slice(0,200)}].slice(-20)});});continue;
      }return;
    }
    await locked(async()=>{const {outbox:q=[]}=await chrome.storage.local.get('outbox');await chrome.storage.local.set({outbox:q.filter(x=>x.key!==entry.key)});});
    const patch={error:null,pendingEvents:Math.max(0,outbox.length-1)};
    if(entry.route==='/v1/request')patch.lastRequestAt=Date.now();
    if(entry.route==='/v1/tool'){patch.lastToolAt=Date.now();patch.lastTool=entry.payload.toolName;}
    if(entry.route==='/v1/bind'){patch.lastBoundAt=Date.now();patch.lastEntity=entry.payload.entityId;}
    await state(patch);more=i===39;
  }}finally{flushing=false;if(more)setTimeout(()=>void flush(),100);}
}
async function inject(tabId,replace=false){
  if(replace)await chrome.scripting.executeScript({target:{tabId},world:'MAIN',func:()=>{window.__agentMonitorObserver?.dispose?.();delete window.__agentMonitorObserver;}});
  // Isolated relay first, then MAIN-world observers. Repeated installation is idempotent.
  await chrome.scripting.executeScript({target:{tabId},world:'ISOLATED',files:['relay.js']});
  await chrome.scripting.executeScript({target:{tabId},world:'MAIN',files:['extractor.js','page-observer.js']});
}
async function reinject(){for(const tab of await chrome.tabs.query({url:'https://chatgpt.com/*'})){if(tab.id!=null)try{await inject(tab.id,true);}catch(e){await state({injectionError:String(e.message).slice(0,200)});}}}
chrome.runtime.onMessage.addListener((message,sender,reply)=>{
  (async()=>{
    const page=sender.id===chrome.runtime.id&&sender.url?.startsWith('https://chatgpt.com/');
    const extension=sender.id===chrome.runtime.id&&sender.url?.startsWith(chrome.runtime.getURL(''));
    if(page&&['observer-status','bind-observed','request-observed','tool-observed'].includes(message.type)&&message.observerVersion!==chrome.runtime.getManifest().version)return {ok:false,staleObserver:true};
    if(message.type==='observer-status'&&page){
      const observer=String(message.kind||'').slice(0,80),captureError=String(message.error||'').slice(0,200),observerAt=Date.now();
      const patch={observer,observerAt};if(observer!=='observer-ready'){patch.captureStatus=observer;patch.captureError=captureError;}
      await state(patch);await heartbeatIfPaired({pageObserver:observer,...(observer!=='observer-ready'?{captureError}:{})});void flush();return {ok:true};
    }
    if(message.type==='bind-observed'&&page)return enqueue('/v1/bind',validate(message.payload));
    if(message.type==='request-observed'&&page)return enqueue('/v1/request',validateRequest(message.payload));
    if(message.type==='tool-observed'&&page)return enqueue('/v1/tool',validateTool(message.payload));
    if(message.type==='pair'&&extension){if(!/^[a-f0-9]{64}$/.test(message.key||''))throw new Error('配对密钥格式不正确。');await chrome.storage.local.set({pairingKey:message.key});await heartbeatIfPaired();await state({pairedAt:Date.now(),error:null});void reinject();void flush();return {ok:true};}
    if(message.type==='heartbeat'&&extension){await heartbeatIfPaired();void flush();return {ok:true};}
    if(message.type==='bind-manual'&&extension){const result=await post('/v1/bind/manual',validate(message.payload));await state({lastBoundAt:Date.now(),error:null});return result;}
    throw new Error('不允许的消息来源。');
  })().then(reply).catch(async error=>{await state({error:String(error.message||error).slice(0,250)});reply({ok:false,error:String(error.message||error)});});
  return true;
});
function startup(){void reinject();void heartbeatIfPaired().catch(e=>state({error:String(e.message).slice(0,250)}));void flush();chrome.alarms.create('capture-retry',{periodInMinutes:0.5});}
chrome.runtime.onInstalled.addListener(startup);
chrome.runtime.onStartup.addListener(startup);
chrome.tabs.onUpdated.addListener((tabId,change,tab)=>{if(change.status==='complete'&&tab.url?.startsWith('https://chatgpt.com/'))void inject(tabId).catch(e=>state({injectionError:String(e.message).slice(0,200)}));});
chrome.alarms.onAlarm.addListener(alarm=>{if(alarm.name==='capture-retry')void flush();});
