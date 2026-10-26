#!/usr/bin/env python3
"""Source preparation and reviewed product builds for onboarding lane 02.

Uses the lane 01 source/geometry helpers without modifying their files.
Each model-plan.json is a reviewed rigid assembly, never an automatic fit.
"""
import argparse, importlib.util, json, re, hashlib
from PIL import Image, ImageDraw
from urllib.parse import urljoin
from pathlib import Path

spec = importlib.util.spec_from_file_location('onboard_sources', Path(__file__).with_name('onboard-lane-01.py'))
sources = importlib.util.module_from_spec(spec); spec.loader.exec_module(sources)
sources.TEMP = Path('output/source/lane02-extra')
PRIORITIES = [16,23,29,35,41,47,53,59,65,71,77,83]

def submission():
    """Prepare a reviewable catalog patch; never writes the shared catalog."""
    path=sources.RUN/'submissions/lane-02-extra.json'
    result=json.loads(path.read_text()) if path.exists() else {'schemaVersion':'1','lane':'02-extra','status':'in-progress','products':[],'firmwares':[],'knowledgeIndex':[]}
    existing={p['id']:p for p in result['products']}
    displays={'m5stack-k049-b':(540,960,'E-Paper','IT8951E'),'m5stack-c139':(540,960,'E-Paper','ET047TC1'),'m5stack-k131':(200,200,'E-Paper','GDEY0154D67'),'m5stack-k130-v11':(240,240,'LCD','GC9A01'),'m5stack-k152':(240,135,'LCD','ST7789V2'),'m5stack-k132':(240,135,'LCD','ST7789V2'),'m5stack-c126':(128,128,'LCD','ST7735'),'m5stack-k034':(320,240,'LCD','ILI9342C'),'m5stack-k016-p':(135,240,'LCD','ST7789V2'),'m5stack-k010-aws':(320,240,'LCD','ILI9342C / ILI9342E'),'m5stack-k006-v27':(320,240,'LCD','ILI9342C'),'m5stack-k128':(320,240,'LCD','ILI9342C / ILI9342E'),'m5stack-k010-v13':(320,240,'LCD','ILI9342C / ILI9342E'),'m5stack-k151':(320,240,'LCD','ILI9342C / ILI9342E'),'m5stack-k001-v27':(320,240,'LCD','ILI9342C')}
    for product in sorted(json.loads((sources.RUN/'queue.json').read_text())['products'],key=lambda p:p['priority']):
        if product['priority'] not in PRIORITIES:continue
        id='m5stack-'+product['directory'].split('_')[0].lower();output=Path('web/platform/assets')/id
        if not (output/'model.json').exists():continue
        data=json.loads((sources.TEMP/id/'prepared.json').read_text());plan=json.loads((output/'model-plan.json').read_text());knowledge=json.loads((Path('web/platform/knowledge')/(id+'.json')).read_text());official=next(x for x in data['sources'] if x['id']=='official-docs')
        rows=[r for t in data['tables'] if t['heading'] in ['Specification','Specifications'] for r in t['rows'][1:]]
        values={r[0].lower():r[1] for r in rows if len(r)>1}
        def capacity(key):
            match=re.search(r'(\d+)\s*MB',values.get(key,''),re.I);return int(match[1]) if match else None
        chip=product['primaryChip'].lower();chip='esp32s3' if 's3' in chip else 'esp32c3' if 'c3' in chip else 'esp32h2' if 'h2' in chip else 'esp32'
        display=displays.get(id);display=dict(zip(['width','height','type','controller'],display)) if display else None
        interfaces=[]
        text=' '.join(' '.join(r) for r in rows)
        if re.search(r'USB Type-C|USB-C|Type-C|USB OTG',text):interfaces.append({'id':'usb-c','type':'USB-C','name':'USB Type-C','external':True,'pins':[]})
        if re.search(r'microSD|TF.card|SD Card',text,re.I):interfaces.append({'id':'micro-sd','type':'microSD','name':'microSD','external':True,'pins':[]})
        for table in data['tables']:
            if not re.search(r'HY2|Grove',table['heading'],re.I):continue
            for row in table['rows'][1:]:
                if len(row)>=5 and any(re.fullmatch(r'G\d+',v.strip()) for v in row[1:]):
                    pins=[{'name':v,**({'gpio':int(v[1:])} if re.fullmatch(r'G\d+',v) else {})} for v in row[1:] if v not in ['/','']]
                    interfaces.append({'id':'grove-'+re.sub(r'[^a-z0-9]+','-',row[0].lower()).strip('-'),'type':'HC1.25-4PLT' if 'HC1.25' in table['heading'] else 'HY2.0-4P','name':row[0],'external':True,'pins':pins})
        item=existing.get(id) or {'id':id,'name':'M5Stack '+product['name'],'manufacturer':'M5Stack','sku':product['skuAsDocumented'],'summary':plan['summary'],'hardwareRevision':product.get('explicitRevision') or 'unspecified','chip':chip,'module':product['primaryChip'],'runtime':{'board':None,'flashMb':capacity('flash'),'psramMb':capacity('psram'),'status':'unavailable'},'display':display,'controls':[],'interfaces':interfaces,'sources':[{'title':official['title'],'url':official['url'],'checkedAt':official['fetchedAt'][:10]}],'limitations':['本型号的板级运行模型尚未实现；导入外观不代表固件可运行。','尺寸、材质、未知内部件和照片核验范围见模型报告。'],'knowledgeUrl':'./knowledge/'+id+'.json','model':{'type':'manufacturer-cad','url':'./assets/'+id+'/model.json','status':'partial'}}
        item.update(plan.get('catalog',{}));
        if item.get('dimensions'):item['dimensions'].setdefault('basis','official')
        existing[id]=item
        # Official documentation links establish publication, not executable compatibility.
        firmware_links=[x for x in data['links'] if re.search(r'Factory|Firmware|Quick Start',x['title'],re.I) and re.search(r'github.com|burner.m5stack.com|/uiflow',x['url'],re.I)]
        for link in firmware_links:
            url=urljoin(official['url'],link['url']);fid=id+'-'+('factory-reference' if re.search('Factory',link['title'],re.I) else 'uiflow2-reference' if '/uiflow2/' in url else 'uiflow1-reference' if '/uiflow/' in url else 'firmware-reference')
            if any(f['id']==fid for f in result['firmwares']):continue
            factory='factory' in fid
            result['firmwares'].append({'id':fid,'name':link['title'],'publisher':'M5Stack','role':'factory' if factory else 'alternative','productIds':[id],'status':'reference','source':None,'sources':[{'title':link['title'],'url':url,'checkedAt':official['fetchedAt'][:10]},{'title':'Official product documentation','url':official['url'],'checkedAt':official['fetchedAt'][:10]}],'limitations':['官方页面收录的项目/教程参考；尚无经本平台验证的可运行固件与板级模拟。','未下载或镜像固件二进制；版本、分区与对应硬件兼容性待单独核验。']})
            if factory and not item.get('defaultFirmwareId'):item['defaultFirmwareId']=fid
        result['knowledgeIndex']=[x for x in result['knowledgeIndex'] if x['id']!=id]+[{'id':id,'kind':'product','catalogId':id,'name':item['name'],'url':'./'+id+'.json','updatedAt':knowledge['updatedAt']}]

    for firmware in result['firmwares']:
        source_product=firmware['productIds'][0]
        actual=json.loads((sources.TEMP/source_product/'prepared.json').read_text())
        checked=next(s['fetchedAt'][:10] for s in actual['sources'] if s['id']=='official-docs')
        for source in firmware['sources']:source.setdefault('checkedAt',checked)
    result['products']=list(existing.values());result['reviewedIds']=[id for id in existing if (Path('docs/platform/evidence')/id/'verification.json').exists() and json.loads((Path('docs/platform/evidence')/id/'verification.json').read_text()).get('browser',{}).get('status')=='reviewed-partial'];result['notes']=['All entries remain partial unless their exact evidence report proves more. Runtime-unavailable products have no executable firmware source.','Source CAD fidelity, dimension failures and independent photograph limitations are retained per product.'];sources.write(path,result);print(f"Submitted {len(result['products'])} products / {len(result['firmwares'])} firmware references")

def finish_review(id, report):
    assets=Path('web/platform/assets')/id; plan=json.loads((assets/'model-plan.json').read_text()); output=Path('docs/platform/evidence')/id
    # Preserve every raw numerical failure: visual review is a separate, partial conclusion.
    for photo in report['photos']:
        photo['method']=photo.get('method','').replace('scene.overrideMaterial black','temporary black materials retaining each alpha mask')
    shots={}
    panel=Image.new('RGB',(2400,440),'white');draw=ImageDraw.Draw(panel)
    for i,view in enumerate(['front','front-gold','front-purple','back','back-gold','back-purple']):
        path=Path('output/playwright')/(id+'-'+view+'.png');assert path.exists(), str(path)
        shots[view]=sources.sha(path.read_bytes());panel.paste(Image.open(path).convert('RGB').resize((400,400)),(i*400,40));draw.text((i*400+8,12),view,fill='black')
    panel.save(output/'appearance-comparison.webp',quality=92)
    report['browser']={'status':'reviewed-partial','harness':'web/platform/review-lane-02.html','views':['front','back','side'],'appearanceViews':['front','back'],'appearancePreset':{'color':'#d0a344','opacity':1,'finish':'metal'},'sourceRenderHashes':shots,'appearanceComparisonSha256':sources.sha((output/'appearance-comparison.webp').read_bytes()),'observations':['Manufacturer shell triangles retain source dimensions; this visual-review harness uses blank framebuffer surfaces and does not execute firmware.','Default, gold and translucent purple front/back renders inspected. Paper labels are independent explicitly masked source-photo layers; shell recoloring does not paint them or screen/connector material. Products without a label texture make no claim that missing text has been reconstructed.','UNKNOWN board placeholders remain explicit. Photo-derived visible connector inserts do not establish contact fit or electrical routing.'],'limitations':['Raw silhouette tests retain failures. Oblique source views, white-on-white segmentation, annotations/coins and uncertain camera pose can make this metric inconclusive; no failed metric is promoted to a precision pass.','No physical-device comparison, internal-board reconstruction, or firmware compatibility claimed.']}
    if id=='m5stack-s013':
        report['browser']['observations']=['All216 manufacturer board/component parts including3openUSB shells are retained. Native XCAF colors applied when available; missing colors use declared neutral display values.','No shell exists and no UNKNOWN board obscures manufacturer board geometry. Gold/purple appearance requests leave all component materials unchanged.','Printed traces, chip text, hardware revision and electrical behavior are not invented from the mechanical CAD.']
        report['browser']['limitations'].append('P4 native CAD revision differs from unspecified user hardware; no silicon or firmware compatibility claimed.')
    report['browser']['appearancePresets']=[{'color':'#d0a344','opacity':1,'finish':'metal'},{'color':'#8054b3','opacity':.35,'finish':'plastic'}]
    report['renderDependencies']={f:sources.sha((Path('web/platform')/f).read_bytes())for f in ['manufacturer-model.js','grove-geometry.js','shell-appearance.js','review-lane-02.js']}
    sources.write(output/'verification.json',report)
    failures=[x for x in report['dimensions']if not x['pass']]
    doc=Path('docs/platform/models')/(id+'.md')
    doc.write_text(f"# {plan['title']}\n\n{plan['summary']}\n\n厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。\n\n装配：{plan['assemblyNotes']}\n\n已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。\n\n几何检查共 {len(report['dimensions'])} 项，{len(failures)} 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/{id}/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/{id}/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。\n\n来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/{id}/source-audit.json)。复现：`python tools/onboard-lane-02-extra.py prepare --ids {id}`、`build --ids {id}`，然后使用 `review-lane-02.html?id={id}` 截取对应正背侧视，运行 `verify --ids {id}`。依赖见 `tools/requirements-model.txt`。\n\n限制：\n\n"+'\n'.join('- '+v for v in plan['limitations'])+'\n')
    return report

def main():
    parser = argparse.ArgumentParser(); parser.add_argument('command', choices=['prepare','build','verify','submit']); parser.add_argument('--count', type=int, default=3); parser.add_argument('--ids', nargs='*'); args = parser.parse_args()
    if args.command=='submit':submission();return
    products = [p for p in json.loads((sources.RUN/'queue.json').read_text())['products'] if p['priority'] in PRIORITIES]
    if args.ids: products = [p for p in products if 'm5stack-'+p['directory'].split('_')[0].lower() in args.ids]
    else: products = products[:args.count]
    for product in products:
        if args.command == 'prepare': sources.prepare(product)
        elif args.command == 'verify':
            id='m5stack-'+product['directory'].split('_')[0].lower()
            report=sources.verify_product(id)
            report['reviewScriptSha256']=sources.sha(Path('web/platform/review-lane-02.js').read_bytes())
            report['laneBuildScriptSha256']=sources.sha(Path(__file__).read_bytes())
            finish_review(id,report)
        else:
            id='m5stack-'+product['directory'].split('_')[0].lower()
            prepared=json.loads((sources.TEMP/id/'prepared.json').read_text())
            output=Path('web/platform/assets')/id
            plan=json.loads((output/'model-plan.json').read_text())
            sources.build_model(prepared,plan,output)
            record=sources.build_knowledge(prepared,plan)
            if id=='m5stack-s013':
                for item in record['unknowns']:
                    if item['id']=='pcb-layout':item.update(id='pcb-electrical-stackup',label='Electrical PCB stackup and user revision',notes='Manufacturer mechanical board/component STEP is available and retained. Copper stackup/electrical behavior and the user hardware revision are not established by that model.')
                sources.write(Path('web/platform/knowledge')/(id+'.json'),record)
            print(id+' built',flush=True)

if __name__ == '__main__': main()
