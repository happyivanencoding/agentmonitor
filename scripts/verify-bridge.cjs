// Exercise the real local bridge without adding any conversation binding.
const fs=require('node:fs/promises');const path=require('node:path');const http=require('node:http');const assert=require('node:assert/strict');
(async()=>{
 const root=path.join(process.env.LOCALAPPDATA,'AgentMonitor');const key=(await fs.readFile(path.join(root,'bridge.key'),'utf8')).trim();
 const results=[];
 function request(method,route,body='',headers={}){return new Promise((resolve,reject)=>{const req=http.request({hostname:'127.0.0.1',port:43217,path:route,method,headers:{Host:'127.0.0.1:43217','Content-Type':'application/json','Content-Length':Buffer.byteLength(body),...headers},timeout:4000},res=>{let text='';res.on('data',chunk=>text+=chunk);res.on('end',()=>resolve({status:res.statusCode,body:text,headers:res.headers}))});req.on('timeout',()=>req.destroy(new Error('Bridge timeout')));req.on('error',reject);req.end(body)})}
 const auth={Authorization:'Bearer '+key};
 async function check(name,expected,method,route,body='',headers={}){const r=await request(method,route,body,headers);assert.equal(r.status,expected,`${name}: ${r.body}`);results.push({name,status:r.status,passed:true});return r;}
 const health=await check('Minimal unauthenticated health',200,'GET','/health');const publicBody=JSON.parse(health.body);assert.deepEqual(Object.keys(publicBody).sort(),['app','version']);
 await check('No unauthenticated snapshot API',401,'GET','/snapshot');
 await check('Missing bearer rejected',401,'POST','/v1/bind','{}');
 await check('Wrong bearer rejected',401,'POST','/v1/bind','{}',{Authorization:'Bearer wrong'});
 await check('Untrusted web origin rejected',403,'POST','/v1/bind','{}',{...auth,Origin:'https://evil.example'});
 await check('Page origin cannot bypass extension relay',403,'POST','/v1/bind','{}',{...auth,Origin:'https://chatgpt.com'});
 await check('DNS-rebinding Host rejected',403,'POST','/v1/bind','{}',{...auth,Host:'evil.example:43217'});
 await check('Oversized body rejected',413,'POST','/v1/bind',JSON.stringify({padding:'x'.repeat(17000)}),auth);
 await check('Non-JSON body rejected',415,'POST','/v1/bind','hello',{...auth,'Content-Type':'text/plain'});
 await check('Malformed JSON rejected',400,'POST','/v1/bind','{',auth);
 await check('Unknown ACP identity rejected',422,'POST','/v1/bind',JSON.stringify({entityId:'acps_ffffffffffffffffffffffffffffffff',conversationUrl:'https://chatgpt.com/c/00000000-0000-4000-8000-000000000003',title:'must not be saved'}),auth);
 await check('No arbitrary authenticated route',404,'POST','/v1/exec','{}',auth);
 const preflight=await check('Extension-origin preflight allowed',204,'OPTIONS','/v1/bind','',{Origin:'chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'});assert.equal(preflight.headers['access-control-allow-origin'],'chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
 const report={testedAt:new Date().toISOString(),passed:results.length,results,bindingWrites:0,credentialLogged:false};
 await fs.writeFile(path.resolve('.local/bridge-verification.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
})().catch(e=>{console.error(e.message);process.exitCode=1});
