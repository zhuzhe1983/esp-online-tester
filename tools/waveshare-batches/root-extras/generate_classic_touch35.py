#!/usr/bin/env python3
"""Exact classic Touch LCD3.5 SKUs; source receipts and partial digital contract."""
import copy
import datetime
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SCRATCH = ROOT.parent
SOURCES = SCRATCH / 'root-sources'
OUT = ROOT / 'docs/platform/research/waveshare-classic-touch35-catalog-20261124'
OUT.mkdir(parents=True, exist_ok=True)
STAMP = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')

def read(p): return json.loads(p.read_text())
def save(p, value):
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n')
def sha(b): return hashlib.sha256(b).hexdigest()
def utc(t): return datetime.datetime.fromisoformat(t.replace('Z','+00:00')).astimezone(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')

rows=[];retained=[]
def retain(sid,p,receipt):
    b=p.read_bytes();assert len(b)==receipt['bytes'] and sha(b)==receipt['sha256'],p
    url=receipt.get('requestedUrl',receipt.get('url'));assert url
    row={'id':sid,'title':p.name,'url':url,'kind':'manufacturer-source','status':'fetched','resolvedUrl':receipt['finalUrl'],'fetchedAt':receipt['fetchedAt'],'bytes':len(b),'sha256':sha(b),'archiveName':sid+'--'+p.name}
    record={'id':sid,'source':receipt}
    if p.suffix=='.bin':record['retention']='remote-links-only; original bytes retained outside repository'
    else:
        name=sid+'--'+p.name+'.gz';encoded=gzip.compress(b,mtime=0);(OUT/name).write_bytes(encoded)
        record.update(retained=name,retainedBytes=len(encoded),retainedSha256=sha(encoded));row['notes']='Original bytes: '+str((OUT/name).relative_to(ROOT))
    rows.append(row);retained.append(record)

family='ESP32-Touch-LCD-3.5';listing='esp32-touch-lcd-3.5';src=SOURCES/family
for i,p in enumerate(sorted(src.glob('*.receipt.json'))):
    receipt=read(p);name='tree.json' if p.name=='tree.receipt.json' else p.name.removesuffix('.receipt.json');original=src/name
    if receipt.get('error') or not original.is_file() or not receipt.get('bytes'):continue
    sid='source-'+str(i)
    if name=='README.md':sid='readme'
    elif name=='firmware__README.md':sid='firmware-readme'
    elif name.endswith('_FactoryOnly_20261021.bin'):sid='factory'
    elif name=='tree.json':sid='tree'
    elif name.endswith('Schematic-V1.2.pdf'):sid='schematic-v12'
    elif name.endswith('bsp__config.h'):sid='bsp-pins'
    retain(sid,original,receipt)
photo=SOURCES/listing
retain('overview',photo/'overview.html',read(photo/'overview.html.receipt.json'))
item=next(x for x in read(SCRATCH/'official_inventory/queue-c5-classic.json')['items'] if x['listingId']==listing)
model_id='waveshare-esp32-touch-lcd-3-5';model_dir=ROOT/'web/platform/assets'/model_id;model=read(model_dir/'model.json');manifest=read(model_dir/'source-manifest.json')
for i,s in enumerate(manifest['sources']):
    rows.append({'id':'model-'+str(i),'title':s['id'],'url':s['url'],'kind':'manufacturer-mechanical-source','status':'fetched','resolvedUrl':s['finalUrl'],'fetchedAt':s['fetchedAt'],'bytes':s['bytes'],'sha256':s['sha256'],'archiveName':model_id+'--'+s['file'],'notes':'Unchanged source/retention hashes in '+str((model_dir/'source-manifest.json').relative_to(ROOT))})
source_map={s['id']:s for s in rows};assert len(source_map)==len(rows)
for s in rows:s['fetchedAt']=utc(s['fetchedAt'])
audit=read(SOURCES/'classic-touch35-factory-audit.json');save(OUT/'factory-layout-audit.json',audit);save(OUT/'source-manifest.json',{'sources':retained,'modelManifest':str((model_dir/'source-manifest.json').relative_to(ROOT)),'rights':'Original sources retain their respective rights; public access is not a separate redistribution licence.'})
short=[{'id':s['id'],'title':s['title'],'url':s['url'],'checkedAt':str(s['fetchedAt'])[:10]}for s in rows if s['id']in ['overview','schematic-v12','bsp-pins','factory','firmware-readme','readme']]
limits=['经典ESP32-D0WDR2-V3的16MiB Flash/2MiB PSRAM配置，不能替换成同尺寸S3/C5板。','SPI RGB565、FT6336单点、TCA9554复位、RTC、codec控制寄存器和SPI TF为数字子集；音频PCM、PMIC到MCU系统断电、充电、电池模拟和触摸手势未验收。','仅BOOT GPIO0作为物理按键输入；PWR/RESET外形保留，但不把SYS_OUT36状态脚冒充电源键。','厂家完整原固件原字节通过芯片/分区/校验审计；实际5s运行已过16MiB Flash/2MiB PSRAM启动与内存测试，进入app_main后PMU配置报ESP_ERR_INVALID_STATE，屏幕仍黑/0更新；保持trial，完整应用、WebGL与实物未验收。','精确PCB没有公开CAD；尺寸/照片重建的外壳墙厚、PCB尺寸/高度、按钮深度和内部连接仍为显式估计/UNKNOWN；只使用本款经典ESP32照片。']
board='waveshare-esp32-touch35';fwid='waveshare-esp32-touch-lcd-3-5-factory-20261021';products=[];knowledge=[]
for variant in item['variants']:
    sku=variant['sku'];pid=model_id if sku=='32870' else model_id+'-no-battery';name='Waveshare ESP32-Touch-LCD-3.5'+(' · 含电池' if sku=='32870' else ' · 不含电池')
    product={'id':pid,'name':name,'manufacturer':'Waveshare','sku':sku,'hardwareRevision':'V1.2 source schematic; retail revision unspecified','chip':'esp32','summary':'经典ESP32，16MiB Flash/2MiB PSRAM；320×480 ST7796、FT6336触摸，SKU '+sku,'knowledgeUrl':'./knowledge/'+pid+'.json','family':{'id':model_id,'name':family},'runtime':{'board':board,'flashMb':16,'psramMb':2,'status':'partial','touch':True},'displayStatus':'known','display':{'width':320,'height':480,'diagonalInches':3.5,'type':'TFT LCD','controller':'ST7796','bus':'SPI','touch':True},'controls':[{'id':'boot','name':'BOOT','type':'button','pin':0,'activeLow':True,'defaultKeyCode':'Space'}],'interfaces':[{'id':'usb-c','name':'USB-C 编程/供电','type':'USB-C','external':True,'pins':[],'sourceIds':['overview']},{'id':'tf','name':'TF / microSD','type':'microSD','external':True,'pins':[{'name':k,'pin':v}for k,v in [('CLK',18),('MOSI',23),('MISO',19),('CS',15)]],'sourceIds':['bsp-pins','schematic-v12']}],'sources':short,'limitations':limits,'chronology':{'date':None,'basis':'unknown','confidence':'unknown','sourceId':None,'sourceUrl':None,'notes':'官方固件时间不是硬件发布日期。'},'model':{'type':'manufacturer-cad','url':'./assets/'+model_id+'/model.json','status':'partial','provenance':'dimension-photo-reconstruction'},'defaultFirmwareId':fwid}
    products.append(product)
    facts=[{'id':'identity','label':'精确型号/库存选项','value':{'sku':sku,'options':variant['options'],'chip':'ESP32-D0WDR2-V3','flashMiB':16,'psramMiB':2},'basis':'official','sourceIds':['overview','schematic-v12']},{'id':'electrical','label':'官方原理图及BSP接线','value':{'lcd':{'controller':'ST7796','pixels':[320,480],'host':'SPI2','clock':18,'mosi':23,'miso':19,'cs':5,'dc':27,'backlight':25,'reset':'TCA9554 EX0'},'touch':{'controller':'FT6336','address':'0x38','irq':37,'reset':'TCA9554 EX1'},'i2c':{'sda':21,'scl':22,'devices':{'0x20':'TCA9554','0x51':'PCF85063','0x18':'ES8311','0x34':'AXP2101'}},'boot':0,'sysOut':36,'sd':{'clock':18,'mosi':23,'miso':19,'cs':15}},'basis':'official','sourceIds':['bsp-pins','schematic-v12']},{'id':'original-image','label':'原厂完整镜像审计','value':{'bytes':source_map['factory']['bytes'],'sha256':source_map['factory']['sha256'],'bootloaderOffset':4096,'chipId':0,'partitionTableOffset':32768,'flashMiB':16,'audit':str((OUT/'factory-layout-audit.json').relative_to(ROOT))},'basis':'official','sourceIds':['factory','firmware-readme']},{'id':'mechanical','label':'来源可核验的外观模型','value':{'modelSha256':sha((model_dir/'model.json').read_bytes()),'outerMm':[63.67,95.11,13.50],'activeAreaMm':[48.96,73.44],'feetTotalDepthMm':14.10,'method':'dimension-photo-reconstruction','unknowns':manifest['limitations']},'basis':'derived','sourceIds':['model-'+str(i)for i in range(len(manifest['sources']))]}]
    facts[-1].update(basis='image-measurement',method='Use manufacturer dimension labels and the declared original-photo registration; rebuild with prepare_classic_touch35_model.py and the production loader.',uncertainty='Annotated outer/active-area dimensions retained at published precision. Wall, PCB and key depths are explicit estimates; hidden geometry remains UNKNOWN, without fit tolerance or hardware acceptance.')
    record={'schemaVersion':'1','id':pid,'kind':'product','catalogId':pid,'title':name,'summary':product['summary'],'summarySourceIds':['overview','schematic-v12'],'updatedAt':STAMP,'hardwareRevision':product['hardwareRevision'],'facts':facts,'revisions':[],'sources':rows,'assets':[{'id':s['id'],'sourceId':s['id'],'role':'source-document','url':s['url']}for s in rows],'unknowns':[{'id':'release-date','label':'产品发布日期','basis':'unknown','value':None,'sourceIds':['overview']},{'id':'hardware-acceptance','label':'实物/整机验收','basis':'unknown','value':None,'sourceIds':[]},{'id':'exact-thirdparty','label':'精确UIFlow/Xiaozhi兼容恢复包','basis':'unknown','value':None,'sourceIds':['tree'],'notes':'仅监视官方生态，未由通用ESP32支持推导此板能运行。'}],'collectionNotes':limits}
    path=ROOT/'web/platform/knowledge'/f'{pid}.json';save(path,record);knowledge.append({'path':str(path.relative_to(ROOT))})
factory=source_map['factory'];firmware={'id':fwid,'name':family+' · 官方FactoryOnly 20261021（trial）','productIds':[p['id']for p in products],'version':'FactoryOnly_20261021','status':'trial','source':{'url':factory['url'],'sha256':factory['sha256'],'bootMode':'rom'},'role':'factory','publisher':'Waveshare','publisherGroup':'官方','simulationVerified':False,'hardwareValidated':False,'storage':'remote-links-only','publicationUrl':source_map['firmware-readme']['url'],'sources':short,'limitations':limits}
watch={'id':fwid+'-watch','name':family+' 官方目录人工复核','category':'factory','metadataProvider':'manual','upstreamRepo':'waveshareteam/'+family,'metadataUrl':source_map['firmware-readme']['url'],'releaseUrl':source_map['firmware-readme']['url'],'version':'51f354d2aeb2245fd7b748c9f3ea93cea1c527ae','checkedAt':STAMP,'assets':[{'name':'ESP32-Touch-LCD-3.5_FactoryOnly_20261021.bin','url':factory['url'],'sha256':factory['sha256'],'bytes':factory['bytes'],'format':'merged'}],'compatibility':{'status':'reference-only','productIds':[p['id']for p in products],'simulationVerified':False,'hardwareValidated':False,'note':'原目录镜像需逐版本人工复核；不自动替换目录。'},'catalogTargets':[],'storage':'remote-links-only','monitoringNote':'Manufacturer repository-directory release, not a GitHub release asset; manual source and image review required.'}
fragment={'listingId':listing,'products':products,'firmwares':[firmware],'watchlist':[watch],'knowledge':knowledge,'boardCapabilities':{board:{'chip':'esp32','flashMb':16,'psramMb':2,'sdCard':True,'boardButtons':[]}},'sourceReceiptPaths':[str((OUT/'source-manifest.json').relative_to(ROOT)),str((OUT/'factory-layout-audit.json').relative_to(ROOT))],'tests':['cargo test -p esp32 --test waveshare_touch35'],'limitations':limits};save(Path(__file__).with_name(listing+'.json'),fragment)
print(json.dumps({'listing':listing,'skus':len(products),'sources':len(rows),'firmwareSha256':factory['sha256']}))
