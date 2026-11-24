#!/usr/bin/env python3
"""Curate five exact S3 Audio/DIN families from checked manufacturer source caches.

Third-party firmware bodies are never copied to this repository. Mechanical
reproduction uses committed model plans, original pinned assets and the public
rebuild-product-model tool independently of this metadata curation cache.
"""
from pathlib import Path
import argparse, copy, datetime, gzip, hashlib, json, re, zipfile

ROOT = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--sources', type=Path, required=True)
parser.add_argument('--inventory', type=Path, required=True)
args = parser.parse_args()
S = args.sources
D = ROOT / 'docs/research/waveshare-s3-industrial-2026-11-24'
E = D / 'batch-02'
O = ROOT / 'tools/waveshare-batches/s3-industrial'
K = ROOT / 'web/platform/knowledge'
NOW = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
rows = json.loads(args.inventory.read_text())['items']
audits = {x['listingId']: x for x in json.loads((S/'batch02-firmware-audit.json').read_text())}

def sha(data): return hashlib.sha256(data).hexdigest()
def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n')
def utc(value):
    return datetime.datetime.fromisoformat(value.replace('Z', '+00:00')).astimezone(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def source(sid, title, receipt, **extra):
    url = receipt.get('requestedUrl') or receipt.get('originalUrl') or receipt.get('url')
    return {'id': sid, 'title': title, 'url': url,
            'resolvedUrl': receipt.get('finalUrl') or receipt.get('resolvedUrl') or url,
            'kind': 'manufacturer-document', 'status': 'fetched',
            'fetchedAt': utc(receipt['fetchedAt']), 'bytes': receipt['bytes'],
            'sha256': receipt['sha256'], 'archiveName': extra.pop('archiveName', url.split('?')[0].rstrip('/').split('/')[-1]), 'mediaType': receipt.get('contentType') or receipt.get('mediaType') or 'application/octet-stream', **extra}
def retain(folder, path, sid, curated_text=False):
    receipt = json.loads(path.with_name(path.name+'.source.json').read_text())
    raw = path.read_bytes()
    assert sha(raw) == receipt['sha256']
    retained = path.with_suffix('.txt').read_bytes() if curated_text else raw
    fn = sid + ('.txt' if curated_text else path.suffix) + '.gz'
    data = gzip.compress(retained, mtime=0)
    (folder/fn).write_bytes(data)
    entry = source(sid, path.name, receipt, archiveName=fn,
                   notes='Original response identity retained; article-only text excludes navigation/session markup.' if curated_text else 'Original public manufacturer source bytes retained without modification.')
    entry['retained'] = {'path': str((folder/fn).relative_to(ROOT)), 'encoding': 'gzip', 'bytes': len(data), 'sha256': sha(data), 'uncompressedSha256': sha(retained), 'transform': 'article-text-extraction' if curated_text else 'none'}
    return entry
def member(folder, archive, entry, sid):
    receipt = json.loads(archive.with_name(archive.name+'.source.json').read_text())
    assert sha(archive.read_bytes()) == receipt['sha256']
    with zipfile.ZipFile(archive) as z: raw = z.read(entry)
    data = gzip.compress(raw, mtime=0)
    fn = sid + Path(entry).suffix + '.gz'
    (folder/fn).write_bytes(data)
    r = {**receipt, 'bytes': len(raw), 'sha256': sha(raw), 'mediaType': 'text/plain'}
    r.pop('contentType', None)
    return source(sid, entry, r, archiveName=fn, archive={'containerSha256': receipt['sha256'], 'entry': entry},
                  retained={'path': str((folder/fn).relative_to(ROOT)), 'encoding': 'gzip', 'bytes': len(data), 'sha256': sha(data)},
                  notes='Exact ZIP member; enclosing archive fetch identity and original member SHA retained. The firmware archive itself is not mirrored.')
def fact(sid, title, value, sources, basis='official', **extra):
    return {'id': sid, 'label': title, 'value': value, 'sourceIds': sources, 'basis': basis, **extra}

cfgs = [
 {'listing':'esp32-s3-audio-board','doc':'ESP32-S3-AUDIO-Board','names':['ESP32-S3-AUDIO-Board','ESP32-S3-AUDIO-Board-EN'],
  'boards':['waveshare-s3-audio-board','waveshare-s3-audio-board-en'], 'i2c':[11,10], 'addresses':[24,32,64,81],
  'sd':[40,42,41], 'rgb':7, 'power':{'usbNominalV':5,'batteryNominalV':3.7,'batteryConnector':'MX1.25','includedBatterySku':'32184','batteryOmittedSku':'32185'},
  'features':'双麦克风、扬声器、七颗RGB；TCA9555三键与codec控制寄存器、RTC、原生SD数字子集。',
  'limits':['ES8311/ES7210 control register endpoints only; microphone capture, PCM playback, voice recognition, camera/display options and cloud services are not implemented.', 'Three keys are TCA9555 inputs P11/P12/P13 (linear9/10/11). Seven RGB chain positions have not been mapped to native3D parts.', 'SKU32184 includes native1000mAh battery. SKU32185/EN excludes it; battery/charging/RTC-backup electrical physics remain unmodeled.']},
 {'listing':'esp32-s3-rs485-can','doc':'ESP32-S3-RS485-CAN','names':['ESP32-S3-RS485-CAN','ESP32-S3-RS485-CAN-U'],
  'boards':['waveshare-s3-rs485-can','waveshare-s3-rs485-can-u'],'i2c':[39,38],'addresses':[81],'sd':None,'rgb':0,
  'power':{'terminalInputV':[7,36],'usbNominalV':5},'features':'RS485/CAN通信主控，精确I2C路由与PCF85063 RTC数字子集。',
  'limits':['RS485 UART17/18 and DE21; TWAI TX15/RX16 physical facts retained. No bus peer, isolation, network or electrical transceiver behavior claimed.', 'RS485-CAN has no Relay1 coil output; a shared manufacturer header name does not add a relay to this SKU.']},
 {'listing':'esp32-s3-relay-1ch','doc':'ESP32-S3-Relay-1CH','names':['ESP32-S3-Relay-1CH','ESP32-S3-Relay-1CH-U'],
  'boards':['waveshare-s3-relay-1ch','waveshare-s3-relay-1ch-u'],'i2c':[39,38],'addresses':[81],'sd':None,'rgb':0,
  'power':{'terminalInputV':5,'usbNominalV':5},'features':'单路继电器，GPIO47实际客体驱动状态及RTC数字子集。',
  'limits':['Only GPIO47 coil control is observed after correct mux/direction. Contact continuity, isolation, voltage/current/load switching are not simulated.', 'RS485 UART17/18 and DE21 physical wiring is documented; no remote Modbus/serial peer or network is synthesized.']},
 {'listing':'esp32-s3-poe-eth-8di-8do','doc':'ESP32-S3-POE-ETH-8DI-8DO','names':['ESP32-S3-POE-ETH-8DI-8DO'],
  'boards':['waveshare-s3-poe-eth-8di-8do'],'i2c':[42,41],'addresses':[32,81],'sd':[48,47,45],'rgb':1,
  'power':{'terminalInputV':[7,36],'usbNominalV':5,'usbRecommendedCurrentA':1,'poe':True},'features':'八路数字输入和Darlington输出；TCA9554、RTC、原生SD及RGB数字子集。',
  'limits':['Eight Darlington output control bits are modeled separately from relay contacts; no load current, isolation or analog voltage simulation.', 'GPIO4..11 input controls inject digital pad levels. External24V optocoupler transfer is unmodeled.', 'W5500 Ethernet, PoE power, CAN/RS485 peers, buzzer acoustics and network services are unimplemented.']},
 {'listing':'esp32-s3-eth-8di-8ro-c','doc':'ESP32-S3-ETH-8DI-8RO-C','names':['ESP32-S3-ETH-8DI-8RO-C','ESP32-S3-POE-ETH-8DI-8RO-C'],
  'boards':['waveshare-s3-eth-8di-8ro-c','waveshare-s3-poe-eth-8di-8ro-c'],'i2c':[42,41],'addresses':[32,81],'sd':[48,47,45],'rgb':1,
  'power':{'terminalInputV':[7,36],'usbNominalV':5,'usbRecommendedCurrentA':1,'poeSku':'31284'},'features':'八路输入与八路SPDT继电器，TCA9554、RTC、原生SD及RGB数字子集。',
  'limits':['Eight TCA9554 relay coil controls are observable, but contact loads, current ratings, isolation and analog switching are unmodeled.', 'GPIO4..11 input controls inject digital pad levels, not optocoupler voltage transfer. PoE SKU31284 remains separate from common Ethernet SKU31282.', 'W5500 Ethernet, PoE power, CAN/RS485 peers, buzzer acoustics and network services are unimplemented.']}
]
releases = ROOT/'docs/research/waveshare-p4-s31-2026-11-24/ecosystem-releases'
release_sources = json.loads((releases/'source-manifest.json').read_text())['sources']
release = {key:json.loads(gzip.decompress((releases/file).read_bytes())) for key,file in [('uiflow','m5stack-uiflow-micropython.json.gz'),('xiaozhi','78-xiaozhi-esp32.json.gz')]}
eco = [source(key+'-release', key+' fixed official release metadata', next(s for s in release_sources if s['archiveName']==file), notes='Original source metadata retained in '+str((releases/file).relative_to(ROOT))) for key,file in [('uiflow','m5stack-uiflow-micropython.json.gz'),('xiaozhi','78-xiaozhi-esp32.json.gz')]]
m5dir = ROOT/'docs/research/waveshare-classic-s2-2026-11-24/ecosystem'
m5source = json.loads((m5dir/'source-manifest.json').read_text())['sources'][0]
m5data = json.loads(gzip.decompress((m5dir/'m5burner-devices.json.gz').read_bytes()))['data']
assert len(m5data)==55 and not any('waveshare' in json.dumps(d).lower() for d in m5data)
eco.append(source('m5burner-devices','M5Burner55 public device categories',m5source,archiveName='m5burner-devices.json.gz',notes='Raw metadata retained in docs/research/waveshare-classic-s2-2026-11-24/ecosystem; exact categories only, not all community uploads.'))
batch = []
for cfg in cfgs:
    listing=cfg['listing']; row=next(x for x in rows if x['listingId']==listing); folder=D/listing; folder.mkdir(parents=True,exist_ok=True)
    sroot=S/listing; audit=audits[listing]; assert audit['status']=='passed'
    archive=sroot/Path(audit['source']['url']).name
    sources=[source('storefront','Official exact SKU table',row['productSource'],notes='Original-source identity, curated SKU options only; anonymous storefront session HTML omitted.')]
    save(folder/'storefront-identity.json',{'listingId':listing,'source':row['productSource'],'variants':row['variants']})
    sources.append(retain(folder,sroot/'overview.html','overview',True))
    sources.append(source('manufacturer-archive','Official program archive',json.loads(archive.with_name(archive.name+'.source.json').read_text()),notes='Source archive hash is separate from selected BIN hash; firmware bodies remain remote.'))
    save(folder/'archive-members.json',json.loads((sroot/'archive-members.json').read_text()))
    if listing.endswith('audio-board'):
        for sid,name in [('firmware-guide','firmware-flashing'),('factory-project-guide','esp-idf')]:sources.append(retain(folder,sroot/(name+'.html'),sid,True))
        sources.append(retain(folder,sroot/'ESP32-S3-AUDIO-Board_1.1.pdf','schematic'))
        prefix='ESP32-S3-AUDIO-Board-Demo/ESP-IDF/factory_01/main/'
        files=[('bsp',prefix+'hardeware_driver/bsp_board.h'),('bsp-impl',prefix+'hardeware_driver/bsp_board.c'),('key-bsp',prefix+'button_driver/button_driver.h'),('rgb-bsp',prefix+'rgb_led_driver/rgb_led_driver.c'),('expander-bsp',prefix+'tca9555_driver/tca9555_driver.c')]
    else:
        files=[]
        with zipfile.ZipFile(archive) as z:
            names=z.namelist();base='Arduino/examples/MAIN_ALL/' if any(n.startswith('Arduino/examples/MAIN_ALL/') for n in names) else 'Arduino/examples/MAIN_WIFI_AP/' if any(n.startswith('Arduino/examples/MAIN_WIFI_AP/') for n in names) else 'Arduino/examples/MAIN_MQTT_ALL/'
            for sid,name in [('bsp','WS_GPIO.h'),('i2c-bsp','I2C_Driver.h'),('sd-bsp','WS_SD.h'),('input-bsp','WS_DIN.h'),('relay-bsp','WS_Relay.h'),('expander-bsp','WS_TCA9554PWR.h'),('ethernet-bsp','WS_ETH.h')]:
                if base+name in names:files.append((sid,base+name))
        for path in sroot.glob('*.pdf'):
            if 'schematic' in path.name.lower():sources.append(retain(folder,path,'schematic'));break
    for sid,entry in files:sources.append(member(folder,archive,entry,sid))
    if listing.endswith('audio-board'):
        for sid,name in [('xiaozhi-config','config.h'),('xiaozhi-target','config.json'),('xiaozhi-readme','README.md')]:sources.append(retain(folder,sroot/'ecosystem'/name,sid))
    with zipfile.ZipFile(archive) as z:binary=z.read(audit['source']['archive']['entry'])
    assert sha(binary)==audit['source']['sha256']
    image_source={**json.loads(archive.with_name(archive.name+'.source.json').read_text()),'bytes':len(binary),'sha256':sha(binary),'contentType':'application/octet-stream'}
    sources.append(source('manufacturer-image','Exact original manufacturer image',image_source,notes='Exact member '+audit['source']['archive']['entry']+'; no firmware bytes mirrored.'))
    save(folder/'firmware-image-audit.json',audit)
    purpose='Manufacturer guide explicitly names Factory bin and factory demonstration.'
    if listing.endswith('audio-board'):
        assert b'factory_01' in binary[0x10020:0x10120]
        purpose='Official ESP-IDF guide identifies01_factory as factory program; official flashing guide points to this exact Firmware directory and0x0; original app descriptor identifies factory_01.'
    common=cfg['limits']+['Source-bounded digital subset only. Original factory execution, browser/WebGL and physical hardware acceptance are separate; no complete compatibility claim.']
    products=[]; knowledge=[]; caps={}; models=[]; family='waveshare-'+listing; fid=family+'-factory'
    pinmap={'i2c':{'sda':cfg['i2c'][0],'scl':cfg['i2c'][1],'addresses':cfg['addresses']},'boot':0,'sdmmc':dict(zip(['clk','cmd','d0'],cfg['sd'])) if cfg['sd'] else None,'rgb':{'gpio':38,'count':cfg['rgb'],'wireOrder':'RGB'} if cfg['rgb'] else None}
    if listing.endswith('audio-board'):pinmap.update(keys={'expander':'TCA9555','linearPins':[9,10,11]},i2s={'mclk':12,'bclk':13,'lrck':14,'sdin':15,'dout':16})
    elif '8di'in listing:pinmap.update(digitalInputs=list(range(4,12)),outputExpander={'chip':'TCA9554','address':32,'pins':list(range(8))},buzzer=46,ethernet={'chip':'W5500','sclk':15,'miso':14,'mosi':13,'cs':16,'irq':12,'reset':39},serial={'tx':17,'rx':18})
    else:pinmap.update(rs485={'tx':17,'rx':18,'de':21},relay=47 if 'relay' in listing else None,twai={'tx':15,'rx':16} if listing.endswith('rs485-can') else None)
    for idx,v in enumerate(row['variants']):
        name=cfg['names'][idx];pid='waveshare-'+name.lower();board=cfg['boards'][idx];mid=board+'-mechanical';models.append(mid);plan=json.loads((ROOT/'web/platform/assets'/mid/'model-plan.json').read_text())
        controls=[{'id':'boot','name':'BOOT · GPIO0','type':'button','transport':'board','pin':0,'activeLow':True,'defaultKeyCode':'Space'}]
        if listing.endswith('audio-board'):controls += [{'id':'key-'+str(i),'name':'KEY '+str(i)+' · TCA9555 P1'+str(i),'type':'button','transport':'board','activeLow':True,'defaultKeyCode':'Digit'+str(i)}for i in range(1,4)]
        if '8di'in listing:controls += [{'id':'di'+str(i),'name':'DI'+str(i)+' · GPIO'+str(i+3),'type':'button','transport':'board','pin':i+3,'activeLow':True,'defaultKeyCode':'Digit'+str(i),'notes':'Virtual digital pad input; external optocoupler voltage transfer unmodeled.'}for i in range(1,9)]
        interfaces=[]
        if cfg['sd']:interfaces.append({'id':'micro-sd','name':'TF · native1-bit SDMMC','type':'microSD','external':True,'pins':[{'name':n,'gpio':g}for n,g in zip(['CLK','CMD','D0'],cfg['sd'])],'sourceIds':['bsp' if listing.endswith('audio-board')else'sd-bsp'],'notes':'默认64MiB FAT32；已测实际客体1bit SDMMC DMA512字节读写、错路由和拔卡；不把它等同原厂完整FAT挂载。'})
        ps={'id':pid,'name':'Waveshare '+name,'manufacturer':'Waveshare','sku':v['sku'],'chip':'esp32s3','hardwareRevision':'unspecified; Audio schematic1.1 retained'if listing.endswith('audio-board')else'unspecified','summary':'16MiB Flash / 8MiB PSRAM。'+cfg['features'],'family':{'id':family,'name':cfg['doc']},'runtime':{'board':board,'flashMb':16,'psramMb':8,'status':'partial','touch':False},'displayStatus':'none','display':None,'controls':controls,'interfaces':interfaces,'model':{'type':'manufacturer-cad','url':'./assets/'+mid+'/model.json','status':'partial','provenance':plan['method']},'knowledgeUrl':'./knowledge/'+pid+'.json','sources':[{'id':s['id'],'title':s['title'],'url':s['url'],'checkedAt':s['fetchedAt'][:10]}for s in sources if s['id']in ['storefront','overview','bsp','schematic','manufacturer-image','firmware-guide']],'limitations':common+plan['limitations'],'chronology':{'date':None,'basis':'unknown','sourceId':None,'sourceUrl':None,'confidence':'unknown','notes':'公开发布日期未核实；店面位置仅为处理次序，固件日期不作为硬件发布日期。'},'defaultFirmwareId':fid};products.append(ps)
        caps[board]={'chip':'esp32s3','flashMb':16,'psramMb':8,'display':False,'touch':False,'gpio':True,'rgb':bool(cfg['rgb']),'sdCard':bool(cfg['sd']),'boardButtons':[c['id']for c in controls]}
        ks=[{k:x for k,x in s.items()if k not in ['retained','archive']}for s in sources]+eco; mids=[]
        for src in plan['sources']:
            if src.get('origin'):continue
            sr=source('model-'+src['id'],'Mechanical source '+src['id'],src,notes='Original retained under assets/'+mid+'; see hash-bound model plan.');ks.append(sr);mids.append(sr['id'])
        facts=[fact('identity','精确SKU与选项',{'model':name,'sku':v['sku'],'options':v['options']},['storefront','overview']),fact('cpu','处理器',{'chip':'ESP32-S3R8','architecture':'Xtensa LX7','cores':2,'maxClockMHz':240},['overview']),fact('memory','存储',{'flashMiB':16,'psramMiB':8},['storefront','overview']+(['schematic']if any(s['id']=='schematic'for s in sources)else[]),notes='S3R8含8MiB封装PSRAM；外部Flash16MiB按当前SKU官方图示。定制非默认芯片不混入此profile。'),fact('display','随板屏幕',{'included':False},['overview','storefront']),fact('digital-wiring','实物引脚与器件地址',pinmap,['bsp','overview'],notes='硬件接线事实不等于全部接口已模拟。'),fact('power','供电与包装',cfg['power'],['storefront','overview'],notes='Physical input specifications; analog power, PoE and charging transfer unmodeled.'),fact('wireless','无线规格',{'wifiGHz':2.4,'bluetooth':'5 LE'},['overview'],notes='No simulated network or RF interoperability acceptance.'),fact('mechanical','机械尺寸与来源',plan['dimensions'],mids,basis='cad-measurement'if plan['method']=='official-cad'else'image-measurement',method='Native STEP retained unscaled, or source-dimension/photo envelope with explicit estimates.',uncertainty='Unknown hidden geometry, component heights and mating tolerances documented per plan; no manufacturing precision claim.')]
        unknowns=[fact('hardware-revision','发货修订与CAD匹配',None,['overview'],basis='unknown',notes='Shipped hardware revision and fit were not verified on physical hardware.'),fact('complete-runtime','完整原厂与外设兼容',None,['manufacturer-image'],basis='unknown',notes='Original bounded execution is recorded separately; native digital checks are not full firmware validation.'),fact('weight','此SKU重量',None,['overview'],basis='unknown'),fact('fit','插合与制造公差',None,mids,basis='unknown',notes='No physical insertion/assembly tolerance test.')]
        knowledge_record={'schemaVersion':'1','id':pid,'catalogId':pid,'kind':'product','title':ps['name'],'summary':ps['summary'],'summarySourceIds':['storefront','overview'],'hardwareRevision':ps['hardwareRevision'],'updatedAt':NOW,'facts':facts,'sources':ks,'revisions':[],'assets':[{'id':'model','sourceId':mids[0],'role':'source-backed-partial-model','url':ks[-len(mids)]['url'],'localUrl':'./assets/'+mid+'/model.json'}],'unknowns':unknowns,'collectionNotes':[purpose,'Exact source URLs, fetch identity, hashes and source-specific deviations preserved. No firmware bodies mirrored.',*common]}
        save(K/(pid+'.json'),knowledge_record);knowledge.append({'path':str((K/(pid+'.json')).relative_to(ROOT))})
    ids=[x['id']for x in products];wid=family+'-official-watch'
    firmware={'id':fid,'name':cfg['doc']+' · 官方原厂程序（试运行）','productIds':ids,'version':'manufacturer-archive-'+audit['source']['archive']['sha256'][:12],'status':'trial','source':audit['source'],'role':'factory','publisher':'Waveshare','publisherGroup':'官方','simulationVerified':False,'hardwareValidated':False,'storage':'remote-links-only','compatibility':'partial','publicationUrl':next(s['url']for s in sources if s['id']=='firmware-guide')if listing.endswith('audio-board')else row['documentationLinks'][0].replace('http://','https://'),'watchlistId':wid,'sources':[{'title':'Original official image','url':audit['source']['url'],'checkedAt':NOW[:10]}],'limitations':[purpose,*common]}
    firmwares=[firmware];watch=[{'id':wid,'name':firmware['name'],'category':'factory','metadataProvider':'manual','upstreamRepo':'waveshareteam/'+cfg['doc'],'metadataUrl':firmware['publicationUrl'],'releaseUrl':firmware['publicationUrl'],'version':firmware['version'],'checkedAt':NOW,'assets':[{'name':Path(audit['source']['archive']['entry']).name,'url':audit['source']['url'],'bytes':audit['containerBytes'],'sha256':audit['source']['archive']['sha256'],'format':'zip-bundle'}],'compatibility':{'status':'reference-only','productIds':ids,'simulationVerified':False,'hardwareValidated':False,'note':'Exact archive/member pinned. No automatic cross-SKU update or full firmware compatibility claim.'},'storage':'remote-links-only','catalogTargets':[]}]
    for proj,pub in [('uiflow','M5Stack'),('xiaozhi','78/xiaozhi-esp32')]:
        exact=proj=='xiaozhi'and listing.endswith('audio-board');note='Exact esp32-s3-audio-board upstream target andv2.6.0 release found; optional camera/display, PCM/cloud and original boot unverified.'if exact else'No exact SKU match in inspected fixed upstream release/source inventory; generic chip binaries are not substituted.'
        qid=family+'-'+proj+'-reference';qw=qid+'-watch';rel=release[proj]
        fw={'id':qid,'name':cfg['doc']+' · '+proj+'资料','productIds':ids,'version':rel['tag_name'],'status':'reference','source':None,'role':'project','publisher':pub,'publisherGroup':'开源项目','simulationVerified':False,'hardwareValidated':False,'storage':'remote-links-only','compatibility':'unavailable','publicationUrl':rel['html_url'],'watchlistId':qw,'sources':[{'title':'Fixed upstream release','url':rel['html_url'],'checkedAt':NOW[:10]}],'limitations':[note]}
        assets=[]
        if exact:
            xa=json.loads((sroot/'ecosystem/xiaozhi-image-audit.json').read_text());zp=sroot/'ecosystem/v2.6.0_waveshare-esp32-s3-audio-board.zip';zr=json.loads(zp.with_name(zp.name+'.source.json').read_text());assert xa['chipId']==9 and xa['productionLayoutValidation']=='passed'
            fw.update(id=family+'-xiaozhi-v2-6-0',name=cfg['doc']+' · 小智v2.6.0（试运行）',status='trial',compatibility='partial',source={'url':zr['requestedUrl'],'sha256':xa['sha256'],'bootMode':'rom','archive':{'format':'zip-bundle','entry':xa['entry'],'sha256':xa['archiveSha256']}})
            assets=[{'name':zp.name,'url':zr['requestedUrl'],'bytes':zr['bytes'],'sha256':zr['sha256'],'format':'zip-bundle'}]
            save(folder/'xiaozhi-image-audit.json',xa)
            for sid,name in [('xiaozhi-config','config.h'),('xiaozhi-target','config.json'),('xiaozhi-readme','README.md')]:retain(folder,sroot/'ecosystem'/name,sid)
        firmwares.append(fw);watch.append({'id':qw,'name':cfg['doc']+' '+proj,'category':'voice'if proj=='xiaozhi'else'development','metadataProvider':'manual','upstreamRepo':'78/xiaozhi-esp32'if proj=='xiaozhi'else'm5stack/uiflow-micropython','metadataUrl':rel['html_url'],'releaseUrl':rel['html_url'],'version':rel['tag_name'],'checkedAt':NOW,'assets':assets,'compatibility':{'status':'reference-only','productIds':ids,'simulationVerified':False,'hardwareValidated':False,'note':note},'storage':'remote-links-only','catalogTargets':[]})
    investigation={'listingId':listing,'checkedAt':NOW,'products':[{'productId':p['id'],'sku':p['sku'],'defaultFirmwareId':fid,'factoryFirmwareIds':[fid]}for p in products],'factory':{'status':'image-pinned','finding':purpose,'sourceIds':['manufacturer-image','overview']+(['firmware-guide','factory-project-guide']if listing.endswith('audio-board')else[])},'UIFlow':{'release':release['uiflow']['tag_name'],'exactMatches':[]},'Xiaozhi':{'release':release['xiaozhi']['tag_name'],'exactTarget':'esp32-s3-audio-board'if listing.endswith('audio-board')else None},'M5Burner':{'sourceId':'m5burner-devices','categoryCount':55,'exactMatches':[]},'runtime':'Original images trial; no original execution/hardware acceptance implied by native digital tests.'}
    for watched in watch:watched['monitoringNote']='Manual pinned-source review; catalogTargets remains empty until an exact release-to-SKU update contract is established.'
    save(folder/'firmware-investigation.json',investigation);save(folder/'sources.json',{'listingId':listing,'sources':sources+eco,'privacy':'Curated article text and exact hardware driver members; no full program archive, binary, sampleWiFi file or raw anonymous storefront HTML retained.'})
    receipt={'listingId':listing,'boardProfiles':cfg['boards'],'checkedAt':NOW,'command':'cargo test --release -p esp32s3 --test waveshare_industrial_controls','result':'6 passed;0 failed','scope':'Real LX7 instructions: exact route/mux/address NACK, RTC time+STOP, TCA9555 keys+codec ports, actualRMT RGB bytes, Relay1 GPIO47, distinct eight-channel control, five actual guest1bit SDMMC IDMAC512-byte roundtrip/wrong-route/eject.','clippy':'cargo clippy --release -p esp32s3 --test waveshare_industrial_controls -- -D warnings: passed','limitations':common}
    save(folder/'runtime-verification.json',receipt)
    frag={'listingId':listing,'products':products,'firmwares':firmwares,'watchlist':watch,'knowledge':knowledge,'boardCapabilities':caps,'modelIds':models,'sourceReceiptPaths':[str((folder/n).relative_to(ROOT))for n in ['sources.json','storefront-identity.json','archive-members.json','firmware-investigation.json','firmware-image-audit.json','runtime-verification.json']],'tests':[{'command':receipt['command'],'status':'passed','scope':receipt['scope'],'evidence':str((folder/'runtime-verification.json').relative_to(ROOT))},{'command':'node tools/check-waveshare-model-loading.mjs <nine model IDs>','status':'passed','scope':'ProductionThree dimensions/source-hashes/shell-material isolation/UNKNOWN reveal-reset; noWebGL claim','evidence':str((E/'loader-checks.json').relative_to(ROOT))}],'limitations':common,'integrationNotes':['Root integrates exact16F8P board registry and five family fragments; SD card slots use actual existing nativeSDMMC transport.','No shared shell or display semantics inherited from sibling devices.']}
    save(O/(listing+'.json'),frag);batch.append({'listingId':listing,'skus':[v['sku']for v in row['variants']],'productIds':ids,'modelIds':models,'fragment':str((O/(listing+'.json')).relative_to(ROOT))});print(listing,len(ids),flush=True)
save(E/'manifest.json',{'families':5,'exactSkuCount':9,'profiles':9,'models':9,'items':batch,'scope':'Source-backed partial digital controls and models; no full original firmware/hardware acceptance.'})
