let current=null;
const status=document.getElementById('status');
function show(text,error=false){status.textContent=text;status.classList.toggle('error',error);}
async function send(message){const response=await chrome.runtime.sendMessage(message);if(!response?.ok)throw new Error(response?.error||'无法连接本机');return response;}
chrome.tabs.query({active:true,currentWindow:true}).then(tabs=>{const tab=tabs[0];if(tab?.url?.match(/^https:\/\/chatgpt\.com\/(?:.*\/)?c\/[0-9a-f-]{36}(?:[?#]|$)/i)){current={url:tab.url,title:(tab.title||'').replace(/\s*[-–·]\s*ChatGPT$/,'')};document.getElementById('current').textContent=current.title||current.url;}}).catch(()=>{});
// No key is read back into the popup; users only paste new credentials explicitly.
chrome.storage.local.get('status').then(({status:s})=>{document.getElementById('observer').textContent=s?.observer?`Observer: ${s.observer}`:'等待 ChatGPT 响应';});
async function check(){try{await send({type:'heartbeat'});show('已连接本机 Agent Monitor · 配对有效。')}catch(e){show(e.message,true)}}
document.getElementById('check').onclick=check;
document.getElementById('pair-form').onsubmit=async e=>{e.preventDefault();try{await send({type:'pair',key:document.getElementById('key').value.trim()});document.getElementById('key').value='';show('已配对。重新加载 ChatGPT 对话以开始观察。')}catch(e){show(e.message,true)}};
document.getElementById('bind-form').onsubmit=async e=>{e.preventDefault();if(!current){show('请先打开一个 ChatGPT 对话。',true);return;}try{await send({type:'bind-manual',payload:{entityId:document.getElementById('entity').value.trim(),conversationUrl:current.url,title:current.title}});show('显式绑定已保存。桌面视图会自动更新。')}catch(e){show(e.message,true)}};
check();
