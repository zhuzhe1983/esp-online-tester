#!/usr/bin/env python3
"""Build per-listing integration fragments from exact pinned inventory and reviewed profiles.
No central catalog/index is mutated. Manufacturer binaries remain remote-only.
"""
import argparse,copy,json,re,hashlib,gzip
from datetime import datetime,timezone
from pathlib import Path
from urllib.parse import quote
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
UTC=datetime.now(timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def dump(p,x):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def ident(s):return re.sub('[^a-z0-9]+','-',s.lower()).strip('-')
def utc(s):return datetime.fromisoformat(s.replace('Z','+00:00')).astimezone(timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def source(s,sid,title,kind,archive=None,notes=''):
 url=s.get('url',s.get('originalUrl',s.get('requestedUrl')));resolved=s.get('resolvedUrl',s.get('finalUrl',url))
 d={'id':sid,'title':title,'url':url.replace('http://','https://'),'kind':kind,'status':'fetched','fetchedAt':utc(s['fetchedAt']),'sha256':s['sha256'],'bytes':s['bytes'],'archiveName':archive or s.get('archiveName',sid+'.html'),'notes':notes}
 if resolved and resolved.startswith('https:'):d['resolvedUrl']=resolved
 if s.get('revision'):d['revision']={'gitCommit':s['revision']}
 return d

def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--inventory',type=Path,required=True);ap.add_argument('--research',type=Path,required=True);ap.add_argument('--only',nargs='*');args=ap.parse_args()
 queue=json.loads((args.inventory/'queue-s3.json').read_text())['items'];profiles=json.loads((HERE/'reviewed-profiles.json').read_text())
 old=json.loads((ROOT/'web/platform/catalog/products.json').read_text())['items'];by_sku={p['sku']:p for p in old if p.get('manufacturer')=='Waveshare'and p.get('chip')=='esp32s3'}
 receipts=json.loads((ROOT/'docs/research/waveshare-s3-20261123/source-manifest.json').read_text());bins=json.loads((args.research/'firmware-manifest.json').read_text());out=[]
 for item in queue:
  lid=item['listingId']
  if lid not in profiles or args.only and lid not in args.only:continue
  cfg=profiles[lid];base='waveshare-'+ident(lid);repo=cfg.get('repository');ss=[]
  ss.append(source(item['productSource'],'official-store','官方商品页与精确SKU选项','storefront','store-response-not-retained.html','原HTTP响应的字节哈希；原始商店HTML含匿名会话值，Git仅保留无会话值的SKU抽取与收据。'))
  for di,path in enumerate(item['documentationCacheFiles']):
   doc=json.loads(Path(path).read_text());ss.append(source(doc['source'],'official-overview'+(''if di==0 else'-'+str(di)),'官方型号、容量、接口与屏幕规格','documentation',ident(lid)+'-overview.html','原始文档响应收据；可核对厂家URL与curated inventory。'))
  if len(ss)==1:
   f=args.research/'wiki'/f'{lid}.json'
   if f.exists():
    d=json.loads(f.read_text());s=d.get('source',d);ss.append(source(s,'official-overview','官方Wiki硬件文档','documentation',lid+'.html'))
  source_candidates=[s for s in receipts if s.get('repository','').split('/')[-1]==repo or s.get('repository')==repo]
  for i,s in enumerate(source_candidates):
   ss.append(source(s,'board-source-'+str(i),'固定提交：'+s.get('path','板级源码'),'manufacturer-source',s['archiveName'],'原始内容以gzip保留于docs/research/waveshare-s3-20261123；压缩包与源字节分别有SHA-256。'))
  overview='official-overview'if any(x['id']=='official-overview'for x in ss)else'official-store';refs=[overview]+[s['id']for s in ss if s['id'].startswith('board-source-')][:12]
  selected_bins=[b for b in bins if b['repository']==repo and (not cfg.get('firmwareFiles')or Path(b['path']).name in cfg['firmwareFiles'])]
  if cfg.get('firmwareLatest')and selected_bins:selected_bins=[max(selected_bins,key=lambda b:Path(b['path']).name)]
  frag={'listingId':lid,'products':[],'firmwares':[],'watchlist':[],'knowledge':[],'boardCapabilities':{},'sourceReceiptPaths':['docs/research/waveshare-s3-20261123/source-manifest.json','docs/research/waveshare-s3-20261123/firmware-manifest.json'],'tests':cfg.get('tests',[]),'limitations':cfg.get('limitations',[]),'scope':'Exact storefront SKU registration with the stated digital subsets. Partial does not mean whole-device or physical acceptance.'}
  for vi,v in enumerate(item['variants']):
   c={**cfg,**cfg.get('skuOverrides',{}).get(v['sku'],{})};previous=by_sku.get(v['sku']);pid=previous['id']if previous else base+(''if vi==0 else'-sku-'+v['sku']);title=(v.get('documentedProductNames')or[]);title=title[0]if title else lid.upper()+(' · '+', '.join(v['attributes'])if v['attributes']else'')
   name='Waveshare '+title;display=copy.deepcopy(c.get('display'));board=c.get('board');flash=c.get('flashMb');psram=c.get('psramMb');status=c.get('status','partial'if board else'unavailable');touch=c.get('touch',False)
   limitations=c.get('limitations',[])+['本型号尚未完成实物验证。目录、数字外设测试、原厂固件原生执行和浏览器worker验收是不同证据层；逐项范围见知识页。','商品页顺序用作带屏优先的工作队列，不能据此认定发布日期。']
   p=copy.deepcopy(previous)if previous and c.get('preserveExisting')else{'id':pid,'name':name,'manufacturer':'Waveshare','sku':v['sku'],'chip':'esp32s3','hardwareRevision':c.get('hardwareRevision','unspecified'),'summary':c.get('summary',title+'；精确SKU '+v['sku']+'。'),'knowledgeUrl':'./knowledge/'+pid+'.json','family':{'id':base,'name':'Waveshare '+lid.upper()},'runtime':{'board':board,'flashMb':flash,'psramMb':psram,'status':status,'touch':touch},'displayStatus':'known'if display else c.get('displayStatus','none'),'display':display,'controls':[{'id':n,'name':n.upper(),'type':'button','pin':pin,'activeLow':active,'defaultKeyCode':'Space'if n=='boot'else'Key'+n[0].upper()}for n,pin,active in c.get('buttons',[])],'interfaces':copy.deepcopy(c.get('interfaces',[])),'limitations':limitations,'chronology':{'date':None,'basis':'unknown','sourceId':None,'sourceUrl':None,'confidence':'unknown','notes':'官方发布日期未建立；商店位置、Git提交与BIN文件名日期不是发布日期。'}}
   p['sources']=[{'id':s['id'],'title':s['title'],'url':s['url'],'checkedAt':'2026-11-23'}for s in ss]
   if c.get('modelId'):
    model=ROOT/'web/platform/assets'/c['modelId']/'model.json'
    if not model.exists():raise ValueError('Model missing '+str(model))
    md=json.loads(model.read_text());p['model']={'type':'manufacturer-cad','url':'./assets/'+c['modelId']+'/model.json','status':'partial','provenance':md['provenance']['kind']}
   if c.get('psramUnknown'):p['runtime']['psramMbBasis']='simulation'
   if c.get('flashUnknown'):p['runtime']['flashMbBasis']='simulation'
   frag['products'].append(p)
   if board:frag['boardCapabilities'][board]={'chip':'esp32s3','flashMb':flash,'psramMb':psram,'sdCard':c.get('sdCard',False),'boardButtons':[n for n,_,_ in c.get('buttons',[])]}
   facts=[{'id':'store-sku','label':'精确商店SKU','value':v['sku'],'basis':'official','sourceIds':['official-store']},{'id':'store-options','label':'此SKU的选项','value':v.get('options',dict(enumerate(v['attributes']))),'basis':'official','sourceIds':['official-store']},{'id':'model-name','label':'厂家命名','value':title,'basis':'official','sourceIds':['official-store']},{'id':'physical-memory','label':'主控存储容量','value':{'flashMiB':None if c.get('flashUnknown')else flash,'psramMiB':None if c.get('psramUnknown')else psram},'basis':'official','sourceIds':[overview],'notes':c.get('memoryNotes','容量属于此SKU/已注明硬件版本；不从通用ESP32名称推断。')},{'id':'display-spec','label':'屏幕规格','value':display if display is not None else {'onboardDisplay':False},'basis':'official','sourceIds':[overview]},{'id':'board-wiring','label':'本次实现的源代码接线','value':c.get('wiring',{}),'basis':'official','sourceIds':refs},{'id':'source-ecosystem','label':'UIFlow与小智调查','value':{'uiflow':'固定提交a61a29409f09f1f4bebae05e15c4bd6dcc4956b6板树中无Waveshare命名目录；此检索范围不证明不存在第三方移植。','xiaozhi':c.get('xiaozhi','固定提交92d400d6374bbe750e0a0188fa969fc4042820c2板树已检索；未建立此精确SKU的可直接运行镜像。')},'basis':'official','sourceIds':['uiflow-tree','xiaozhi-tree']},{'id':'factory-candidates','label':'官方原版镜像调查','value':[{'name':Path(b['path']).name,'sha256':b['sha256'],'bytes':b['bytes'],'mergedS3ChecksumsPass':b['layout']['mergedS3ChecksumsPass']}for b in selected_bins],'basis':'official','sourceIds':refs,'notes':c.get('firmwareNotes','完整布局校验不等同原ROM启动或全外设验收；原二进制仅登记远程链接。')}]
   kss=copy.deepcopy(ss)+[{'id':'uiflow-tree','title':'UIFlow 固定板树调查','url':'https://github.com/m5stack/uiflow-micropython/tree/a61a29409f09f1f4bebae05e15c4bd6dcc4956b6','kind':'pinned-source-tree-inspection','status':'linked','notes':'完整固定提交板树的生成检查收据见 docs/platform/research/waveshare-shared-ecosystem-sources；不是HTTP原文。'},{'id':'xiaozhi-tree','title':'小智固定板树调查','url':'https://github.com/78/xiaozhi-esp32/tree/92d400d6374bbe750e0a0188fa969fc4042820c2/main/boards/waveshare','kind':'pinned-source-tree-inspection','status':'linked','notes':'精确目标目录与型号关联须独立检查；不使用generic镜像替换。'}]
   for b in selected_bins:kss.append(source(b,'firmware-'+b['sha256'][:12],Path(b['path']).name,'original-manufacturer-binary',Path(b['path']).name,'下载并检查原始镜像字节，BIN未收入Git；结构检查详情在firmware-manifest.json。'))
   k={'schemaVersion':'1','id':pid,'kind':'product','catalogId':pid,'title':name,'summary':p['summary'],'summarySourceIds':['official-store',overview]if overview!='official-store'else[overview],'updatedAt':UTC,'hardwareRevision':p['hardwareRevision'],'identity':{'listingId':lid,'sku':v['sku'],'storeOptions':v.get('options',{}),'manufacturerProductNames':v.get('documentedProductNames',[])},'facts':facts,'revisions':[],'sources':kss,'assets':[],'unknowns':[{'id':'physical-acceptance','label':'实物验收','value':None,'basis':'unknown','sourceIds':[],'notes':'未连接实物，未声明通过。'},{'id':'unimplemented-scope','label':'未覆盖范围','value':None,'basis':'unknown','sourceIds':[],'notes':'；'.join(limitations)}],'collectionNotes':c.get('tests',[])+[frag['scope'],'精确SKU包内电池、外壳、排针和天线差异保持独立身份；共用数字路径不表示机械与RF等价。']}
   ecosystem=[f for f in k['facts'] if f['id']=='source-ecosystem'];k['facts']=[f for f in k['facts'] if f['id']!='source-ecosystem'];k['collectionNotes'] += ['UIFlow/小智是固定Git板树的范围化调查，生成检查收据不冒称HTTP响应：'+json.dumps(f['value'],ensure_ascii=False)for f in ecosystem]
   kp=ROOT/'web/platform/knowledge'/f'{pid}.json';dump(kp,k);frag['knowledge'].append({'path':str(kp.relative_to(ROOT))})
  for bi,b in enumerate(selected_bins):
   name=Path(b['path']).name;targets=[p['id']for p in frag['products']]
   if lid=='esp32-s3-lr1121-epaper-2.9':targets=[p['id']for p in frag['products']if ('HF' in p['name'])==('-HF.'in name)]
   role=cfg.get('firmwareRole','official-demo');complete=b['layout']['mergedS3ChecksumsPass'];fstatus='trial'if complete and all(p['runtime']['board']for p in frag['products'])else'reference';fid=base+'-'+('factory'if role=='factory'else'demo')+(''if len(selected_bins)==1 else'-'+ident(name.removesuffix('.bin'))[-40:]);wid=fid+'-watch'
   fw={'id':fid,'name':name,'productIds':targets,'status':fstatus,'source':{'url':b['url'],'sha256':b['sha256'],'bootMode':'rom'}if fstatus=='trial'else None,'role':role,'publisher':'Waveshare','publisherGroup':'官方','version':name.removesuffix('.bin'),'simulationVerified':False,'hardwareValidated':False,'storage':'remote-links-only','compatibility':'partial','publicationUrl':next((s['url']for s in ss if s['title'].endswith('firmware/README.md')),next((s['url']for s in ss if s['title'].endswith('README.md')),item['url'])),'sources':[{'title':'原厂原始BIN固定链接','url':b['url'],'checkedAt':'2026-11-23'}],'limitations':[cfg.get('firmwareNotes','原版结构校验通过；完整应用与物理外设尚未验收。'),'已注册trial表示可按精确板型尝试ROM启动，不代表完整应用已通过。'],'watchlistId':wid}
   frag['firmwares'].append(fw);frag['watchlist'].append({'id':wid,'name':name,'category':'factory'if role=='factory'else'development','upstreamRepo':'waveshareteam/'+repo,'metadataUrl':'https://api.github.com/repos/waveshareteam/'+repo+'/releases/latest','releaseUrl':'https://github.com/waveshareteam/'+repo+'/releases','version':b['revision'],'checkedAt':utc(b['fetchedAt']),'assets':[{'name':name,'url':b['url'],'sha256':b['sha256'],'bytes':b['bytes'],'format':'merged'if complete else'unknown'}],'compatibility':{'status':'reference-only','productIds':targets,'simulationVerified':False,'hardwareValidated':False,'note':'精确来源/芯片布局已调查；运行范围见产品知识。'},'storage':'remote-links-only','catalogTargets':[],'monitoringNote':'厂家固定提交原二进制。发现新版本须重新确认硬件修订与缺失分区。'})
   if role=='factory'and fstatus=='trial':
    for p in frag['products']:
     if p['id']in targets:p['defaultFirmwareId']=fid
  dump(HERE/(lid+'.json'),frag);out.append((lid,len(frag['products'])))
 print(json.dumps({'listings':len(out),'skus':sum(x[1]for x in out),'items':out},ensure_ascii=False))
if __name__=='__main__':main()
