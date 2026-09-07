/* Shared pure parser: browser MAIN world and Node tests. No network, DOM, storage or secrets. */
(function(root){
  'use strict';
  const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  const ACP=/^acps_[a-z0-9]{8,64}$/i;
  const SAFE_ACTIONS=new Set(['create','new','start','open','resume','prompt']);
  function conversationId(url){try{const u=new URL(url);if(u.protocol!=='https:'||u.hostname!=='chatgpt.com')return null;const m=u.pathname.match(/(?:^|\/)c\/([^/]+)$/);return m&&UUID.test(m[1])?m[1]:null}catch{return null}}
  function parse(text){if(typeof text!=='string'||text.length>1048576)return null;try{return JSON.parse(text)}catch{return null}}
  function messageText(message){
    if(!message||typeof message!=='object')return '';
    const c=message.content||{};const parts=Array.isArray(c.parts)?c.parts:[];
    return (typeof c.text==='string'?c.text:parts.filter(p=>typeof p==='string').join('\n')).trim().slice(0,4000);
  }
  function userRecord(message){
    if(message?.author?.role!=='user'||!message.id||message.metadata?.is_visually_hidden_from_conversation===true)return null;
    if(['user_editable_context','model_editable_context'].includes(message.content?.content_type))return null;
    return {id:message.id,text:messageText(message)||'[附件消息]'};
  }
  function requestUser(body){
    if(!body||typeof body!=='object'||!Array.isArray(body.messages))return null;
    for(let i=body.messages.length-1;i>=0;i--){const m=body.messages[i];if(m?.author?.role==='user'){const record=userRecord(m);if(record)return record;}}
    return null;
  }
  function retryDelay(value,now=Date.now()){
    const raw=typeof value==='string'?value.trim():'';
    const seconds=raw&&/^\d+(?:\.\d+)?$/.test(raw)?Number(raw):NaN;
    const delay=Number.isFinite(seconds)?seconds*1000:Date.parse(raw)-now;
    return Number.isFinite(delay)?Math.max(60000,delay):60000;
  }
  function canonicalToolName(raw){
    if(typeof raw!=='string')return null;
    const m=raw.match(/^\/?AgentDock[/.](?:link_[a-z0-9]+\/)?([a-z][a-z0-9_.]*)$/i);
    return m?'AgentDock.'+m[1]:null;
  }
  function invocationIdentity(invocation){
    let call=invocation;
    if(call?.author?.role==='assistant'){
      const recipient=String(call.recipient||'');const c=call.content||{};
      if(canonicalToolName(recipient)){
        const parsed=parse(c.text)||parse(c.parts?.find(p=>typeof p==='string'))||{};
        return {name:canonicalToolName(recipient),args:parsed.arguments||parsed.args||parsed.parameters||parsed||{}};
      }
      if(!/^api_tool(?:\.|$)/.test(recipient))return null;
      call=parse(c.text)||parse(c.parts?.find(p=>typeof p==='string'));
    }
    if(!call||typeof call!=='object')return null;
    const raw=call.name||call.tool||call.tool_name||call.path||call.uri||'';
    const qualified=canonicalToolName(raw);
    if(typeof qualified!=='string'||!/^AgentDock\./i.test(qualified))return null;
    return {name:qualified,args:call.arguments||call.args||call.parameters||{}};
  }
  function toolIdentity(message,invocation){
    const author=message?.author;if(author?.role!=='tool')return null;
    const name=typeof author.name==='string'?author.name:'';
    const invoked=invocationIdentity(invocation);if(invoked)return invoked;
    if(canonicalToolName(name))return {name:canonicalToolName(name),args:{}};
    if(!/^api_tool(?:\.|$)/.test(name))return null;
    return null;
  }
  function safeActivity(identity,context,userTurn,invocationId){
    if(!identity||!context||!UUID.test(context.conversationId||'')||!userTurn?.id)return null;
    const a=identity.args||{};
    const safe={};for(const k of ['action','path','workdir','project','runtime','task_id','step_id','title'])if(typeof a[k]==='string'||typeof a[k]==='number'||typeof a[k]==='boolean')safe[k]=String(a[k]).slice(0,500);
    const directProject=[a.project,a.workdir].find(v=>typeof v==='string'&&v.trim());const pathHint=typeof a.path==='string'&&!/^skill:\/\//i.test(a.path)&&!/[\\/]\.agentdock(?:[\\/]|$)/i.test(a.path)&&!/AppData[\\/]Local[\\/]AgentDock/i.test(a.path)?a.path:'';const projectHint=directProject||pathHint||'';
    // A create result proves creation; an explicit checkpoint proves participation, not creation.
    // Merely reading/listing a Task is never a plan association.
    const taskId=/\.task_manage$/i.test(identity.name)&&a.action==='checkpoint'&&typeof a.task_id==='string'&&/^tsk_[a-z0-9]+$/i.test(a.task_id)?a.task_id:'';
    return {invocationId:String(invocationId||'').slice(0,160),toolName:identity.name.slice(0,160),action:typeof a.action==='string'?a.action.slice(0,80):'',argsSummary:JSON.stringify(safe).slice(0,1200),projectHint:String(projectHint).slice(0,500),taskId,entityId:'',conversationId:context.conversationId,conversationUrl:`https://chatgpt.com/c/${context.conversationId}`,title:typeof context.title==='string'?context.title.slice(0,250):'',userMessageId:String(userTurn.id).slice(0,160),userMessageText:String(userTurn.text||'').slice(0,4000)};
  }
  function invocationActivity(invocation,context,userTurn,invocationId){return safeActivity(invocationIdentity(invocation),context,userTurn,invocationId||invocation?.id)}
  function objects(content){
    const out=[];
    function visit(v,depth){if(depth>5||v==null)return;if(typeof v==='string'){const j=parse(v);if(j)visit(j,depth+1);return;}if(Array.isArray(v)){v.slice(0,25).forEach(x=>visit(x,depth+1));return;}if(typeof v!=='object')return;
      out.push(v);
      // Known transport envelopes only. Never recursively scan arbitrary stdout/text/messages.
      for(const k of ['structuredContent','structured_content','result','data','session'])if(v[k]&&typeof v[k]==='object')visit(v[k],depth+1);
      if(Array.isArray(v.content))for(const part of v.content.slice(0,25))if(part.type==='text')visit(part.text,depth+1);
    }
    if(['text','code','execution_output'].includes(content?.content_type)){visit(content.parts||content.text,0)}
    else if(content?.parts){visit(content.parts,0)}
    else visit(content,0);
    return out;
  }
  function resultActivity(message,context,invocation,userTurn,invocationId){
    const identity=toolIdentity(message,invocation),base=safeActivity(identity,context,userTurn,invocationId||invocation?.id);if(!base)return null;
    const rows=objects(message.content);let taskId=base.taskId,entityId='',ok=null;
    for(const row of rows){
      const tid=row.task_id||row.task?.id||row.task_summary?.id;if(/\.task_manage$/i.test(identity.name)&&(identity.args?.action==='create'||(!identity.args?.action&&row.action==='create'))&&typeof tid==='string'&&/^tsk_[a-z0-9]+$/i.test(tid))taskId=tid;
      const eid=row.acp_session_id||(/^acps_[a-z0-9]{8,64}$/i.test(row.id||'')?row.id:'');if(eid)entityId=eid;
      if(row.ok===false||row.command_ok===false||row.status==='failed'||row.status==='error')ok=false;else if(row.ok===true||row.command_ok===true)ok=true;
    }
    let argsSummary=base.argsSummary;
    if(/\.task_manage$/i.test(identity.name)&&identity.args?.action==='create'&&!taskId){
      const c=message.content||{},raw=typeof c.text==='string'?c.text:(c.parts||[]).find?.(x=>typeof x==='string')||'';
      const kind=parse(raw)?'json':/^Resource uri:/i.test(raw)?'resource-prefixed':/redacted/i.test(raw)?'redacted':/^```/.test(raw)?'fenced':'other';
      // Structural diagnostics only: never include the result text or any credentials.
      argsSummary=(argsSummary+` [result-shape ${c.content_type||'unknown'};${Object.keys(c).join(',')};${kind};task-field=${/"task_id"/.test(raw)}]`).slice(0,1200);
    }
    return {...base,argsSummary,taskId,entityId,ok};
  }
  function extractTool(message,context,invocation,userTurn){
    if(!context||!UUID.test(context.conversationId||''))return [];
    const identity=toolIdentity(message,invocation);if(!identity)return [];
    const exec=/\.exec_command$/i.test(identity.name),acpTool=/^AgentDock\.acp(?:_|\.)/i.test(identity.name);
    if(!exec&&!acpTool)return [];
    const action=identity.args?.action;
    if(!exec&&action&&!SAFE_ACTIONS.has(action))return [];
    // A list/get/inspect response is not evidence that this conversation launched the session.
    if(!exec&&/(?:list|inspect|status|observe|get)(?:$|_)/i.test(identity.name))return [];
    const rows=objects(message.content);
    const found=[];
    for(const row of rows){
      if(Array.isArray(row.sessions)||Array.isArray(row.items))continue;
      const marker=row.agent_monitor;
      const explicit=marker?.schema_version===1&&marker?.event==='acp.started'&&UUID.test(marker.conversation_id||'')&&marker.conversation_id===context.conversationId;
      if(exec&&!explicit)continue;
      const candidate=explicit?marker:row;
      const id=candidate.acp_session_id||candidate.id;
      const remote=candidate.remote_session_id;
      if(!ACP.test(id||'')||!UUID.test(remote||''))continue;
      if(!explicit&&candidate.agent&&candidate.agent!=='codex')continue;
      found.push({entityId:id,remoteSessionId:remote,conversationId:context.conversationId,conversationUrl:`https://chatgpt.com/c/${context.conversationId}`,title:typeof context.title==='string'?context.title.slice(0,250):'',evidence:'structured-acp-invocation-result',messageId:typeof message.id==='string'?message.id:'',userMessageId:typeof userTurn?.id==='string'?userTurn.id.slice(0,120):'',userMessageText:typeof userTurn?.text==='string'?userTurn.text.slice(0,2000):''});
    }
    return [...new Map(found.map(x=>[x.entityId,x])).values()];
  }
  function userAncestor(mapping,start){
    let cursor=start;const seen=new Set();
    while(cursor&&!seen.has(cursor)&&seen.size<20000){seen.add(cursor);const node=mapping[cursor];if(!node)break;const message=node.message;const record=userRecord(message);if(record)return record;cursor=node.parent;}
    return null;
  }
  function latestUser(data){
    if(!data?.mapping||typeof data.mapping!=='object')return null;
    const current=userAncestor(data.mapping,data.current_node);if(current)return current;
    if(data.current_node&&data.mapping[data.current_node])return null;
    let best=null,bestAt=-Infinity;
    for(const node of Object.values(data.mapping)){const message=node?.message;if(message?.author?.role!=='user')continue;const record=userRecord(message);if(!record)continue;const at=Number(message.create_time??node.create_time??0);if(at>=bestAt){bestAt=at;best=record;}}
    return best;
  }
  function isFinal(message){
    return message?.author?.role==='assistant'&&(!message.recipient||message.recipient==='all')&&message.end_turn===true&&message.status!=='in_progress'&&message.channel!=='analysis'&&message.channel!=='commentary';
  }
  function normalizeConversation(data){
    if(!data||typeof data!=='object')return data;
    if(data.mapping)return data;
    for(const k of ['conversation','data'])if(data[k]?.mapping)return data[k];
    if(Array.isArray(data.linear_conversation)){const mapping={};for(const node of data.linear_conversation){const m=node.message||node;if(m?.id)mapping[m.id]={message:m,parent:node.parent||m.metadata?.parent_id||null};}return {...data,mapping};}
    return data;
  }
  function inspectRequestActivities(data,expectedId,expectedUserMessageId){
    data=normalizeConversation(data);
    if(!data||typeof data!=='object'||!data.mapping||typeof data.mapping!=='object')return {user:null,activities:[],final:false};
    const cid=data.conversation_id||data.id||expectedId;if(!UUID.test(cid||'')||(expectedId&&cid!==expectedId))return {user:null,activities:[],final:false};
    const context={conversationId:cid,title:typeof data.title==='string'?data.title:''};
    let user=expectedUserMessageId?null:latestUser(data);
    if(expectedUserMessageId){for(const node of Object.values(data.mapping)){const message=node?.message;if(message?.author?.role==='user'&&message.id===expectedUserMessageId){user=userRecord(message);break;}}}
    if(!user)return {user:null,activities:[],final:false};
    const activities=[];let final=false;
    for(const [nodeId,node] of Object.entries(data.mapping)){const message=node?.message;if(!message)continue;const causal=userAncestor(data.mapping,nodeId);if(causal?.id!==user.id)continue;
      if(message.author?.role==='assistant'){
        if(invocationIdentity(message)){const activity=invocationActivity(message,context,user,message.id||nodeId);if(activity)activities.push({...activity,phase:'started'});}
        else if(isFinal(message))final=true;
      } else if(message.author?.role==='tool'&&message.status!=='in_progress'){
        const invocationNode=data.mapping[node.parent],invocation=invocationNode?.message;const activity=resultActivity(message,context,invocation,user,invocation?.id||node.parent||nodeId);if(activity)activities.push({...activity,phase:'completed'});
      }
    }
    return {user,activities:[...new Map(activities.map(activity=>[`${activity.invocationId}:${activity.phase}`,activity])).values()],final};
  }
  function inspectConversation(data,expectedId,expectedUserMessageId){
    data=normalizeConversation(data);
    if(!data||typeof data!=='object'||!data.mapping||typeof data.mapping!=='object')return [];
    const cid=data.conversation_id||data.id||expectedId;if(!UUID.test(cid||'')||(expectedId&&cid!==expectedId))return [];
    const context={conversationId:cid,title:typeof data.title==='string'?data.title:''};const out=[];
    for(const [nodeId,node] of Object.entries(data.mapping)){if(node?.message?.author?.role!=='tool')continue;const invocationNode=data.mapping[node.parent],previous=invocationNode?.message,user=userAncestor(data.mapping,invocationNode?.parent);if(expectedUserMessageId&&user?.id!==expectedUserMessageId)continue;out.push(...extractTool(node.message,context,previous,user));}
    return [...new Map(out.map(x=>[`${x.conversationId}:${x.entityId}`,x])).values()];
  }
  const api={retryDelay,userRecord,canonicalToolName,isFinal,normalizeConversation,conversationId,extractTool,inspectConversation,inspectRequestActivities,latestUser,requestUser,messageText,invocationIdentity,invocationActivity,resultActivity,UUID,ACP};
  if(typeof module==='object'&&module.exports)module.exports=api;
  else {root.AgentMonitorParserV054=api;if(!Object.getOwnPropertyDescriptor(root,'AgentMonitorParser')||Object.getOwnPropertyDescriptor(root,'AgentMonitorParser').configurable)Object.defineProperty(root,'AgentMonitorParser',{value:api,writable:false,configurable:true});}
})(typeof globalThis==='object'?globalThis:this);
