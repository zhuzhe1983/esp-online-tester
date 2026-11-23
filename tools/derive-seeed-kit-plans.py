#!/usr/bin/env python3
"""Compose exact Seeed EE kit identities as board + dimensioned external screen.

The deliberately separated layout is for inspecting firmware output. It does not
invent an enclosure, FPC routing, assembled mounting position or firmware support.
Inputs are committed manufacturer PCB plans and hash-pinned mechanical excerpts.
"""
import argparse,copy,hashlib,json
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[1];ASSETS=ROOT/'web/platform/assets';OWNER='seeed-epaper-panel-sources';SHARED=ASSETS/OWNER

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def shared(source,owner):
    value=copy.deepcopy(source)
    if value.get('file'):value['sharedAssetProductId']=owner
    return value

def derive(config):
    pid=config['id'];board=config['board'].lower();baseid='seeed-xiao-epaper-'+board
    base=json.loads((ASSETS/baseid/'model-plan.json').read_text());p=ASSETS/pid;p.mkdir(exist_ok=True)
    sku=config['panel']['sku'];parameters=next(x for x in json.loads((SHARED/'mechanical-dimensions.json').read_text()) if x['sku']==sku)
    active=parameters['activeAreaMm'];width,height,thickness=parameters['glassOutlineMm']
    panel_sources=[s for s in json.loads((SHARED/'source-manifest.json').read_text()) if s['id'] in (sku+'-mechanical',sku+'-photo','official-hub-config')]
    assert len(panel_sources)==3
    sources=[shared(s,baseid) for s in base['sources']]+[shared(s,OWNER) for s in panel_sources]
    for name,source_id in [('mechanical-dimensions.json','derived-mechanical-dimensions'),('kit-configurations.json','derived-kit-configurations')]:
        sources.append({'id':source_id,'file':name,'sharedAssetProductId':OWNER,'status':'derived','kind':'repository-curated-manufacturer-facts','bytes':(SHARED/name).stat().st_size,'sha256':sha(SHARED/name),'sourceIds':[sku+'-mechanical'] if name.startswith('mechanical') else ['official-hub-config'],'notes':'Repository-curated facts, not a raw HTTP body or a manufacturer assembly CAD.'})
    # The source board remains at 1:1 mm scale. Both objects translate rigidly to
    # a centered, separated inspection layout; there is no imaginary interconnect.
    boardwidth=base['dimensions']['width'];gap=12;panelx=-(boardwidth+gap)/2;boardx=(width+gap)/2
    plan=copy.deepcopy(base);plan.update(id=pid,method='manufacturer-pcb-with-dimensioned-screen-debug',sources=sources,matrixRows=np.eye(4).tolist())
    for part in plan['parts']:
        matrix=np.array(part.get('matrixRows',base['matrixRows']),float);matrix[0,3]+=boardx;part['matrixRows']=matrix.tolist()
    for part in plan['derivedParts']:part['geometry']['centerMm'][0]+=boardx
    plan['parts'] += [
      {'name':'panel-glass-backing','role':'component','color':'#555c55','geometry':{'shape':'box','sizeMm':[width,height,thickness],'centerMm':[panelx,0,0]},'basis':'manufacturer-datasheet-outline-with-stated-thickness-basis','sourceIds':[sku+'-mechanical'],'notes':parameters['thicknessBasis']+'; simplified rectangular panel, not a complete display-module CAD.'},
      {'name':'panel-debug-rim','role':'glass','color':'#d7d8cf','geometry':{'shape':'ring','outerSizeMm':[width,height],'innerSizeMm':active,'zRangeMm':[thickness/2,thickness/2+.03],'centerMm':[panelx,0,0]},'basis':'manufacturer-active-dimensions-debug-registration','sourceIds':[sku+'-mechanical'],'notes':parameters['activeOffsetBasis']}
    ]
    combined=[width+gap+boardwidth,max(height,base['dimensions']['height']),max(thickness,base['dimensions']['depth'])]
    plan['dimensions']={'width':combined[0],'height':combined[1],'depth':combined[2],'unit':'mm','basis':'explicit-separated-debug-layout','scope':'Combined preview layout only. Manufacturer board and glass XY dimensions remain separate and unscaled.'}
    plan['presentation']['bodySizeMm']=combined
    plan['presentation']['screen']={'sizeMm':active,'positionMm':[panelx,0,thickness/2+.05],'basis':'manufacturer-datasheet-active-area','sourceIds':[sku+'-mechanical'],'notes':'Actual framebuffer target. Window is centered for debugging; panel edge registration is not calibrated. No static photograph covers the running display.'}
    plan['presentation']['inspectionGroups'].insert(0,{'id':'external-display','parts':['panel-glass-backing','panel-debug-rim','live-display'],'offsetMm':[0,0,max(combined)*.3],'sourceIds':[sku+'-mechanical'],'notes':'Independent external panel inspection group; offset is a viewer convention.'})
    for photo in plan['referencePhotos']:photo['sharedAssetProductId']=baseid
    photo_source=next(s for s in panel_sources if s['id']==sku+'-photo')
    plan['referencePhotos'].append({'file':photo_source['file'],'sharedAssetProductId':OWNER,'sourceId':photo_source['id'],'sourceSha256':photo_source['sha256'],'view':'official-exact-panel-SKU-reference'})
    plan['assemblyNotes']='Explicit separated debug layout of '+board.upper()+' and exact panel SKU '+sku+'. Board parts translate rigidly by '+str(boardx)+' mm on X; source PCB dimensions are preserved. The external panel glass is at X='+str(panelx)+' mm. This is not a manufacturer assembled pose.'
    plan['sourceIdentity']={'targetProductId':pid,'boardSourceProductId':baseid,'boardSku':{'ee02':'100067144','ee03':'100080566','ee04':'100075670','ee05':'100057220'}[board],'panelSku':sku,'panelConfigId':config['panel']['id'],'configurationEvidence':'official-hub-config and exact-SKU datasheet/photo','sharedSourceScope':'PCB/manufacturer reference data only; no inherited geometry-review, browser, firmware or hardware acceptance','assembly':'separated-debug-layout'}
    plan['dimensionChecks'].append({'part':'external-display','basis':'manufacturer-datasheet','sourceIds':[sku+'-mechanical'],'manufacturerActiveAreaMm':active,'manufacturerGlassOutlineMm':parameters['glassOutlineMm'],'thicknessBasis':parameters['thicknessBasis'],'activeOffsetBasis':parameters['activeOffsetBasis'],'sourcePagesOneBased':parameters['sourcePagesOneBased']})
    plan['limitations']=[s for s in plan['limitations'] if not s.startswith('This SKU is a bare controller board')]
    plan['limitations'] += [
      'This exact kit choice uses board '+board.upper()+' and panel SKU '+sku+'. Bare-board product identity remains separate; same-source geometry does not establish cross-firmware compatibility.',
      'Board and panel are placed side by side for debugging. No case, battery, antenna, wire/FPC routing or mechanical mounting pose is invented.',
      'Panel active and glass XY dimensions come from the retained datasheet; the active-window offset is deliberately centered and not a calibrated reproduction of the unequal border margins.',
      'The display is a simple dimensioned glass/debug surface. Flexible-panel bending, adhesives, laminations and connector mating are not simulated. The FPC tail is omitted rather than converted to a solid extension.',
      'Official reference photographs may use a different rotation from the native framebuffer. The model uses the catalog/runtime native orientation.'
    ]
    if parameters.get('limitation'):plan['limitations'].append(parameters['limitation'])
    if parameters.get('outlineInterpretation'):plan['limitations'].append(parameters['outlineInterpretation'])
    plan['reviewViews']={view:{'position':position,'span':max(combined)*1.4} for view,position in [('front',[0,0,600]),('back',[0,0,-600]),('side',[600,0,0])]}
    write(p/'source-manifest.json',sources);write(p/'model-plan.json',plan)
    print(pid,flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('ids',nargs='*');args=parser.parse_args()
    configs=json.loads((SHARED/'kit-configurations.json').read_text())
    selected=[c for c in configs if not args.ids or c['id'] in args.ids]
    if args.ids:assert len(selected)==len(set(args.ids))
    for config in selected:derive(config)
