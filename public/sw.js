// Cache only UI assets. Never cache API data, identity, credentials, or login responses.
const CACHE='agent-monitor-ui-v0.2.0';
self.addEventListener('install',event=>event.waitUntil(
 caches.open(CACHE).then(cache=>Promise.all(['/offline.html','/icons/icon-192.png','/icons/icon-512.png'].map(async url=>{const response=await fetch(url,{redirect:'error',cache:'no-store'});if(!response.ok||response.type!=='basic')throw new Error('UI asset unavailable');await cache.put(url,response);}))).then(()=>self.skipWaiting())
));
self.addEventListener('activate',event=>event.waitUntil(
 caches.keys().then(keys=>Promise.all(keys.filter(k=>k.startsWith('agent-monitor-ui-')&&k!==CACHE).map(k=>caches.delete(k)))).then(()=>self.clients.claim())
));
self.addEventListener('fetch',event=>{
 const request=event.request,url=new URL(request.url);
 if(request.method!=='GET'||url.origin!==self.location.origin||url.pathname.startsWith('/api/')||url.pathname.startsWith('/cdn-cgi/'))return;
 if(request.mode==='navigate'){
  event.respondWith(fetch(request).catch(()=>caches.match('/offline.html')));
  return;
 }
 if(!url.pathname.startsWith('/assets/')&&!url.pathname.startsWith('/icons/')&&url.pathname!=='/offline.html')return;
 event.respondWith(fetch(request).then(response=>{
  if(response.ok&&!response.redirected&&response.type==='basic'){
   const copy=response.clone();event.waitUntil(caches.open(CACHE).then(cache=>cache.put(request,copy)));
  }
  return response;
 }).catch(()=>caches.match(request)));
});
