/* Observe structured ChatGPT requests/results for local Agent Monitor attribution and progress. No transcript persistence. */
(() => {
  'use strict';
  const parser=globalThis.AgentMonitorParser;if(!parser)return;
  const nativeFetch=window.fetch;const CHANNEL='agent-monitor:attribution:v1';
  const emittedBindings=new Set(),emittedRequests=new Set(),emittedTools=new Set(),activeUsers=new Map(),mappingPolls=new Set();let synthetic=0;
  function titleFor(cid){return parser.conversationId(location.href)===cid?document.title.replace(/\s*[-–·]\s*ChatGPT$/,''):''}
  function emitBindings(items){for(const item of items){const key=`${item.conversationId}:${item.entityId}`;if(emittedBindings.has(key))continue;if(emittedBindings.size>2000)emittedBindings.clear();emittedBindings.add(key);window.postMessage({channel:CHANNEL,type:'binding',payload:item},'https://chatgpt.com');}}
  function emitRequest(user,cid,phase){if(!user?.id||!user?.text||!parser.UUID.test(cid||''))return;const key=`${cid}:${user.id}:${phase}`;if(emittedRequests.has(key))return;if(emittedRequests.size>4000)emittedRequests.clear();emittedRequests.add(key);window.postMessage({channel:CHANNEL,type:'request',payload:{conversationId:cid,conversationUrl:`https://chatgpt.com/c/${cid}`,title:titleFor(cid),userMessageId:String(user.id).slice(0,160),userMessageText:String(user.text).slice(0,4000),phase}},'https://chatgpt.com');}
  function emitTool(activity,phase){if(!activity?.invocationId||!activity?.userMessageId)return;const key=`${activity.conversationId}:${activity.invocationId}:${phase}`;if(emittedTools.has(key))return;if(emittedTools.size>6000)emittedTools.clear();emittedTools.add(key);window.postMessage({channel:CHANNEL,type:'tool',payload:{...activity,phase}},'https://chatgpt.com');}
  function report(kind){window.postMessage({channel:CHANNEL,type:'observer-status',kind},'https://chatgpt.com');}
  async function boundedText(response,max){const reader=response.body?.getReader();if(!reader)return '';let text='',size=0;const decoder=new TextDecoder();try{for(;;){const {done,value}=await reader.read();if(done)break;size+=value.length;if(size>max){void reader.cancel();throw new Error('response too large');}text+=decoder.decode(value,{stream:true});}return text+decoder.decode();}finally{reader.releaseLock();}}
  function applyMapping(data,cid,preferredUser){
    if(!data?.mapping||!parser.UUID.test(cid||''))return false;
    const view=parser.inspectRequestActivities(data,cid,preferredUser?.id),user=view.user||preferredUser;if(!user)return false;
    activeUsers.set(cid,user);emitRequest(user,cid,'started');
    for(const activity of view.activities||[])emitTool(activity,activity.phase||'completed');
    emitBindings(parser.inspectConversation(data,cid,user.id));
    if(view.final){emitRequest(user,cid,'completed');activeUsers.delete(cid);}
    return !!view.final;
  }
  async function hydrateMapping(cid,preferredUser){
    if(!parser.UUID.test(cid||''))return false;
    for(const path of [`/backend-api/conversation/${cid}`,`/backend-api/f/conversation/${cid}`]){
      try{const response=await nativeFetch.call(window,path,{method:'GET',credentials:'include',cache:'no-store'});if(!response.ok)continue;const text=await boundedText(response,16*1024*1024),data=JSON.parse(text);if(applyMapping(data,cid,preferredUser))return true;if(data?.mapping)return false;}catch{}
    }
    return false;
  }
  function pollMapping(cid,user){
    if(!cid||!user?.id)return;const key=`${cid}:${user.id}`;if(mappingPolls.has(key))return;mappingPolls.add(key);
    const delays=[1200,3000,7000,15000,30000,60000];let i=0;
    const tick=()=>{if(i>=delays.length){mappingPolls.delete(key);return;}const delay=delays[i++];setTimeout(()=>{void hydrateMapping(cid,user).then(done=>{if(done)mappingPolls.delete(key);else tick();});},delay);};tick();
  }
  let lastDomUser='';
  function pollLatestMapping(cid){
    if(!parser.UUID.test(cid||''))return;const key=`latest:${cid}`;if(mappingPolls.has(key))return;mappingPolls.add(key);
    const delays=[0,700,1800,4000,8000,15000,30000,60000];for(const delay of delays)setTimeout(()=>void hydrateMapping(cid,null),delay);setTimeout(()=>mappingPolls.delete(key),61000);
  }
  function scanDomUser(){
    const nodes=document.querySelectorAll?.('[data-message-author-role="user"]');const node=nodes?.[nodes.length-1];if(!node)return;const text=String(node.textContent||'').trim().slice(0,500);if(!text||text===lastDomUser)return;lastDomUser=text;const cid=parser.conversationId(location.href);if(cid)pollLatestMapping(cid);
  }
  function startDomObserver(){scanDomUser();const target=document.body||document.documentElement;if(!target)return;new MutationObserver(scanDomUser).observe(target,{subtree:true,childList:true,characterData:true});setInterval(scanDomUser,2000);}
  async function stream(response,initialId,initialUser){
    const reader=response.body?.getReader();if(!reader)return;let cid=initialId,buffer='',lastInvocation=null,lastUser=initialUser||(initialId?activeUsers.get(initialId):null)||null,finalSeen=false;const decoder=new TextDecoder();
    if(cid&&lastUser){activeUsers.set(cid,lastUser);emitRequest(lastUser,cid,'started');}
    try{for(;;){const {done,value}=await reader.read();if(done)break;buffer+=decoder.decode(value,{stream:true});if(buffer.length>2*1024*1024){buffer='';report('unsupported-large-stream-frame');}
      let cut;while((cut=buffer.indexOf('\n\n'))>=0){const frame=buffer.slice(0,cut);buffer=buffer.slice(cut+2);for(const line of frame.split('\n')){if(!line.startsWith('data:'))continue;let item;try{item=JSON.parse(line.slice(5).trim())}catch{continue;}
        const frameId=item.conversation_id;if(frameId&&parser.UUID.test(frameId)){if(cid&&cid!==frameId){report('conversation-id-conflict');return;}cid=frameId;if(!lastUser)lastUser=activeUsers.get(cid)||null;if(lastUser){activeUsers.set(cid,lastUser);emitRequest(lastUser,cid,'started');}}
        const message=item.message;if(!message||!cid)continue;
        if(message.author?.role==='user'){const text=parser.messageText(message);if(text){lastUser={id:typeof message.id==='string'?message.id:'',text};activeUsers.set(cid,lastUser);emitRequest(lastUser,cid,'started');}continue;}
        if(message.author?.role==='assistant'&&!message.recipient&&parser.messageText(message)){finalSeen=true;}
        if(message.author?.role==='assistant'&&/^(?:api_tool(?:\.|$)|AgentDock\.)/i.test(message.recipient||'')){
          const invocationId=typeof message.id==='string'&&message.id?message.id:`inv_${Date.now()}_${++synthetic}`;
          const activity=parser.invocationActivity(message,{conversationId:cid,title:titleFor(cid)},lastUser,invocationId);if(activity)emitTool(activity,'started');
          lastInvocation={message,user:lastUser,id:invocationId};continue;
        }
        if(message.author?.role==='tool'){
          emitBindings(parser.extractTool(message,{conversationId:cid,title:titleFor(cid)},lastInvocation?.message,lastInvocation?.user));
          const activity=parser.resultActivity(message,{conversationId:cid,title:titleFor(cid)},lastInvocation?.message,lastInvocation?.user,lastInvocation?.id);if(activity)emitTool(activity,'completed');
        }
      }}}
    }finally{if(finalSeen&&cid&&lastUser){emitRequest(lastUser,cid,'completed');activeUsers.delete(cid);}reader.releaseLock();}
  }
  window.fetch=function(...args){
    let requestUrl,expectedId=null;try{requestUrl=new URL(typeof args[0]==='string'||args[0] instanceof URL?String(args[0]):args[0].url,location.href);}catch{return nativeFetch.apply(this,args);}
    const relevant=requestUrl.origin==='https://chatgpt.com'&&/^\/backend-api\/(?:f\/)?conversation(?:\/|$)/.test(requestUrl.pathname);
    if(!relevant)return nativeFetch.apply(this,args);
    const pathId=requestUrl.pathname.split('/').filter(Boolean).pop();if(parser.UUID.test(pathId||''))expectedId=pathId;
    const body=args[1]?.body;let requestUser=null;if(typeof body==='string'&&body.length<1048576){try{const parsed=JSON.parse(body);const id=parsed.conversation_id;if(parser.UUID.test(id||''))expectedId=id;requestUser=parser.requestUser(parsed);}catch{}}
    if(expectedId&&requestUser){activeUsers.set(expectedId,requestUser);emitRequest(requestUser,expectedId,'started');pollMapping(expectedId,requestUser);}
    const originalPromise=nativeFetch.apply(this,args);
    originalPromise.then(response=>{if(!response.ok)return;const type=response.headers.get('content-type')||'';let copy;try{copy=response.clone()}catch{return;}
      if(type.includes('text/event-stream'))void stream(copy,expectedId,requestUser).catch(()=>report('stream-format-unavailable'));
      else if(type.includes('json'))void boundedText(copy,16*1024*1024).then(t=>{let data;try{data=JSON.parse(t)}catch{report('json-unavailable');return;}const cid=data?.conversation_id||data?.id||expectedId;if(requestUser&&parser.UUID.test(cid||''))emitRequest(requestUser,cid,'started');if(data?.mapping){emitBindings(parser.inspectConversation(data,expectedId));report('conversation-json-observed');const final=Object.values(data.mapping).some(n=>n?.message?.author?.role==='assistant'&&!n.message.recipient&&parser.messageText(n.message));if(final&&requestUser&&parser.UUID.test(cid||'')){emitRequest(requestUser,cid,'completed');activeUsers.delete(cid);}}else report('unsupported-response-shape');}).catch(()=>report('bounded-history-limit'));
    }).catch(()=>{});
    return originalPromise;
  };
  report('observer-ready');
  const currentId=parser.conversationId(location.href);if(currentId)void hydrateMapping(currentId,null);
  if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',startDomObserver,{once:true});else startDomObserver();
  setInterval(()=>report('observer-ready'),30000);
})();

