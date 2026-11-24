#!/usr/bin/env python3
"""Exact5/4.3B/4V4 profiles with retained primary evidence and honest trial roles."""
import datetime,gzip,hashlib,importlib.util,json,re,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];CACHE=ROOT.parent/'root-s3-five-sources';OUT=ROOT/'docs/platform/research/waveshare-s3-large-20261124'
STAMP=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def read(p):return json.loads(p.read_text())
def sha(b):return hashlib.sha256(b).hexdigest()
def utc(v):return datetime.datetime.fromisoformat(v.replace('Z','+00:00')).isoformat(timespec='seconds').replace('+00:00','Z')
def save(p,d):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
def source_record(sid,path,receipt,folder,store=True):
 body=path.read_bytes();assert len(body)==receipt['bytes']and sha(body)==receipt['sha256'];parent=receipt.get('archive',receipt.get('sourceArchive',receipt));url=parent.get('requestedUrl',parent.get('url'));r=dict(id=sid,title=path.name,url=url,resolvedUrl=parent['finalUrl'],status='fetched',kind='manufacturer-source',fetchedAt=utc(parent['fetchedAt']),bytes=len(body),sha256=sha(body),archiveName=sid+'--source'+path.suffix)
 rec=dict(id=sid,receipt=receipt)
 if store:
  n=re.sub('[^A-Za-z0-9._-]','_',sid+'--'+path.name)+'.gz';data=gzip.compress(body,mtime=0);(folder/n).write_bytes(data);r['archiveName']=n;rec.update(retained=n,retainedBytes=len(data),retainedSha256=sha(data))
 else:rec['retention']='Remote original retained only in audit cache; catalog firmware and raw repository/storefront metadata are not mirrored.'
 if 'archiveMember'in receipt:r['archiveMember']=receipt['archiveMember']
 return r,rec

def main():
 spec=importlib.util.spec_from_file_location('audit',ROOT/'tools/audit-factory-firmware-sources.py');audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
 configs=[('ESP32-S3-Touch-LCD-5','esp32-s3-touch-lcd-5',[
 ('28117','waveshare-esp32-s3-touch-lcd-5','waveshare-s3-touch-lcd5',800,480,True,'20260225'),('28151','waveshare-esp32-s3-touch-lcd-5b','waveshare-s3-touch-lcd5-1024',1024,600,True,'20260225'),('30321','waveshare-esp32-s3-lcd-5','waveshare-s3-lcd5',800,480,False,'20260224'),('30320','waveshare-esp32-s3-lcd-5b','waveshare-s3-lcd5-1024',1024,600,False,'20260224')]),
 ('ESP32-S3-Touch-LCD-4.3B','esp32-s3-touch-lcd-4.3b',[
 ('27848','waveshare-esp32-s3-touch-lcd-4-3b','waveshare-s3-touch-lcd43b',800,480,True,'20260225'),('28141','waveshare-esp32-s3-touch-lcd-4-3b-box','waveshare-s3-touch-lcd43b',800,480,True,'20260225')]),
 ('ESP32-S3-Touch-LCD-4','esp32-s3-touch-lcd-4',[
 ('28154','waveshare-esp32-s3-touch-lcd-4','waveshare-s3-touch-lcd4-v4',480,480,True,'20251122'),('30871','waveshare-esp32-s3-lcd-4','waveshare-s3-lcd4-v4',480,480,False,'20251218')])]
 for family,listing,variants in configs:
  is4=family.endswith('-4');is43='4.3B'in family;folder=OUT/listing;folder.mkdir(parents=True,exist_ok=True);cache=CACHE/family;rows=[];retained=[];inputs=[]
  def add(sid,path,store=True,receipt=None):
   r,rec=source_record(sid,path,receipt or read(Path(str(path)+'.receipt.json')),folder,store);rows.append(r);retained.append(rec);return r
  inventory=read(ROOT.parent/'official_inventory/products'/(listing+'.json'));assert set(v['sku']for v in inventory['variants'])==set(v[0]for v in variants)
  add('product',ROOT.parent/'official_inventory/products'/(listing+'.html'),False,inventory['source'])
  documents=['docs-cn.html','resources-cn.html','README.md','firmware__README.md','firmware__manifest.json','hardware__HARDWARE_REFERENCE.md','hardware__schematics__ESP32-S3-Touch-LCD-4 V4.0.pdf','examples__arduino__libraries__WS_CH32_IO__src__WS_CH32_IO.h','examples__arduino__libraries__WS_CH32_IO__src__WS_CH32_IO.cpp','examples__esp-idf__02_SD_Test__main__sd_card_example_main.c'] if is4 else ['overview.html','resources.html','firmware-flashing.html','faq.html',family+'-Sch.pdf','upstream-head.json','upstream-tree.json','upstream-README.md','upstream-firmware__README.md','upstream-firmware__manifest.json']
  for i,name in enumerate(documents):
   path=cache/name;rp=Path(str(path)+'.receipt.json')
   if path.exists()and rp.exists():
    receipt=read(rp)
    if receipt.get('status')=='fetched' or receipt.get('httpStatus')==200:add('doc-'+str(i),path,name!='upstream-head.json')
  schematic='doc-6'if is4 else'doc-4';overview='doc-0';fwdoc='doc-3'if is4 else'doc-8';short=[dict(id=r['id'],title=r['title'],url=r['url'],checkedAt=r['fetchedAt'][:10])for r in rows if r['id']in['product',overview,schematic,fwdoc]]
  if not is4:
   manifest=read(cache/'selected-source-manifest.json')
   for i,entry in enumerate(manifest):
    path=cache/entry['retained']
    if path.suffix.lower()not in['.bin','.zip']:add('demo-'+str(i),path,True,entry)
  # ST7262 is explicitly listed by the manufacturer's pinned board map.
  if not is4:
   p=CACHE/'ESP32-S3-Touch-LCD-5/upstream-panel-map.md';add('panel-map',p)
  products=[];firmwares=[];watch=[];knowledge=[];caps={};firmware_rows={}
  for sku,pid,board,w,h,touch,date in variants:
   model_id=pid[:-4]if pid.endswith('-box')else pid;model_dir=ROOT/'web/platform/assets'/model_id;plan=read(model_dir/'model-plan.json');model=read(model_dir/'model.json');local_rows=list(rows)
   for s in plan['sources']:local_rows.append(dict(id='mech-'+s['id'],title=s['file'],url=s['url'],resolvedUrl=s['resolvedUrl'],fetchedAt=utc(s['fetchedAt']),bytes=s['bytes'],sha256=s['sha256'],status='fetched',kind='manufacturer-mechanical-source',archiveName='mech-'+s['id']+'--'+s['file'],notes='Original bytes, ZIP member relationships and retained encoding in model source-manifest.json.'))
   stem=('ESP32-S3-Touch-LCD-4'if touch else'ESP32-S3-LCD-4')if is4 else ('ESP32-S3-Touch-LCD-4.3B'if is43 else 'ESP32-S3-'+('Touch-'if touch else'')+'LCD-5'+('B'if w==1024 else''))
   filename=stem+'_FactoryOnly_'+date+'.bin';binary=cache/(('firmware__'if is4 else'upstream-firmware__')+filename);br=read(Path(str(binary)+'.receipt.json'));b=binary.read_bytes();assert sha(b)==br['sha256'] and len(b)==br['bytes'];assert audit.image(b,0)==9
   parts=[]
   for at in range(0x8000,0x9000,32):
    if b[at:at+2]==b'\xaaP':
     _,kind,subtype,offset,size,label,flags=struct.unpack_from('<HBBII16sI',b,at);assert offset+size<=16<<20
     if kind==0 and offset<len(b)and b[offset]==0xe9:assert audit.image(b,offset)==9
     parts.append(dict(type=kind,offset=offset,size=size,label=label.split(b'\0')[0].decode()))
   assert parts,'Merged image needs its original partition table'
   fwid=model_id+('-factory'if is4 else'-integrated-lvgl');role='factory'if is4 else'example'
   source_id='binary-'+sku;r,rec=source_record(source_id,binary,br,folder,False);local_rows.append(r);retained.append(rec);inputs.append(dict(productId=pid,file=filename,bytes=len(b),sha256=sha(b),chipId=9,offset=0,partitions=parts,role=role,roleBasis='Pinned exact README explicitly identifies factory recovery.'if is4 else'Pinned README describes an integrated LVGL demo. Filename contains FactoryOnly but preinstalled factory purpose is not explicitly established; preserve example role.'))
   limits=['数字协议、真实固件输出、模型来源与实机验收分别记录；全部镜像保持trial，未完成原厂应用全流程或浏览器WebGL验收。','CAN/TWAI、RS485对端、USB、无线、模拟供电/充电、隔离输入输出电压及蜂鸣器声音未实现。','带TF卡槽默认准备64MiB FAT32；客体挂载依赖本板真实总线实现，不把镜像准备当作读写证明。']+plan['limitations']
   if is4:limits += ['两份原厂V4镜像各运行5秒并返回app_main：触控236个RGB帧、非触控239帧且实际挂载/卸载FAT32（28条SD命令、2次超时、读取1616字节）；两者均未写CH32 PWM，背光初态未知，保留黑屏，未证实可见原厂界面。证据：docs/evidence/waveshare-touch4-v4-runtime-2026-11-24。','V4使用CH32V003而非旧TCA9554；EX2为GT911中断，GPIO16属于SYS_OUT电源控制网，未暴露普通GPIO电源键。旧原理图另存，未混用版本。','两个官方FactoryOnly恢复镜像的触控/非触控身份由固定README区分；完整应用兼容性仍未验证。']
   else:limits += ['CH422G采用多I2C命令地址和单数据字节；EX1触控复位、EX2背光、EX3经已装R20=100Ω复位LCD、EX4选择SPI TF。RTC是PCF85063A@0x51。','官方固定版本文件名含FactoryOnly，但说明将其称为LVGL示例；按example分类，不据文件名声称预装出厂用途。','800×480/1024×600触控原始镜像5秒均输出2053色LVGL画面；非触控两镜像仍黑屏。完整应用和TF文件系统没有因此全部通过。']
   if pid.endswith('-box'):limits.append('带壳SKU28141当前显示明确标注的裸板CAD参考，外壳与实体按键帽未建模，不能称带壳整机完整还原。')
   product=dict(id=pid,name='Waveshare '+stem+(' · BOX（裸板参考）'if pid.endswith('-box')else'')+(' · V4'if is4 else''),manufacturer='Waveshare',sku=sku,chip='esp32s3',hardwareRevision='V4.0'if is4 else'official unversioned schematic',summary=f'{w}×{h} '+('触控'if touch else'非触控')+'RGB屏，16MiB Flash / 8MiB PSRAM；按精确SKU提供数字子集与有来源的部分机械模型。',knowledgeUrl='./knowledge/'+pid+'.json',family=dict(id=listing.replace('.','-'),name=family),runtime=dict(board=board,flashMb=16,psramMb=8,status='partial',touch=touch),displayStatus='known',display=dict(width=w,height=h,type='IPS LCD',bus='RGB',diagonalInches=4 if is4 else 4.3 if is43 else 5,controller='ST7701S'if is4 else'ST7262',touch=touch,shape='rectangle'),controls=[dict(id='boot',name='BOOT',type='button',pin=0,activeLow=True,defaultKeyCode='Space')],interfaces=[dict(id='usb-c',name='USB-C供电/下载',type='USB-C',external=True,pins=[],sourceIds=[schematic]),dict(id='tf',name='microSD / TF',type='microSD',external=True,pins=[],sourceIds=[schematic]),dict(id='rs485',name='RS485端子（电气对端未模拟）',type='RS485',external=True,pins=[],sourceIds=[schematic]),dict(id='can',name='CAN端子（收发器未模拟）',type='CAN',external=True,pins=[],sourceIds=[schematic])],model=dict(type='manufacturer-cad',url='./assets/'+model_id+'/model.json',status='partial',provenance='official-cad'),sources=short,limitations=limits,defaultFirmwareId=fwid,chronology=dict(date=None,basis='unknown',confidence='unknown',sourceId=None,sourceUrl=None,notes='未从资料日期、SKU序号或抓取时间推算硬件上市日。'))
   if pid.endswith('-box'):product.update(visualVariantOf=model_id,firmwareTargetId=model_id)
   products.append(product);caps[board]=dict(chip='esp32s3',flashMb=16,psramMb=8,sdCard=True,boardButtons=[])
   electrical=dict(i2c=dict(sda=15 if is4 else 8,scl=7 if is4 else 9,rtcAddress=81),touch=dict(controller='GT911'if touch else None,addressAfterLowStrap=93 if touch else None,reset='CH32 EX1'if is4 else'CH422 EX1',irq='CH32 EX2'if is4 else'GPIO4'),lcd=dict(pixels=[w,h],bus='RGB565',clock=41 if is4 else 7,enable=40 if is4 else 5,hsync=38 if is4 else 46,vsync=39 if is4 else 3,dataLsbFirst=[5,45,48,47,21,14,13,12,11,10,9,46,3,8,18,17]if is4 else[14,38,18,17,10,39,0,45,48,47,21,1,2,42,41,40]),sd=dict(bus='1-bit SDMMC'if is4 else'SPI',clock=2 if is4 else 12,commandMosi=1 if is4 else 11,data0Miso=4 if is4 else 13,select='CH32 EX4'if is4 else'CH422 EX4'))
   facts=[dict(id='identity',label='SKU、版本与真实内存',basis='official',sourceIds=['product',schematic],value=dict(sku=sku,revision=product['hardwareRevision'],flashMiB=16,psramMiB=8,touch=touch)),dict(id='electrical',label='精确板型线路',basis='official',sourceIds=[schematic],value=electrical),dict(id='firmware',label='原始完整镜像与用途证据',basis='official',sourceIds=[source_id,fwdoc],value=dict(filename=filename,sha256=sha(b),bytes=len(b),role=role,chipId=9,offset=0)),dict(id='geometry',label='原CAD来源和有限派生',basis='cad-measurement',sourceIds=['mech-official-cad','mech-mechanical-drawing'],value=dict(nativeComponents=plan['sourceTopologyCounts']['source.stp.gz'],included=plan['sourceCoverage']['source.stp.gz'],modelSha256=sha((model_dir/'model.json').read_bytes()),screen=model['presentation']['screen']),method='Native millimetres retained without scaling; component omissions and physical glass alternatives explicit in model plan.',uncertainty='Electrical revision correspondence, optical calibration, hidden BOM and mechanical fit remain unverified.')]
   record=dict(schemaVersion='1',id=pid,kind='product',catalogId=pid,title=product['name'],summary=product['summary'],summarySourceIds=['product',overview],updatedAt=STAMP,hardwareRevision=product['hardwareRevision'],facts=facts,revisions=[],sources=local_rows,assets=[dict(id=s['id'],sourceId=s['id'],role='source-document',url=s['url'])for s in local_rows],unknowns=[dict(id=k,label=v,basis='unknown',value=None,sourceIds=['product'])for k,v in [('release-date','硬件上市日期'),('uiflow','精确型号官方UIFlow映射'),('xiaozhi','精确型号官方小智镜像；不能套用4B或4.3C'),('physical-validation','实机验收'),('factory-purpose','5/4.3B预装出厂用途'if not is4 else'完整出厂程序兼容性')]],collectionNotes=limits)
   kp=ROOT/'web/platform/knowledge'/(pid+'.json');save(kp,record);knowledge.append(dict(path=str(kp.relative_to(ROOT))))
   if fwid in firmware_rows:firmware_rows[fwid]['productIds'].append(pid)
   else:
    fw=dict(id=fwid,name=stem+(' 官方出厂恢复（trial）'if is4 else' 官方整合LVGL示例（trial）'),productIds=[pid],version=date,status='trial',role=role,publisher='Waveshare',publisherGroup='官方',simulationVerified=False,hardwareValidated=False,storage='remote-links-only',publicationUrl=next(x['url']for x in rows if x['id']==fwdoc),sources=short,limitations=limits,source=dict(url=br['requestedUrl'],sha256=sha(b),bootMode='rom'));firmware_rows[fwid]=fw
  firmwares=list(firmware_rows.values())
  for fw in firmwares:
   watch.append(dict(id=fw['id']+'-watch',name=fw['name']+' · 人工复核',category='factory'if is4 else'development',metadataProvider='manual',upstreamRepo='waveshareteam/'+family,metadataUrl=fw['publicationUrl'],releaseUrl=fw['source']['url'],version=fw['version'],checkedAt=STAMP,assets=[dict(name=fw['source']['url'].split('/')[-1],url=fw['source']['url'],sha256=fw['source']['sha256'],format='bin')],compatibility=dict(status='reference-only',productIds=fw['productIds'],simulationVerified=False,hardwareValidated=False,note='Exact pinned source/model mapping only. Requalify any changed firmware and preserve trial.'),catalogTargets=[],storage='remote-links-only',monitoringNote='Manual check of exact SKU, source branch, original member hash and factory/example purpose; never infer target mapping from file spelling alone.'))
  save(folder/'source-manifest.json',dict(schemaVersion=1,sources=retained));save(folder/'original-image-audit.json',inputs)
  save(Path(__file__).with_name(listing+'.json'),dict(listingId=listing,products=products,firmwares=firmwares,watchlist=watch,knowledge=knowledge,boardCapabilities=caps,sourceReceiptPaths=[str((folder/n).relative_to(ROOT))for n in ['source-manifest.json','original-image-audit.json']],limitations=['All product/firmware/mechanical acceptance remains explicitly partial.']))
  print(listing,len(products),'exact SKU profiles',flush=True)
if __name__=='__main__':main()
