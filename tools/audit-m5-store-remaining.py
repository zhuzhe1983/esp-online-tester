#!/usr/bin/env python3
"""Reproduce the source-level second pass over the 363 previously unclassified listings.

Inputs are the three unmodified official Shopify products.json responses from the
source receipts, not a claim of PCB/BOM completeness. No network or firmware IO.
"""
import argparse
import collections
import copy
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import lxml.html

ROOT = Path(__file__).resolve().parents[1]
BASE = '8e11bfcf2aab31fc7cc5f96d1a29f6ff3b6a1e10'
SCOPE = 'docs/platform/research/m5-full-store-scope-20261126.json'
REPORT = 'docs/platform/research/m5-remaining-store-audit-20261126.json'
MAPPING = 'docs/platform/research/m5-host-kit-mapping-20261126.json'
# These SKU decisions were reviewed against primary description/specification and
# the actual Includes heading, not generic host compatibility or title keywords.
HOSTS = {
 'K137-V11': ('AtomS3R', 'C126'), 'K059-B': ('AtomS3-Lite', 'C124'),
 'K016-H2': ('StickC Plus2', 'K016-P2'), 'K016-H': ('StickC Plus', 'K016-P'),
 'K016-F': ('StickC', 'K016-C'), 'K115': ('Atom-PSRAM', None),
 'A145': ('Stamp-S3 PIN1.27', 'S007-PIN127'), 'A145-V033': ('Stamp-S3A', None),
 'K010-SI': ('Core2 for AWS', 'K010-AWS'), 'K029': ('StickC', 'K016-C'),
 'K023': ('Basic', None), 'K023-B': ('Basic', None), 'K119': ('Basic', None),
 'K014-B': ('FIRE', None), 'K017': ('M5Core', None),
}
for _sku in ['K041-B','K115-B','K065','K063','K061','K057','K055','K054','K053','K052','K050','K042-D','K047','K046','K045','K044','K043','K118','K059','K040','K042','K041']:
    HOSTS[_sku] = ('Atom-Lite', 'C008')
NEW_HOSTS = {'A145','A145-V033','K023-B','K010-SI','K119','K118','K059','K040','K042','K041','K029','K023','K017','K014-B'}
INDEPENDENT = {'U138': {'module': 'ESP32-WROOM-32U', 'flashMb': 4, 'psramMb': None, 'components': ['IP101G','RMII','PoE'], 'basis': 'Own ESP32 control core in primary description and specification; preloaded ESP-AT. UART host-control use does not remove its own programmable ESP32.'},
               'U037': {'module': 'ESP32 (exact package unspecified)', 'flashMb': 4, 'psramMb': 4, 'components': ['OV2640','160-degree fisheye','CP2104'], 'basis': 'Own ESP32/Flash/PSRAM camera board explicitly described; not a bare OV2640 camera add-on.'}}
NON_ESP_COMPUTE = {'U078-V-M12','U078-USB','U078-M12','U078-D','U078-C','U078'}
# Manual source-role groups. These do not assert that every component has been
# identified, nor that an accessory has complete geometry or working protocols.
PASSIVE = {'A184','A181','A173','U210','U211','A163','A162','A161','F086-L','A153','A148-B','A148-A','A147','A146','A144','A128-B','A141','A139','A129','A128','K011-B-V11','K025-B','K026-B','U151','A125','A119','A126','A123','M120','F059','F060','A117','A121','A118','K116','A089','U125','A111','F057','A101','M037','M033','M032','A086','K039','A077','A022-1','F045','U081','U080','U060-B','K009-F','A070','K025','K026','F001','A051','A061','A031','A050-B','A002','U020','U022','M002','M001','A013','M024'}
NON_ESP_MCU = re.compile(r'(?<![A-Z0-9])(?:PY32[A-Z0-9]*|STM32[A-Z0-9]*|ATMEGA[A-Z0-9]*|MEGA(?:328P?|8A)|ATTINY[A-Z0-9]*|CC2630[A-Z0-9]*|ASR6501|CI1302|CI-03T|GW1NR[A-Z0-9-]*)(?![A-Z0-9])', re.I)
CHIP = re.compile(r'(?<![A-Z0-9])(?:ESP32[A-Z0-9-]*|PY32[A-Z0-9]*|STM32[A-Z0-9]*|ATMEGA[A-Z0-9]*|MEGA(?:328P?|8A)|CC2630[A-Z0-9]*|ASR6501|CI1302|CI-03T|K210|SSD202D|W5500|IP101G|OV2640|OV7740|OV3660|GC0308|GW1NR[A-Z0-9-]*|AT6668|SX12[0-9A-Z]*|SIM[0-9][0-9A-Z]*|PCA[0-9A-Z]*|ADS[0-9A-Z]*|SHT[0-9A-Z]*|BMP[0-9A-Z]*|BMM[0-9A-Z]*|BMI[0-9A-Z]*|MPU[0-9A-Z]*|MLX[0-9A-Z]*|MAX[0-9][0-9A-Z]*|HX711|SK6812|WS2812[A-Z]*)(?![A-Z0-9])', re.I)


def digest(data): return hashlib.sha256(data).hexdigest()
def git_json(revision, name): return json.loads(subprocess.check_output(['git','show',f'{revision}:{name}'],cwd=ROOT))
def save(name, value):
    dest = ROOT / name
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n')


def source_sections(product):
    doc = lxml.html.fromstring(product.get('body_html') or '<div/>')
    for node in doc.xpath('//style|//script'): node.drop_tree()
    text = ' '.join(doc.text_content().split())
    includes = []
    # Heading scope prevents the verb "includes" in descriptive prose from
    # swallowing the real kit inventory and hiding its controller.
    for head in doc.xpath('//*[self::h1 or self::h2 or self::h3 or self::h4 or self::strong or self::b]'):
        if not re.fullmatch(r'Includes?',head.text_content().strip(),re.I): continue
        node = head
        while node.getparent() is not None and node.tag in ['strong','b']: node=node.getparent()
        for sibling in node.itersiblings():
            if sibling.tag in ['h1','h2','h3','h4']: break
            lis=sibling.xpath('.//li')
            includes += [' '.join(x.text_content().split()) for x in lis] if lis else [' '.join(sibling.text_content().split())]
    rows = []
    for tr in doc.xpath('//table//tr'):
        cells=[' '.join(c.text_content().split()) for c in tr.xpath('./td|./th')]
        if len(cells)==2 and re.search(r'\b(?:SoC|MCU|CPU|processor|main controller|chip|IC|controller|flash|PSRAM)\b',cells[0],re.I): rows.append(cells)
    links=list(dict.fromkeys(x for x in doc.xpath('//a/@href') if x.startswith(('https://docs.m5stack.com/','https://github.com/m5stack/'))))
    return text,list(dict.fromkeys(x for x in includes if x)),rows,links


def run(args):
    scope=git_json(args.baseline,SCOPE)
    catalog=git_json(args.baseline,'web/platform/catalog/products.json')['items']
    by_sku={s.strip().upper():p for p in catalog if p.get('manufacturer')=='M5Stack' for s in p.get('sku','').split('/')}
    old_unreviewed=[p for p in scope['products'] if p['classification']=='not-reviewed']
    assert len(old_unreviewed)==363
    raw, pages = {}, {}
    for page,path in enumerate(args.pages,1):
        body=path.read_bytes();expected=scope['sources'][page-1]
        if digest(body)!=expected['sha256']: raise ValueError(f'Original source hash mismatch on page {page}')
        values=json.loads(body)['products']
        for p in values:
            key=str(p['id'])
            if key in raw: raise ValueError('Duplicate Shopify identity')
            raw[key]=p;pages[key]=page
    assert len(raw)==687
    decisions, mappings = [], []
    old_host_skus={s for p in scope['products'] if p['classification']=='esp32-host-kit-variant' for s in p['skus']}
    assert len(old_host_skus)==23 and set(HOSTS)==old_host_skus|NEW_HOSTS and len(HOSTS)==37
    for row in scope['products']:
        skus=row['skus'];sku=skus[0];p=raw[row['shopProductId']];text,includes,rows,links=source_sections(p)
        source=dict(sourcePage=pages[row['shopProductId']],sourcePageSha256=scope['sources'][pages[row['shopProductId']]-1]['sha256'],shopProductId=row['shopProductId'],bodyUtf8Sha256=digest(p.get('body_html','').encode()),officialLinks=links)
        if sku in HOSTS:
            family,parent=HOSTS[sku];is_old=sku in old_host_skus
            entry=dict(sku=sku,name=row['name'],shopUrl=row['url'],hostFamily=family,parentSku=parent,parentCatalogId=by_sku[parent]['id'] if parent else None,mappingStatus='named-host-configuration-needs-individual-review' if parent else 'host-family-confirmed-revision-unresolved',existingExactCatalogIds=[by_sku[s]['id'] for s in skus if s in by_sku],hostPresence='confirmed',previousClassification=row['classification'],newInSecondPass=not is_old,primaryIncludes=includes,source=source,runtimeIdentityInherited=False,firmwareIdentityInherited=False,accessoryBehavior='not-covered-by-this-source-mapping')
            entry['primaryChipAndMemoryRows']=rows
            entry['hostPresenceEvidence']={'section':'Includes','namedHost':family}
            if sku in ['K137-V11','K118','K029','K017','K014-B']:
                entry['hostPresenceEvidence']={'section':'Primary description and/or host specification','namedHost':family,'assembledProductListedInIncludes':True}
            if sku=='K137-V11':entry['hostConfiguration']={'module':'ESP32-S3-PICO-1-N8R8','flashMb':8,'psramMb':8}
            if sku=='K118':entry['hostConfiguration']={'module':'ESP32-PICO-D4','flashMb':4,'psramMb':0}
            if sku=='K115':
                entry.update(mappingStatus='distinct-controller-configuration-needs-adaptation',hostConfiguration={'module':'ESP32-PICO-V3-02','flashMb':8,'psramMb':2,'usbBridge':'CH9102'},parentSku=None,parentCatalogId=None,notes='No independently listed Atom-PSRAM SKU established. C008 Atom-Lite has 4MB Flash/no PSRAM and is not the same controller configuration.')
            if sku=='A145-V033':entry.update(candidateParentSkus=['S007-V033','S007-PIN127-V033'],notes='Includes names Stamp-S3A without a pin-form SKU. Exact physical parent variant unresolved; do not silently choose DIP/header form.')
            if sku in ['K023','K023-B','K119']:entry.update(candidateParentSkus=['K001','K001-V26','K001-V27'],notes='Basic controller included, but kit primary source does not pin controller hardware revision/Flash generation. Family mapping is not a claim of exact runtime compatibility.')
            if sku=='K014-B':entry.update(candidateParentSkus=['K007','K007-V26','K007-V27'],hostConfiguration={'flashMb':16,'psramMb':4,'displayDriver':'ILI9341'},notes='Kit source explicitly has older FIRE ILI9341/4MB PSRAM. Current FIRE catalog/runtime variants differ; exact parent revision cannot be inferred.')
            if sku=='K017':entry.update(candidateParentSkus=[],notes='Robot description explicitly includes M5Core and ESP-NOW remote. Core family/revision and remote host memory are unresolved; no modern Basic/Fire host identity assumed.')
            if sku=='K010-SI':entry.update(hostConfiguration={'module':'ESP32-D0WDQ6-V3','flashMb':16,'psramMb':8},notes='Named Core2 for AWS host; source hardware PSRAM is 8MB. Existing parent uses a bounded simulated memory configuration; mapping does not rewrite hardware facts or validate bundle peripherals.')
            if sku=='K040':entry['notes']='Official source explicitly says no pre-programmed factory flight firmware; functional example tests are not a factory flight image.'
            if sku=='K118':entry['notes']='Controller is embedded Atom-Lite, although Includes lists the assembled Atom Printer rather than separate host. Primary Features identify ESP32-PICO-D4/4MB.'
            mappings.append(entry)
        if row['classification']!='not-reviewed': continue
        classification='source-excluded-external-host-peripheral'
        basis='Primary description/specification identifies an expansion, sensor, transceiver, display or actuator function; Includes does not establish an ESP32 host. No own ESP32 primary-core evidence found in this listing. This is source-level screening, not a complete PCB/BOM exclusion.'
        if sku in INDEPENDENT: classification='confirmed-esp32-device';basis=INDEPENDENT[sku]['basis']
        elif sku in HOSTS: classification='esp32-host-kit-variant';basis='Actual included/named host identified in primary description and/or Includes; no additional independent MCU design inferred from accessories.'
        elif sku in NON_ESP_COMPUTE: classification='non-esp32-compute-platform';basis='Primary compute platform explicitly identified as Kendryte K210 or SigmaStar SSD202D, not ESP32.'
        elif sku=='A072': classification='source-identity-conflict';basis='Shop title says with-core; current official description, Includes and M5-Bus pin map identify only the dual-relay base/terminal. Host inclusion unresolved; no ESP32 counted.'
        elif sku in PASSIVE: classification='mechanical-passive-or-power-accessory';basis='Primary purpose and supplied items are mounting, prototype/interconnect, power, repair hardware or merchandise; no included ESP32 controller is established.'
        elif NON_ESP_MCU.search(text): classification='non-esp32-controller-accessory';basis='Primary controller/driver role uses the listed non-ESP32 controller or FPGA; any mentioned M5/ESP32 host is external to the supplied peripheral.'
        declared_chips=sorted(set(m.group(0).upper() for m in CHIP.finditer(text)) - {'ESP32-IDF','ESP32-BASED'})
        decision=dict(shopProductId=row['shopProductId'],sku=sku,skus=skus,name=row['name'],url=row['url'],previousClassification='not-reviewed',classification=classification,classificationBasis=basis,reviewLevel='official-primary-description-specification-and-includes',declaredComponentNames=declared_chips,primaryChipAndMemoryRows=rows,includesInventory=includes,source=source,existingExactCatalogIds=[by_sku[s]['id'] for s in skus if s in by_sku],pcbBomReviewed=False,adaptationComplete=False)
        if sku in INDEPENDENT:decision['confirmedHardware']=INDEPENDENT[sku]
        if sku=='K024-B':decision['specificExclusion']='Demo Board-SHT30 Includes has board/power supply/cable/cards/jumpers only; M5Core is an external control core. Same boundary as corrected K024.'
        if sku in ['A133','A133-B']:decision['specificExclusion']='ESP32 is explicitly in the M5Atom host, not the supplied barcode base.'
        if sku in ['M039','M039-V11']:decision['specificExclusion']='The ESP32 phrase follows stacking with an M5 controller. Supplied driver module uses TCA9554 or STM32/HR8825; no host included.'
        if sku in ['S006','A105']:decision['specificExclusion']='ESP32 is the programming target, not the USB/UART programmer controller.'
        if sku=='A043':decision['specificExclusion']='SK6812 RGB headband is programmed by an external M5Core through Grove; integrated LED driver is not an ESP32 board.'
        if sku=='A106-V21':decision['specificExclusion']='SIM7028 + M5IOE1 DTU base supplied, no separate Atom host in Includes. Do not confuse with K059-B kit.'
        if sku=='K009-E':decision['specificExclusion']='Shop title explicitly excludes Core. Historical prose shows an M5Core application, but no supplied core is established; physical kit revision remains unresolved.'
        decisions.append(decision)
        row.update(classification=classification,classificationBasis=basis,evidence=[{'sourcePage':source['sourcePage'],'bodyUtf8Sha256':source['bodyUtf8Sha256'],'auditReport':REPORT,'sku':sku}],reviewState='source-classified-not-adaptation-complete',secondPassReviewedAt=args.checked_at)
    assert len(decisions)==363 and len({r['shopProductId'] for r in decisions})==363 and len(mappings)==37
    # Distinct Atom-PSRAM memory is an adaptation gap even though packaged as a kit.
    next(r for r in scope['products'] if 'K115' in r['skus']).update(classification='esp32-host-kit-distinct-configuration',classificationBasis='Atom-PSRAM is ESP32-PICO-V3-02 with 8MB Flash+2MB PSRAM, not the C008 Atom-Lite 4MB/no-PSRAM configuration.',reviewState='distinct-host-configuration-needs-adaptation')
    scope['classificationPassHistory']=[{'baselineCommit':args.baseline,'previousCounts':copy.deepcopy(scope['counts']),'previousNotReviewed':363,'report':REPORT,'hostMapping':MAPPING,'level':'Official source role screening only, not PCB/BOM or adaptation completion.'}]
    scope['counts']=dict(collections.Counter(r['classification'] for r in scope['products']))
    assert sum(scope['counts'].values())==687 and scope['counts'].get('not-reviewed',0)==0
    scope['missingConfirmedDeviceSkus']=sorted(set(scope['missingConfirmedDeviceSkus'])|set(INDEPENDENT)|{'K115'})
    scope['checkedAt']=args.checked_at
    scope['sourceLimitations'].append('Second pass reviewed the remaining363 current primary descriptions/specifications/Includes. Source identity conflict A072 and unpinned kit host revisions are explicit; zero not-reviewed is not a complete PCB/BOM/model/runtime claim.')
    report=dict(schemaVersion='1',checkedAt=args.checked_at,catalogAuditCommit=args.baseline,scope='Second-pass screening of exactly363 previously not-reviewed Shopify listings, with first-pass host kit mapping reconciled separately.',sourceReceipts=scope['sources'][:3],sourceMethod=['Verify all three original response SHA-256 values and687 unique product IDs.','Read each remaining primary description, chip/memory rows and actual Includes heading. Search ESP32/WROOM/WROVER/PICO/ESP-NOW only as a discovery aid, never as sole classification.','Review named/assembled kit hosts separately. Compatibility mentions, programmer targets, analog-resolution examples and successor notices do not establish an onboard ESP32.'],newIndependentDeviceSkus=sorted(INDEPENDENT),distinctHostConfigurationSkus=['K115'],newHostContainingListings=sorted(NEW_HOSTS),counts=dict(collections.Counter(r['classification'] for r in decisions)),needsSourceResolution=['A072'],hostRevisionOrPhysicalVariantUnresolved=[m['sku'] for m in mappings if m['mappingStatus']=='host-family-confirmed-revision-unresolved'],boundaries=['Not a complete PCB/BOM review; unidentified hidden circuitry cannot be excluded by source prose.','No new geometry, protocol, firmware, execution or physical verification implied.','First-pass classifications outside these363 and the23 prior host kits are retained, not silently re-audited.'],products=decisions)
    matrix=dict(schemaVersion='1',checkedAt=args.checked_at,catalogAuditCommit=args.baseline,scope='37 source-confirmed listings containing an ESP32 host:23 prior host-kit rows +14 newly found. K115 is a distinct host configuration and not mapped to Atom-Lite. Exact SKU identity remains separate from controller-family reuse.',counts=dict(collections.Counter(m['mappingStatus'] for m in mappings)),sourceReceipts=scope['sources'][:3],mappings=mappings,unconfirmedHostInclusion=[{'sku':'A072','status':'source-identity-conflict','reason':'Shop title with-core conflicts with primary base-only description/Includes; current official docs agree with base-only contents. Not silently counted as an ESP32 host kit.'}])
    doc_receipts=json.loads((ROOT/'docs/platform/research/m5-remaining-doc-source-receipts-20261126.json').read_text())
    report['additionalOfficialDocReceipts']=doc_receipts
    matrix['additionalOfficialDocReceipts']=doc_receipts
    report['sourceIdentityConflictEvidence']=[{'sku':'A072','sourceIds':['docs-a072'],'decision':'Do not count host presence while title conflicts with current official base-only contents.'}]
    for entry in mappings:
        matches=[d['id'] for d in doc_receipts if d.get('status')=='fetched' and d.get('sku')==entry['sku']]
        if matches:entry['additionalOfficialSourceIds']=matches
    save(SCOPE,scope);save(REPORT,report);save(MAPPING,matrix)
    print(json.dumps({'reviewed':len(decisions),'counts':report['counts'],'newIndependentDeviceSkus':report['newIndependentDeviceSkus'],'distinctHostConfigurationSkus':['K115'],'confirmedHostListings':len(mappings),'hostMappingCounts':matrix['counts'],'unresolved':['A072']},ensure_ascii=False,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline',default=BASE)
    parser.add_argument('--checked-at',required=True,help='UTC review completion time retained in the report')
    parser.add_argument('pages',nargs=3,type=Path,help='Unmodified official Shopify pages1,2,3 matching the retained SHA receipts')
    run(parser.parse_args())
