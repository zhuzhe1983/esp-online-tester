"""Export exact 4G V2 fragments; preserve V1 sources and unsupported modem boundaries."""
from pathlib import Path
import argparse, datetime, gzip, hashlib, json, re

ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--sources',type=Path,required=True);a=ap.parse_args()
ROOT=Path(__file__).resolve().parents[3];S=a.sources/'s3-tail-4g-sources';OUT=Path(__file__).parent
ARCH=ROOT/'docs/platform/research/waveshare-s3-cellular-sources';ARCH.mkdir(parents=True,exist_ok=True)
EVID=ROOT/'docs/evidence/waveshare-s3-cellular-2026-11-24';EVID.mkdir(parents=True,exist_ok=True)
def save(p,d):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
def sha(b):return hashlib.sha256(b).hexdigest()
def utc(t):return datetime.datetime.fromisoformat(t.replace('Z','+00:00')).isoformat(timespec='seconds').replace('+00:00','Z')
NOW=utc(datetime.datetime.now(datetime.timezone.utc).isoformat())
F={x['key']:x for x in json.loads((S/'examples-verified.json').read_text())};RUN={x['board']:x for x in json.loads((EVID/'native-runs.json').read_text())}
save(EVID/'original-image-verification.json',list(F.values()))
receipts=[]
for rp in sorted(S.rglob('*.receipt.json')):
 p=Path(str(rp).removesuffix('.receipt.json'));rel=str(p.relative_to(S))
 keep=p.suffix.lower()in['.pdf','.html'] or p.name in ['RGB.ino','SD.ino','bat.ino','camera_pins.h','board_config.h','sdkconfig.defaults']
 # Other demo network code is not needed for these digital/identity conclusions.
 # Binary receipts remain public, original bytes remain remote-only.
 if not keep and p.suffix.lower()!='.bin':continue
 r=json.loads(rp.read_text());b=p.read_bytes();assert len(b)==r['bytes'] and sha(b)==r['sha256']
 r.pop('containerFile',None);r.update(path=rel,repo='Waveshare shared A/SIM7670X V1/V2 resources')
 if keep:
  name=re.sub('[^a-zA-Z0-9._-]','_',rel)+'.gz';data=gzip.compress(b,mtime=0);(ARCH/name).write_bytes(data)
  r.update(archive=name,archiveSha256=sha(data))
 receipts.append(r)
for p in sorted(S.glob('*-inventory.json')):
 raw=p.read_bytes();d=json.loads(raw);name=p.name+'.gz';data=gzip.compress(raw,mtime=0);(ARCH/name).write_bytes(data)
 # The original response is the ZIP. This is explicitly a derived central directory.
 container=Path(str(p).removesuffix('-inventory.json')+'.zip');cr=json.loads(Path(str(container)+'.receipt.json').read_text())
 receipts.append({'repo':'Waveshare ZIP central directory','path':p.name,'url':cr['url'],'resolvedUrl':cr['resolvedUrl'],
  'fetchedAt':cr['fetchedAt'],'bytes':len(raw),'sha256':sha(raw),'archive':name,'archiveSha256':sha(data),
  'captureKind':'derived-archive-member-inventory','containerSha256':cr['sha256'],'containerBytes':cr['bytes']})
save(ARCH/'manifest.json',{'schemaVersion':'1','createdAt':NOW,'scope':'Original selected source bytes retained gzip. Derived ZIP inventories identify their source containers. Firmware binaries remain remote-only; raw storefront bodies are excluded.','sources':receipts})

for modem,sku in [('a7670e','26749'),('sim7670g','26750')]:
 listing='esp32-s3-'+modem+'-4g';pid='waveshare-'+listing;name='ESP32-S3-'+modem.upper()+'-4G-EN';board='waveshare-s3-'+modem+'-4g-v2'
 mp=json.loads((ROOT/'web/platform/assets'/pid/'model-plan.json').read_text());store=json.loads((a.sources/'products'/(listing+'.json')).read_text())
 f=F[modem];run=RUN[board];refs=[]
 for r in mp['sources']:
  refs.append({'id':r['id'],'title':r['file'],'url':r['url'],'resolvedUrl':r.get('resolvedUrl',r['url']),
   'kind':'derived-public-source' if r.get('captureKind') else 'manufacturer-source','status':'fetched',
   'fetchedAt':utc(r['fetchedAt']),'bytes':r['bytes'],'sha256':r['sha256'],'archiveName':r['file'],
   'notes':'Retained in web/platform/assets/'+pid+'/'+r['file']+('; derived public fields, not original HTTP bytes' if r.get('captureKind') else '')})
 for i,r in enumerate(receipts):
  if 'archive' not in r:continue
  # Exact model-specific docs stay separate; electrical shared sources apply to both.
  if '-4G-' in r['path'] and r['path'].endswith('.html') and modem.upper() not in r['path']:continue
  refs.append({'id':'electrical-'+str(i),'title':r['path'],'url':r['url'],'resolvedUrl':r['resolvedUrl'],
   'kind':'derived-public-source' if r.get('captureKind') else 'primary-electrical-source','status':'fetched',
   'fetchedAt':utc(r['fetchedAt']),'bytes':r['bytes'],'sha256':r['sha256'],'archiveName':r['archive'],
   'notes':str(ARCH.relative_to(ROOT))+'/'+r['archive']+'; original member/container identities retained in manifest.'})
 refs.append({'id':'firmware','title':f['archiveMember'],'url':f['url'],'resolvedUrl':f['resolvedUrl'],'kind':'manufacturer-firmware',
  'status':'fetched','fetchedAt':utc(f['fetchedAt']),'bytes':f['bytes'],'sha256':f['sha256'],'archiveName':Path(f['archiveMember']).name,
  'notes':'Exact-model original example member in hash-pinned ZIP; remote-only. Current docs link it, but V2 complete application compatibility is unverified.'})
 eco={}
 for key,file in [('xiaozhi','xiaozhi-esp32-board-tree.json'),('uiflow','uiflow-micropython-board-tree.json')]:
  ed=json.loads((ROOT/'docs/platform/research/waveshare-shared-ecosystem-sources'/file).read_text());eco[key]=ed
  refs.append({'id':key+'-tree','title':ed['repository']+' pinned exact-target investigation','url':ed['url'],'kind':'upstream-source-tree','status':'linked',
   'notes':'Pinned source receipt at docs/platform/research/waveshare-shared-ecosystem-sources/'+file+'. Similar 4G boards are not this product.'})
 electrical=[r['id'] for r in refs if r['id'].startswith('electrical-')]
 pins={'rgb':{'gpio':38,'part':'WS2812B-0807','wireOrder':'GRB','bitsPerSecond':800000},
  'tf':{'transport':'native SDMMC one-bit','CLK':5,'CMD':4,'D0':6,'cardDetectGpio':None},
  'boot':{'gpio':0,'activeLow':True},'reset':'CHIP_PU',
  'MAX17048':{'address7':54,'V2':{'SDA':15,'SCL':16},'V1':{'SDA':3,'SCL':2},'simulation':'not implemented'},
  'modemUart':{'ESP_TX':17,'ESP_RX':18,'RI':40,'DTR':45,'simulation':'not implemented'},'usb':{'DM':19,'DP':20,'simulation':'modem/hub switching not implemented'},
  'camera':{'SDA':15,'SCL':16,'Y2_Y9':[7,8,9,10,11,12,13,14],
   'V2':{'XCLK':39,'VSYNC':42,'HREF':41,'PCLK':46},'V1':{'XCLK':34,'VSYNC':36,'HREF':35,'PCLK':37},'simulation':'not implemented'}}
 conflicts=[
  'Register only the current V2 electrical profile. Official resource page identifies V2 examples as after December 2025; this is a resource-version statement, not an exact SKU release date.',
  'V1 gauge SDA3/SCL2 becomes V2 SDA15/SCL16. Camera timing pads also change; the retained V1 and V2 schematic/source files are not merged into a guessed hybrid.',
  'Old SD_CD_PIN46 names the TF DAT3/CD signal, not a proven independent insertion switch. Current V2 schematic has a separate SD_CS net and V2 SD example removes that macro; GPIO46 is camera PCLK. No fake GPIO46 card-detect level is supplied.',
  'V1 battery demo reads VCELL register0x02 and multiplies by5 while labeling percent; this is not accepted as an accurate battery state-of-charge formula. MAX17048 measurements/ACK are unimplemented.',
  'Current official firmware pages warn against interchanging A7670E/SIM7670G images. Each exact-model ZIP is separately hash/layout checked; the V2 shared demo ZIP contains only the older A7670E example, not a SIM replacement.',
  'These linked images are precompiled examples, not proven factory recovery. A7670E build is November2023; SIM7670G April2024. Both original headers declare4MiB while source hardware is16MiB; original bytes and 16MiB board capacity are preserved.',
  'Both original examples reach usb_cdc_4g_module app_main, print modem initialization wait, and consume the5s bound without a reboot. No USB modem, PPP, network access, GNSS fix, camera capture or battery-gauge success is claimed.',
  'The original example logs a modem reset on GPIO13; V2 camera data also uses GPIO13. This application observation is retained, not used to invent a physical reset connection or rewrite the firmware.']
 limits=['部分适配：已实现 GPIO38 RGB、原生一位 TF 总线和 BOOT0。蜂窝网络、USB 调制解调器、GNSS、摄像头、电量计与模拟电源尚未模拟。',
         '销售套件含亚克力支架；3D 当前仅厂家历史 PCBA 参考，支架、螺柱、线缆及天线装配未建模。未实机验证。']+conflicts+mp['limitations']
 fid='waveshare-s3-'+modem+'-4g-official-example'
 product={'id':pid,'name':'Waveshare '+name+'（PCBA参考，亚克力支架未建模）','manufacturer':'Waveshare','sku':sku,'chip':'esp32s3',
  'hardwareRevision':'V2 electrical profile; current official resources after December2025; native PCBA CAD dated2024-03-08',
  'summary':modem.upper()+' 4G 套件；ESP32-S3R8，16 MiB Flash / 8 MiB PSRAM，无板载屏幕。当前适配 RGB 与 TF 数字路径；3D 为历史 PCBA 参考，亚克力支架未建模。',
  'knowledgeUrl':'./knowledge/'+pid+'.json','family':{'id':listing,'name':'Waveshare '+name},
  'runtime':{'board':board,'flashMb':16,'psramMb':8,'touch':False,'status':'partial'},'displayStatus':'none','display':None,
  'controls':[{'id':'boot','name':'BOOT','type':'button','pin':0,'activeLow':True,'defaultKeyCode':'Space'}],
  'interfaces':[{'id':'tf','name':'TF / microSD','type':'microSD','external':True,'pins':[{'name':'CLK','gpio':5},{'name':'CMD','gpio':4},{'name':'D0','gpio':6}],
   'notes':'Default64MiB FAT32 card; exact one-bit SDMMC and sector DMA tested with actual LX7 instructions. No independent detect GPIO. Four-bit mode is rejected.'},
   {'id':'usb','name':'USB-C / switched USB hub and modem','type':'USB-C','external':True,'pins':[],'notes':'Native manufacturer connector reference only. Port-to-hub switch position and individual connector contact mapping are unverified; modem CDC/USB hub routing and physical cable mating are not implemented.'}],
  'sources':[{'id':r['id'],'title':r['title'],'url':r['url'],'checkedAt':'2026-11-24'} for r in refs if r['id'] in ['overview','resources','product']],
  'limitations':limits,'chronology':{'date':None,'basis':'unknown','sourceId':None,'sourceUrl':None,'confidence':'unknown','notes':'V2 example chronology afterDecember2025 is recorded separately; no exact product release date inferred.'},
  'model':{'type':'manufacturer-cad','url':'./assets/'+pid+'/model.json','status':'partial','provenance':'official-cad'},'defaultFirmwareId':fid}
 firmware={'id':fid,'name':name+' 官方 USB 4G 示例（验证中）','productIds':[pid],'version':Path(f['archiveMember']).stem,'status':'trial',
  'source':{'url':f['url'],'sha256':f['sha256'],'bootMode':'rom','archive':{'format':'zip-bundle','entry':f['archiveMember'],'sha256':f['containerSha256']}},
  'role':'example','publisher':'Waveshare','publisherGroup':'官方','simulationVerified':False,'hardwareValidated':False,
  'sources':[{'title':'Exact modem-specific official precompiled example','url':f['url'],'checkedAt':'2026-11-24'}],
  'limitations':conflicts+['No complete factory image was identified as factory-purpose in the inspected official pages/archives. This default remains an example.']}
 facts=[{'id':'exact-hardware','label':'精确型号、容量与 V2 接线','basis':'official','sourceIds':['overview','resources','product']+electrical,
   'value':{'sku':sku,'partNumber':name,'chip':'ESP32-S3R8','flashMiB':16,'psramMiB':8,'display':None,'pins':pins}},
  {'id':'revision-conflicts','label':'版次、源码和原始示例限制','basis':'official','sourceIds':['resources','firmware']+electrical,'value':conflicts},
  {'id':'kit-scope','label':'实际销售套件与 3D 范围','basis':'official','sourceIds':['product','dimension-photo','official-cad'],'value':{
   'package':json.loads((ROOT/'web/platform/assets'/pid/'sku-selection.json').read_text()),'dimensions':mp['dimensions'],'mechanicalLimitations':mp['limitations']}},
  {'id':'original-example','label':'原始示例身份、布局与有界试运行','basis':'official','sourceIds':['firmware'],'value':{
   'role':'example','factoryInvestigation':'not-found-as-factory-purpose-in-retained-scope','sha256':f['sha256'],'layout':f['layout'],
   'runtimeObservation':run['state'],'evidence':str(EVID.relative_to(ROOT))+'/native-runs.json'},
   'notes':'Image identity/layout is source-derived. RuntimeObservation is a simulator measurement, not a manufacturer compatibility statement.'},
  {'id':'ecosystem-investigation','label':'原厂、UIFlow、小智调查','basis':'official','sourceIds':['resources','firmware','xiaozhi-tree','uiflow-tree'],
   'value':{'factoryPurposeImage':None,'officialExampleAvailable':True,'xiaozhiExactAcceptedBinary':False,'uiflowExactAcceptedBinary':False,
    'scope':'Pinned exact resource pages, V1/V2 ZIP inventories and upstream board trees. Generic or neighboring 4G targets are not assigned.'}}]
 knowledge={'schemaVersion':'1','id':pid,'kind':'product','catalogId':pid,'title':product['name'],'summary':product['summary'],
  'summarySourceIds':['overview','resources','product'],'updatedAt':NOW,'hardwareRevision':product['hardwareRevision'],'facts':facts,'revisions':[],
  'sources':refs,'assets':[{'id':r['id'],'sourceId':r['id'],'role':'firmware-download' if r['id']=='firmware' else 'source-document','url':r['url']} for r in refs],
  'unknowns':[{'id':key,'label':label,'basis':'unknown','value':None,'sourceIds':ids} for key,label,ids in [
   ('exact-release-date','准确硬件发布日期',['product','resources']),('v2-mechanical-layout','V2 精确 PCB/亚克力装配尺寸与接口锚点',['official-cad','dimension-photo']),
   ('factory-recovery','明确标为出厂恢复的完整镜像',['resources','firmware']),('full-acceptance','完整应用和实物验收',['firmware'])]],'collectionNotes':limits}
 save(ROOT/'web/platform/knowledge'/(pid+'.json'),knowledge)
 watch=[{'id':fid+'-watch','name':firmware['name'],'category':'development','upstreamRepo':'manufacturer:Waveshare/'+name,
  'metadataProvider':'manual','metadataUrl':'https://docs.waveshare.com/ESP32-S3-'+modem.upper()+'-4G/Firmware','releaseUrl':f['url'],
  'version':firmware['version'],'checkedAt':NOW,'assets':[{'name':Path(f['path']).name,'url':f['url'],'bytes':f['containerBytes'],'sha256':f['containerSha256'],
   'format':'zip-bundle','archiveMember':f['archiveMember'],'memberBytes':f['bytes'],'memberSha256':f['sha256']}],
  'compatibility':{'status':'reference-only','productIds':[pid],'simulationVerified':False,'hardwareValidated':False,
   'note':'Exact-model source/layout and5s native observation only. Modem remains unavailable; future versions must be independently reviewed.'},
  'storage':'remote-links-only','catalogTargets':[],'monitoringNote':'Keep example identity; no source labels this exact image as factory recovery. Do not interchange A7670E/SIM7670G.'},
  {'id':listing+'-factory-investigation','name':name+' factory-purpose investigation','category':'factory','upstreamRepo':'manufacturer:Waveshare/'+name,
   'metadataProvider':'manual','metadataUrl':store['url'],'releaseUrl':'https://docs.waveshare.com/ESP32-S3-'+modem.upper()+'-4G/Resources-And-Documents',
   'version':'no-factory-purpose-image-found','checkedAt':NOW,'assets':[],'compatibility':{'status':'reference-only','productIds':[pid],
   'simulationVerified':False,'hardwareValidated':False,'note':'Official example available; exact factory-purpose binary absent from retained scope.'},'storage':'remote-links-only','catalogTargets':[]}]
 for key,ed in eco.items():
  matches=[p for p in ed['waveshareBoardPaths'] if '/'+listing+'/' in p]
  watch.append({'id':listing+'-'+key+'-investigation','name':name+' '+key+' exact-target investigation','category':'voice' if key=='xiaozhi' else 'development',
   'upstreamRepo':ed['repository'],'metadataProvider':'manual','metadataUrl':ed['url'],'releaseUrl':ed['url'],'version':ed['commit'],'checkedAt':NOW,'assets':[],
   'compatibility':{'status':'reference-only','productIds':[pid],'simulationVerified':False,'hardwareValidated':False,
    'note':'Exact source paths '+json.dumps(matches)+'. No exact-model accepted complete binary; neighboring4G targets are not substituted.'},'storage':'remote-links-only','catalogTargets':[]})
 for entry in watch:entry.setdefault('monitoringNote','Manual exact-model source investigation; no automatic firmware acceptance or neighboring-board substitution.')
 save(OUT/(listing+'.json'),{'listingId':listing,'products':[product],'firmwares':[firmware],'watchlist':watch,
  'knowledge':[{'path':'web/platform/knowledge/'+pid+'.json'}],
  'boardCapabilities':{board:{'chip':'esp32s3','flashMb':16,'psramMb':8,'sdCard':True,'boardButtons':[]}},
  'sourceReceiptPaths':[str(ARCH.relative_to(ROOT))+'/manifest.json','web/platform/assets/'+pid+'/source-manifest.json'],
  'tests':['cargo test -p esp32s3 --lib waveshare_s3_cellular','cargo test -p esp32s3 --test waveshare_cellular_guest',
   str(EVID.relative_to(ROOT))+'/native-runs.json',str(EVID.relative_to(ROOT))+'/model-loader-check.json'],'limitations':limits})
print('2 listing families;2 exact V2 SKU profiles;2 independent official examples;qualified shared native PCBA geometry')
