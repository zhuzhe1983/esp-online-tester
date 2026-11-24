"""Prepare reviewed source plans; source images are retained byte-for-byte."""
from pathlib import Path
import copy, datetime, gzip, hashlib, json, math, shutil
from PIL import Image

import argparse
_parser=argparse.ArgumentParser(description=__doc__);_parser.add_argument('--sources',type=Path,required=True);_args=_parser.parse_args()
R=_args.sources; S=R/'c6-sources'
ROOT=Path(__file__).resolve().parents[3]; A=ROOT/'web/platform/assets'
RIGHTS='Manufacturer source rights retained. Public availability is not evidence of an MIT redistribution licence. Original photographs are unchanged. Model code is repository code; referenced assets retain their original rights.'
NOW=datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z')
def sha(b): return hashlib.sha256(b).hexdigest()
def save(p,d): p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
def source(dst,path,id,file=None,role='reference-only',receipt=None,compress=False):
    b=path.read_bytes(); file=file or path.name
    rec=receipt if receipt is not None else json.loads(Path(str(path)+'.receipt.json').read_text())
    assert len(b)==rec['bytes'] and sha(b)==rec['sha256'],path
    out={**rec,'id':id,'file':file+('.gz' if compress else ''),'role':role,'rights':RIGHTS}
    out['url']=rec.get('url',rec.get('requestedUrl',rec.get('sourceUrl')))
    assert out['url']
    if compress:
        gz=gzip.compress(b,mtime=0);(dst/out['file']).write_bytes(gz);out['retained']={'encoding':'gzip','bytes':len(gz),'sha256':sha(gz)}
    else: (dst/file).write_bytes(b)
    if path.suffix.lower() in ['.jpg','.png','.webp']: out['imagePixels']=list(Image.open(path).size)
    return out
def common(pid,repo,listing=None):
    dst=A/pid;dst.mkdir(parents=True,exist_ok=True);sources=[]
    for suffix,id in [('', 'overview'),('__Resources-And-Documents','resources')]:
        meta=json.loads((R/'docs'/(repo+suffix+'.json')).read_text());sources.append(source(dst,R/'docs'/(repo+suffix+'.html'),id,id+'.html',receipt=meta['source'],compress=True))
    tree=S/repo/'git-tree.json'
    if not tree.exists():tree=S/repo/'tree.json'
    data=tree.read_bytes();t=json.loads(data)
    rec=({'url':'https://github.com/waveshareteam/'+repo+'/tree/'+t['sha'],'bytes':len(data),'sha256':sha(data),'fetchedAt':t['fetchedAt'],'captureKind':'derived-git-ls-tree','sourceMethod':t['sourceMethod'],'commit':t['sha']} if 'sourceMethod'in t else json.loads(Path(str(tree)+'.receipt.json').read_text()))
    sources.append(source(dst,tree,'repository-tree','repository-tree.json',receipt=rec,compress=True))
    # Storefront bytes contain anonymous session fields and remain in scratch.
    # This explicit derived SKU selection never claims to be an HTTP body.
    store=json.loads((R/'products'/((listing or repo.lower())+'.json')).read_text())
    selection={k:store[k] for k in ['listingId','url','source','variants','skuInit','exactSkuExtraction','optionLabels']}
    raw=(json.dumps(selection,ensure_ascii=False,indent=2)+'\n').encode();p=dst/'sku-selection.json';p.write_bytes(raw)
    sources.append({'id':'product','file':p.name,'url':store['url'],'bytes':len(raw),'sha256':sha(raw),'captureKind':'derived-storefront-field-selection','fetchedAt':store['source']['fetchedAt'],'role':'reference-only','rights':RIGHTS,'notes':'Selected public SKU fields and source receipt; not original storefront HTTP response bytes.'})
    return dst,sources,t
def picture(dst,sources,repo,glob,id,file=None,role='reference-only'):
    matches=list((S/repo/'images').glob(glob));assert len(matches)==1,(repo,glob,matches)
    p=matches[0];s=source(dst,p,id,file or(id+p.suffix),role);sources.append(s);return s
def basics(pid,sources,t):
    return {'schemaVersion':1,'id':pid,'method':'dimension-photo-reconstruction','sources':sources,'matrixRows':[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]],'sourceTopologyCounts':{},'parts':[],'derivedParts':[],'dimensions':{},'presentation':{},'dimensionTargets':[],'validation':{'controls':[],'ledIndices':[]},'sourceSearch':{'status':'no-matching-CAD-in-recorded-scope','inspected':['overview','resources','product','repository-tree'],'repositoryCommit':t['sha'],'repositoryTreeTruncated':False,'repositoryTreeEntries':len(t['tree']),'result':'Canonical resources and the complete pinned official repository tree were inspected for CAD/STEP/mechanical files. No matching solid assembly was exposed in this recorded scope. This is not a global assertion that no CAD exists.'},'materialBasis':'Published product photographs and explicit reconstruction estimates; unseen structures remain UNKNOWN.','assemblyNotes':'Front points toward +Z. Mechanical reconstruction is in millimetres; uncertain depth and hidden structures are not manufacturer measurements.','limitations':['This model is a dimension/photo reconstruction, not manufacturer CAD.','Unmeasured depths, corner radii and connector contact geometry are explicit illustrative estimates; no fit, drop-in enclosure or hardware acceptance claim.','Source photographs are unchanged. Any display artwork shown in them is reference only; the live screen receives the simulator framebuffer.'],'referencePhotos':[]}
def rect(w,h,r=0,n=12):
    if not r:return [[-w/2,-h/2],[w/2,-h/2],[w/2,h/2],[-w/2,h/2]]
    pts=[]
    for cx,cy,start in [(w/2-r,h/2-r,0),(-w/2+r,h/2-r,90),(-w/2+r,-h/2+r,180),(w/2-r,-h/2+r,270)]:
        for i in range(n+1):
            a=math.radians(start+i*90/n);pts.append([cx+r*math.cos(a),cy+r*math.sin(a)])
    return pts
def circle(r,n=96,c=(0,0)):return [[c[0]+r*math.cos(2*math.pi*i/n),c[1]+r*math.sin(2*math.pi*i/n)]for i in range(n)]
def plate(id,outline,depth,z,color,holes=None,role=None):
    d={'id':id,'kind':'plate','color':color,'geometry':{'outlineMm':outline,'holesMm':holes or[],'depthMm':depth,'centerMm':[0,0,z]},'basis':'Published dimensions where stated; unmeasured depth explicitly estimated.','sourceIds':['dimension-photo']}
    if role:d['role']=role
    return d
def photo_part(name,outline,z,file,cx,cy,scale,holes=None):
    return {'name':name,'role':'photo-appearance','color':'#253e5f','geometry':{'shape':'photo-face','outlineMm':outline,'holesMm':holes or[],'zMm':z,'back':True},'texture':{'file':file,'projection':'xy','side':'back','boundsMm':[(cx-800)/scale,(cy-800)/scale,cx/scale,cy/scale],'minNormalZ':.99},'basis':'Unchanged original 800×800 rear photograph projected onto a photo-derived XY envelope. No component height or hidden layout is asserted.','sourceIds':['back-photo']}
def finish(dst,p):
    p.setdefault('photoRegistration',{})
    for group in p.get('presentation',{}).get('inspectionGroups',[]):group.setdefault('sourceIds',['dimension-photo'])
    p['referencePhotos']=[{'sourceId':s['id'],'file':s['file'],'views':'Original manufacturer image, unchanged.'}for s in p['sources']if'imagePixels'in s]
    (dst/'SOURCE-RIGHTS.txt').write_text(RIGHTS+'\n');save(dst/'source-manifest.json',{'generatedAt':NOW,'sources':p['sources']});save(dst/'model-plan.json',p)
    print(p['id'],flush=True)

# Exact C6 native STEP: independently retained, line endings alone differ from C5.
repo='ESP32-C6-Touch-LCD-2.8';pid='waveshare-esp32-c6-touch-lcd-2-8';dst,sources,t=common(pid,repo)
old=A/'waveshare-esp32-c5-touch-lcd-2-8';new=S/repo/'files/hardware/mechanical'/f'{repo}.stp'
oldbytes=gzip.decompress((old/'source.stp.gz').read_bytes());newbytes=new.read_bytes()
assert oldbytes.decode().splitlines()==newbytes.decode().splitlines()
sources.append(source(dst,new,'official-cad','source.stp','cad-source',compress=True))
sources.append(source(dst,new.with_suffix('.pdf'),'mechanical-drawing','mechanical.pdf','dimension-reference'))
sources.append(source(dst,S/repo/'files/hardware/schematic'/f'{repo}.pdf','schematic','schematic.pdf',compress=True))
picture(dst,sources,repo,'*-2.jpg','front-photo','front-photo.jpg')
picture(dst,sources,repo,'*-3.jpg','back-photo','back-photo.jpg')
picture(dst,sources,repo,'*-4.jpg','angle-photo','angle-photo.jpg')
picture(dst,sources,repo,'C6-2.8-SIZE-*.webp','dimension-photo','dimension-photo.webp')
p=json.loads((old/'model-plan.json').read_text());p=json.loads(json.dumps(p).replace('waveshare-esp32-c5','waveshare-esp32-c6').replace('ESP32-C5','ESP32-C6').replace('GPIO28','GPIO9').replace('SKU34957','SKU35065').replace('SKU34958','SKU35066').replace('explicit C5 assembly','published C6 assembly').replace('actual C5 module','actual C6 module'))
p['id']=pid;p['sources']=sources;p.pop('photoRegistration',None)
p['sourceSearch']={'status':'official-fixed-commit-STEP-found','inspected':['overview','resources','product','repository-tree'],'repositoryCommit':t['sha'],'repositoryTreeEntries':len(t['tree']),'repositoryTreeTruncated':False,'result':'Exact C6 source STEP and mechanical PDF downloaded from the pinned official tree. All 728 native components retain their original millimetre coordinates. Canonical resource links use an older directory layout.'}
p['sourceReuseVerification']={'c6RawSha256':sha(newbytes),'priorGeometrySourceSha256':sha(oldbytes),'linewiseIdentical':True,'difference':'CRLF versus LF only','scope':'Reuse is limited to native mechanical component grouping and renderer annotations; C6 electrical configuration and sources are independent.'}
def replace_refs(v):
    if isinstance(v,dict):
        return {k:([('back-photo' if x=='component-photo' else x)for x in val]if k=='sourceIds'else replace_refs(val))for k,val in v.items()}
    if isinstance(v,list):return [replace_refs(x)for x in v]
    return v
p=replace_refs(p);p['limitations'].append('The independently downloaded exact C6 STEP is linewise identical to the preceding C5 mechanical export; only line endings differ. This does not equate their chips, GPIO routes or firmware. C6 board-specific source and simulation are recorded separately.')
finish(dst,p)

# Distinct round bare-board shapes and distinct -B enclosures.
for touch in [True,False]:
    repo='ESP32-C6-'+('Touch-'if touch else'')+'LCD-1.28';base='waveshare-'+repo.lower().replace('.','-')
    for case in [False,True]:
        pid=base+('-b'if case else'');dst,sources,t=common(pid,repo);p=basics(pid,sources,t)
        dim=picture(dst,sources,repo,repo+('-B'if case else'')+'-ProductSize-*.webp','dimension-photo','dimension-photo.webp')
        back=picture(dst,sources,repo,('esp32-c6-touch-lcd-1.28-4_1.jpg'if touch else'esp32-c6-lcd-1.28-3.jpg'),'back-photo','back-photo.jpg')
        picture(dst,sources,repo,('esp32-c6-touch-lcd-1.28-5_1.jpg'if touch else'esp32-c6-lcd-1.28-4.jpg'),'angle-photo','angle-photo.jpg')
        p['validation']['display']={'width':240,'height':240};p['presentation']['screen']={'shape':'circle','sizeMm':[32.512,32.512],'positionMm':[0,0,.01]}
        p['dimensions']['display']={'nominalDiameterMm':32.512,'basis':'Nominal 1.28 inch diagonal ×25.4, round panel; exact active diameter not published in inspected sources.','sourceIds':['overview']}
        if case:
            depth=12.5 if touch else 11.5;body=44.2 if touch else 46;window=35.85 if touch else 33
            p['parts']=[{'name':'glass','role':'display-stack','color':'#0c1011','geometry':{'shape':'cylinder','radiusMm':window/2,'depthMm':.7,'centerMm':[0,0,-.35]},'basis':'Published circular viewing aperture; glass thickness .7 mm is an estimate.'}]
            p['derivedParts']=[plate('front-bezel',circle(24),2,-1,'#222527',[circle(window/2)],'shell'),plate('case-wall',circle(body/2),depth-2,-(depth+2)/2,'#202425',[circle(body/2-1.2)],'shell'),plate('rear-cover',circle(body/2),1,-depth+.5,'#222527',role='shell'),{'id':'pcb-main','kind':'pcb-placeholder','color':'#627680','geometry':{'sizeMm':[34,34,1.6],'shape':'disc','centerMm':[0,0,-depth/2],'labelSide':'front','labelSizeMm':[23,6]},'basis':'Hidden PCB fit and position inside this enclosure are UNKNOWN; 34 mm envelope is an explicit debug placeholder, not the photographed bare-board outline.','sourceIds':['resources','dimension-photo']}]
            p['dimensions']['enclosure']={'outerDiameterMm':48,'bodyDiameterMm':body,'overallDepthMm':depth,'viewingApertureDiameterMm':window,'sourceIds':['dimension-photo'],'wallThicknessMm':None}
            p['dimensionTargets']=[{'part':'front-bezel','sizeMm':[48,48,None],'toleranceMm':.001,'sourceId':'dimension-photo'}]
            p['presentation']['inspectionGroups']=[{'id':'outer-optics','parts':['front-bezel','glass','live-display'],'offsetMm':[60,0,15],'notes':'Debug separation only; reveals UNKNOWN interior.'}]
            p['validation']['shellNames']=['front-bezel','case-wall','rear-cover'];p['validation']['unknownPcbInspection']=[{'label':'pcb-main-unknown-front'}]
            p['limitations']+=['The -B exterior differs between touch and non-touch: only this exact SKU dimension drawing is used.','Rear openings, shell seams, actuators and internal PCB placement are not shown dimensionally in the retained -B drawing and remain unmodeled. Ordinary BOOT remains a runtime keyboard control; no invented 3D external button is bound.','The rear cover is an explicitly approximate closed visual envelope, not a claim that the actual enclosure lacks ports or openings.']
        else:
            r=18.75 if touch else 18.25;bottom=-20.74 if touch else-21.2;cy=383 if touch else 379;scale=425/(r*2)
            # Arc plus the photographed lower neck; only radius/overall height where stated are manufacturer dimensions.
            outline=[]
            for i in range(81):
                a=math.radians(-45+i*270/80);outline.append([r*math.cos(a),r*math.sin(a)])
            outline += [[-6.2,bottom],[6.2,bottom]]
            p['parts']=[photo_part('pcb-back-photo',outline,-4.61,'back-photo.jpg',400,cy,scale)]
            for id,x,y in [('boot',10.6,11.8),('reset',-10.4,11.8)]:
                part={'name':id+'-key','role':'component','color':'#b8b8ad','geometry':{'shape':'box','sizeMm':[2.8,2.8,.65],'centerMm':[x,y,-5.0]},'sourceIds':['back-photo'],'basis':'Photographed rear actuator position, ±0.8 mm XY; thickness estimate only.'}
                if id=='boot':part['controlId']='boot'
                p['parts'].append(part)
            p['derivedParts']=[plate('optical-cover',circle(r),.7,-.35,'#101519'),{'id':'pcb-main','kind':'pcb-placeholder','color':'#28517c','geometry':{'sizeMm':[2*r,2*max(r,abs(bottom)),1.6],'shape':'polygon','outlineMm':outline,'centerMm':[0,0,-3.8],'labelSide':'front','labelSizeMm':[25,5]},'basis':'Photo-derived PCB outline; unmeasured PCB thickness 1.6 mm, hidden inner layout UNKNOWN.','sourceIds':['dimension-photo','back-photo']}]
            p['dimensions']['pcb']={'outerRadiusMm':r,'overallHeightMm':39.49 if touch else None,'sourceIds':['dimension-photo'],'nominalVisualThicknessMm':1.6,'outlineUncertaintyMm':.8}
            p['presentation']['inspectionGroups']=[{'id':'optical-stack','parts':['optical-cover','live-display'],'offsetMm':[48,0,15],'notes':'Debug separation exposes UNKNOWN inner face, not physical disassembly instructions.'}]
            p['validation'].update(controls=[{'id':'boot','type':'button'}],controlViewNormals={'boot':[0,0,-1]},unknownPcbInspection=[{'label':'pcb-main-unknown-front'}])
            p['dimensionTargets']=[{'part':'optical-cover','sizeMm':[2*r,2*r,None],'toleranceMm':.05,'sourceId':'dimension-photo','scope':'Visual front-cover diameter follows the dimensioned PCB circle as an explicit estimate; not independently dimensioned glass.'}]
            p['limitations']+=['Bare touch and non-touch PCB shapes are distinct. The non-touch dimension image is rotated relative to its gallery; runtime preview consistently places USB at negative Y.','The rear photograph supplies component appearance only; no invented package heights, traces, hidden underside or multilayer stack are generated.','Front optical diameter follows the measured board circular envelope as a photographic estimate; exact front laminate dimensions remain unknown. USB, header and TF mechanics remain reference-only.']
        finish(dst,p)

# C6 LCD2.73: true dimensioned XY, unchanged exact rear photo, unknown hidden face.
repo='ESP32-C6-LCD-2.73';pid='waveshare-esp32-c6-lcd-2-73';dst,sources,t=common(pid,repo);p=basics(pid,sources,t)
picture(dst,sources,repo,'*details-size.jpg','dimension-photo','dimension-photo.jpg')
picture(dst,sources,repo,'*-4.jpg','back-photo','back-photo.jpg')
picture(dst,sources,repo,'*-5.jpg','angle-photo','angle-photo.jpg')
picture(dst,sources,repo,'*-3.jpg','front-photo','front-photo.jpg')
outline=rect(68,68,2.5);holes=[circle(1.1,32,(x,y))for x in [-31,31]for y in [-31,31]]
p['parts']=[photo_part('pcb-back-photo',outline,-6.41,'back-photo.jpg',399.5,398,6.4,holes)]
for id,x,y in [('prev',33,-18),('power',33,-25),('next',-33,-18),('boot',-33,-25)]:
    part={'name':id+'-key','role':'component','color':'#d9d8c9','geometry':{'shape':'box','sizeMm':[3,3.8,1.6],'centerMm':[x,y,-7.2]},'basis':'Rear photo-matched side actuator; ±1 mm XY and unmeasured depth.','sourceIds':['back-photo','angle-photo']}
    if id!='power':part['controlId']=id
    p['parts'].append(part)
p['derivedParts']=[plate('front-cover',outline,.8,-.4,'#101616'),plate('optical-body',rect(65.5,65.5,1.5),3,-2.3,'#a5a7a7'),{'id':'pcb-main','kind':'pcb-placeholder','color':'#252d2c','geometry':{'sizeMm':[68,68,1.6],'shape':'polygon','outlineMm':outline,'holesMm':holes,'centerMm':[0,0,-5.6],'labelSide':'front','labelSizeMm':[40,8]},'basis':'PCB nominal68 mm from62 mm mounting pitch+3 mm edges; hidden internal face and thickness UNKNOWN.','sourceIds':['dimension-photo','back-photo']}]
p['dimensions']={'cover':{'sizeMm':[68,68,None],'sourceIds':['dimension-photo']},'display':{'activeSizeMm':[48.96,48.96],'sourceIds':['dimension-photo']},'mounting':{'pitchMm':[62,62],'holeDiameterMm':2.2,'edgeOffsetMm':3,'sourceIds':['dimension-photo']},'depthMm':None}
p['presentation']={'screen':{'shape':'rectangle','sizeMm':[48.96,48.96],'positionMm':[0,0,.008]},'inspectionGroups':[{'id':'optical-stack','parts':['front-cover','optical-body','live-display'],'offsetMm':[80,0,15],'notes':'Debug separation exposes the UNKNOWN inner PCB face.'}]}
p['validation']={'display':{'width':320,'height':320},'controls':[{'id':k,'type':'button'}for k in ['prev','next','boot']],'controlViewNormals':{k:[0,0,-1]for k in ['prev','next','boot']},'ledIndices':[],'unknownPcbInspection':[{'label':'pcb-main-unknown-front'}]}
p['dimensionTargets']=[{'part':'front-cover','sizeMm':[68,68,None],'toleranceMm':.001,'sourceId':'dimension-photo'},{'part':'screen','sizeMm':[48.96,48.96,None],'toleranceMm':.001,'sourceId':'dimension-photo'}]
p['limitations']+=['The current official overview references a mechanical drawing but the retained resource page and complete fixed tree expose only the schematic. Exact mechanical-file availability remains unresolved.','Published front68×68 mm, active48.96×48.96 mm and62 mm mounting pitch/2.2 mm holes are used; depth, PCB thickness and radii are unmeasured estimates.','Separate battery is included in SKU35303 and omitted in SKU35302 (-EN); the preview represents the same board without inventing battery placement.','The exact rear photo supplies package appearance at a uniform6.4 px/mm registration; all package heights and connector geometry are unverified. Physical PWR is unbound; PREV4/NEXT5/BOOT9 are source-backed runtime controls.']
finish(dst,p)

# LCD1.54 / touch1.54 share the documented case envelope, not touch electronics.
repo='ESP32-C6-Touch-LCD-1.54';pid='waveshare-esp32-c6-lcd-1-54';dst,sources,t=common(pid,repo,'esp32-c6-lcd-1.54');p=basics(pid,sources,t)
picture(dst,sources,repo,'*details-size-*.webp','dimension-photo','dimension-photo.webp')
picture(dst,sources,repo,'*-5.jpg','back-photo','back-photo.jpg')
picture(dst,sources,repo,'*-3.jpg','front-photo','front-photo.jpg')
picture(dst,sources,repo,'*-4.jpg','angle-photo','angle-photo.jpg')
outer=rect(46,46,5.8);inner=rect(42,42,4)
p['parts']=[photo_part('rear-photo',outer,-22.51,'back-photo.jpg',400,402,8.35)]
for id,x in [('boot',-10),('power',0),('user',10)]:
    part={'name':id+'-key','role':'component','color':'#d9dbd9','geometry':{'shape':'box','sizeMm':[4.8,1.3,2.7],'centerMm':[x,23.4,-12.9]},'sourceIds':['dimension-photo','back-photo'],'basis':'Three top actuators at published10 mm spacing, depth from9.6 mm rear-edge reference; body size/radius estimated.'}
    if id!='power':part['controlId']=id
    p['parts'].append(part)
p['derivedParts']=[plate('case-wall',outer,20.5,-11.25,'#f2f2ed',[inner],'shell'),plate('front-frame',outer,1,-.5,'#f2f2ed',[rect(40.15,40.15,3)],'shell'),plate('rear-cover',outer,1,-22,'#f2f2ed',role='shell'),plate('display-glass',rect(40.15,40.15,3),.7,-.35,'#dadcd8'),{'id':'pcb-main','kind':'pcb-placeholder','color':'#648083','geometry':{'sizeMm':[38,38,1.6],'centerMm':[0,0,-7],'labelSide':'front','labelSizeMm':[28,6]},'sourceIds':['dimension-photo'],'basis':'Hidden board outline, placement and layer stack UNKNOWN. 38 mm square is an illustrative fit envelope only.'}]
p['presentation']={'screen':{'shape':'rectangle','sizeMm':[27.72,27.72],'positionMm':[0,0,.008]},'inspectionGroups':[{'id':'front-optics','parts':['front-frame','display-glass','live-display'],'offsetMm':[60,0,15],'notes':'Debug opening exposes UNKNOWN interior; not manufacturer assembly guidance.'}]}
p['dimensions']={'enclosure':{'sizeMm':[46,46,22.5],'cornerRadiusMm':5.8,'sourceIds':['dimension-photo']},'display':{'activeSizeMm':[27.72,27.72],'sourceIds':['overview'],'drawingWindowMm':[28.12,28.12],'scope':'Published active area is distinct from drawing window.'},'pcb':{'sizeMm':None,'basis':'unknown'}}
p['validation']={'display':{'width':240,'height':240},'controls':[{'id':'boot','type':'button'},{'id':'user','type':'button'}],'controlViewNormals':{'boot':[0,1,0],'user':[0,1,0]},'ledIndices':[],'shellNames':['case-wall','front-frame','rear-cover'],'unknownPcbInspection':[{'label':'pcb-main-unknown-front'}]}
p['dimensionTargets']=[{'part':'case-wall','sizeMm':[46,46,20.5],'toleranceMm':.001,'sourceId':'dimension-photo','scope':'20.5 mm wall span plus the1 mm front/back layers preserves published22.5 mm enclosure depth; wall thickness is estimated.'},{'part':'screen','sizeMm':[27.72,27.72,None],'toleranceMm':.001,'sourceId':'overview'}]
p['limitations']+=['All four official SKUs use the shared documented46×46×22.5 mm external enclosure. Touch is present only on34659/34661; sharing a mechanical model does not add touch to34658/34660.','The source gallery depicts the non-touch label. It is kept as the manufacturer shared-page reference and does not assert a shipping label for the touch variant.','Internal PCB, battery, microphone and speaker mechanics are UNKNOWN; no shell interior component is invented.','Rear speaker slots and side USB/TF openings are visible in photos but unmeasured. This bounded exterior reconstruction does not reproduce their fit or opening depth.','10 mm button spacing is dimensioned. BOOT9 and user18 are modeled; center PWR is a separate power circuit and remains visibly unbound.']
finish(dst,p)
