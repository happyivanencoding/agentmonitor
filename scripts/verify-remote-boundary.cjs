// Verify the actual public-origin boundary and real HTTP data path without weakening Access.
const http=require('node:http');const https=require('node:https');const zlib=require('node:zlib');const assert=require('node:assert/strict');const fs=require('node:fs/promises');const path=require('node:path');
const publicHost=process.env.AGENT_MONITOR_PUBLIC_HOST,local='http://127.0.0.1:43218';if(!publicHost)throw new Error('AGENT_MONITOR_PUBLIC_HOST is required for public-boundary verification');
function request(url,headers={},method='GET',body){return new Promise((resolve,reject)=>{const u=new URL(url);const r=(u.protocol==='https:'?https:http).request(u,{method,headers,timeout:15000,agent:false},response=>{let data=[];response.on('data',b=>data.push(b));response.on('end',()=>resolve({status:response.statusCode,headers:response.headers,body:Buffer.concat(data)}));});r.on('error',reject);r.on('timeout',()=>r.destroy(new Error('HTTP verification timed out')));if(body)r.write(body);r.end();})}
async function main(){
 const tests=[];const check=async(label,url,headers,expected,method='GET',body)=>{const r=await request(url,headers,method,body);assert.equal(r.status,expected,label);tests.push(label);console.log("Verified:",label);return r};
 await check('Configured public host requires a signed Access JWT',local+'/api/snapshot',{Host:publicHost},401);
 await check('An identity header alone cannot authenticate',local+'/api/snapshot',{Host:publicHost,'Cf-Access-Authenticated-User-Email':'not-an-identity@example.invalid'},401);
 await check('A forwarded loopback request cannot bypass authentication',local+'/api/snapshot',{'Cf-Connecting-Ip':'198.51.100.42'},403);
 await check('Unknown host is refused',local+'/api/snapshot',{Host:'other.example.invalid'},403);
 await check('Foreign browser origin is refused',local+'/api/snapshot',{Origin:'https://example.invalid'},403);
 await check('Mutations require same-origin request',local+'/api/bind',{'Content-Type':'application/json','X-Agent-Monitor':'1'},403,'POST','{}');
 await check('Mutations require explicit Monitor client header',local+'/api/bind',{Origin:local,'Content-Type':'application/json'},403,'POST','{}');
 for(const p of ['/bridge.key','/remote.json','/api/get_pairing_token','/api/exec'])await check('No remote native/secret endpoint '+p,local+p,{},404);
 await check('JSON body limit is enforced',local+'/api/bind',{Origin:local,'Content-Type':'application/json','X-Agent-Monitor':'1','Content-Length':'17000'},413,'POST',' '.repeat(17000));
 const gzip=await check('Live snapshot is available to the local cooperating operator',local+'/api/snapshot',{'Accept-Encoding':'gzip'},200);
 assert.equal(gzip.headers['content-encoding'],'gzip');assert.equal(gzip.headers['cache-control'],'no-store');const raw=zlib.gunzipSync(gzip.body),snapshot=JSON.parse(raw);assert.ok(snapshot.agents.length>0);assert.ok(gzip.body.length<raw.length/2);tests.push('Gzip materially reduces the actual snapshot; API responses are not cached');
 const version=Math.max(snapshot.generatedAt||0,snapshot.lastAttemptAt||0);const unchanged=await request(local+'/api/snapshot?since='+version);
 if(unchanged.status===204){assert.equal(unchanged.body.length,0);tests.push('Unchanged snapshot returns body-free 204');}else{assert.equal(unchanged.status,200);assert.notEqual(JSON.parse(unchanged.body).generatedAt,snapshot.generatedAt);tests.push('A newer collector snapshot is not incorrectly suppressed');}
 const setup=JSON.parse((await request(local+'/api/setup')).body);assert.equal(setup.publicUrl,'https://'+publicHost);assert.ok(!JSON.stringify(setup).includes('bridgeToken'));assert.equal(setup.capabilities.agentControl,false);
 const publicResults=[];
 for(const p of ['/','/api/snapshot']){
  const r=await request('https://'+publicHost+p);assert.equal(r.status,302,'Cloudflare must challenge anonymous '+p);
  const target=new URL(r.headers.location);assert.ok(target.hostname.endsWith('.cloudflareaccess.com'));assert.ok(!r.body.toString().includes('tokensUsed'));
  publicResults.push({path:p,status:r.status,loginHost:target.hostname});
 }
 tests.push('Public HTTPS page and API both challenge anonymous requests through Cloudflare Access');
 const result={status:'passed',tests,publicResults,rawSnapshotBytes:raw.length,compressedBytes:gzip.body.length,verifiedAt:new Date().toISOString(),authenticatedPublicUserLogin:'not verified; requires the owner login'};
 await fs.mkdir('.local/web-qa',{recursive:true});await fs.writeFile('.local/web-qa/boundary-report.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}
main().catch(e=>{console.error(e.message);process.exitCode=1});
