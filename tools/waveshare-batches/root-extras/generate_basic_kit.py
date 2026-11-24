#!/usr/bin/env python3
"""Bind exact controller-containing kit SKUs to their documented Zero boards.

External loose accessories are not an onboard display or a prewired scene.
The accessory-only SKU is excluded from the ESP32 controller census.
"""
import copy
import datetime
import gzip
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SRC=ROOT.parent/'root-sources/esp32-xx-basic-kit'
OUT=ROOT/'docs/platform/research/waveshare-basic-kit-20261124'
STAMP=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def read(p):return json.loads(p.read_text())
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,d):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
def utc(t):return datetime.datetime.fromisoformat(t.replace('Z','+00:00')).isoformat(timespec='seconds').replace('+00:00','Z')


def main():
    OUT.mkdir(parents=True,exist_ok=True)
    listing='esp32-xx-basic-kit';item=read(ROOT.parent/'official_inventory/products'/f'{listing}.json')
    rows=[];receipts=[]
    def retain(sid,p,r):
        b=p.read_bytes();assert len(b)==r['bytes'] and sha(b)==r['sha256'],p
        filename=sid+'--'+p.name+'.gz';encoded=gzip.compress(b,mtime=0);(OUT/filename).write_bytes(encoded)
        rows.append(dict(id=sid,title=p.name,url=r.get('requestedUrl',r.get('url')),kind='manufacturer-source',status='fetched',resolvedUrl=r.get('finalUrl',r.get('resolvedUrl')),fetchedAt=utc(r['fetchedAt']),bytes=len(b),sha256=sha(b),archiveName=sid+'--'+p.name))
        receipts.append(dict(id=sid,source=r,retained=filename,retainedBytes=len(encoded),retainedSha256=sha(encoded)))
    retain('kit-product',ROOT.parent/'official_inventory/products'/f'{listing}.html',item['source'])
    # The complete tree and audit already pin the commit. Do not redistribute
    # the redundant commit API response's author identities and email fields.
    for name,sid in [('overview.html','kit-wiki'),('s3-overview.html','kit-s3-docs'),('c6-overview.html','kit-c6-docs'),('c3-overview.html','kit-c3-docs'),('s3-tree.json','kit-s3-tree')]:
        retain(sid,SRC/name,read(SRC/(name+'.receipt.json')))
    audits={'s3':read(SRC/'s3-audit.json')}
    for chip in ['c6','c3']:
        folder=ROOT.parent/'official_inventory/c6-sources'/f'ESP32-{chip.upper()}-Zero-Basic-Kit'
        retain('kit-'+chip+'-tree',folder/'tree.json',read(folder/'tree.json.receipt.json'))
        audits[chip]=read(folder/'audit.json')
    save(OUT/'repository-audits.json',audits)
    # A fetch failure stays a failure: no fabricated byte/hash receipt for the
    # optional current store introduction images.
    for f in sorted(SRC.glob('controller-*.failed.json')):
        rec=read(f);sid='kit-image-'+f.name.split('.')[0].removeprefix('controller-')
        rows.append(dict(id=sid,title=f.name,url=rec['requestedUrl'],kind='manufacturer-photo',status='failed',attemptedAt=rec['attemptedAt'],error=rec['error']))
        receipts.append(dict(id=sid,attempt=rec))
    save(OUT/'source-manifest.json',dict(schemaVersion=1,sources=receipts,sourceScope='Exact sale page, its linked kit selector, three current variant documentation pages and pinned complete official tutorial repository trees.'))
    catalogue={x['id']:x for x in read(ROOT/'web/platform/catalog/products.json')['items']}
    specifications=[('s3','33356','waveshare-esp32-s3-zero','waveshare-esp32-s3-zero-basic-kit'),
                    ('c6','33357','waveshare-esp32-c6-zero','waveshare-esp32-c6-zero-basic-kit'),
                    ('c6','33357','waveshare-esp32-c6-zero-fh4','waveshare-esp32-c6-zero-basic-kit-fh4'),
                    ('c3','33358','waveshare-esp32-c3-zero','waveshare-esp32-c3-zero-basic-kit')]
    package=['1.5inch SSD1327 OLED module','PIR motion sensor','active buzzer','8-bit WS2812 RGB module','10k potentiometer','3 pushbuttons','3 discrete LEDs','10x330ohm resistors','830-point breadboard','USB-A/USB-C cable','jumper wires','40-pin header','storage box']
    limits=['本条目适配套件内精确Zero主控板；1.5英寸SSD1327 OLED、PIR、蜂鸣器、8位RGB及面包板是松散外设，未自动接线或冒充板载功能。',
            'SKU33355只有配件，不含ESP32主控，从本次主控设备统计中排除。33356/33357/33358三个含板SKU各计一次。',
            'C6当前套件链接说明8MiB FH8，但旧原理图/教材对应4MiB FH4；同一套件SKU提供两个明确容量profile，交付批次/销售版次未经证实。',
            '模型仅表示官方明确随附的主控板，复用同款原厂CAD或照片模型；未虚构三套新PCB、预焊排针、箱内布局或整套配件3D结构。',
            'S3/C6固定教程仓库没有完整BIN；C3仓库中的候选BIN实际为S3芯片，已拒绝用于C3。没有把教程目录或自编诊断标成出厂恢复固件。',
            '真实指令的板级数字子集不等于整套教程、无线、MicroPython全功能、浏览器WebGL或实物验收。']
    products=[];knowledge=[];caps={}
    for chip,sku,base_id,pid in specifications:
        assert any(v['sku']==sku for v in item['variants'])
        base=copy.deepcopy(catalogue[base_id]);record=read(ROOT/'web/platform/knowledge'/f'{base_id}.json')
        name=f'Waveshare ESP32-{chip.upper()}-Zero-Basic-Kit'+(' · FH4资料版本'if pid.endswith('-fh4')else '')
        product=copy.deepcopy(base)
        for key in ['defaultFirmwareId','revisionGroup','revisionName','variantGroup','variantName']:
            product.pop(key,None)
        product.update(id=pid,name=name,sku=sku,knowledgeUrl='./knowledge/'+pid+'.json',family=dict(id=listing,name='ESP32-XX Basic Kit'),
                       summary=f'套件含ESP32-{chip.upper()}-Zero主控；{base["runtime"]["flashMb"]}MiB Flash/{base["runtime"]["psramMb"]}MiB PSRAM。外接OLED及其他松散外设未预接线。',
                       limitations=limits+base['limitations'],chronology=dict(date=None,basis='unknown',confidence='unknown',sourceId=None,sourceUrl=None,notes='官方SKU/文档/仓库日期不构成硬件发布时间。'))
        product['sources']=[dict(id=r['id'],title=r['title'],url=r['url'],checkedAt=r['fetchedAt'][:10])for r in rows if r['status']=='fetched']+[{**s,'id':'controller-'+s['id']}for s in base['sources']]
        for interface in product.get('interfaces',[]):interface['sourceIds']=['controller-'+x for x in interface.get('sourceIds',[])]
        products.append(product);runtime=product['runtime'];caps[runtime['board']]=dict(chip=product['chip'],flashMb=runtime['flashMb'],psramMb=runtime['psramMb'],sdCard=False,boardButtons=[])
        sources=copy.deepcopy(rows)
        for s in copy.deepcopy(record['sources']):
            s['id']='controller-'+s['id']
            if s.get('discoveredFrom'):s['discoveredFrom']='controller-'+s['discoveredFrom']
            sources.append(s)
        facts=[dict(id='kit-sku',label='精确含主控套件SKU',basis='official',sourceIds=['kit-product','kit-wiki','kit-'+chip+'-docs'],value=dict(sku=sku,controllerProductId=base_id,controllerName=f'ESP32-{chip.upper()}-Zero',runtime=runtime,controllerOnlyModel=True)),
               dict(id='loose-accessories',label='独立外设和配件，非板载显示',basis='official',sourceIds=['kit-product'],value=dict(items=package,prewired=False,onboardDisplay=False,accessoryOnlySkuExcluded='33355')),
               dict(id='tutorial-source',label='完整固定官方教程目录',basis='official',sourceIds=['kit-'+chip+'-tree'],value=dict(repository=f'waveshareteam/ESP32-{chip.upper()}-Zero-Basic-Kit',commit=audits[chip]['commit'],binaryCandidates=audits[chip]['binaryCandidates'],factoryFirmwareEstablished=False))]
        for fact in copy.deepcopy(record['facts']):
            fact['id']='controller-'+fact['id'];fact['label']='随附主控板 · '+fact.get('label',fact['id']);fact['sourceIds']=['controller-'+x for x in fact['sourceIds']];facts.append(fact)
        knowledge_record=dict(schemaVersion='1',id=pid,kind='product',catalogId=pid,title=name,summary=product['summary'],summarySourceIds=['kit-product','kit-'+chip+'-docs'],updatedAt=STAMP,
                              hardwareRevision=base['hardwareRevision'],facts=facts,revisions=[],sources=sources,
                              assets=[dict(id=s['id'],sourceId=s['id'],role='source-document',url=s['url'])for s in sources],
                              unknowns=[dict(id=key,label=label,basis='unknown',value=None,sourceIds=['kit-product'])for key,label in [('release-date','硬件/套件发布日期'),('shipping-revision','当前套件实际附板版次'),('full-kit','全套教程和外设接线验收'),('physical','实物验收')]],collectionNotes=product['limitations'])
        path=ROOT/'web/platform/knowledge'/f'{pid}.json';save(path,knowledge_record);knowledge.append(dict(path=str(path.relative_to(ROOT))))
    watch=[]
    for chip in ['s3','c6','c3']:
        repo=f'waveshareteam/ESP32-{chip.upper()}-Zero-Basic-Kit';commit=audits[chip]['commit'];url=f'https://github.com/{repo}/tree/{commit}'
        watch.append(dict(id='waveshare-'+chip+'-zero-basic-kit-tutorial-watch',name=f'ESP32-{chip.upper()} Basic Kit 官方教程人工复核',category='development',metadataProvider='manual',upstreamRepo=repo,metadataUrl=url,releaseUrl=url,version=commit,checkedAt=STAMP,assets=[],catalogTargets=[],storage='remote-links-only',
                          compatibility=dict(status='reference-only',productIds=[p['id']for p in products if p['chip']=='esp32'+chip],simulationVerified=False,hardwareValidated=False,note='Pinned official tutorial tree; no matching factory image identified. Full kit wiring and tutorial behavior are not inferred from controller support.'),
                          monitoringNote='Official tutorial source tree, not a release endpoint. Review exact controller, chip identity, image purpose and original bytes manually before adding runnable firmware.'))
    fragment=dict(listingId=listing,products=products,firmwares=[],watchlist=watch,knowledge=knowledge,boardCapabilities=caps,
                  sourceReceiptPaths=[str((OUT/n).relative_to(ROOT))for n in ['source-manifest.json','repository-audits.json']],tests=['node --test tools/platform-waveshare-registry-test.mjs tools/platform-waveshare-c6-zero-test.mjs'],limitations=limits)
    save(Path(__file__).with_name(listing+'.json'),fragment)
    print(json.dumps(dict(listing=listing,skus=3,profiles=len(products),sourceReceipts=len(rows),excludedAccessoryOnlySku='33355')))


if __name__=='__main__':main()
