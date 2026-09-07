import {invoke as nativeInvoke,isTauri} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import type {Snapshot} from './types';
export const desktop=isTauri();
export const APP_VERSION='0.5.5';

async function request<T>(path:string,body?:unknown):Promise<T>{
 const controller=new AbortController();const timer=setTimeout(()=>controller.abort(),12000);
 try{
  const response=await fetch(path,{method:body===undefined?'GET':'POST',credentials:'same-origin',cache:'no-store',redirect:'manual',signal:controller.signal,headers:body===undefined?{}:{'Content-Type':'application/json','X-Agent-Monitor':'1'},body:body===undefined?undefined:JSON.stringify(body)});
  if(response.status===401||response.type==='opaqueredirect'||response.status>=300&&response.status<400)throw new Error('登录已过期或尚未登录，请重新连接并登录。');
  if(response.status===204)return null as T;
  if(!response.headers.get('content-type')?.includes('application/json'))throw new Error('家里电脑暂不可达，或登录需要更新。请检查连接后重试。');
  const value=await response.json();if(!response.ok)throw new Error(value.error||`HTTP ${response.status}`);return value as T;
 }catch(e){if(e instanceof DOMException&&e.name==='AbortError')throw new Error('连接家里电脑超时。显示的是最后收到的快照，不是实时状态。');throw e;}finally{clearTimeout(timer)}
}
/** The web transport exposes only implemented Monitor operations, never arbitrary native IPC. */
export async function invoke<T=unknown>(command:string,args:Record<string,unknown>={}):Promise<T>{
 if(desktop)return nativeInvoke<T>(command,args);
 switch(command){
  case 'get_snapshot':return request<T>('/api/snapshot');
  case 'get_detail':return request<T>(`/api/detail?id=${encodeURIComponent(String(args.id||''))}`);
  case 'get_analytics':return request<T>(`/api/analytics?days=${Number(args.days)||1}`);
  case 'get_setup_info':return request<T>('/api/setup');
  case 'bind_agent':return request<T>('/api/bind',args.input);
  case 'unbind_agent':return request<T>('/api/unbind',args);
  case 'set_agent_archived':return request<T>('/api/archive',args);
  case 'link_task':return request<T>('/api/task',args);
  case 'open_conversation':{
   const url=new URL(String(args.url));
   if(url.origin!=='https://chatgpt.com'||!/\/c\/[0-9a-f-]{36}(?:\/|$)/i.test(url.pathname))throw new Error('无效的 ChatGPT 对话地址');
   window.open(url.href,'_blank','noopener,noreferrer');return undefined as T;
  }
  default:throw new Error('这项操作仅在家里电脑的 Windows 桌面版中提供。');
 }
}
export function watchSnapshots(accept:(v:Snapshot)=>void,failed:(reason:string)=>void):()=>void{
 let version=0;
 const deliver=(value:Snapshot)=>{const stamp=Math.max(value.generatedAt||0,value.lastAttemptAt||0,value.taskProgressAt||0,value.collectionCompletedAt||0);if(stamp>=version){version=stamp;accept(value);}};
 let stopped=false,busy=false,timer:ReturnType<typeof setTimeout>|undefined,unlisten:(()=>void)|undefined;
 const poll=async()=>{
  if(stopped||busy)return;
  if(!desktop&&document.visibilityState==='hidden'){timer=setTimeout(poll,3000);return;}
  busy=true;
  try{const value=desktop?await invoke<Snapshot>('get_snapshot'):await request<Snapshot|null>(`/api/snapshot?since=${version}`);if(value&&!stopped){deliver(value);}}catch(e){if(!stopped)failed(e instanceof Error?e.message:String(e));}
  finally{busy=false;if(!stopped){clearTimeout(timer);timer=setTimeout(poll,desktop?12000:3000);}}
 };
 const resume=()=>{clearTimeout(timer);void poll();};
 const offline=()=>failed('手机或浏览器当前离线。没有把旧数据当作实时结果。');
 if(desktop)listen<Snapshot>('monitor:snapshot',e=>{if(!stopped)deliver(e.payload)}).then(u=>{if(stopped)u();else unlisten=u;}).catch(()=>{});
 document.addEventListener('visibilitychange',resume);window.addEventListener('online',resume);window.addEventListener('offline',offline);void poll();
 return()=>{stopped=true;clearTimeout(timer);unlisten?.();document.removeEventListener('visibilitychange',resume);window.removeEventListener('online',resume);window.removeEventListener('offline',offline);};
}
