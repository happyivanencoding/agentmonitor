(() => {
  'use strict';
  if(globalThis.__agentMonitorRelay055)return;globalThis.__agentMonitorRelay055=true;
  const channel='agent-monitor:attribution:v1',inFlight=new Set();
  window.addEventListener('message',event=>{
    if(event.source!==window||event.origin!=='https://chatgpt.com')return;
    const data=event.data;if(data?.channel!==channel||data.version!==chrome.runtime.getManifest().version)return;
    if(data.type==='observer-status'){if(data.version!==chrome.runtime.getManifest().version)return;chrome.runtime.sendMessage({type:'observer-status',observerVersion:data.version,kind:String(data.kind||'').slice(0,80),error:String(data.error||'').slice(0,200)}).catch(()=>{});return;}
    const p=data.payload;if(!p||typeof p.conversationUrl!=='string'||typeof p.title!=='string')return;
    const id=typeof data.eventId==='string'?data.eventId.slice(0,600):null;if(!id||inFlight.has(id)||inFlight.size>3000)return;
    const base={conversationUrl:p.conversationUrl,conversationTitle:p.title.slice(0,250),userMessageId:String(p.userMessageId||'').slice(0,160),userMessageText:String(p.userMessageText||'').slice(0,4000)};
    let message;
    if(data.type==='binding'&&typeof p.entityId==='string')message={type:'bind-observed',payload:{...base,title:p.title.slice(0,250),entityId:p.entityId}};
    else if(data.type==='request')message={type:'request-observed',payload:{...base,phase:String(p.phase||'')}};
    else if(data.type==='tool')message={type:'tool-observed',payload:{...base,invocationId:String(p.invocationId||'').slice(0,160),toolName:String(p.toolName||'').slice(0,160),action:String(p.action||'').slice(0,80),argsSummary:String(p.argsSummary||'').slice(0,1200),projectHint:String(p.projectHint||'').slice(0,500),entityId:String(p.entityId||'').slice(0,100),taskId:String(p.taskId||'').slice(0,100),phase:String(p.phase||''),ok:typeof p.ok==='boolean'?p.ok:null}};
    if(!message)return;message.observerVersion=data.version;inFlight.add(id);
    chrome.runtime.sendMessage(message).then(result=>{if(result?.ok)window.postMessage({channel,type:'ack',eventId:id,ok:true},'https://chatgpt.com');}).catch(()=>{}).finally(()=>inFlight.delete(id));
  });
})();
