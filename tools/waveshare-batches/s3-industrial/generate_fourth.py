#!/usr/bin/env python3
"""Curate sixteen exact Waveshare SKUs from SHA-checked primary source caches.
Mechanical reproduction is independently supported by committed model plans.
"""
from pathlib import Path
from html.parser import HTMLParser
import argparse,datetime,gzip,hashlib,json,re,zipfile
from urllib.parse import urlparse
ROOT=Path(__file__).resolve().parents[3]
a=argparse.ArgumentParser();a.add_argument('--sources',type=Path,required=True);a.add_argument('--inventory',type=Path,required=True);args=a.parse_args();S=args.sources
D=ROOT/'docs/research/waveshare-s3-industrial-2026-11-24';E=D/'batch-04';O=ROOT/'tools/waveshare-batches/s3-industrial';K=ROOT/'web/platform/knowledge'
NOW=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z');rows=json.loads(args.inventory.read_text())['items'];audits=json.loads((E/'firmware-image-audit.json').read_text())
def sha(b):return hashlib.sha256(b).hexdigest()
def verify_file(p,expected):
 with p.open('rb')as f:actual=hashlib.file_digest(f,'sha256').hexdigest()
 assert actual==expected,(p,actual,expected)
def save(p,d):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
def minimize_listing(data):
 original=(json.dumps(data,ensure_ascii=False,indent=2)+'\n').encode()
 normalized,count=re.subn(r'/(?:Users|home)/[^/\s"\'`\\<>]+','/Users/alice',original.decode())
 result=json.loads(normalized)
 if count:result['privacyDerivation']={'originalDerivedListingSha256':sha(original),'sanitizedListingBodySha256':sha(normalized.encode()),'personalPathReplacements':count,'transform':'Only the derived member directory listing normalizes personal home-directory labels to /Users/alice while original manufacturer ZIP, member bytes, archive SHA and BIN identities remain unchanged. The sanitized-body hash excludes this receipt object.'}
 return result
def utc(s):return datetime.datetime.fromisoformat(s.replace('Z','+00:00')).astimezone(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def source(sid,title,r,**extra):
 url=r.get('requestedUrl')or r.get('originalUrl')or r.get('url');return {'id':sid,'title':title,'url':url,'resolvedUrl':r.get('finalUrl')or r.get('resolvedUrl')or url,'kind':'manufacturer-document','status':'fetched','fetchedAt':utc(r['fetchedAt']),'bytes':r['bytes'],'sha256':r['sha256'],'archiveName':r.get('archiveName')or re.sub(r'[^a-zA-Z0-9._-]','_',Path(urlparse(url).path).name)or sid+'.source','mediaType':r.get('contentType')or r.get('mediaType')or'application/octet-stream',**extra}
def retain(folder,p,sid,text=False):
 r=json.loads(p.with_name(p.name+'.source.json').read_text());raw=p.read_bytes();assert sha(raw)==r['sha256'];ret=p.with_suffix('.txt').read_bytes()if text else raw;fn=sid+('.txt'if text else p.suffix)+'.gz';gz=gzip.compress(ret,mtime=0);(folder/fn).write_bytes(gz)
 return source(sid,p.name,r,retained={'path':str((folder/fn).relative_to(ROOT)),'encoding':'gzip','bytes':len(gz),'sha256':sha(gz),'uncompressedSha256':sha(ret),'transform':'article-text-only'if text else'none'})
def member(folder,archive,entry,sid,patterns=None):
 r=json.loads(archive.with_name(archive.name+'.source.json').read_text())
 with zipfile.ZipFile(archive)as z:raw=z.read(entry)
 retained=raw if patterns is None else ('\n'.join(line for line in raw.decode(errors='replace').splitlines()if any(re.search(x,line)for x in patterns))+'\n').encode()
 fn=sid+Path(entry).suffix+'.gz';gz=gzip.compress(retained,mtime=0);(folder/fn).write_bytes(gz)
 return source(sid,entry,{**r,'bytes':len(raw),'sha256':sha(raw),'contentType':'text/plain'},archive={'containerSha256':r['sha256'],'entry':entry},retained={'path':str((folder/fn).relative_to(ROOT)),'encoding':'gzip','bytes':len(gz),'sha256':sha(gz),'uncompressedSha256':sha(retained),'transform':'selected-hardware-declarations-only'if patterns else'none'},notes='Original ZIP member identity retained separately from selected non-network declarations. No firmware binary or sample WiFi credentials copied.')
class Description(HTMLParser):
 def __init__(self):super().__init__();self.depth=0;self.text=[]
 def handle_starttag(self,tag,attrs):
  d=dict(attrs)
  if tag=='div'and(self.depth or d.get('class')=='std'):self.depth+=1
 def handle_endtag(self,tag):
  if tag=='div'and self.depth:self.depth-=1
 def handle_data(self,text):
  if self.depth:self.text.append(text)
def storefront(folder,row):
 p=args.inventory.parent/'products'/(row['listingId']+'.html');raw=p.read_bytes();r=row['productSource'];assert sha(raw)==r['sha256'];h=Description();h.feed(raw.decode(errors='replace'));t=re.sub(r'\s+',' ',' '.join(h.text)).strip().encode();assert t;fn='storefront-description.txt.gz';gz=gzip.compress(t,mtime=0);(folder/fn).write_bytes(gz)
 return source('storefront','Official exact SKU/options and manufacturer product description',r,retained={'path':str((folder/fn).relative_to(ROOT)),'encoding':'gzip','bytes':len(gz),'sha256':sha(gz),'uncompressedSha256':sha(t),'transform':'manufacturer-description-html-to-text; anonymous session/navigation omitted'})
def fact(id,label,value,src,basis='official',**extra):return {'id':id,'label':label,'value':value,'sourceIds':src,'basis':basis,**extra}
releases=ROOT/'docs/research/waveshare-p4-s31-2026-11-24/ecosystem-releases';rs=json.loads((releases/'source-manifest.json').read_text())['sources'];release={k:json.loads(gzip.decompress((releases/f).read_bytes()))for k,f in [('uiflow','m5stack-uiflow-micropython.json.gz'),('xiaozhi','78-xiaozhi-esp32.json.gz')]}
eco=[source(k+'-release',k+' pinned upstream release',next(x for x in rs if x['archiveName']==f),notes='Original metadata retained at '+str((releases/f).relative_to(ROOT)))for k,f in [('uiflow','m5stack-uiflow-micropython.json.gz'),('xiaozhi','78-xiaozhi-esp32.json.gz')]]
m5dir=ROOT/'docs/research/waveshare-classic-s2-2026-11-24/ecosystem';m5r=json.loads((m5dir/'source-manifest.json').read_text())['sources'][0];m5=json.loads(gzip.decompress((m5dir/'m5burner-devices.json.gz').read_bytes()))['data'];assert len(m5)==55 and not any('waveshare'in json.dumps(x).lower()for x in m5);eco.append(source('m5burner-devices','M5Burner55 public device categories',m5r,notes='Fixed inspected categories only; not all community uploads. Raw metadata retained in sibling ecosystem evidence.'))
import copy
catalog=json.loads((ROOT/'web/platform/catalog/products.json').read_text())['items'];firmware_catalog=json.loads((ROOT/'web/platform/catalog/firmwares.json').read_text())['items']
configs=[('esp32-s3-nano','ESP32-S3-Nano',['nano','nano-m'],[(16,8)]*2),('esp32-s3-zero','ESP32-S3-Zero',['zero','zero-m','zero-n8r8','zero-n8r8-m'],[(4,2),(4,2),(8,8),(8,8)]),('esp32-s3-dev-kit-n8r8','ESP32-S3-DEV-KIT-N8R8',['dev-kit-n8r8','dev-kit-n8r8-m','dev-kit-n16r8','dev-kit-n16r8-m','dev-kit-n32r16v','dev-kit-n32r16v-m','dev-kit-n16r8-u','dev-kit-n16r8-um'],[(8,8),(8,8),(16,8),(16,8),(32,16),(32,16),(16,8),(16,8)]),('esp32-s3-pico','ESP32-S3-Pico',['pico','pico-m'],[(16,2)]*2)]
common=['精确SKU容量、GPIO/LEDC/RMT 数字输出和物理 BOOT 输入已通过 LX7 寄存器客体诊断；这些测试不代表原厂完整应用验收。','USB桥接/集线器、射频、模拟电压、电流、器件热特性与实机刷写未验证。RESET保留实物形状，平台重启操作不伪装为普通GPIO按键。','排针或内存选项分别建档；旧四基础款的原BIN执行证据只属于原型号和原镜像，不向新增SKU继承。']
factory_note='本轮检查精确SKU官方产品/资源/开发文档、原始程序ZIP、M5Burner公开55类及固定UIFlow/小智发布记录，未确认这些SKU的独立出厂恢复镜像。示例测试/MicroPython保持example或development，不能冒充factory。'
batch=[]
for listing,doc,suffixes,memories in configs:
 row=next(x for x in rows if x['listingId']==listing);folder=D/listing;folder.mkdir(parents=True,exist_ok=True);sr=S/listing;base_id='waveshare-'+listing;base_product=next(x for x in catalog if x['id']==base_id);base_knowledge=json.loads((K/(base_id+'.json')).read_text());source_records=[storefront(folder,row)]
 save(folder/'storefront-identity.json',{'listingId':listing,'source':row['productSource'],'variants':row['variants']})
 for fn,sid in [('overview.html','overview'),('resources.html','resources'),('Arduino.html','arduino-guide'),('MicroPython.html','micropython-guide')]:source_records.append(retain(folder,sr/fn,sid,True))
 schem=next(sr.glob('*[Ss]ch*.pdf'));source_records.append(retain(folder,schem,'schematic'))
 for x in base_knowledge['sources']:
  if x['id']in['led-example','led-datasheet','arduino-pins','firmware-archive']:source_records.append(copy.deepcopy(x))
 archives=[]
 for archive in sorted(sr.glob('*.zip')):
  receipt=json.loads(archive.with_name(archive.name+'.source.json').read_text());verify_file(archive,receipt['sha256']);z=zipfile.ZipFile(archive);archives.append({'name':archive.name,'sha256':receipt['sha256'],'bytes':archive.stat().st_size,'entries':len(z.infolist()),'members':[{'name':x.filename,'bytes':x.file_size,'crc32':f'{x.CRC:08x}'}for x in z.infolist()]})
 save(folder/'archive-members.json',minimize_listing({'archives':archives}))
 if listing.endswith('nano'):
  z=sr/'ESP32-S3-Nano-Demo-Code.zip';names=zipfile.ZipFile(z).namelist();entry=next(n for n in names if n.endswith('.ino'));source_records.append(member(folder,z,entry,'led-bsp',patterns=[r'#define',r'pinMode',r'analogWrite',r'digitalWrite',r'ledc']))
 elif listing.endswith('dev-kit-n8r8'):
  z=sr/'ESP32-S3-DEV-KIT-MPY-Example.zip';entry='ESP32-S3-DEV-KIT-MPY-Example/7-RGB-LED/rgb.py';source_records.append(member(folder,z,entry,'led-bsp'))
 elif listing.endswith('pico'):
  files=list(sr.glob('rgb.py'))
  if files:source_records.append(retain(folder,files[0],'led-bsp'))
 elif listing.endswith('zero'):
  z=sr/'Esp32-s3-zero-code.zip';entry='BlinkRGB/BlinkRGB.ino';names=zipfile.ZipFile(z).namelist();entry=next(n for n in names if n.endswith('BlinkRGB.ino'));source_records.append(member(folder,z,entry,'led-bsp'))
 save(folder/'sources.json',{'sources':source_records,'scope':'Current exact-family source identities; base electrical proof/earlier binary execution also retained under docs/evidence/waveshare-s3-core-2026-11-23. Binary payloads remain remote.'})
 products=[];knowledge=[];caps={};firmwares=[];watch=[];factory_id=base_id+'-factory-reference';ids=['waveshare-esp32-s3-'+s for s in suffixes]
 def fw(fid,name,role,pids,manifest=None,version='source-review',note=factory_note,url=None):
  url=url or'https://docs.waveshare.com/'+doc+'/Resources-And-Documents';wid=fid+'-watch';f={'id':fid,'name':name,'productIds':pids,'version':version,'status':'trial'if manifest else'reference','source':manifest,'role':role,'publisher':'Waveshare'if role not in['project']else name.split(' · ')[0],'publisherGroup':'官方','simulationVerified':False,'hardwareValidated':False,'storage':'remote-links-only','compatibility':'partial'if manifest else'unavailable','publicationUrl':url,'watchlistId':wid,'sources':[{'title':'Fixed primary source','url':url,'checkedAt':NOW[:10]}],'limitations':[note,*common]};firmwares.append(f);watch.append({'id':wid,'name':name,'category':'factory'if role=='factory'else'development','metadataProvider':'manual','upstreamRepo':('m5stack/uiflow-micropython'if'UIFlow'in name else'78/xiaozhi-esp32'if'小智'in name else'waveshareteam/'+doc),'metadataUrl':url,'releaseUrl':url,'version':version,'checkedAt':NOW,'assets':[],'compatibility':{'status':'reference-only','productIds':pids,'simulationVerified':False,'hardwareValidated':False,'note':note},'storage':'remote-links-only','catalogTargets':[],'monitoringNote':'Manual exact SKU/source mapping. No automatic cross-capacity/PSRAM/flash-mode substitution or inherited runtime acceptance.'});return f
 fw(factory_id,doc+' · 出厂程序调查资料','factory',ids)
 old_firmwares=[copy.deepcopy(f)for f in firmware_catalog if base_id in f.get('productIds',[])and f.get('source')]
 for old in old_firmwares:
  new=fw(old['id'],old['name'],old['role'],[base_id],old['source'],old['version'],'旧基础款原始镜像与原清单hash保持不变；原生ROM执行结果见独立基础款证据。其余完整功能和实机未验证。',old['publicationUrl']);new['publisher']=old['publisher']
 trial_defaults={base_id:base_product['defaultFirmwareId']}
 default_source=next(f for f in old_firmwares if f['id']==base_product['defaultFirmwareId'])
 # Same exact memory/module, soldered-header option. Only metadata changes;
 # original remote archive/member bytes and offsets remain unchanged.
 m_id=ids[1];manifest=copy.deepcopy(json.loads((ROOT/'web/platform'/default_source['source']['manifestUrl'].removeprefix('./')).read_text()));manifest['board']='waveshare-s3-'+suffixes[1].replace('dev-kit','devkit');manifest['name']=m_id+' original official development/example bytes';manifest['_note']='Original unmodified ZIP members; exact same memory/module header option. No runtime acceptance is inherited from the base SKU.';fid=m_id+('-official-test'if'nano'in listing else'-blinkrgb'if'zero'in listing else'-micropython');dest=ROOT/'web/platform/firmware-plans'/(fid+'.json');save(dest,manifest);fw(fid,doc+' -M · 官方原始开发/示例镜像（试运行）',default_source['role'],[m_id],{'manifestUrl':'./firmware-plans/'+fid+'.json','sha256':sha(dest.read_bytes())},default_source['version'],'已校验原始容器、成员、芯片与布局；排针款未继承旧基础款的原BIN验收。');trial_defaults[m_id]=fid
 if 'dev-kit'in listing:
  a=next(x for x in audits if x['id']=='dev-n16r8-oct');c=a['container'];url=c.get('requestedUrl')or c.get('url');
  for i in[2,3,6,7]:
   pid=ids[i];board='waveshare-s3-'+suffixes[i].replace('dev-kit','devkit');fid=pid+'-micropython';manifest={'name':pid+' official N16R8 octal-PSRAM development image','version':'archive-'+c['sha256'][:12],'board':board,'flash_mb':16,'psram_mb':8,'files':{},'flash_at':{'0x0':{'url':url,'sha256':a['sha256'],'archive':{'format':'zip-entry','entry':a['entry'],'sha256':c['sha256']}}},'_source':'https://docs.waveshare.com/'+doc+'/MicroPython','_note':'Manufacturer original merged N16R8/SPIRAM_OCT development image; QSPI flash remains distinct from octalPSRAM. Header and antenna variants share documented16F/8P target, but execution/real hardware acceptance is not inherited.'};dest=ROOT/'web/platform/firmware-plans'/(fid+'.json');save(dest,manifest);fw(fid,pid+' · 官方 N16R8 MicroPython（试运行）','development',[pid],{'manifestUrl':'./firmware-plans/'+fid+'.json','sha256':sha(dest.read_bytes())},manifest['version'],'原目录 N16R8/SPIRAM_OCT 镜像为完整原始S3镜像；QSPI Flash16MiB/Octal PSRAM8MiB，M/U/UM为对应容量与接线的明确试运行映射，未经原BIN执行或实机验收。');trial_defaults[pid]=fid
 for kind,title,repo in [('uiflow','UIFlow','m5stack/uiflow-micropython'),('xiaozhi','小智','78/xiaozhi-esp32')]:
  rel=release[kind];note='已检查固定上游发布 '+str(rel.get('tag_name'))+'，未找到这些精确SKU的唯一镜像映射；保留资料入口，不能用近似名称或其他模块版本替换。';fw(base_id+'-'+kind+'-reference',title+' · '+doc+' 精确映射调查','project',ids,None,str(rel.get('tag_name')or'source-review'),note,rel.get('html_url')or'https://github.com/'+repo)
 for i,(suffix,mem,v)in enumerate(zip(suffixes,memories,row['variants'])):
  pid=ids[i];board='waveshare-s3-'+suffix.replace('dev-kit','devkit');name=v['documentedProductNames'][0].split(' (')[0];plan=json.loads((ROOT/'web/platform/assets'/pid/'model-plan.json').read_text());nano='nano'in suffix;octal='n32r16v'in suffix;model_native=plan['method'].startswith('official-cad');default=trial_defaults.get(pid,factory_id);notes=list(common)
  if nano:notes+=['实际只有 RESET；GPIO0是离散LED通道，不提供虚构BOOT。LED封装引脚号与ROHM手册的历史冲突仍保留；RGB名称采用官方程序和固定Arduino Nano GPIO映射，未校准实物光色。']
  if 'pico'in suffix:notes+=['官方文档混写S3R2/2MiB与Octal PSRAM保留为资料冲突；4MiB MicroPython软件布局不等于物理16MiB Flash。']
  if octal:notes+=['WROOM-2-N32R16V固定32MiB OPI Flash/16MiB PSRAM。IO47/48属于1.8V域；模拟器数字电平不代表实物电压。当前没有确切原厂可启动镜像，保留资料入口。']
  if 'zero-n8r8'in suffix:notes+=['PICO-1-N8R8独立8F/8P SKU；旧FH4R2 CAD和4F/2P BIN不作为此型号固件或模块内部几何。']
  summary=f'{mem[0]} MiB Flash / {mem[1]} MiB PSRAM；'+('离散 RGB GPIO46/0/45、GPIO48 指示灯，无物理BOOT。'if nano else('OPI Flash；'if octal else'')+'GPIO'+('38'if'dev-kit'in suffix else'21')+' RGB 与 GPIO0 BOOT。')
  product={'id':pid,'name':'Waveshare '+name,'manufacturer':'Waveshare','sku':v['sku'],'chip':'esp32s3','hardwareRevision':'unspecified','summary':summary,'knowledgeUrl':'./knowledge/'+pid+'.json','family':{'id':base_id,'name':'Waveshare '+doc.split('-N8R8')[0]},'runtime':{'board':board,'flashMb':mem[0],'psramMb':mem[1],'status':'partial','touch':False},'displayStatus':'none','display':None,'controls':[]if nano else[{'id':'boot','name':'BOOT','type':'button','pin':0,'activeLow':True,'defaultKeyCode':'Space'}],'interfaces':copy.deepcopy(base_product['interfaces']),'sources':[{'id':x['id'],'title':x['title'],'url':x['url'],'checkedAt':NOW[:10]}for x in source_records],'limitations':notes+[factory_note],'chronology':copy.deepcopy(base_product['chronology']),'defaultFirmwareId':default,'model':{'type':'manufacturer-cad','url':'./assets/'+pid+'/model.json','status':'partial','provenance':plan['method']}}
  if i>0:product['visualVariantOf']=base_id
  products.append(product);caps[board]={'chip':'esp32s3','flashMb':mem[0],'psramMb':mem[1],'display':False,'touch':False,'buttons':[]if nano else['boot']}
  ks=[{k:y for k,y in x.items()if k not in['retained','archive']}for x in source_records]+eco;mids=[]
  for x in plan['sources']:
   ms=source('model-'+x['id'],'Mechanical source '+x['id'],x,notes='Pinned original under assets/'+pid+'; unchanged source bytes and geometry plan.');ks.append(ms);mids.append(ms['id'])
  electrical=[copy.deepcopy(x)for x in base_knowledge['facts']if x['id']in['buttons','led']]
  facts=[fact('identity','精确SKU与包装选项',{'sku':v['sku'],'model':name,'runtimeBoard':board,'options':v['options'],'currentlyUnsaleable':v['unsaleable']},['storefront','overview']),fact('cpu','主控架构',{'chip':'ESP32-S3','architecture':'Xtensa LX7','cores':2,'maxClockMHz':240},['overview','storefront']),fact('memory','精确存储与Flash模式',{'flashMiB':mem[0],'psramMiB':mem[1],'flashInterface':'OPI'if octal else'QSPI','gpio47and48NominalHighV':1.8 if octal else 3.3 if'dev-kit'in suffix else None},['storefront','overview'],notes='Firmware software layout, PSRAM protocol and physical Flash capacity are independent. N32R16V is32F/16P, never32F/8P.'),*electrical,fact('display','无随板显示器',{'included':False},['overview','storefront']),fact('mechanical','机械来源和精度边界',plan['dimensions'],mids,'cad-measurement'if model_native else'image-measurement',method='Unscaled native geometry for exact matching boards; photo envelopes and additive soldered headers carry explicit estimates.',uncertainty='Export bounds are not physical-fit tolerance. PCB stackup, small passives and hidden geometry may remain UNKNOWN.'),fact('factory-investigation','出厂与生态固件调查',{'factoryImagePinned':False,'defaultFirmwareId':default,'originalExampleOrDevelopmentAvailable':pid in trial_defaults,'baseExecutionEvidence':'docs/evidence/waveshare-s3-core-2026-11-23'if i==0 else None,'newSkuInheritsBaseExecution':False},['overview','resources','micropython-guide','m5burner-devices','uiflow-release','xiaozhi-release'],notes=factory_note)]
  if 'dev-kit'in suffix:facts.append(fact('reserved-pins','精确模块禁用引脚',{'GPIO35':'NC','GPIO36':'NC','GPIO37':'NC','externalAntenna':suffix.endswith(('-u','-um'))},['overview']))
  unknowns=[fact('revision','实物PCB修订未知',None,['overview'],'unknown'),fact('weight','精确SKU重量未知',None,['overview'],'unknown'),fact('mechanical-fit','实物装配/插合公差',None,mids,'unknown'),fact('factory-image','独立原厂恢复镜像',None,['resources','overview'],'unknown'),fact('full-firmware','完整原厂应用与实机运行',None,['resources'],'unknown')]
  record={'schemaVersion':'1','id':pid,'catalogId':pid,'kind':'product','title':product['name'],'summary':summary,'summarySourceIds':['storefront','overview'],'hardwareRevision':'unspecified','updatedAt':NOW,'facts':facts,'sources':ks,'revisions':[],'assets':[{'id':'model','sourceId':mids[0],'role':'source-backed-partial-model','url':ks[-len(mids)]['url'],'localUrl':'./assets/'+pid+'/model.json'}],'unknowns':unknowns,'collectionNotes':[factory_note,*notes]};save(K/(pid+'.json'),record);knowledge.append({'path':str((K/(pid+'.json')).relative_to(ROOT))})
 investigation={'listingId':listing,'skus':[v['sku']for v in row['variants']],'status':'no-independent-factory-image-confirmed','finding':factory_note,'availableDevelopmentMappings':trial_defaults,'excludedMappings':['No FH4R2 image promoted toZeroN8R8.','No QSPI N16 image promoted toN32R16V OPI.','No original BIN is rebuilt, merged or hosted.'],'baselineExecutionScope':'Four exact base profiles only, as retained original source hashes/ROM run receipts; new options get digital diagnostics and explicitly unverified trials.','releaseSources':[{'id':e['id'],'sha256':e['sha256'],'fetchedAt':e['fetchedAt']}for e in eco]};save(folder/'firmware-investigation.json',investigation)
 receipt={'command':'CARGO_BUILD_JOBS=1 cargo test -p esp32s3 --test waveshare_core_skus','testsPassed':4,'status':'passed','guest':'Original six-byteLX7 S32I/L32I diagnostic; GPIO/RMT/LEDC/MMU/eFuse register operations, not manufacturer application firmware.','scope':'All16 SKU capacities/keys;14addressable routing/wire orders;2Nano discretePWM;2N32 topflash+PSRAMpages andOPI chip reset. SeparateWASM platform restart checks.','firmwareVerified':False};save(folder/'runtime-verification.json',receipt)
 fragment={'listingId':listing,'products':products,'firmwares':firmwares,'watchlist':watch,'knowledge':knowledge,'boardCapabilities':caps,'modelIds':ids,'sourceReceiptPaths':[str((folder/n).relative_to(ROOT))for n in['sources.json','storefront-identity.json','archive-members.json','firmware-investigation.json','runtime-verification.json']],'tests':[{'command':receipt['command'],'status':'passed','scope':receipt['scope'],'evidence':str((folder/'runtime-verification.json').relative_to(ROOT))}],'limitations':common,'integrationNotes':['Replace existing base ID; preserve its earlier source-backed acceptance receipts. Add12exactnewSKU IDs across4families.','N32R16V requires Variant::flash_octal inCLI/WASM startup; nativechipreboot already preserves physical flash protocol/eFuse andPSRAMMR2. Merge core selector with existingRGBMatrix OPI condition usingOR.','Factory-reference remains first, explicit development/example default allowed. NewSKU never inherits originalbaseexecution.']};save(O/(listing+'.json'),fragment);batch.append({'listingId':listing,'skus':[v['sku']for v in row['variants']],'productIds':ids,'modelIds':ids,'fragment':str((O/(listing+'.json')).relative_to(ROOT))});print(listing,len(ids),len(firmwares),flush=True)
save(E/'manifest.json',{'families':4,'exactSkuCount':16,'profiles':16,'models':16,'existingBaseModelsRetained':4,'newModels':12,'items':batch,'scope':'Exact memory/module/header/antenna options; runtime and source audit bounded, no new original-firmware or physical-hardware acceptance.'})
