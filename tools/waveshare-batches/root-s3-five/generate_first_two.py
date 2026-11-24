#!/usr/bin/env python3
"""Exact LCD169 revisions and independently sourced flat/curved Touch21 models."""
import datetime
import gzip
import hashlib
import importlib.util
import json
import re
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CACHE = ROOT.parent / 'root-s3-five-sources'
OUT = ROOT / 'docs/platform/research/waveshare-lcd169-touch21-20261124'
STAMP = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')


def read(path): return json.loads(path.read_text())
def sha(body): return hashlib.sha256(body).hexdigest()
def utc(value): return datetime.datetime.fromisoformat(value.replace('Z', '+00:00')).astimezone(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
def save(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    rows, retained = [], []

    def retain(sid, path, receipt, store=True, member=None):
        body = path.read_bytes()
        assert len(body) == receipt['bytes'] and sha(body) == receipt['sha256']
        source = receipt.get('archive', receipt)
        row = dict(id=sid, title=member or path.name, url=source.get('requestedUrl', source.get('url')),
                   resolvedUrl=source['finalUrl'], fetchedAt=utc(source['fetchedAt']), bytes=len(body), sha256=sha(body),
                   status='fetched', kind='manufacturer-archive-member' if member else 'manufacturer-source',
                   archiveName=sid + '--source' + path.suffix)
        rec = dict(id=sid, source=receipt)
        if member: row['archiveMember'] = member
        if store:
            name = re.sub(r'[^a-zA-Z0-9._-]', '_', sid + '--' + path.name) + '.gz'
            encoded = gzip.compress(body, mtime=0)
            (OUT / name).write_bytes(encoded)
            row['archiveName'] = name
            rec.update(retained=name, retainedBytes=len(encoded), retainedSha256=sha(encoded))
        else:
            rec['retention'] = 'Firmware/archive original retained for local audit only; catalogue uses remote links.'
        rows.append(row); retained.append(rec)

    listing = 'esp32-s3-lcd-1.69'
    family = CACHE / 'ESP32-S3-LCD-1.69'
    inventory = read(ROOT.parent / 'official_inventory/products' / (listing + '.json'))
    assert inventory['singleSku'] == '27932'
    retain('product', ROOT.parent / 'official_inventory/products' / (listing + '.html'), inventory['source'], False)
    for sid, name in [('overview', 'overview.html'), ('resources', 'resources.html'),
                      ('schematic-v1', 'ESP32-S3-LCD-1.69(2).pdf'), ('schematic-v2', 'ESP32-S3-LCD-1.69_V2.pdf')]:
        retain(sid, family / name, read(family / (name + '.receipt.json')))
    for sid, name in [('factory-flashing', 'firmware-flashing.html'), ('faq', 'faq.html')]:
        p = family / name
        if p.exists() and read(Path(str(p) + '.receipt.json')).get('status') == 'fetched':
            retain(sid, p, read(Path(str(p) + '.receipt.json')))
    archive_name = 'ESP32-S3-LCD-1.69_DemoCode.zip'
    archive = read(family / (archive_name + '.receipt.json'))
    retain('demo-archive', family / archive_name, archive, False)
    selected = read(family / 'selected-source-manifest.json')
    firmware = next(x for x in selected if x['archiveMember'].endswith('_factoryOnly.bin'))
    for i, entry in enumerate(selected):
        if entry is firmware: continue
        retain('demo-source-' + str(i), family / entry['retained'], entry, True, entry['archiveMember'])
    body = (family / firmware['retained']).read_bytes()
    assert sha(body) == firmware['sha256'] and len(body) == firmware['bytes']
    helper = importlib.util.spec_from_file_location('original_image_audit', ROOT / 'tools/audit-factory-firmware-sources.py')
    audit = importlib.util.module_from_spec(helper); helper.loader.exec_module(audit)
    assert audit.image(body, 0) == 9
    partitions = []; md5 = False
    for at in range(0x8000, 0x9000, 32):
        magic = int.from_bytes(body[at:at + 2], 'little')
        if magic == 0x50aa:
            _, kind, subtype, offset, size, label, flags = struct.unpack_from('<HBBII16sI', body, at)
            assert offset + size <= 16 << 20
            item = dict(type=kind, subtype=subtype, offset=offset, size=size, label=label.split(b'\0')[0].decode(), flags=flags)
            if kind == 0 and offset < len(body) and body[offset] == 0xe9:
                assert audit.image(body, offset) == 9
                item['chipId'] = 9
            partitions.append(item)
        elif magic == 0xebeb:
            assert hashlib.md5(body[0x8000:at]).digest() == body[at + 16:at + 32]
            md5 = True; break
        elif magic == 0xffff: break
        else: raise ValueError(('partition magic', at, magic))
    assert md5 and any(x.get('chipId') == 9 for x in partitions)
    save(OUT / 'lcd169-original-image-audit.json', dict(schemaVersion=1, archive=archive,
         archiveMember=firmware['archiveMember'], bytes=len(body), sha256=sha(body), chipId=9,
         flashHeaderMiB=2 ** (body[3] >> 4), physicalFlashMiB=16, partitions=partitions,
         partitionMd5Valid=md5, declaredImageChecksumsValid=True, modified=False,
         electricalRevisionQualification='Family factory image; exact V1/V2 power-pin qualification is not declared by the manufacturer.',
         fullApplicationValidated=False, hardwareValidated=False))

    model_id = 'waveshare-esp32-s3-lcd-1-69'
    model_dir = ROOT / 'web/platform/assets' / model_id
    plan = read(model_dir / 'model-plan.json')
    for source in plan['sources']:
        rows.append(dict(id='mechanical-' + source['id'], title=source['file'], url=source['url'],
                         resolvedUrl=source['resolvedUrl'], fetchedAt=utc(source['fetchedAt']), bytes=source['bytes'],
                         sha256=source['sha256'], status='fetched', kind='manufacturer-mechanical-source', archiveName='mechanical-' + source['id'] + '--' + source['file'],
                         notes='Unchanged retained bytes and container/member receipts are in the model source-manifest.json.'))
    short = [dict(id=r['id'], title=r['title'], url=r['url'], checkedAt=r['fetchedAt'][:10])
             for r in rows if r['id'] in ['product', 'overview', 'schematic-v1', 'schematic-v2', 'demo-archive']]
    limits = ['非触控LCD169与Touch-LCD169是不同商品，未添加CST触摸、TF、RGB灯或AXP2101；ETA6098为模拟充电器。',
              '两版的SYS_OUT/SYS_EN/蜂鸣器/RTC中断引脚不同；BOOT和低有效PWR只模拟真实数字输入，未模拟电源保持、充电、蜂鸣声音或RTC中断。',
              'ST7789V2支持RGB565串行子集，240×280玻璃取240×320 RAM的第20至299行；物理光学、完整模拟伽马与262K色路径未验证。',
              '原始出厂BIN按两版各运行5秒，均13个RAMWR帧、9931种颜色及0重启；该结果不证明闭源BIN对两版电源引脚的完整资格。',
              '原生实际LX7测试验证SPI模式0/3与全双工、错线、复位/PWM、GPIO按键、QMI/RTC数字路径；无线、USB、电池、完整应用及实物未验收。',
              '原CAD未标明电气版次；保留287个原生组件及原坐标，不能据此宣称V1/V2机械完全等同。',
              '已检查官方说明、固件包和定向来源检索，未定位此非触控S3型号的精确UIFlow/小智恢复镜像；不借用C6或Touch版本。']
    products = []; knowledge = []; capabilities = {}
    for v2 in [False, True]:
        pid = model_id + ('-v2' if v2 else '')
        board = 'waveshare-s3-lcd169-v' + ('2' if v2 else '1')
        revision = 'V2' if v2 else 'V1'
        pins = dict(boot=0, powerKey=40 if v2 else 36, powerEnable=41 if v2 else 35,
                    buzzer=42 if v2 else 33, rtcInterrupt=39 if v2 else 41)
        product = dict(id=pid, name='Waveshare ESP32-S3-LCD-1.69 · ' + revision,
             manufacturer='Waveshare', sku='27932', chip='esp32s3', hardwareRevision=revision,
             summary='1.69英寸非触控240×280屏，16MiB Flash/8MiB PSRAM；独立电气版次，原厂多色画面已观察，数字子集部分适配。',
             knowledgeUrl='./knowledge/' + pid + '.json', family=dict(id=listing.replace('.', '-'), name='ESP32-S3-LCD-1.69'),
             runtime=dict(board=board, flashMb=16, psramMb=8, status='partial', touch=False),
             displayStatus='known', display=dict(width=240, height=280, type='IPS LCD', bus='SPI', diagonalInches=1.69, controller='ST7789V2', touch=False, shape='rectangle'),
             controls=[dict(id='boot', name='BOOT', type='button', pin=0, activeLow=True, defaultKeyCode='Space'),
                       dict(id='power', name='PWR · 数字按键', type='button', pin=pins['powerKey'], activeLow=True, defaultKeyCode='KeyP')],
             interfaces=[dict(id='usb-c', name='USB-C供电/下载', type='USB-C', external=True, pins=[], sourceIds=['overview'])],
             model=dict(type='manufacturer-cad', url='./assets/' + model_id + '/model.json', status='partial', provenance='official-cad'),
             sources=short, limitations=limits, defaultFirmwareId=model_id + '-factory',
             chronology=dict(date=None, basis='unknown', confidence='unknown', sourceId=None, sourceUrl=None, notes='无可靠硬件发布日期；资料或抓取时间不冒充上市日。'))
        if v2: product.update(visualVariantOf=model_id, firmwareTargetId=model_id)
        products.append(product); capabilities[board] = dict(chip='esp32s3', flashMb=16, psramMb=8, sdCard=False, boardButtons=[])
        facts = [dict(id='identity', label='销售身份与实际内存', basis='official', sourceIds=['product', 'overview'], value=dict(sku='27932', hardwareRevision=revision, flashMiB=16, psramMiB=8, processor='ESP32-S3R8')),
                 dict(id='display', label='屏幕与物理SPI接线', basis='official', sourceIds=['overview', 'schematic-v2' if v2 else 'schematic-v1'], value=dict(controller='ST7789V2', pixels=[240, 280], ramPixels=[240, 320], visibleRows=[20, 299], clock=6, mosi=7, chipSelect=5, dataCommand=4, reset=8, backlight=15, touch=False)),
                 dict(id='revision-pins', label='版本专属数字与模拟电源线路', basis='official', sourceIds=['schematic-v2' if v2 else 'schematic-v1'], value=pins),
                 dict(id='i2c', label='源支持的板载I2C器件', basis='official', sourceIds=['schematic-v2' if v2 else 'schematic-v1'], value=dict(sda=11, scl=10, qmi8658cAddress=107, pcf85063Address=81, charger='ETA6098 analog; not an I2C endpoint')),
                 dict(id='original-firmware', label='未修改官方完整恢复镜像', basis='official', sourceIds=['demo-archive'], value=dict(entry=firmware['archiveMember'], sha256=sha(body), bytes=len(body), chipId=9, offset=0, exactElectricalRevisionUnknown=True)),
                 dict(id='mechanical', label='原装配与活屏几何', basis='cad-measurement', sourceIds=['mechanical-official-cad', 'mechanical-mechanical-drawing'], value=dict(nativeComponents=287, modelSha256=sha((model_dir / 'model.json').read_bytes()), activeAreaMm=[27.972, 32.634], mechanicalElectricalRevision='unversioned; correspondence unknown'), method='Preserve native solid/surface coordinates and all287 components. Retained dimensioned photo registers PCB/buttons/USB; no enclosure inferred.', uncertainty='Unversioned mechanical assembly; no optical, key-travel or connector mating calibration.')]
        record = dict(schemaVersion='1', id=pid, kind='product', catalogId=pid, title=product['name'], summary=product['summary'],
                      summarySourceIds=['product', 'overview'], updatedAt=STAMP, hardwareRevision=revision, facts=facts, revisions=[], sources=rows,
                      assets=[dict(id=r['id'], sourceId=r['id'], role='source-document', url=r['url']) for r in rows],
                      unknowns=[dict(id=key, label=label, basis='unknown', value=None, sourceIds=['overview']) for key, label in [('release-date', '硬件发布日期'), ('factory-power-revision', '闭源出厂BIN电源引脚资格'), ('uiflow', '此精确型号UIFlow镜像'), ('xiaozhi', '此精确型号小智镜像'), ('physical-validation', '真实硬件验收')]], collectionNotes=limits)
        p = ROOT / 'web/platform/knowledge' / (pid + '.json'); save(p, record); knowledge.append(dict(path=str(p.relative_to(ROOT))))
    fwid = model_id + '-factory'; pids = [p['id'] for p in products]
    fw = dict(id=fwid, name='LCD1.69 · 官方闭源出厂恢复程序（trial）', productIds=pids, version=archive['sha256'][:12],
              status='trial', role='factory', publisher='Waveshare', publisherGroup='官方', simulationVerified=False,
              hardwareValidated=False, storage='remote-links-only', publicationUrl='https://docs.waveshare.com/ESP32-S3-LCD-1.69/Firmware-Flashing',
              sources=short, limitations=limits, source=dict(url=archive['requestedUrl'], sha256=sha(body), bootMode='rom',
                   archive=dict(format='zip-bundle', entry=firmware['archiveMember'], sha256=archive['sha256'])))
    watch = dict(id=fwid + '-watch', name='LCD1.69官方完整BIN人工复核', category='factory', metadataProvider='manual',
                 upstreamRepo='manufacturer:Waveshare/ESP32-S3-LCD-1.69', metadataUrl=fw['publicationUrl'], releaseUrl=archive['requestedUrl'],
                 version=archive['sha256'][:12], checkedAt=STAMP, assets=[dict(name=Path(firmware['archiveMember']).name, url=archive['requestedUrl'],
                      bytes=archive['bytes'], sha256=archive['sha256'], format='manufacturer-zip-candidate', archiveMember=firmware['archiveMember'], memberBytes=len(body), memberSha256=sha(body))],
                 compatibility=dict(status='reference-only', productIds=pids, simulationVerified=False, hardwareValidated=False, note='Static original ZIP; no release API. Preserve trial and review exact revision independently.'),
                 catalogTargets=[], storage='remote-links-only', monitoringNote='Manual exact member/board revision review; do not infer compatibility from a shared display diagonal.')
    save(OUT / 'source-manifest.json', dict(schemaVersion=1, sources=retained))
    save(Path(__file__).with_name(listing + '.json'), dict(listingId=listing, products=products, firmwares=[fw], watchlist=[watch], knowledge=knowledge,
         boardCapabilities=capabilities, sourceReceiptPaths=[str((OUT / n).relative_to(ROOT)) for n in ['source-manifest.json', 'lcd169-original-image-audit.json']], limitations=limits))

    # Preserve existing independently tested electrical and firmware evidence.
    pids = ['waveshare-esp32-s3-touch-lcd-2-1', 'waveshare-esp32-s3-touch-lcd-2-1b']
    catalog = read(ROOT / 'web/platform/catalog/products.json')['items']
    products = [dict(next(p for p in catalog if p['id'] == pid)) for pid in pids]
    knowledge = []; capabilities = {}
    for product in products:
        pid = product['id']; directory = ROOT / 'web/platform/assets' / pid
        plan = read(directory / 'model-plan.json'); model = read(directory / 'model.json')
        product['model'] = dict(type='manufacturer-cad', url='./assets/' + pid + '/model.json', status='partial', provenance='official-cad')
        product['family'] = dict(id='esp32-s3-touch-lcd-2-1', name='ESP32-S3-Touch-LCD-2.1 / 2.1B')
        product['limitations'] = list(dict.fromkeys(product['limitations'] + plan['limitations']))
        path = ROOT / 'web/platform/knowledge' / (pid + '.json'); record = read(path)
        ids = []
        for source in plan['sources']:
            sid = 'native-mech-' + source['id']; ids.append(sid)
            record['sources'] = [r for r in record['sources'] if r['id'] != sid]
            record['sources'].append(dict(id=sid, title=source['file'], url=source['url'], resolvedUrl=source['resolvedUrl'],
                  status='fetched', kind='manufacturer-mechanical-source', archiveName=sid + '--' + source['file'], fetchedAt=utc(source['fetchedAt']), bytes=source['bytes'], sha256=source['sha256'], notes='Retained native model source and exact parent/member receipt; mechanical revision does not certify electrical revision.'))
        fact = next(x for x in record['facts'] if x['id'] == 'model-dimensions')
        fact.update(value=dict(dimensions=model['dimensions'], nativeComponents=523 if pid.endswith('1b') else 522,
                              liveDisplay=model['presentation']['screen'], modelSha256=sha((directory / 'model.json').read_bytes()),
                              sku=product['sku'], mechanicalRevision='source-specific; electrical revision unspecified'),
                    sourceIds=ids, method='Preserve every original native component and millimetre coordinate. Exact flat/curved sources remain separate; optical assembly and live display form one rigid inspection group.',
                    notes=' '.join(plan['limitations']), uncertainty='No hardware fit, PCB pose, key travel, connector mating, curved refraction or browser/WebGL acceptance.')
        record['collectionNotes'] = list(dict.fromkeys(record['collectionNotes'] + plan['limitations']))
        record['updatedAt'] = STAMP; save(path, record); knowledge.append(dict(path=str(path.relative_to(ROOT))))
        capabilities[product['runtime']['board']] = dict(chip='esp32s3', flashMb=16, psramMb=8, sdCard=True, boardButtons=[])
    firmwares = []
    for fw in read(ROOT / 'web/platform/catalog/firmwares.json')['items']:
        if set(fw.get('productIds', [])) & set(pids):
            row = dict(fw); row['productIds'] = [p for p in fw['productIds'] if p in pids]; firmwares.append(row)
    watchlist = []
    for entry in read(ROOT / 'web/platform/catalog/firmware-watchlist.json')['entries']:
        if set(entry.get('compatibility', {}).get('productIds', [])) & set(pids):
            row = json.loads(json.dumps(entry)); row['compatibility']['productIds'] = [p for p in row['compatibility']['productIds'] if p in pids]; watchlist.append(row)
    save(Path(__file__).with_name('esp32-s3-touch-lcd-2.1.json'), dict(listingId='esp32-s3-touch-lcd-2.1', products=products,
         firmwares=firmwares, watchlist=watchlist, knowledge=knowledge, boardCapabilities=capabilities,
         sourceReceiptPaths=['docs/research/waveshare-tca-rgb-sources/manifest.json', 'docs/research/waveshare-tca-rgb-sources/firmwares.json'],
         limitations=['Existing original image/early touch/SDMMC evidence retained; later BLE stage and physical hardware remain unverified.'] ))
    print(json.dumps(dict(listings=2, profiles=4, uniqueSkus=3, sources=len(rows))))


if __name__ == '__main__': main()
