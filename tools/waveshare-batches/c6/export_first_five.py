"""Exact first five C6 listing fragments. Never mutates shared catalogues."""
import datetime,gzip,hashlib,json,re,shutil
from pathlib import Path
import argparse
_parser=argparse.ArgumentParser(description=__doc__);_parser.add_argument('--sources',type=Path,required=True);_args=_parser.parse_args()
R=_args.sources;S=R/'c6-sources';ROOT=Path(__file__).resolve().parents[3]
OUT=ROOT/'tools/waveshare-batches/c6';OUT.mkdir(parents=True,exist_ok=True)
ARCH=ROOT/'docs/platform/research/waveshare-c6-new-lcd-sources';ARCH.mkdir(parents=True,exist_ok=True)
EVID=ROOT/'docs/evidence/waveshare-c6-new-lcd-2026-11-23'
NOW=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,d):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
factories=json.loads((S/'factory-first-five-verified.json').read_text());runs=json.loads((EVID/'native-runs.json').read_text())
save(EVID/'factory-image-verification.json',factories)
profiles=[
 dict(listing='esp32-c6-touch-lcd-1.28',repo='ESP32-C6-Touch-LCD-1.28',name='ESP32-C6-Touch-LCD-1.28',sku='35442',board='waveshare-c6-touch-lcd128',touch='CST816S',w=240,h=240,inch=1.28,controller='GC9A01A',factoryIndex=0,spi=dict(CLK=11,MOSI=21,CS=10,DC=19,RST=20,BL=7),i2c=dict(SDA=18,SCL=8),sd=dict(CLK=11,MOSI=21,MISO=22,CS=15),parts=['CST816S@0x15 RST23 IRQ4','QMI8658@0x6b (common register subset)','PCF85063@0x51'],conflicts=['Touch and non-touch1.28 share SPI pin numbers but have distinct PCB shapes and different -B case dimensions. Do not infer shared mechanical geometry.','IMU INT2 sharesGPIO4 with touch IRQ; only source-backed touch IRQ semantics are modeled, not a fabricated IMU IRQ.']),
 dict(listing='esp32-c6-lcd-1.28',repo='ESP32-C6-LCD-1.28',name='ESP32-C6-LCD-1.28',sku='35450',board='waveshare-c6-lcd128',touch=None,w=240,h=240,inch=1.28,controller='GC9A01A',factoryIndex=1,spi=dict(CLK=11,MOSI=21,CS=10,DC=19,RST=20,BL=7),i2c=dict(SDA=18,SCL=8),sd=dict(CLK=11,MOSI=21,MISO=22,CS=15),parts=['QMI8658@0x6b (common register subset)','PCF85063@0x51'],conflicts=['The non-touch board has no CST816 device; neither touch reporting nor touch IRQ is invented.','Current mechanical drawing specifiesR18.25 for bare board and11.5 mm case depth; touch variant dimensions do not apply.']),
 dict(listing='esp32-c6-lcd-2.73',repo='ESP32-C6-LCD-2.73',name='ESP32-C6-LCD-2.73',sku='35303',board='waveshare-c6-lcd273',touch=None,w=320,h=320,inch=2.73,controller='ILI9488',factoryIndex=2,spi=dict(CLK=0,MOSI=1,CS=8,DC=19,RST='CH32 EX0',BL='CH32 PWM register5'),i2c=dict(SDA=20,SCL=2),sd=dict(CLK=0,MOSI=1,MISO=18,CS=10),parts=['CH32V006@0x24 16-bit little-endian registers','QMI8658C@0x6b','PCF85063A@0x51'],conflicts=['The overview calls its expander CH32V003; exact schematic identifiesCH32V006K8U7 and BSP uses16-bit register writes. Runtime deliberately implementsCH32V006 rather than8-bitV003.','Current overview saysESP32-C6-WROOM-1-N16/16 MiB; retained schematic namesN8. Runtime follows current N16 capacity, preserving the revision conflict rather than claiming one schematic proves both.','ILI9488 physical/GRAM320×480 has320×320 visible window and uses RGB666; this is not an ST7789 alias.']),
 dict(listing='esp32-c6-touch-lcd-2.8',repo='ESP32-C6-Touch-LCD-2.8',name='ESP32-C6-Touch-LCD-2.8',sku='35065',board='waveshare-c6-touch28',touch='CST3530',w=240,h=320,inch=2.8,controller='ST7789',factoryIndex=3,spi=dict(CLK=0,MOSI=1,CS=11,DC=10,RST='CH32 EX1',BL='CH32 PWM register5'),i2c=dict(SDA=6,SCL=7),sd=dict(CLK=0,MOSI=1,MISO=8,CS=23),parts=['CH32V003@0x24 8-bit registers','CST3530@0x58 RST=CH32 EX0 IRQ18','QMI8658C@0x6b','PCF85063A@0x51','ES8311@0x18'],conflicts=['The exact C6 native STEP is linewise identical to a C5 mechanical export, but chips, GPIO mappings, flash capacity and factory images are independent.','Current resource page points at an older mechanical directory; pinned repository tree supplies the retrieved native STEP and dimension PDF.']),
 dict(listing='esp32-c6-lcd-1.54',repo='ESP32-C6-Touch-LCD-1.54',name='ESP32-C6-LCD-1.54',sku='34658',board='waveshare-c6-lcd154',touch=None,w=240,h=240,inch=1.54,controller='ST7789',factoryIndex=4,spi=dict(CLK=1,MOSI=2,CS=11,DC=3,RST=4,BL=6),i2c=dict(SDA=8,SCL=7),sd=dict(CLK=1,MOSI=2,MISO=16,CS=17),parts=['QMI8658C@0x6b','ES8311@0x18','ES7210@0x40'],conflicts=['External W25Q128 and QFN40 C6 are16 MiB, notC6FH8/8 MiB.','Overview incorrectly calls0x30 and0x80 seven-bit codec addresses; those are eight-bit forms. Schematic straps and BSP requireES8311@0x18 andES7210@0x40.','Schematic title block saysESP32-S3-Touch-LCD1.54, but the actual MCU symbol and current exact product areESP32-C6.','Touch reset shares LCD GPIO4 through the real network; both lines use actual pulldowns, not fabricated pullups.']),
]
profiles[0]['variants']=[('35442','ESP32-C6-Touch-LCD-1.28',False),('35443','ESP32-C6-Touch-LCD-1.28-B',True)]
profiles[1]['variants']=[('35450','ESP32-C6-LCD-1.28',False),('35451','ESP32-C6-LCD-1.28-B',True)]
profiles[2]['variants']=[('35303','ESP32-C6-LCD-2.73',False),('35302','ESP32-C6-LCD-2.73-EN',False)]
profiles[3]['variants']=[('35065','ESP32-C6-Touch-LCD-2.8',False),('35066','ESP32-C6-Touch-LCD-2.8-EN',False)]
profiles[4]['variants']=[('34658','ESP32-C6-LCD-1.54',False),('34660','ESP32-C6-LCD-1.54-EN',False),('34659','ESP32-C6-Touch-LCD-1.54',False),('34661','ESP32-C6-Touch-LCD-1.54-EN',False)]
allreceipts=[]
shared=ROOT/'docs/platform/research/waveshare-shared-ecosystem-sources'
ecos={name:json.loads((shared/name).read_text())for name in ['xiaozhi-esp32-board-tree.json','uiflow-micropython-board-tree.json']}
def retain(f,receipt,subdir):
 b=f.read_bytes();assert len(b)==receipt['bytes'] and sha(b)==receipt['sha256'],f
 name=re.sub(r'[^a-zA-Z0-9._-]','_',str(f.relative_to(S)))
 row={**receipt,'repo':subdir,'path':str(f.relative_to(S/subdir/'files')) if 'files'in f.parts else f.name}
 row['url']=receipt.get('url',receipt.get('requestedUrl'));row['resolvedUrl']=receipt.get('resolvedUrl',receipt.get('finalUrl',row['url']));row['archive']=name+'.gz'
 if f.suffix!='.bin':
  data=gzip.compress(b,mtime=0);(ARCH/row['archive']).write_bytes(data);row['archiveSha256']=sha(data)
 else:row.pop('archive')
 allreceipts.append(row);return row
def utc(s):return datetime.datetime.fromisoformat(s.replace('Z','+00:00')).astimezone(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
for q in profiles:
 base='waveshare-'+q['name'].lower().replace('.','-');models={}
 for _,name,case in q['variants']:
  mid=('waveshare-'+name.lower().replace('.','-')) if'1.28'in name else base
  models[name]=mid
 modelplan=json.loads((ROOT/'web/platform/assets'/base/'model-plan.json').read_text())
 rows=[]
 for p in sorted((S/q['repo']/'files').rglob('*.receipt.json')):
  f=Path(str(p).removesuffix('.receipt.json'))
  if f.suffix not in ['.c','.h','.md','.csv','.json','.txt','.pdf','.bin']:continue
  # One canonical BSP example plus exact schematic/firmware/readmes; avoid copied demo duplicates.
  rel=str(f.relative_to(S/q['repo']/'files'))
  if rel.startswith(('example/','examples/')) and not any(v in rel for v in ['01_factory/','examples/factory/','08_LVGL_Demo/','06_esp-brookesia/','03_qmi8658/']):continue
  rows.append(retain(f,json.loads(p.read_text()),q['repo']))
 sourceentries=[]
 for s in modelplan['sources']:
  if s['id']=='product':continue
  sourceentries.append({'id':s['id'],'title':s['file'],'url':s['url'],'kind':'manufacturer-'+('cad'if s['id']=='official-cad'else'source'),'status':'fetched','resolvedUrl':s.get('resolvedUrl',s.get('finalUrl',s['url'])),'fetchedAt':utc(s.get('fetchedAt',s.get('fetchedAtUTC'))),'bytes':s['bytes'],'sha256':s['sha256'],'archiveName':s['file'],'notes':('Derived public Git tree listing, not original HTTP bytes. 'if s.get('captureKind')=='derived-git-ls-tree'else'')+'Retained in web/platform/assets/'+base+'/'+s['file']})
 store=json.loads((R/'products'/(q['listing']+'.json')).read_text())
 sourceentries.append({'id':'product','title':'Official exact-SKU storefront selection','url':store['url'],'kind':'manufacturer-product','status':'linked','notes':'Public SKU selection and original-page receipt retained in model source manifest. Full storefront response remains scratch because it contains anonymous session fields; the selection is not presented as raw response bytes.'})
 selected=[]
 for r in rows:
  if r['path'].endswith('.bin'):continue
  sid='electrical-'+str(len(selected));selected.append({'id':sid,'title':r['path'],'url':r['url'],'kind':'manufacturer-source','status':'fetched','resolvedUrl':r['resolvedUrl'],'fetchedAt':utc(r['fetchedAt']),'bytes':r['bytes'],'sha256':r['sha256'],'archiveName':r['archive'],'notes':'Original response bytes retained at '+str(ARCH.relative_to(ROOT))+'/'+r['archive']})
 sourceentries+=selected
 for key,d in ecos.items():
  sid='xiaozhi-tree'if key.startswith('xiaozhi')else'uiflow-tree';sourceentries.append({'id':sid,'title':d['repository']+' exact board-path inventory','url':d['url'],'kind':'upstream-source-tree','status':'linked','notes':'Pinned Git tree inspected at '+d['fetchedAt']+'; method='+d['method']+'. Shared investigation receipt: docs/platform/research/waveshare-shared-ecosystem-sources/'+key})
 products=[];knowledge=[];firmwares=[];watch=[];fwgroups={};caps={}
 for sku,name,case in q['variants']:
  v={**q};v.pop('variants')
  if'Touch-LCD-1.54'in name:
   v.update(board='waveshare-c6-touch-lcd154',touch='CST816',factoryIndex=5,parts=q['parts']+['CST816@0x15 RST4 IRQ5'])
  pid='waveshare-'+name.lower().replace('.','-');f=factories[v['factoryIndex']];run=next(a for a in runs if a['board']==v['board']);fid='waveshare-'+v['board'].removeprefix('waveshare-')+'-factory'
  sources=[dict(s)for s in sourceentries]
  if case:
   caseplan=json.loads((ROOT/'web/platform/assets'/models[name]/'model-plan.json').read_text())
   s=next(a for a in caseplan['sources']if a['id']=='dimension-photo');sources=[a for a in sources if a['id']!='dimension-photo']
   sources.append({'id':'dimension-photo','title':s['file'],'url':s['url'],'kind':'manufacturer-source','status':'fetched','resolvedUrl':s.get('resolvedUrl',s['url']),'fetchedAt':utc(s['fetchedAt']),'bytes':s['bytes'],'sha256':s['sha256'],'archiveName':s['file'],'notes':'Exact -B enclosure drawing retained in web/platform/assets/'+models[name]+'/'+s['file']})
  sources.append({'id':'factory','title':f['path'],'url':f['url'],'kind':'manufacturer-firmware','status':'fetched','resolvedUrl':f.get('resolvedUrl',f['url']),'fetchedAt':utc(f['fetchedAt']),'bytes':f['bytes'],'sha256':f['sha256'],'archiveName':Path(f['path']).name,'notes':'Original binary validated in scratch and kept remote-only. Retained checksum/partition evidence is docs/evidence/waveshare-c6-new-lcd-2026-11-23/factory-image-verification.json.'})
  controls=[{'id':'boot','name':'BOOT','type':'button','pin':9,'activeLow':True,'defaultKeyCode':'Space'}]
  if v['board']=='waveshare-c6-lcd273':controls=[{'id':'prev','name':'PREV / KEY_L','type':'button','pin':4,'activeLow':True,'defaultKeyCode':'ArrowLeft'},{'id':'next','name':'NEXT / KEY_R','type':'button','pin':5,'activeLow':True,'defaultKeyCode':'ArrowRight'}]+controls
  if'154'in v['board']:controls+=[{'id':'user','name':'+ / KEY','type':'button','pin':18,'activeLow':True,'defaultKeyCode':'Enter'}]
  limits=['部分适配：数字显示/复位/触摸/BOOT和精确总线接线已实现；原厂完整交互、无线和实物尚未验收。','使用原始厂家合并镜像、真实ESP32-C6 ROM；五秒执行记录有0异常，板级像素测试独立于原厂UI。','音频PCM、模拟电源/充电/电池ADC、SHTC3、外部扩展、物理扫描/色彩和传感器姿态未逐项验收。','3D来源与未确认内部结构见模型；重建模型的估计厚度不作为厂家尺寸。','TF数字卡接口已接入；本次原厂记录未插卡，不把无卡错误声明为卡槽不存在。']
  colors=run.get('framebuffer',{}).get('uniqueRgbColors',0)
  limits.append(('原厂启动产生多色帧，但完整应用仍保持验证中。'if colors>1 else'五秒原厂执行已写入显示像素，但最终快照单色；仍需逐项核验界面资源、时序与交互。'))
  if v['board']=='waveshare-c6-lcd273':limits.append('16MiB按当前N16官方概要配置；旧原理图N8冲突已明确记录，不能据同名板自动接受所有历史版本。')
  product={'id':pid,'name':'Waveshare '+name,'manufacturer':'Waveshare','sku':sku,'chip':'esp32c6','hardwareRevision':'unspecified','summary':f"{v['inch']} 英寸 {v['w']}×{v['h']} {v['controller']}，16 MiB Flash、无PSRAM；"+('触摸 '+v['touch']if v['touch']else'无触摸')+'。','knowledgeUrl':'./knowledge/'+pid+'.json','family':{'id':v['board'],'name':'Waveshare '+q['name']},'runtime':{'board':v['board'],'flashMb':16,'psramMb':0,'touch':bool(v['touch']),'status':'partial'},'displayStatus':'known','display':{'width':v['w'],'height':v['h'],'type':'TFT LCD','controller':v['controller'],'bus':'SPI2','diagonalInches':v['inch'],'touch':bool(v['touch'])},'controls':controls,'interfaces':[{'id':'tf','name':'TF / microSD','type':'microSD','external':True,'pins':[{'name':k,'gpio':val}for k,val in v['sd'].items()],'notes':'真实SPI卡槽接线；本次原厂运行未插卡。'}],'sources':[{'id':s['id'],'title':s['title'],'url':s['url'],'checkedAt':'2026-11-23'}for s in sources if s['id']in ['overview','resources','product','factory']],'limitations':limits,'defaultFirmwareId':fid,'chronology':{'date':None,'basis':'unknown','confidence':'unknown','sourceId':None,'sourceUrl':None,'notes':'官方类别Position倒序仅为当前上架位置代理，不能冒充发布日期；固件日期不等于硬件日期。'},'model':{'type':'manufacturer-cad','url':'./assets/'+models[name]+'/model.json','status':'partial','provenance':'official-cad'if q['inch']==2.8 else'dimension-photo-reconstruction'}}
  mid=models[name];mp=json.loads((ROOT/'web/platform/assets'/mid/'model-plan.json').read_text())
  if case:
   e=mp['dimensions']['enclosure'];product['dimensions']={'width':48,'height':48,'depth':e['overallDepthMm'],'unit':'mm','basis':'official','sourceId':'dimension-photo'}
  elif v['inch']==1.54:product['dimensions']={'width':46,'height':46,'depth':22.5,'unit':'mm','basis':'official','sourceId':'dimension-photo'}
  elif v['inch']==2.8:product['dimensions']={'width':50.54,'height':73.06,'depth':10.19999944,'unit':'mm','basis':'cad-measurement','sourceId':'official-cad'}
  refs=[s['id']for s in sources if s['id'].startswith('electrical-')]+['overview'];selectedSku=next(a for a in store['variants']if a['sku']==sku)
  matching=[p for p in ecos['xiaozhi-esp32-board-tree.json']['waveshareBoardPaths']if('/'+q['repo'].lower()+'/')in p]
  facts=[{'id':'hardware','label':'精确SKU硬件与接线','value':{'sku':sku,'storeOptions':dict(zip(store['optionLabels'],selectedSku['attributes'])),'flashMiB':16,'psramMiB':0,'display':product['display'],'spi':v['spi'],'i2c':v['i2c'],'sd':v['sd'],'components':v['parts'],'buttons':controls},'basis':'official','sourceIds':refs+['product']}, {'id':'factory-layout','label':'原厂镜像字节与分区验证','value':{'sha256':f['sha256'],'bytes':f['bytes'],'layout':f['layout'],'evidencePath':'docs/evidence/waveshare-c6-new-lcd-2026-11-23/factory-image-verification.json'},'basis':'official','sourceIds':['factory']}, {'id':'ecosystem-investigation','label':'原厂、小智与UIFlow调查','value':{'officialFactory':'exact downloaded merged image; checksums pass; application partial','xiaozhiExactBoardPaths':matching,'xiaozhiBinaryVerified':False,'uiflowExactNamedBoardPaths':[],'uiflowBinaryVerified':False,'note':'Exact path presence is source support, not a verified flashable image; lack of a matching named path is limited to the pinned inspected tree.'},'basis':'official','sourceIds':['factory','resources','xiaozhi-tree','uiflow-tree']}, {'id':'source-conflicts','label':'厂家资料冲突及采用依据','value':v['conflicts'],'basis':'official','sourceIds':refs}]
  k={'schemaVersion':'1','id':pid,'kind':'product','catalogId':pid,'title':product['name'],'summary':product['summary'],'summarySourceIds':refs,'updatedAt':NOW,'hardwareRevision':'unspecified','facts':facts,'revisions':[],'sources':sources,'assets':[{'id':s['id'],'sourceId':s['id'],'role':'firmware-download'if s['id']=='factory'else'source-document','url':s['url']}for s in sources],'unknowns':[{'id':'release-date','label':'真实硬件发布日期','value':None,'basis':'unknown','sourceIds':['overview','product']},{'id':'full-app-acceptance','label':'完整原厂应用验收','value':None,'basis':'unknown','sourceIds':['factory'],'notes':'Native 5-second bounded run: '+run['log']+'; '+str(colors)+' RGB colors. No browser/WebGL or hardware acceptance inferred.'},{'id':'hardware-depth','label':'未出版内部/深度尺寸','value':None,'basis':'unknown','sourceIds':['resources'],'notes':'Exact source/model limits are retained in '+mid+'/model-plan.json.'}],'collectionNotes':limits+v['conflicts']}
  # The model may be physically shared by package variants; exact SKU knowledge stays separate.
  products.append(product);knowledge.append(k);caps[v['board']]={'chip':'esp32c6','flashMb':16,'psramMb':0,'sdCard':True,'boardButtons':[]}
  fwgroups.setdefault(fid,{'f':f,'v':v,'ids':[]})['ids'].append(pid)
 for fid,g in fwgroups.items():
  f=g['f'];v=g['v'];version=Path(f['path']).stem.split('_',1)[1];remote=f['url']
  firmwares.append({'id':fid,'name':Path(f['path']).name.split('_')[0]+' · 官方出厂镜像（验证中）','productIds':g['ids'],'version':version,'status':'trial','source':{'url':remote,'sha256':f['sha256']},'role':'factory','publisher':'Waveshare','publisherGroup':'官方','simulationVerified':False,'hardwareValidated':False,'sources':[{'title':'Pinned original manufacturer binary','url':remote,'checkedAt':'2026-11-23'}],'limitations':['精确芯片/分区/校验通过，已执行真实ROM五秒启动；完整应用、浏览器和实物验收未完成。','镜像保持厂家原始字节，不加入项目诊断或硬编码跳过初始化。']})
  repo='waveshareteam/'+v['repo'];watch.append({'id':fid+'-watch','name':v['name']+' official factory','category':'factory','upstreamRepo':repo,'metadataUrl':'https://api.github.com/repos/'+repo+'/releases/latest','releaseUrl':'https://github.com/'+repo+'/releases','version':version,'checkedAt':NOW,'assets':[{'name':Path(f['path']).name,'url':remote,'bytes':f['bytes'],'sha256':f['sha256'],'format':'merged'}],'compatibility':{'status':'reference-only','productIds':g['ids'],'simulationVerified':False,'hardwareValidated':False,'note':'Bounded native execution recorded; full application remains partial.'},'storage':'remote-links-only','catalogTargets':[],'monitoringNote':'Track both firmware-directory commits and upstream releases; a missing release API entry does not mean no manufacturer image exists. Firmware dates are not hardware release dates.'})
 fragment={'listingId':q['listing'],'products':products,'firmwares':firmwares,'watchlist':watch,'knowledge':knowledge,'boardCapabilities':caps,'sourceReceiptPaths':[str(ARCH.relative_to(ROOT))+'/manifest.json','web/platform/assets/'+base+'/source-manifest.json'],'tests':['cargo test -p esp32c6 waveshare --lib','docs/evidence/waveshare-c6-new-lcd-2026-11-23/native-runs.json','docs/evidence/waveshare-c6-new-lcd-2026-11-23/model-loader-check.json'],'limitations':['Runtime partial. Exact SKU mechanics and electrical revisions are distinct. No release date invented.']}
 save(OUT/(q['listing']+'.json'),fragment)
save(ARCH/'manifest.json',{'schemaVersion':'1','createdAt':NOW,'scope':'Original HTTP source bodies hash verified and gzip retained; third-party factory binaries remain remote-only. Original storefront HTML stays scratch; exact public SKU selections are explicitly derived.','sources':allreceipts})
print('5 listings',sum(len(q['variants'])for q in profiles),'SKUs',len(allreceipts),'raw source receipts')
