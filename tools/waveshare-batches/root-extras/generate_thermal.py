#!/usr/bin/env python3
"""Exact Thermal45/90 source knowledge, catalogue and original-image receipts."""
import datetime
import gzip
import hashlib
import importlib.util
import json
import struct
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SRC = ROOT.parent/'root-sources/thermal-45-camera-esp32-module'
OUT = ROOT/'docs/platform/research/waveshare-thermal-20261124'
STAMP = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')


def sha(data): return hashlib.sha256(data).hexdigest()
def read(path): return json.loads(path.read_text())
def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n')
def utc(value): return datetime.datetime.fromisoformat(value.replace('Z','+00:00')).isoformat(timespec='seconds').replace('+00:00','Z')


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    rows, retained, originals = [], [], {}
    def retain(sid, path, receipt, store=True):
        body=path.read_bytes();assert len(body)==receipt['bytes'] and sha(body)==receipt['sha256']
        row=dict(id=sid,title=path.name,url=receipt['requestedUrl'],kind='manufacturer-source',status='fetched',
                 resolvedUrl=receipt['finalUrl'],fetchedAt=utc(receipt['fetchedAt']),bytes=len(body),sha256=sha(body),archiveName=sid+'--'+path.name)
        rec=dict(id=sid,source=receipt)
        if store:
            name=sid+'--'+path.name+'.gz';encoded=gzip.compress(body,mtime=0);(OUT/name).write_bytes(encoded)
            rec.update(retained=name,retainedBytes=len(encoded),retainedSha256=sha(encoded))
        else:rec['retention']='Original ZIP retained outside repository; remote-links-only firmware. Exact member receipts retained separately.'
        rows.append(row);retained.append(rec);originals[sid]=body

    listing='thermal-45-camera-esp32-module'
    item=read(ROOT.parent/'official_inventory/products'/f'{listing}.json')
    retain('product',ROOT.parent/'official_inventory/products'/f'{listing}.html',item['source'])
    retain('overview',SRC/'overview.html',read(SRC/'overview.html.receipt.json'))
    specs=[('desktop-factory','Thermal-ESP32-Demo.zip','Thermal-ESP32-Demo-main/firmware/senxorESP32S3_merged.bin'),
           ('web-factory','Thermal-ESP32-Web-Demo.zip','Thermal-ESP32-Web-Demo-main/firmware/factory.bin')]
    helper=importlib.util.spec_from_file_location('factory_audit',ROOT/'tools/audit-factory-firmware-sources.py')
    audit=importlib.util.module_from_spec(helper);helper.loader.exec_module(audit)
    images=[]
    for sid,name,entry in specs:
        rec=read(SRC/(name+'.receipt.json'));retain(sid,SRC/name,rec,False)
        with zipfile.ZipFile(SRC/name) as z:
            assert z.namelist().count(entry)==1;body=z.read(entry)
        assert len(body)<=16<<20
        assert audit.image(body,0)==9
        parts=[];md5=False
        for pos in range(0x8000,0x9000,32):
            magic=int.from_bytes(body[pos:pos+2],'little')
            if magic==0x50aa:
                _,kind,sub,offset,size,label,flags=struct.unpack_from('<HBBII16sI',body,pos)
                assert offset+size<=16<<20
                part=dict(type=kind,subtype=sub,offset=offset,size=size,label=label.split(b'\0')[0].decode(),flags=flags)
                if kind==0 and offset<len(body) and body[offset]==0xe9:
                    assert audit.image(body,offset)==9;part['chipId']=9
                parts.append(part)
            elif magic==0xebeb:
                assert hashlib.md5(body[0x8000:pos]).digest()==body[pos+16:pos+32];md5=True;break
            elif magic==0xffff:break
            else:raise ValueError(('partition magic',magic,pos))
        assert md5 and any(p.get('chipId')==9 for p in parts)
        image_headers=[]
        for image_offset in [0]+[p['offset'] for p in parts if p.get('chipId')==9]:
            at=image_offset+24
            for _ in range(body[image_offset+1]):
                size=struct.unpack_from('<I',body,at+4)[0];at+=8+size
            end=(at|15)+1;has_sha=body[image_offset+23]==1
            image_headers.append(dict(offset=image_offset,hashAppended=has_sha,xorValid=True,appendedSha256=body[end:end+32].hex() if has_sha else None,declaredDigestValid=True if has_sha else None))
        images.append(dict(id=sid,archive=rec,entry=entry,bytes=len(body),sha256=sha(body),chipId=9,
                           flashHeaderMiB=2**(body[3]>>4),bootloaderOffset=0,partitionTableOffset=32768,
                           partitions=parts,partitionMd5Valid=True,declaredImageChecksumsValid=True,espImages=image_headers,
                           modified=False,executed=False,purposeSourceId='overview'))

    # Existing bounded SDK extraction retains exact archive/member receipts.
    sdk=read(OUT/'source-manifest.json')
    for i,rec in enumerate(sdk['sources']):
        if 'archiveMember' not in rec:continue
        data=gzip.decompress((OUT/rec['retained']).read_bytes())
        assert sha(data)==rec['sha256'] and len(data)==rec['bytes']
        sid='sdk-'+str(i)
        rows.append(dict(id=sid,title=rec['archiveMember'],url=rec['archive']['requestedUrl'],kind='manufacturer-archive-member',status='fetched',
                         resolvedUrl=rec['archive']['finalUrl'],fetchedAt=utc(rec['archive']['fetchedAt']),bytes=rec['bytes'],sha256=rec['sha256'],
                         archiveName=rec['retained'].removesuffix('.gz'),archiveMember=rec['archiveMember'],
                         notes='Exact original member; archive URL/hash and retained compressed identity in source-manifest.json.'))
    sdk_ids=[r['id'] for r in rows if r['id'].startswith('sdk-')]
    model_ids=[f'waveshare-thermal-{angle}-camera-esp32-module' for angle in [45,90]]
    for angle,pid in zip([45,90],model_ids):
        manifest=read(ROOT/'web/platform/assets'/pid/'source-manifest.json')
        for source in manifest['sources']:
            sid=f'model-{angle}-'+source['id']
            rows.append(dict(id=sid,title=source['id'],url=source['url'],kind='manufacturer-mechanical-source',status='fetched',
                             resolvedUrl=source['finalUrl'],fetchedAt=utc(source['fetchedAt']),bytes=source['bytes'],sha256=source['sha256'],
                             archiveName=pid+'--'+source['file'],notes='Unchanged source hash/retention in the model source-manifest.json.'))
    assert len(set(r['id'] for r in rows))==len(rows)
    save(OUT/'catalog-source-receipts.json',dict(schemaVersion=1,sources=retained,sdkManifest='source-manifest.json',
         excludedRestrictedDocument=dict(url=read(SRC/'MI0802--Datasheet--v1.0.1.pdf.receipt.json')['requestedUrl'],retained=False,
                                        reason='Publicly linked PDF is marked confidential/NDA; not redistributed. No undocumented sensor implementation derived from it.')))
    save(OUT/'factory-layout-audit.json',dict(checkedAt=STAMP,images=images,scope='Unchanged original archive members, chip/partition/checksum identity only; execution receipts are separate.'))
    short=[dict(id=r['id'],title=r['title'],url=r['url'],checkedAt=r['fetchedAt'][:10]) for r in rows if r['id'] in ['product','overview','desktop-factory','web-factory']]
    limits=['ESP32-S3-WROOM-1的16MiB Flash/8MiB PSRAM；45°与90°为光学选项，共用主控接线。',
            '真实LX7 GPIO数字子集覆盖BOOT0和分立低有效RGB1/38/2；SPI2路由/片选/复位/掉电仅观察。',
            'MI0802寄存器、校准存储、DATA_AV4中断和热辐射测温未实现；不会生成虚构设备ID、温度或热图。',
            '没有板载显示屏、TF插槽或虚拟摄像头帧；80×62是传感器像素，不能用作显示分辨率。',
            '两份原厂ZIP原字节的芯片/分区/XOR及已声明附加SHA通过审计；Web版原镜像未声明附加SHA，ROM实际打印SHA比较警告后继续。桌面版头声明2MiB但物理板为16MiB，均保持原字节。',
            '原厂两个版本均实跑到app_main与GPIO/RGB/SPI初始化，随后SenXor初始化失败；完整传感器/无线/原厂应用/实物未验收。',
            '精确CAD未找到；29×29mm载板与3孔轮廓有尺寸/照片来源，镜筒、子板、USB高度及内部结构为估计/UNKNOWN。']
    products=[];knowledge=[];board='waveshare-s3-thermal-camera'
    for angle,variant,pid in zip([45,90],item['variants'],model_ids):
        assert variant['sku']==('30136' if angle==45 else '30139')
        name=f'Waveshare Thermal-{angle} Camera ESP32 Module'
        product=dict(id=pid,name=name,manufacturer='Waveshare',sku=variant['sku'],chip='esp32s3',hardwareRevision='SDK S3MINI_C; retail PCB revision unspecified',
                     summary=f'{angle}°热成像传感器选项；S3 16MiB Flash/8MiB PSRAM，无板载屏幕；数字子集部分适配。',
                     knowledgeUrl='./knowledge/'+pid+'.json',family=dict(id=listing,name='Thermal Camera ESP32 Module'),
                     runtime=dict(board=board,flashMb=16,psramMb=8,status='partial',touch=False),displayStatus='none',display=None,
                     controls=[dict(id='boot',name='BOOT',type='button',pin=0,activeLow=True,defaultKeyCode='Space')],
                     interfaces=[dict(id='usb-c',name='USB-C 编程/供电',type='USB-C',external=True,pins=[],sourceIds=['overview'])],
                     sources=short,limitations=limits,chronology=dict(date=None,basis='unknown',confidence='unknown',sourceId=None,sourceUrl=None,notes='未建立硬件发布日期；ZIP/SDK时间不等同发布日。'),
                     model=dict(type='manufacturer-cad',url='./assets/'+pid+'/model.json',status='partial',provenance='dimension-photo-reconstruction'),
                     defaultFirmwareId='waveshare-thermal-web-factory')
        products.append(product)
        model=ROOT/'web/platform/assets'/pid
        facts=[dict(id='identity',label='精确销售选项和内存',basis='official',sourceIds=['product','overview'],value=dict(sku=variant['sku'],fovDegrees=angle,chip='ESP32-S3-WROOM-1',flashMiB=16,psramMiB=8)),
               dict(id='sensor',label='传感器规格，非显示屏',basis='official',sourceIds=['overview'],value=dict(sensorPixels=[80,62],onboardDisplay=False,physicalFovDegrees=[56,45,34] if angle==45 else [122,90,67],runtimeThermalSimulation=False)),
               dict(id='electrical',label='公开原始SDK接线',basis='official',sourceIds=sdk_ids,value=dict(configuration='CONFIG_ESPMODEL_S3MINI_C',boot=0,rgb=dict(kind='discrete-active-low',red=1,green=38,blue=2),spi=dict(host=2,clock=6,miso=7,mosi=15,sensorCs=16,calibrationCs=5,reset=17,powerDown=3,dataAvailable=4),sensorResponse='unsupported; SPI returns0xff, no fabricated successful identity')),
               dict(id='original-firmware',label='官方完整镜像原字节审计',basis='official',sourceIds=['overview','desktop-factory','web-factory'],value=[dict(id=x['id'],entry=x['entry'],bytes=x['bytes'],sha256=x['sha256'],chipId=9)for x in images]),
               dict(id='mechanical',label='尺寸/照片模型及显式估计',basis='image-measurement',sourceIds=[f'model-{angle}-dimension-photo',f'model-{angle}-reverse-photo',f'model-{angle}-optics-photo'],
                    value=dict(modelSha256=sha((model/'model.json').read_bytes()),carrierMm=[29,29],observedHoles=3,sourceImagesUnchanged=True,limits=read(model/'source-manifest.json')['limitations']),
                    method='Use the original29mm drawing registration and observed L-shaped carrier, separate module and sensor parts; rebuild with prepare_thermal_models.py and production loader.',
                    uncertainty='Published XY envelope is retained; hole diameter, carrier thickness, daughterboard, barrel, aperture and all Z dimensions are explicitly estimated. No fit, optical or radiometric calibration.')]
        record=dict(schemaVersion='1',id=pid,kind='product',catalogId=pid,title=name,summary=product['summary'],summarySourceIds=['product','overview'],updatedAt=STAMP,
                    hardwareRevision=product['hardwareRevision'],facts=facts,revisions=[],sources=rows,
                    assets=[dict(id=r['id'],sourceId=r['id'],role='source-document',url=r['url'])for r in rows],
                    unknowns=[dict(id=key,label=label,basis='unknown',value=None,sourceIds=['overview'])for key,label in [('release-date','硬件发布日期'),('full-factory','完整原厂应用验收'),('physical','真实硬件/测温标定'),('third-party','精确UIFlow或小智恢复包')]],collectionNotes=limits)
        path=ROOT/'web/platform/knowledge'/f'{pid}.json';save(path,record);knowledge.append(dict(path=str(path.relative_to(ROOT))))
    firmwares=[];watch=[]
    for result in images:
        sid=result['id'];rec=result['archive'];fwid='waveshare-thermal-'+sid;entry=result['entry']
        firmwares.append(dict(id=fwid,name='Thermal Camera · 官方'+('Web' if sid.startswith('web') else '桌面预览')+'恢复固件（trial）',productIds=model_ids,version=rec['sha256'][:12],status='trial',role='factory',publisher='Waveshare',publisherGroup='官方',simulationVerified=False,hardwareValidated=False,storage='remote-links-only',publicationUrl=rows[1]['url'],sources=short,limitations=limits,
                              source=dict(url=rec['requestedUrl'],sha256=result['sha256'],bootMode='rom',archive=dict(format='zip-bundle',entry=entry,sha256=rec['sha256']))))
        watch.append(dict(id=fwid+'-watch',name='Thermal Camera 官方'+sid+'人工复核',category='factory',metadataProvider='manual',upstreamRepo='manufacturer:Waveshare/Thermal-Camera-ESP32-Module',metadataUrl=rows[1]['url'],releaseUrl=rec['requestedUrl'],version=rec['sha256'][:12],checkedAt=STAMP,
                          assets=[dict(name=Path(entry).name,url=rec['requestedUrl'],bytes=rec['bytes'],sha256=rec['sha256'],format='manufacturer-zip-candidate',archiveMember=entry,memberBytes=result['bytes'],memberSha256=result['sha256'])],
                          compatibility=dict(status='reference-only',productIds=model_ids,simulationVerified=False,hardwareValidated=False,note='Official static ZIP image; keep trial until complete original execution is demonstrated. Future changes require independent exact-member review.'),
                          catalogTargets=[],storage='remote-links-only',monitoringNote='Official static manufacturer ZIP; no release API established. Review archive/member SHA, original firmware purpose and exact hardware manually.'))
    fragment=dict(listingId=listing,products=products,firmwares=firmwares,watchlist=watch,knowledge=knowledge,
                  boardCapabilities={board:dict(chip='esp32s3',flashMb=16,psramMb=8,sdCard=False,boardButtons=[])},
                  sourceReceiptPaths=[str((OUT/name).relative_to(ROOT))for name in ['catalog-source-receipts.json','source-manifest.json','factory-layout-audit.json']],
                  tests=['cargo test --release -p esp32s3 --lib waveshare_thermal'],limitations=limits)
    save(Path(__file__).with_name(listing+'.json'),fragment)
    print(json.dumps(dict(listing=listing,products=len(products),sources=len(rows),originalImages=len(images))))


if __name__=='__main__':main()
