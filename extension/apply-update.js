// Extension-owned, deliberately launched updater. The URL flag prevents an automatic reload loop.
(async()=>{
  const url=new URL(location.href);
  if(url.searchParams.has('applied')){document.getElementById('status').textContent=`扩展 ${chrome.runtime.getManifest().version} 已加载；可以关闭此页。`;return;}
  url.searchParams.set('applied','1');history.replaceState(null,'',url.href);
  if(chrome.scripting)for(const tab of await chrome.tabs.query({url:'https://chatgpt.com/*'})){
    try{await chrome.scripting.executeScript({target:{tabId:tab.id},world:'MAIN',func:()=>{window.__agentMonitorObserver?.dispose?.();delete window.__agentMonitorObserver;}});}catch{}
    try{await chrome.scripting.executeScript({target:{tabId:tab.id},world:'ISOLATED',func:()=>{delete globalThis.__agentMonitorRelay055;}});}catch{}
  }
  chrome.runtime.reload();
})().catch(()=>{document.getElementById('status').textContent='请在浏览器扩展管理页重新加载 Agent Monitor。';});
