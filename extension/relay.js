(() => {
  'use strict';
  let received=0,windowStart=Date.now();
  window.addEventListener('message',event=>{
    if(event.source!==window||event.origin!=='https://chatgpt.com')return;
    const data=event.data;if(data?.channel!=='agent-monitor:attribution:v1')return;
    if(Date.now()-windowStart>60000){received=0;windowStart=Date.now();}if(++received>400)return;
    if(data.type==='observer-status'){chrome.runtime.sendMessage({type:'observer-status',kind:String(data.kind||'').slice(0,80)}).catch(()=>{});return;}
    const p=data.payload;if(!p||typeof p.conversationUrl!=='string'||typeof p.title!=='string')return;
    if(data.type==='binding'){
      if(typeof p.entityId!=='string')return;
      chrome.runtime.sendMessage({type:'bind-observed',payload:{entityId:p.entityId,conversationUrl:p.conversationUrl,title:p.title.slice(0,250),userMessageId:String(p.userMessageId||'').slice(0,160),userMessageText:String(p.userMessageText||'').slice(0,4000)}}).catch(()=>{});return;
    }
    if(data.type==='request'){
      chrome.runtime.sendMessage({type:'request-observed',payload:{conversationUrl:p.conversationUrl,conversationTitle:p.title.slice(0,250),userMessageId:String(p.userMessageId||'').slice(0,160),userMessageText:String(p.userMessageText||'').slice(0,4000),phase:String(p.phase||'')}}).catch(()=>{});return;
    }
    if(data.type==='tool'){
      chrome.runtime.sendMessage({type:'tool-observed',payload:{conversationUrl:p.conversationUrl,conversationTitle:p.title.slice(0,250),userMessageId:String(p.userMessageId||'').slice(0,160),userMessageText:String(p.userMessageText||'').slice(0,4000),invocationId:String(p.invocationId||'').slice(0,160),toolName:String(p.toolName||'').slice(0,160),action:String(p.action||'').slice(0,80),argsSummary:String(p.argsSummary||'').slice(0,1200),projectHint:String(p.projectHint||'').slice(0,500),entityId:String(p.entityId||'').slice(0,100),taskId:String(p.taskId||'').slice(0,100),phase:String(p.phase||''),ok:typeof p.ok==='boolean'?p.ok:null}}).catch(()=>{});
    }
  });
})();
