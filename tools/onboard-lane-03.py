#!/usr/bin/env python3
"""Source preparation and reviewed product builds for onboarding lane 03.

Use tools/requirements-model.txt. Reuses the source fetcher owned by lane 01;
all lane outputs remain in separate product-specific paths.
"""
import argparse, importlib.util, json
from pathlib import Path

spec = importlib.util.spec_from_file_location('onboard_sources', Path(__file__).with_name('onboard-lane-01.py'))
sources = importlib.util.module_from_spec(spec); spec.loader.exec_module(sources)
sources.TEMP = Path('output/source/lane03')
PRIORITIES = [4,7,10,13,16,20,23,26,29,32,35,38,41,44,47,50,53,56,59,62,65,68,71,74,77,80,83]

def main():
    parser = argparse.ArgumentParser(); parser.add_argument('command', choices=['prepare','build','finalize']); parser.add_argument('--count', type=int, default=3); parser.add_argument('--ids', nargs='*'); args = parser.parse_args()
    products = [p for p in json.loads((sources.RUN/'queue.json').read_text())['products'] if p['priority'] in PRIORITIES]
    if args.ids: products = [p for p in products if 'm5stack-'+p['directory'].split('_')[0].lower() in args.ids]
    else: products = products[:args.count]
    if args.command == 'finalize':
        finalize(products);return
    for product in products:
        if args.command == 'prepare': sources.prepare(product)
        else:
            id='m5stack-'+product['directory'].split('_')[0].lower()
            prepared=json.loads((sources.TEMP/id/'prepared.json').read_text())
            output=Path('web/platform/assets')/id
            plan=json.loads((output/'model-plan.json').read_text())
            sources.build_model(prepared,plan,output)
            sources.build_knowledge(prepared,plan)
            print(id+' built',flush=True)


def finalize(products):
    items=[]; indexes=[]; firmware=[]
    for product in products:
        id='m5stack-'+product['directory'].split('_')[0].lower(); output=Path('web/platform/assets')/id
        if not (output/'model.json').exists():continue
        prepared=json.loads((sources.TEMP/id/'prepared.json').read_text());plan=json.loads((output/'model-plan.json').read_text());meta=json.loads((output/'model.json').read_text());record=json.loads((Path('web/platform/knowledge')/(id+'.json')).read_text());ev=Path('docs/platform/evidence')/id;report=json.loads((ev/'verification.json').read_text())
        shots=sorted(ev.glob('browser-*.png'));report['browser']={'harness':'web/platform/review-lane-03.html','status':'captured' if shots else 'pending','screenshots':{f.name:sources.sha(f.read_bytes()) for f in shots},'method':'Actual headless Chromium WebGL renders; independent official photographs inspected side-by-side. No pixel alignment or silhouette pass asserted.'}
        review=plan.get('photoReview')
        if review and shots:
            for p in report['photos']:p['status']='qualitative-reviewed-partial'
            report['photoReview']=review
        sources.write(ev/'verification.json',report)
        chip=product['primaryChip'];family=next((a for a,b in [('esp32s3','ESP32-S3'),('esp32c6','ESP32-C6'),('esp32c3','ESP32-C3'),('esp32p4','ESP32-P4'),('esp32h2','ESP32-H2')] if b in chip.upper()),'esp32')
        specs={row[0]:row[1] for t in prepared['tables'] if t['heading'] in ['Specifications','Specification'] for row in t['rows'][1:] if len(row)==2}
        def mb(key):
            value=specs.get(key,'');m=__import__('re').search(r'(\d+)\s*MB',value);return int(m.group(1)) if m else None
        item={'id':id,'name':plan['title'],'manufacturer':'M5Stack','sku':product['skuAsDocumented'],'summary':plan['summary'],'knowledgeUrl':f'./knowledge/{id}.json','hardwareRevision':plan.get('hardwareRevision','unspecified'),'chip':family,'module':chip,'runtime':{'board':None,'status':'unavailable','flashMb':mb('Flash'),'psramMb':mb('PSRAM')},'display':plan.get('display'),'controls':plan.get('controls',[]),'interfaces':plan.get('interfaces',[]),'model':{'type':'manufacturer-cad','url':f'./assets/{id}/model.json','status':'partial'},'limitations':plan['limitations'],'sources':[{'title':s['title'],'url':s['url'],'checkedAt':s.get('fetchedAt','').split('T')[0]} for s in prepared['sources'] if s['id']=='official-docs'],'inventory':{'priority':product['priority'],'directory':product['directory'],'lastProductsChangeAt':product.get('lastProductsChangeAt'),'orderMeaning':'latest Products directory change; not release date'}}
        if item['display'] is None: item.pop('display')
        items.append(item);indexes.append({'id':id,'kind':'product','catalogId':id,'name':plan['title'],'url':f'./{id}.json','updatedAt':record['updatedAt']})
        refs=[{'title':l['title'],'url':l['url']} for l in prepared['links'] if l['url'].startswith('https://') and any(k in (l['title']+' '+l['heading']).lower() for k in ['factory','firmware','easyloader','webburner'])]
        firmware.append({'productId':id,'status':'reference-only','references':refs,'source':None,'simulationVerified':False,'hardwareValidated':False,'notes':'No binary downloaded. Official documentation links only; no executable compatibility inferred.' if refs else 'No factory firmware publication found in audited official page links; driver/example links remain in knowledge sources.'})
        failures=[x for x in report['dimensions'] if not x['pass']]
        text=f"# {plan['title']}\n\n厂家 CAD 按毫米导入；状态为 partial。此型号尚无可运行的板级模拟器。\n\n{plan['summary']}\n\n装配：{plan['assemblyNotes']}\n\n核查结果：{len(meta['parts'])} 个保留组件，{len(plan.get('omittedComponents',[]))} 个排除组件；{len(report['dimensions'])} 项几何检查，{len(failures)} 项未通过。失败值保存在 [verification.json](../evidence/{id}/verification.json)。\n\n浏览器照片核对：{review or '尚待独立照片对照；不能视为外观精度通过。'}\n\n限制：\n\n"+'\n'.join('- '+x for x in plan['limitations'])+f"\n\n来源与装配计划：[source-audit.json](../evidence/{id}/source-audit.json)。复现：`python tools/onboard-lane-03.py prepare --ids {id}`，然后 `python tools/onboard-lane-03.py build --ids {id}`；依赖 `tools/requirements-model.txt`。厂家 CAD 保留 MIT；官方照片未核实另有开放许可，不归入项目 MIT。\n"
        path=Path('docs/platform/models')/(id+'.md');path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text)
    sources.write(sources.RUN/'submissions/lane-03.json',{'schemaVersion':'1','lane':'03','updatedAt':sources.now(),'products':items,'knowledgeIndexEntries':indexes,'firmwareReferences':firmware,'pendingPriorities':[p for p in PRIORITIES if p not in [x['inventory']['priority'] for x in items]],'acceptance':'Each model remains partial; individual dimension failures and missing components are preserved. Runtime unavailable unless separately implemented and tested.'})

if __name__ == '__main__': main()
