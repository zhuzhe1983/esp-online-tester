#!/usr/bin/env python3
"""Complete the first-pass324 source roles and refine legacy host mapping.
Run after audit-m5-store-remaining.py, using the same three hash-pinned responses.
No network, model, firmware, catalog or runtime mutation.
"""
import argparse
import collections
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('m5_remaining_audit',ROOT/'tools/audit-m5-store-remaining.py')
old=importlib.util.module_from_spec(spec);spec.loader.exec_module(old)
BASE=old.BASE
SCOPE=old.SCOPE
MAPPING=old.MAPPING
REPORT='docs/platform/research/m5-prior-store-audit-20261126.json'
SOURCES='docs/platform/research/m5-host-revision-sources-20261126.json'
MORE_HOSTS={'K137':('AtomS3','C123'),'K033-C':('StickC Plus2','K016-P2'),'K038':('StickC','K016-C'),'K038-B':('StickC Plus','K016-P'),'K014-C':('Gray','K002'),'K014-E':('FIRE','K007'),'K017-C':('M5Core',None)}
ACTIVE_NOT_PASSIVE={'U184-AS923','U184-US915','U184-EU868','U184-CN470','U190','U187','U170-433','U111-B','U170','U137','U111','U117','U116','U115','U110'}


def save(name,value):
    path=ROOT/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')


def run(args):
    baseline=old.git_json(BASE,SCOPE);scope=json.loads((ROOT/SCOPE).read_text())
    # Freeze the catalog comparison to an existing committed integration snapshot.
    catalog=old.git_json(args.catalog_commit,'web/platform/catalog/products.json')['items']
    by_sku={s.strip().upper():p for p in catalog if p.get('manufacturer')=='M5Stack'for s in p.get('sku','').split('/')}
    raw={}
    for i,path in enumerate(args.pages):
        body=path.read_bytes();assert old.digest(body)==baseline['sources'][i]['sha256']
        for p in json.loads(body)['products']:
            key=str(p['id']);assert key not in raw;raw[key]=p
    assert len(raw)==687
    prior=[r for r in baseline['products']if r['classification']!='not-reviewed'];assert len(prior)==324
    current={r['shopProductId']:r for r in scope['products']}
    matrix=json.loads((ROOT/MAPPING).read_text());mappings={x['sku']:x for x in matrix['mappings']}
    decisions=[];corrections=[]
    for row in prior:
        p=raw[row['shopProductId']];text,includes,rows,links=old.source_sections(p);sku=row['skus'][0]
        classification=row['classification'];basis='Primary description, chip/memory specification and actual Includes reviewed; earlier role supported by the inspected public source.'
        if sku in MORE_HOSTS:
            classification='esp32-host-kit-variant';basis='An ESP32 host is explicitly supplied/named. STM32 MCU/Base-MCU rows describe a motor/input coprocessor and cannot erase the ESP32 host.'
        if sku=='K115':classification='esp32-host-kit-distinct-configuration';basis='Atom-PSRAM 8MB Flash+2MB PSRAM is distinct from C008 Atom-Lite 4MB/no PSRAM.'
        if sku=='U109-B':classification='confirmed-esp32-device';basis='Exact product body identifies ESP32-WROOM-32E/4MB/OV2640 complete Unit Camera; Enclosure in its title does not mean an empty case accessory.'
        if sku in ACTIVE_NOT_PASSIVE:
            classification='non-esp32-controller-accessory'if old.NON_ESP_MCU.search(text)else'source-excluded-external-host-peripheral'
            basis='Active radio/GNSS/audio/modem peripheral identified in the primary source. Antenna in its title is not a passive-accessory classification. No supplied ESP32 host established.'
        if sku=='K143':classification='non-esp32-platform-with-esp32-coprocessor';basis='AX630C is the primary compute SoC; an onboard ESP32-C6 network coprocessor is explicitly present. Not an ESP32-primary product and not an ESP32-free platform.'
        source=dict(sourcePage=row['sourcePage'],sourcePageSha256=baseline['sources'][row['sourcePage']-1]['sha256'],shopProductId=row['shopProductId'],bodyUtf8Sha256=old.digest(p.get('body_html','').encode()),officialLinks=links)
        decision=dict(shopProductId=row['shopProductId'],skus=row['skus'],name=row['name'],url=row['url'],previousClassification=row['classification'],classification=classification,classificationBasis=basis,reviewLevel='official-primary-description-specification-and-includes',primaryChipAndMemoryRows=rows,includesInventory=includes,source=source,pcbBomReviewed=False,adaptationComplete=False)
        if sku=='U109-B':decision['confirmedHardware']={'module':'ESP32-WROOM-32E','flashMb':4,'psramMb':None,'camera':'OV2640','dfovDegrees':65,'dimensionsMm':[40,24,11]}
        if sku=='K143':decision['esp32Coprocessor']={'chip':'ESP32-C6','role':'2.4GHz Wi-Fi network card','primarySoc':'AX630C','independentEsp32HostCounted':False,'flashAndFirmwareUnresolved':True}
        decisions.append(decision)
        target=current[row['shopProductId']]
        target['sourceRoleReviewAt']=args.checked_at;target['sourceRoleReviewReport']=REPORT
        if classification!=row['classification']:
            target.update(classification=classification,classificationBasis=basis,evidence=[{'sourcePage':source['sourcePage'],'bodyUtf8Sha256':source['bodyUtf8Sha256'],'auditReport':REPORT,'sku':sku}],reviewState='source-classified-not-adaptation-complete')
            corrections.append({'sku':sku,'from':row['classification'],'to':classification,'reason':basis})
        if sku in MORE_HOSTS:
            family,parent=MORE_HOSTS[sku]
            entry=dict(sku=sku,name=row['name'],shopUrl=row['url'],hostFamily=family,parentSku=parent,parentCatalogId=by_sku[parent]['id']if parent else None,mappingStatus='named-host-configuration-needs-individual-review'if parent else'host-family-confirmed-revision-unresolved',existingExactCatalogIds=[by_sku[s]['id']for s in row['skus']if s in by_sku],hostPresence='confirmed',previousClassification=row['classification'],newInSecondPass=False,newInPriorRoleReview=True,primaryIncludes=includes,primaryChipAndMemoryRows=rows,hostPresenceEvidence={'section':'Primary description and Includes','namedHost':family},source=source,runtimeIdentityInherited=False,firmwareIdentityInherited=False,accessoryBehavior='not-covered-by-this-source-mapping')
            if sku=='K137':entry['hostConfiguration']={'module':'ESP32-S3FN8','flashMb':8,'psramMb':0}
            if sku=='K014-C':entry.update(hostConfiguration={'flashMb':16,'psramMb':None,'displayDriver':'ILI9342C','imu':'BMM150+MPU6886'},parentRevisionLocked=False,notes='Named Gray host and 16MB/ILI9342C configuration match the existing K002 family. Source does not state4MB PSRAM or ILI9341; PSRAM is unstated and no new distinct controller configuration inferred.')
            if sku=='K014-E':entry.update(hostConfiguration={'flashMb':16,'psramMb':8,'displayDriver':'ILI9342C','imu':'MPU6886'},parentRevisionLocked=False,notes='Named Fire host, modern16MB/8MB/ILI9342C configuration. K007 is a family reference; exact v2.6/v2.7 supply revision is not locked by kit sources. This is not legacy K014-B16MB/4MB/ILI9341.')
            if sku=='K017-C':entry.update(candidateParentSkus=[],notes='Current official LidarBot page is K017-C. It records2019.7 K017->K017-C LiDAR/sheet-metal changes but does not lock the main/remote M5Core Flash, PSRAM or parent purchase SKU. STM32 specification concerns the base, not the whole kit.')
            mappings[sku]=entry
    # Refine six requested identities using exact photos, public documents and
    # fixed source examples; manufacturer SKU claims and inferences stay distinct.
    x=mappings['A145-V033'];x.update(parentSku='S007-PIN127-V033',parentCatalogId=by_sku['S007-PIN127-V033']['id'],mappingStatus='named-host-configuration-needs-individual-review',hostConfiguration={'module':'ESP32-S3FN8','flashMb':8,'psramMb':0,'headerPitchMm':1.27,'headerCounts':[17,11]},parentSkuBasis='Inferred correspondence from exact kit photographs and official PIN1.27 drawing/photographs, not an explicit full purchase SKU in kit Includes.',parentRevisionLocked=True,exactPurchaseSkuClaimed=False,notes='Exact A145-V033 official gallery2/3/4 shows pre-soldered dense1.27mm header form. Same S3A controller configuration as S007-PIN127-V033; kit enclosure is black whereas standalone SKU photos are orange. Do not inherit orange material or claim the kit BOM literally names S007-PIN127-V033. Module-only4.7mm dimension does not include the full pin envelope.',deepEvidenceSourceIds=['host-depth-a145-v033-gallery-2','host-depth-a145-v033-gallery-4','host-depth-pin127-stamps3a-size','host-depth-docs-s007-pin127-v033'])
    x.pop('candidateParentSkus',None)
    x=mappings['K014-B'];x.update(parentSku=None,parentCatalogId=None,mappingStatus='distinct-controller-configuration-needs-adaptation',hostConfiguration={'flashMb':16,'psramMb':4,'displayDriver':'ILI9341','imuVariants':['MPU6886','SH200Q'],'magnetometer':'BMM150'},parentRevisionLocked=False,notes='Exact K014-B shop specification establishes16MB Flash/4MB PSRAM/ILI9341. Current app/bala page labelsSKU K014 and corroborates only the family; it cannot be relabeled as exact K014-B. K007/K007-V26/K007-V27 catalog displays are ILI9342C. No parent is silently substituted, no8MB PSRAM inherited. Exact legacy board revision remains unknown; this documented configuration needs its own K014-B adaptation.',deepEvidenceSourceIds=['host-depth-bala-family-doc','host-depth-m5stack-source-2'])
    for sku in ['K023','K023-B','K119']:
        mappings[sku].update(parentSku=None,parentCatalogId=None,mappingStatus='host-family-confirmed-revision-unresolved',runtimeReuseBlocked=True,notes='Exact kit docs and supplied items name Basic only. Inspected photos do not reveal a legible controller memory/package or locked revision. Linked schematics document sensor/base circuits. Fixed manufacturer examples use M5Stack.h/M5.begin without pinning supplied Core Flash/PSRAM/LCD revision. Public sources inspected do not establish4MB versus16MB or an immutable K001/v2.6/v2.7 supply contract.',deepEvidenceSourceIds=['host-depth-docs-'+sku.lower()])
    mappings['K017'].update(parentSku=None,parentCatalogId=None,runtimeReuseBlocked=True,hostRoles=['robot M5Core','remote M5Core'],notes='Exact K017 store and fixed manufacturer old car/remote source establish two M5Core ESP-NOW roles plus separate MEGA328 base/handle controllers. They do not lock Core Flash/PSRAM, LCD or parent purchase revision. Current app/lidarbot page isK017-C, not exact original K017. Preserve unresolved parent configuration.',deepEvidenceSourceIds=['host-depth-applications-lidarbot-source-0','host-depth-applications-lidarbot-source-1','host-depth-applications-lidarbot-source-2','host-depth-lidarbot-family-doc'])
    for sku in ['K017','K017-C']:
        mappings[sku]['hostRoles']=['robot M5Core','remote M5Core']
        mappings[sku]['runtimeReuseBlocked']=True
    entries=list(mappings.values());assert len(entries)==44 and len({x['sku']for x in entries})==44
    for x in entries:
        x['catalogComparisonCommit']=args.catalog_commit
        if x['parentSku']:
            parent=by_sku[x['parentSku']];x['parentCatalogId']=parent['id'];assert parent['chip'].startswith('esp32')
        assert x['runtimeIdentityInherited']is False and x['firmwareIdentityInherited']is False
    matrix.update(checkedAt=args.checked_at,scope='44 additional source-confirmed ESP32-host listings:23 original host rows +14 from the363 pass +7 recovered from prior Base-MCU misclassifications. K115 and legacy K014-B require distinct host configurations; common-host mappings do not establish kit peripheral behavior.',mappings=entries,counts=dict(collections.Counter(x['mappingStatus']for x in entries)),revisionFollowupReport=REPORT)
    src=json.loads((ROOT/SOURCES).read_text());matrix['deepRevisionSourceReceipts']=src
    target=next(x for x in scope['products']if 'K014-B'in x['skus']);target.update(classification='esp32-host-kit-distinct-configuration',classificationBasis='Legacy FIRE16MB Flash/4MB PSRAM/ILI9341 in exact K014-B source is not a modern ILI9342C FIRE identity.',reviewState='distinct-host-configuration-needs-adaptation')
    scope['checkedAt']=args.checked_at;scope['counts']=dict(collections.Counter(x['classification']for x in scope['products']))
    scope['missingConfirmedDeviceSkus']=sorted(set(scope['missingConfirmedDeviceSkus'])|{'U109-B','K014-B'})
    scope['missingConfirmedDeviceSkusBasis']='Historical exact-SKU/configuration gap queue relative to the original91-entry baseline, not the current remaining work or unique silicon-design count.'
    scope['distinctHostConfigurationSkus']=['K115','K014-B']
    scope['cumulativeControllerFindingsFromRoleRechecks']=['U037','U138','U109-B','K115','K014-B']
    scope['sourceRoleReview']={'previousNotReviewedPass':old.REPORT,'prior324Pass':REPORT,'totalListingsSourceScreened':687,'pcbBomComplete':False,'adaptationComplete':False,'sourceIdentityConflicts':['A072'],'hostKitMapping':MAPPING,'hostSkuMappings':44}
    history=scope.setdefault('classificationPassHistory',[]);history[:]=[h for h in history if h.get('report')!=REPORT]
    history.append({'baselineCommit':BASE,'reviewedPriorClassifiedListings':324,'report':REPORT,'corrections':corrections,'distinctConfigurationCorrection':'K014-B','level':'Official primary-role recheck; Base-MCU or Enclosure/Antenna words cannot override documented ESP32 presence.'})
    assert len(scope['products'])==687 and sum(scope['counts'].values())==687
    counts=collections.Counter(x['classification']for x in decisions)
    report=dict(schemaVersion='1',checkedAt=args.checked_at,baselineCommit=BASE,catalogComparisonCommit=args.catalog_commit,scope='All324 initially classified listings re-read at primary description/specification/Includes level; follow-up to the363-listing pass, not a complete PCB/BOM audit.',sourceReceipts=baseline['sources'][:3],reviewedProductIds=len(decisions),previousClassificationCounts=dict(collections.Counter(x['classification']for x in prior)),reviewedClassificationCounts=dict(counts),newOwnEsp32SkuFindings=['U109-B'],newHostContainingSkus=sorted(MORE_HOSTS),distinctHostConfigurationSkus=['K115','K014-B'],nonEspPrimaryWithEsp32CoprocessorSkus=['K143'],corrections=corrections,revisionFindings={sku:{k:mappings[sku].get(k)for k in ['hostFamily','parentSku','parentCatalogId','mappingStatus','hostConfiguration','parentSkuBasis','parentRevisionLocked','runtimeReuseBlocked','hostRoles','notes','deepEvidenceSourceIds']}for sku in ['A145-V033','K023-B','K023','K119','K017','K014-B','K014-C','K014-E','K017-C']},unresolvedParentFamilies=[x['sku']for x in entries if x['mappingStatus']=='host-family-confirmed-revision-unresolved'],sources=src,products=decisions,boundaries=['Source role screening only; no complete populated PCB/BOM audit.','Header-form correspondence is a documented image/drawing inference, not a manufacturer full purchase-SKU statement.','Software board defaults and generic M5Stack headers cannot prove supplied physical Flash/LCD revision.','No model, protocol, firmware execution, flashing or physical verification added.'])
    save(SCOPE,scope);save(MAPPING,matrix);save(REPORT,report)
    remaining=json.loads((ROOT/old.REPORT).read_text());remaining['followupPrior324Review']=REPORT;remaining['globalFindingsMustIncludeFollowup']=True;save(old.REPORT,remaining)
    print(json.dumps({'priorReviewed':len(decisions),'newOwnEsp32SkuFindings':['U109-B'],'distinctHostConfigurations':['K115','K014-B'],'newHostContainingSkus':sorted(MORE_HOSTS),'hostMappings':len(entries),'mappingCounts':matrix['counts'],'all687SourceRoleCounts':scope['counts'],'unresolvedParents':report['unresolvedParentFamilies']},ensure_ascii=False,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--checked-at',required=True)
    parser.add_argument('--catalog-commit',required=True)
    parser.add_argument('pages',nargs=3,type=Path)
    run(parser.parse_args())
