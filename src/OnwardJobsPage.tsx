import React,{useEffect,useMemo,useState} from 'react';
import {Activity,AlertTriangle,Building2,CheckCircle2,Clock,Cloud,Database,HardDrive,Layers3,MapPin,Network,RefreshCw,Timer,Wifi} from 'lucide-react';
import {invoke} from './transport';
import type {OnwardDistribution,OnwardFinanceSource,OnwardJobsDashboard,OnwardPipelineStatus,OnwardProviderContribution,OnwardSourceHealth,OnwardTaskHealth} from './types';

type Notify=(message:string)=>void;
const compact=(n:number|null|undefined)=>n==null?'—':Intl.NumberFormat('en',{notation:'compact',maximumFractionDigits:2}).format(n);
const full=(n:number|null|undefined)=>n==null?'—':n.toLocaleString('en-US');
const stamp=(ms?:number|null)=>ms?new Date(ms).toLocaleString('zh-CN',{month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false}):'—';
const ago=(ms?:number|null)=>{if(!ms)return '—';const s=Math.max(0,Math.floor((Date.now()-ms)/1000));return s<60?`${s}s 前`:s<3600?`${Math.floor(s/60)}m 前`:s<86400?`${Math.floor(s/3600)}h 前`:`${Math.floor(s/86400)}d 前`};
const size=(bytes?:number|null)=>bytes==null?'—':bytes>1024**3?`${(bytes/1024**3).toFixed(2)} GB`:bytes>1024**2?`${(bytes/1024**2).toFixed(1)} MB`:`${Math.round(bytes/1024)} KB`;
const statusLabel:Record<OnwardPipelineStatus,string>={success:'成功',running:'运行中',waiting:'等待',failed:'失败',missing:'缺失'};
function Health({status}:{status:OnwardPipelineStatus}){return <span className={`oj-health ${status}`}><i/>{statusLabel[status]}</span>}

function Kpi({label,value,detail,kind='default'}:{label:string;value:React.ReactNode;detail:string;kind?:string}){return <div className={`oj-kpi ${kind}`}><span>{label}</span><strong>{value}</strong><small>{detail}</small></div>}

function Bars({rows,maxRows=8}:{rows:OnwardDistribution[];maxRows?:number}){
 const shown=rows.slice(0,maxRows),max=Math.max(1,...shown.map(x=>x.count));
 return <div className="oj-bars">{shown.map(row=><div className="oj-bar-row" key={row.name}><div><span title={row.name}>{row.name}</span><b>{full(row.count)}</b></div><i><span style={{width:`${Math.max(1,row.count/max*100)}%`}}/></i></div>)}</div>
}

function ProviderBars({rows}:{rows:OnwardProviderContribution[]}){
 const max=Math.max(1,...rows.map(x=>x.primaryAttributedJobs));
 return <div className="oj-bars provider">{rows.map(row=><div className="oj-bar-row" key={row.provider}><div><span>{row.provider}</span><b>{full(row.primaryAttributedJobs)}</b></div><i><span style={{width:`${Math.max(.5,row.primaryAttributedJobs/max*100)}%`}}/></i><small>{full(row.providerUnique)} provider unique</small></div>)}</div>
}

function RunBars({rows}:{rows:{at:number;value:number;secondary?:number}[]}){
 const max=Math.max(1,...rows.flatMap(x=>[Math.abs(x.value),Math.abs(x.secondary||0)]));
 return <div className="oj-run-chart">{rows.map((row,i)=><div className="oj-run-col" key={`${row.at}-${i}`} title={`${stamp(row.at)} · +${row.value}${row.secondary!=null?` / -${row.secondary}`:''}`}><div className="oj-run-bars"><i style={{height:`${Math.max(2,Math.abs(row.value)/max*100)}%`}}/><i className="secondary" style={{height:`${Math.max(0,Math.abs(row.secondary||0)/max*100)}%`}}/></div><span>{new Date(row.at).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',hour12:false})}</span></div>)}</div>
}

function TaskPipeline({tasks}:{tasks:OnwardTaskHealth[]}){
 return <div className="oj-pipeline">{tasks.map((task,index)=><React.Fragment key={task.id}><div className={`oj-pipeline-node ${task.status}`}><div className="oj-pipeline-head"><Health status={task.status}/>{task.hidden&&<span className="oj-headless">无窗口</span>}</div><strong>{task.label}</strong><small>上次 {stamp(task.lastRunAt)}</small><small>下次 {stamp(task.nextRunAt)}</small><div className="oj-result">Exit {task.lastResult??'—'} · {task.state}</div></div>{index<tasks.length-1&&<div className="oj-pipeline-link"><span/><b>→</b></div>}</React.Fragment>)}</div>
}

function SourceGrid({sources}:{sources:OnwardSourceHealth[]}){
 return <div className="oj-source-grid">{sources.map(source=><article key={source.id} className={`oj-source ${source.status}`}><header><div className="oj-source-icon"><Network size={15}/></div><div><strong>{source.label}</strong><small>{source.id}</small></div><Health status={source.status}/></header><div className="oj-source-number"><b>{full(source.recentJobs)}</b><span>21天贡献</span></div>{source.lastBatchAdded!=null&&<div className="oj-last-batch"><b>+{full(source.lastBatchAdded)}</b><span>最近一次抓取 · {ago(source.lastBatchAt)}</span></div>}<footer><span>成功 {ago(source.lastSuccessAt)}</span><span>{source.nextRefreshAt?`下次 ${stamp(source.nextRefreshAt)}`:'持续调度'}</span></footer>{source.errorCount>0&&<div className="oj-source-error"><AlertTriangle size={12}/>{source.errorCount} 个近期错误{source.lastError?` · ${source.lastError}`:''}</div>}</article>)}</div>
}

function FinanceGrid({rows}:{rows:OnwardFinanceSource[]}){
 const max=Math.max(1,...rows.map(x=>x.recordCount));
 return <div className="oj-finance-grid">{rows.map(row=><article key={row.id}><header><Building2 size={14}/><strong>{row.label}</strong><Health status={row.status}/></header><div><b>{full(row.recordCount)}</b><span>当前官方岗位记录</span></div><i><span style={{width:`${Math.max(2,row.recordCount/max*100)}%`}}/></i><small>{row.nextRefreshAt?`下一次 ${stamp(row.nextRefreshAt)}`:`最近 ${stamp(row.lastSuccessAt)}`}</small>{row.lastError&&<p>{row.lastError}</p>}</article>)}</div>
}

export function OnwardJobsPage({notify}:{notify:Notify}){
 const [data,setData]=useState<OnwardJobsDashboard|null>(null),[loading,setLoading]=useState(true),[error,setError]=useState('');
 const load=async(silent=false)=>{if(!silent)setLoading(true);try{const value=await invoke<OnwardJobsDashboard>('get_onward_jobs');setData(value);setError('')}catch(e){const message=e instanceof Error?e.message:String(e);setError(message);if(!silent)notify(message)}finally{if(!silent)setLoading(false)}};
 useEffect(()=>{void load();const timer=setInterval(()=>{if(document.visibilityState==='visible')void load(true)},15000);return()=>clearInterval(timer)},[]);
 const latestRaw=useMemo(()=>data?.rawRuns.at(-1),[data]);
 const latestBuild=useMemo(()=>data?.buildRuns.at(-1),[data]);
 const latestSync=useMemo(()=>data?.syncRuns.at(-1),[data]);
 if(loading&&!data)return <div className="oj-loading"><RefreshCw className="spin" size={24}/><strong>读取 Onward 招聘管线</strong><span>只读状态、摘要与日志尾部，不扫描 Raw Layer。</span></div>;
 if(!data||!data.available)return <div className="notice danger"><AlertTriangle size={18}/><div><strong>Onward 招聘数据不可用</strong><p>{data?.error||error||'没有找到本机 job-data 状态。'}</p></div></div>;
 const fresh=data.vps.lastImportAt?Date.now()-data.vps.lastImportAt<45*60*1000:false;
 return <div className="onward-jobs-dashboard">
  {error&&<div className="notice danger"><AlertTriangle size={17}/><p>刷新失败，继续显示上一份成功快照：{error}</p></div>}
  <section className="oj-hero">
   <div className="oj-hero-copy"><span className="eyebrow">RAW → DEDUPE → INDEX → VPS → ONWARD V1</span><h2>法国招聘数据管线</h2><p>历史数据永久保留，Onward 搜索与市场视图只使用最近 21 天。Dashboard 只读派生状态，不读取原始岗位正文，也不会触发 API。</p></div>
   <div className={`oj-vps-state ${data.vps.healthy?'good':'bad'}`}><Cloud size={19}/><div><strong>{data.vps.healthy?'VPS 已同步':'VPS 同步需检查'}</strong><small>{data.vps.activeJobs?`${full(data.vps.activeJobs)} active jobs · `:''}{ago(data.vps.lastImportAt)}</small></div><i/></div>
  </section>
  <section className="oj-kpis">
   <Kpi label="CANONICAL INDEX" value={compact(data.totals.canonicalAll)} detail={`${full(data.totals.rawRecords)} raw occurrences`} kind="accent"/>
   <Kpi label="UNIQUE · 21D" value={compact(data.totals.unique21d)} detail={`${data.totals.duplicatePct.toFixed(1)}% raw→canonical dedupe`}/>
   <Kpi label="STAGE · 21D" value={full(data.totals.stage21d)} detail={`Alternance ${full(data.totals.alternance21d)}`} kind="green"/>
   <Kpi label="PARIS · 21D" value={compact(data.totals.paris21d)} detail={`Île-de-France ${full(data.totals.ileDeFrance21d)}`}/>
   <Kpi label="FINANCE OFFICIAL" value={compact(data.totals.financeOfficialCurrent)} detail={`${data.financeSources.length} 个官方机构源`} kind="blue"/>
   <Kpi label="LAST CANONICAL Δ" value={latestBuild?`${latestBuild.delta>=0?'+':''}${full(latestBuild.delta)}`:'—'} detail={`build ${ago(data.builtAt)}`}/>
  </section>

  <section className="panel oj-section pipeline-section"><div className="section-heading"><div><span className="eyebrow">SCHEDULED PIPELINE</span><h2>后台任务</h2><p>四个任务都应使用隐藏 launcher；红色或 Exit ≠ 0 需要检查。</p></div><span className="proof-badge connected"><Activity size={14}/>15 秒刷新</span></div><TaskPipeline tasks={data.tasks}/></section>

  <div className="oj-two-col">
   <section className="panel oj-section"><div className="section-heading"><div><span className="eyebrow">RAW INGEST</span><h2>每轮新增 Raw 岗位</h2></div><span className="small-tag">最近 {data.rawRuns.length} 轮</span></div><RunBars rows={data.rawRuns.map(x=>({at:x.at,value:x.added}))}/><div className="oj-chart-summary"><span>最新一轮 <b>+{full(latestRaw?.added||0)}</b></span><span>Raw 总累计 <b>{full(data.totals.rawRecords)}</b></span></div></section>
   <section className="panel oj-section"><div className="section-heading"><div><span className="eyebrow">VPS DELTA</span><h2>每轮同步变化</h2></div><span className={`proof-badge ${data.vps.healthy?'connected':''}`}><Wifi size={13}/>{data.vps.status}</span></div><RunBars rows={data.syncRuns.map(x=>({at:x.at,value:x.upserts,secondary:x.deletes}))}/><div className="oj-chart-summary"><span>最新 <b>+{full(latestSync?.upserts||0)}</b> / -{full(latestSync?.deletes||0)}</span><span>{size(data.vps.bytes)} · {fresh?'fresh':'stale'}</span></div></section>
  </div>

  <section className="panel oj-section"><div className="section-heading"><div><span className="eyebrow">SOURCE HEALTH</span><h2>API / 公共招聘源</h2><p>等待表示已完成当前轮、尚未到下次刷新；错误数来自近期日志尾部，不等于永久失效。</p></div><span className="small-tag">21 天 canonical contribution</span></div><SourceGrid sources={data.sources}/></section>

  <div className="oj-three-col">
   <section className="panel oj-section"><div className="section-heading"><h2>合同类型</h2><Layers3 size={17}/></div><Bars rows={data.contracts} maxRows={8}/></section>
   <section className="panel oj-section"><div className="section-heading"><h2>来源贡献</h2><Network size={17}/></div><ProviderBars rows={data.providers}/></section>
   <section className="panel oj-section"><div className="section-heading"><h2>行业分布</h2><Database size={17}/></div><Bars rows={data.industries} maxRows={10}/></section>
  </div>

  <section className="panel oj-section"><div className="section-heading"><div><span className="eyebrow">FINANCE COVERAGE</span><h2>金融机构官方招聘源</h2><p>重点覆盖银行、CIB、资管、保险、PE / Private Debt；数字为当前官方源已保留记录，不与21天搜索统计口径混淆。</p></div><span className="proof-badge"><Building2 size={13}/>{data.financeSources.length} sources</span></div><FinanceGrid rows={data.financeSources}/></section>

  <div className="oj-two-col bottom">
   <section className="panel oj-section oj-vps-panel"><div className="section-heading"><div><span className="eyebrow">VPS LINK</span><h2>Onward V1 搜索库</h2></div>{data.vps.healthy?<CheckCircle2 size={20}/>:<AlertTriangle size={20}/>}</div><dl><div><dt>连接判断</dt><dd className={data.vps.healthy?'good':'bad'}>{data.vps.healthy?'Healthy':'Needs attention'}</dd></div><div><dt>最后 import</dt><dd>{stamp(data.vps.lastImportAt)}</dd></div><div><dt>VPS active jobs</dt><dd>{full(data.vps.activeJobs)}</dd></div><div><dt>SQLite</dt><dd>{size(data.vps.bytes)}</dd></div><div><dt>最后 upsert / delete</dt><dd>+{full(data.vps.lastUpserts)} / -{full(data.vps.lastDeletes)}</dd></div><div><dt>下次同步</dt><dd>{stamp(data.vps.taskNextRunAt)}</dd></div></dl></section>
   <section className="panel oj-section oj-coverage"><div className="section-heading"><div><span className="eyebrow">DATA COVERAGE</span><h2>索引完整性</h2></div><HardDrive size={18}/></div><div className="oj-coverage-grid"><div><Database size={15}/><b>{full(data.totals.rawFiles)}</b><span>Raw snapshot files</span></div><div><Timer size={15}/><b>{data.bootstrapDays} 天</b><span>bootstrap window</span></div><div><MapPin size={15}/><b>{full(data.totals.paris21d)}</b><span>Paris · 21d</span></div><div><Clock size={15}/><b>{ago(data.coverage.summaryBuiltAt)}</b><span>summary age</span></div></div><p>{data.coverage.note}</p></section>
  </div>
 </div>
}
