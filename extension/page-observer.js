/* Exact page observations only. Credentials stay in this page closure; never sent to the extension. */
(() => {
  'use strict';
  const parser=globalThis.AgentMonitorParserV055;if(!parser)return;
  const VERSION='0.5.6', CHANNEL='agent-monitor:attribution:v1';
  if(window.__agentMonitorObserver?.version===VERSION)return;
  window.__agentMonitorObserver?.dispose?.();
  const nativeFetch=window.fetch;
  const sent=new Set(),pending=new Map(),titles=new Map(),active=new Map(),hydrating=new Set(),nextMappingRead=new Map(),timers=[];
  let stopped=false,domObserver=null,lastDomKey='',authorization='',sessionAttemptAt=0,mappingNotBefore=0;
  function title(cid){if(titles.has(cid))return titles.get(cid);return parser.conversationId(location.href)===cid?document.title.replace(/\s*[-–·]\s*ChatGPT$/,''):'';}
  function report(kind,error=''){window.postMessage({channel:CHANNEL,type:'observer-status',version:VERSION,kind,error:String(error).slice(0,200)},'https://chatgpt.com');}
  function send(type,payload,key){
    if(sent.has(key)||pending.has(key))return;
    if(pending.size>=5000){report('capture-queue-full','页面待发送队列已满');return;}
    const event={channel:CHANNEL,type,payload,eventId:key,version:VERSION};pending.set(key,event);window.postMessage(event,'https://chatgpt.com');
  }
  function ack(event){if(event.source!==window||event.origin!=='https://chatgpt.com')return;const d=event.data;if(d?.channel!==CHANNEL||d.type!=='ack'||!d.ok)return;if(pending.delete(d.eventId)){sent.add(d.eventId);if(sent.size>20000)sent.delete(sent.values().next().value);}}
  window.addEventListener('message',ack);
  function request(user,cid,phase){
    if(!user?.id||!parser.UUID.test(cid||''))return;
    send('request',{conversationId:cid,conversationUrl:`https://chatgpt.com/c/${cid}`,title:title(cid),userMessageId:user.id,userMessageText:user.text||'[附件消息]',phase},`${cid}:${user.id}:request:${phase}`);
  }
  function tool(activity,phase){if(!activity?.invocationId||!activity?.userMessageId)return;send('tool',{...activity,phase},`${activity.conversationId}:${activity.userMessageId}:${activity.invocationId}:${phase}`);}
  function bindings(items){for(const item of items)send('binding',item,`${item.conversationId}:${item.entityId}:binding`);}
  function track(cid,user){if(!cid||!user?.id)return;const key=`${cid}:${user.id}`;if(!active.has(key))active.set(key,{cid,user,next:0});request(user,cid,'started');}
  async function text(response,max=16*1024*1024){
    const reader=response.body?.getReader();if(!reader)return '';let out='',size=0;const dec=new TextDecoder();
    try{for(;;){const {done,value}=await reader.read();if(done)break;if((size+=value.length)>max){void reader.cancel();throw new Error('response-size-limit');}out+=dec.decode(value,{stream:true});}return out+dec.decode();}finally{reader.releaseLock();}
  }
  function applyMapping(raw,expected,user){
    const data=parser.normalizeConversation(raw),cid=data?.conversation_id||data?.id||expected;
    if(!data?.mapping||!parser.UUID.test(cid||'')||(expected&&cid!==expected))return false;
    const view=parser.inspectRequestActivities(data,cid,user?.id),current=view.user;if(!current)return false;
    if(typeof data.title==='string'&&data.title)titles.set(cid,data.title.slice(0,250));
    if(current.text==='[附件消息]'&&user?.id===current.id&&user.text)current.text=user.text;
    track(cid,current);for(const a of view.activities)tool({...a,userMessageText:current.text},a.phase);bindings(parser.inspectConversation(data,cid,current.id));
    if(view.final){request(current,cid,'completed');active.delete(`${cid}:${current.id}`);}
    report(view.activities.length?'mapping-tools-observed':'mapping-request-observed');
    return true;
  }
  async function sessionAuthorization(force=false){
    // Use the same-origin signed-in web session. Do not read browser profiles/cookies or persist the token.
    if(!force&&Date.now()-sessionAttemptAt<60000)return;sessionAttemptAt=Date.now();
    try{const res=await nativeFetch.call(window,'/api/auth/session',{credentials:'include',cache:'no-store'});if(!res.ok)return;const s=JSON.parse(await text(res,128*1024));if(typeof s.accessToken==='string')authorization='Bearer '+s.accessToken;}catch{}
  }
  async function hydrate(cid,user){
    if(stopped||!parser.UUID.test(cid||'')||hydrating.has(cid)||Date.now()<mappingNotBefore||Date.now()<(nextMappingRead.get(cid)||0))return;
    hydrating.add(cid);nextMappingRead.set(cid,Date.now()+15000);
    try{
      // An unauthenticated conversation read can return 404 rather than 401. Authenticate first.
      if(!authorization)await sessionAuthorization();
      const failures=[];
      for(const route of ['/backend-api/conversation/','/backend-api/f/conversation/']){
        for(let attempt=0;attempt<2;attempt++){
          const res=await nativeFetch.call(window,route+cid,{credentials:'include',cache:'no-store',headers:authorization?{Authorization:authorization}:{}});
          if(res.status===429||res.status===503){
            const delay=parser.retryDelay(res.headers.get('retry-after'));mappingNotBefore=Date.now()+delay;
            report('mapping-rate-limited',`HTTP ${res.status}; next mapping read after ${Math.ceil(delay/1000)}s`);return;
          }
          if(res.status===401&&attempt===0){const hadToken=!!authorization;authorization='';await sessionAuthorization(hadToken);if(authorization)continue;}
          if(!res.ok){failures.push(`${route.includes('/f/')?'f':'standard'}:${res.status}`);break;}
          let raw;try{raw=JSON.parse(await text(res));}catch{failures.push('invalid-json');break;}
          if(applyMapping(raw,cid,user))return;
          failures.push('missing-exact-mapping');break;
        }
      }
      report('mapping-unavailable',`${authorization?'authenticated':'no-session-token'} ${failures.join(', ')}`);
    }catch{report('mapping-network-error','Conversation GET 网络失败');}finally{hydrating.delete(cid);}
  }
  function scanDom(){
    const cid=parser.conversationId(location.href);if(!cid)return;
    const nodes=document.querySelectorAll('[data-message-author-role="user"]'),node=nodes[nodes.length-1];if(!node)return;
    const id=node.getAttribute('data-message-id')||node.closest('[data-message-id]')?.getAttribute('data-message-id');
    if(!id){if(lastDomKey!==`${cid}:missing`){lastDomKey=`${cid}:missing`;void hydrate(cid,null);}return;}
    const key=`${cid}:${id}`;if(key===lastDomKey)return;lastDomKey=key;
    const user={id,text:String(node.textContent||'').trim().slice(0,4000)||'[附件消息]'};
    track(cid,user);void hydrate(cid,user);
  }
  function readBody(input,init){
    const body=init?.body;if(typeof body==='string')return Promise.resolve(body);
    if(input instanceof Request)return input.clone().text().catch(()=>'');
    return Promise.resolve('');
  }
  async function stream(response,expected,user){
    const reader=response.body?.getReader();if(!reader)return;
    let cid=expected,buffer='',current=null,lastInvocation=null;const dec=new TextDecoder(),mapping={},owners={};
    function accept(item){
      if(!item||typeof item!=='object')return;
      if(item.v&&typeof item.v==='object'&&!Array.isArray(item.v)&&item.v.message){accept(item.v);return;}
      if(item.conversation_id&&parser.UUID.test(item.conversation_id)){if(cid&&cid!==item.conversation_id){report('conversation-id-conflict');return;}cid=item.conversation_id;}
      // ChatGPT delta streams can wrap the full message in v, then patch a known message field.
      if(item.p==='/message'&&item.v&&typeof item.v==='object'){item={message:item.v};}
      if(item.message)current=item.message;
      else if(current&&typeof item.p==='string'&&item.p.startsWith('/message/')&&['append','replace','add'].includes(item.o)){
        const parts=item.p.slice(9).split('/');if(parts.some(p=>['__proto__','prototype','constructor'].includes(p)))return;
        let obj=current;for(let i=0;i<parts.length-1;i++){if(!obj||typeof obj!=='object')return;obj=obj[parts[i]];}if(!obj)return;const k=parts[parts.length-1];
        if(item.o==='append'&&typeof obj[k]==='string'&&typeof item.v==='string')obj[k]+=item.v;else if(item.o==='append'&&Array.isArray(obj[k]))obj[k].push(...(Array.isArray(item.v)?item.v:[item.v]));else obj[k]=item.v;
      }else return;
      const message=current;if(!cid||!message)return;
      if(message.author?.role==='user'){user={id:message.id,text:parser.messageText(message)};track(cid,user);return;}
      if(user)track(cid,user);
      const context={conversationId:cid,title:title(cid)};
      if(message.id)mapping[message.id]=message;
      if(parser.invocationIdentity(message)){const a=parser.invocationActivity(message,context,user,message.id);if(a){tool(a,'started');owners[message.id]=user;}lastInvocation=message;}
      else if(message.author?.role==='tool'){
        const parent=message.metadata?.parent_id||item.parent_id;
        const invocation=parent?mapping[parent]:lastInvocation;
        const owner=invocation&&owners[invocation.id];
        if(invocation&&owner&&['finished_successfully','finished_failed'].includes(message.status)){const a=parser.resultActivity(message,context,invocation,owner,invocation.id);if(a)tool(a,'completed');bindings(parser.extractTool(message,context,invocation,owner));}
      }
      if(parser.isFinal(message)&&user){request(user,cid,'completed');active.delete(`${cid}:${user.id}`);}
    }
    try{for(;;){const {done,value}=await reader.read();if(done)break;buffer+=dec.decode(value,{stream:true});if(buffer.length>2*1024*1024){report('stream-frame-limit');return;}
      let match;while((match=/\r?\n\r?\n/.exec(buffer))){const frame=buffer.slice(0,match.index);buffer=buffer.slice(match.index+match[0].length);const raw=frame.split(/\r?\n/).filter(l=>l.startsWith('data:')).map(l=>l.slice(5).trimStart()).join('\n');try{accept(JSON.parse(raw));}catch{}}
    }}finally{reader.releaseLock();if(cid)void hydrate(cid,user);}
  }
  const wrapped=function(...args){
    let url;try{url=new URL(typeof args[0]==='string'||args[0] instanceof URL?String(args[0]):args[0].url,location.href);}catch{return nativeFetch.apply(this,args);}
    const relevant=url.origin===location.origin&&/^\/backend-api\/(?:f\/)?conversation(?:\/|$)/.test(url.pathname);
    if(!relevant)return nativeFetch.apply(this,args);
    // Existing request authorization stays in memory and is used only for the same-origin mapping read.
    try{const headers=new Headers(args[1]?.headers||(args[0] instanceof Request?args[0].headers:undefined));const a=headers.get('authorization');if(a?.startsWith('Bearer '))authorization=a;}catch{}
    const metadata=readBody(args[0],args[1]).then(body=>{let parsed={};try{if(body.length<1048576)parsed=JSON.parse(body);}catch{}const pathId=url.pathname.split('/').pop(),cid=parser.UUID.test(parsed.conversation_id||'')?parsed.conversation_id:parser.UUID.test(pathId||'')?pathId:null,user=parser.requestUser(parsed);if(cid&&user)track(cid,user);return {cid,user};});
    const promise=nativeFetch.apply(this,args);
    promise.then(async response=>{if(!response.ok)return;const {cid,user}=await metadata;let copy;try{copy=response.clone();}catch{return;}const type=response.headers.get('content-type')||'';
      if(type.includes('text/event-stream'))void stream(copy,cid,user).catch(()=>report('stream-format-error'));
      else if(type.includes('json'))void text(copy).then(raw=>{let data;try{data=JSON.parse(raw);}catch{return;}if(!applyMapping(data,cid,user)){
        const id=data?.conversation_id||data?.conversation?.id;if(user&&parser.UUID.test(id||'')){track(id,user);void hydrate(id,user);}
        // prepare/resume envelopes are not conversation mappings; don't overwrite a real diagnostic.
      }}).catch(()=>report('mapping-size-limit'));
    }).catch(()=>{});return promise;
  };
  window.fetch=wrapped;
  function start(){scanDom();domObserver=new MutationObserver(()=>{scanDom();});domObserver.observe(document.body||document.documentElement,{subtree:true,childList:true});}
  if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',start,{once:true});else start();
  timers.push(setInterval(()=>{scanDom();for(const item of active.values())if(item.next<Date.now()){item.next=Date.now()+15000;void hydrate(item.cid,item.user);}for(const event of pending.values())window.postMessage(event,'https://chatgpt.com');},5000));
  timers.push(setInterval(()=>report('observer-ready'),30000));
  window.__agentMonitorObserver={version:VERSION,dispose(){stopped=true;timers.forEach(clearInterval);domObserver?.disconnect();window.removeEventListener('message',ack);document.removeEventListener('DOMContentLoaded',start);if(window.fetch===wrapped)window.fetch=nativeFetch;}};
  report('observer-ready');const cid=parser.conversationId(location.href);if(cid)void hydrate(cid,null);
})();
