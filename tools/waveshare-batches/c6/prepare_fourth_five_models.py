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
    if tree.exists():
        data=tree.read_bytes();t=json.loads(data)
        rec=({'url':'https://github.com/waveshareteam/'+repo+'/tree/'+t['sha'],'bytes':len(data),'sha256':sha(data),'fetchedAt':t['fetchedAt'],'captureKind':'derived-git-ls-tree','sourceMethod':t['sourceMethod'],'commit':t['sha']} if 'sourceMethod'in t else json.loads(Path(str(tree)+'.receipt.json').read_text()))
        sources.append(source(dst,tree,'repository-tree','repository-tree.json',receipt=rec,compress=True))
    else:
        # This product's official resource page exposes ZIPs, not a repository.
        import zipfile
        arc=S/repo/'files/official-resources/ESP32-C6-LCD-1.69-Demo.zip'
        rec=json.loads(Path(str(arc)+'.receipt.json').read_text())
        with zipfile.ZipFile(arc)as z:entries=[{'path':i.filename,'bytes':i.file_size,'crc32':i.CRC}for i in z.infolist()]
        t={'sha':None,'tree':entries,'method':'derived-ZIP-central-directory','source':rec}
        raw=(json.dumps(t,indent=2)+'\n').encode();out=dst/'archive-inventory.json';out.write_bytes(raw)
        sources.append({'id':'archive-inventory','file':out.name,'url':rec['url'],'bytes':len(raw),'sha256':sha(raw),'fetchedAt':rec['fetchedAt'],'captureKind':'derived-archive-member-inventory','role':'reference-only','rights':RIGHTS,'notes':'Derived ZIP central directory; not a Git tree or HTTP response.'})
    # Storefront bytes contain anonymous session fields and remain in scratch.
    # This explicit derived SKU selection never claims to be an HTTP body.
    store=json.loads((R/'products'/((listing or repo.lower())+'.json')).read_text())
    selection={k:store[k] for k in ['listingId','url','source','variants','skuInit','exactSkuExtraction','optionLabels','singleSku','partNumber'] if k in store}
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


def add_all_photos(dst,sources,repo):
    for i,f in enumerate(sorted((S/repo/'images').glob('*.webp'))):
        if any(k in f.name.lower()for k in ['size','0.96inch_lcd']):continue
        sources.append(source(dst,f,'photo-'+str(i),'photo-'+str(i)+'.webp'))


# Exact latest native 1.43 STEP: preserve every topology while batching equal
# native material groups. The front optical stack and each control stay separate.
repo='ESP32-C6-Touch-AMOLED-1.43'
for case in [False,True]:
    pid='waveshare-esp32-c6-touch-amoled-1-43'+('-b'if case else'')
    dst,sources,t=common(pid,repo);p=basics(pid,sources,t);adir=S/repo/'files/official-resources'
    arc=adir/'ESP32-C6-Touch-AMOLED-1.43-20250915.zip';sources.append(source(dst,arc,'official-archive','source-archive.zip','cad-container'))
    rec=json.loads((adir/'bare-latest.stp.receipt.json').read_text());s=source(dst,adir/'bare-latest.stp','official-cad','source.stp','cad-source',compress=True)
    s.update(origin='zip-member',containerSourceId='official-archive',archive={'entry':rec['archiveMember'],'containerSha256':rec['containerSha256']});sources.append(s)
    picture(dst,sources,repo,'*ProductSize-1-*.webp','dimension-photo','dimension-photo.webp')
    picture(dst,sources,repo,'*ProductSize-2-*.webp','case-dimension-photo','case-dimension-photo.webp')
    add_all_photos(dst,sources,repo)
    top=json.loads((adir/'topology.json').read_text());colors=json.loads((adir/'topology-names.json').read_text());assert len(top)==689
    names={0:'pcb-main',87:'tf-socket',282:'boot-key',283:'power-key',284:'bottom-connector-left',285:'bottom-connector-right',641:'usb-shell',668:'header-right',669:'header-left',670:'battery-connector',671:'speaker-connector',675:'front-cover',676:'emissive-stack-flex',677:'optical-adhesive',678:'optical-frame'}
    groups={}
    for i in range(689):
        rgb=colors[i].get('color');color='#'+''.join(f'{round(255*(12.92*x if x<=.0031308 else 1.055*x**(1/2.4)-.055)):02x}'for x in rgb)if rgb else'#a9a9a9'
        key=(names.get(i),color)if i in names else(None,color);groups.setdefault(key,[]).append(i)
    for (name,color),indices in groups.items():
        part={'name':name or'native-material-'+color.removeprefix('#'),'sourceFile':'source.stp.gz','sourceComponents':indices,'sourceTopology':'solid-or-shell','color':color,'role':'display-stack'if any(i in [675,676,677,678]for i in indices)else'component'}
        if name in ['boot-key','power-key'] and not case:part['controlId']='boot'if name=='boot-key'else'power'
        p['parts'].append(part)
    p['method']='manufacturer-cad';p['sourceTopologyCounts']={'source.stp.gz':689};p['sourceCoverage']={'source.stp.gz':{'includedIndices':list(range(689)),'omittedIndices':[],'grouping':'689 unscaled native components grouped only by identical retained material; mechanical/optical controls stay separate. Every source index is exported and checked.'}}
    p['sourceSearch']={'status':'official-native-STEP-found','inspected':['overview','resources','repository-tree','official-archive'],'repositoryCommit':t['sha'],'archiveMember':rec['archiveMember'],'result':'Latest exact September2025 native board assembly retained. Case-B older archive was separately inspected and contains PDF/DWG, no solid case assembly.'}
    p['materialBasis']='Native linear colors converted to sRGB; unsupplied materials neutral grey. No generated silkscreen.'
    p['assemblyNotes']='Native source coordinates and millimetres retained. Front+Z, USB at-Y. No scaling or flattening; inspection offsets only.'
    p['dimensions']={'display':{'nativeEmissiveDiameterMm':36.35,'nativeEmissiveFace':'component676 face70 z=-0.5','nativeViewingDiameterMm':36.75,'nativeViewingFace':'component675 face8 z=0.58','coverDiameterMm':46,'sourceIds':['official-cad','dimension-photo']},'pcb':{'nativeSizeMm':top[0]['size'],'sourceComponent':0,'sourceIds':['official-cad']}}
    p['presentation']={'screen':{'shape':'circle','sizeMm':[36.35,36.35],'positionMm':[0,0,.608],'basis':'Native emissive36.35 diameter projected above native36.75 viewing window; offset only for live framebuffer.'},'inspectionGroups':[{'id':'native-optics','parts':['front-cover','emissive-stack-flex','optical-adhesive','optical-frame','live-display'],'offsetMm':[56,0,17],'sourceIds':['official-cad'],'notes':'Whole optical source parts move together; not operating spacing.'}]}
    p['validation']={'display':{'width':466,'height':466},'controls':[] if case else[{'id':'boot','type':'button'},{'id':'power','type':'button'}],'controlViewNormals':{}if case else{'boot':[-1,0,0],'power':[1,0,0]},'ledIndices':[],'screenAspectRelativeTolerance':.001}
    p['dimensionTargets']=[{'part':'screen','sizeMm':[36.35,36.35,None],'toleranceMm':.001,'sourceId':'official-cad'}]
    p['limitations']=['Native source preserves every689 topology;24 original/fallback colors are grouped for rendering, not omitted or geometrically merged.','Native emissive36.35 mm differs from36.75 mm cover viewing aperture. The latter is not relabelled active area.','Native total height11.8003 mm includes headers; current photograph9.60 mm differs. Source is not rescaled; revision/measurement scope remains unresolved.','Bare native switch282 maps to BOOT and283 to PWR from the unchanged rear photo and USB/connector frame. Case actuators have no retained3D geometry and remain workbench-only controls.','No physical assembly fit, calibrated optical appearance, touch registration or browser WebGL acceptance claim.']
    if case:
        outer=adir/'ESP32-C6-Touch-AMOLED-1.43-3dFile.zip';sources.append(source(dst,outer,'case-archive','case-source.zip','cad-container'))
        r=json.loads((adir/'case.rar.receipt.json').read_text());ss=source(dst,adir/'case.rar','case-rar','case.rar','cad-container');ss.update(origin='zip-member',containerSourceId='case-archive',archive={'entry':r['archiveMember'],'containerSha256':r['containerSha256']});sources.append(ss)
        r=json.loads((adir/'case-drawing.pdf.receipt.json').read_text());ss=source(dst,adir/'case-drawing.pdf','case-drawing','case-drawing.pdf','dimension-reference');ss.update(origin='rar-member',containerSourceId='case-rar',archive={'entry':r['archiveMember'],'containerSha256':r['containerSha256']});sources.append(ss)
        p['derivedParts']=[plate('case-front-rim',circle(24),.7,.25,'#1b1d1f',[circle(23.05)],'shell'),plate('case-wall',circle(24),10.9,-5.55,'#1b1d1f',[circle(23.1)],'shell'),plate('case-rear',circle(24),.5,-11.45,'#202326',[rect(38,16),[[x,y+14.5]for x,y in rect(24,7)]],'shell')]
        for d in p['derivedParts']:d['sourceIds']=['case-drawing','case-dimension-photo'];d['basis']='Published48 mm case diameter and12.3 mm overall depth. Wall/rim thickness and broad debug openings are explicit estimates; original bare CAD remains intact.'
        p['dimensions']['enclosure']={'outerSizeMm':[48,48,12.3],'bodyDepthMm':11.6,'wallThicknessMm':None,'sourceIds':['case-drawing','case-dimension-photo']}
        p['validation']['shellNames']=['case-front-rim','case-wall','case-rear']
        p['presentation']['inspectionGroups'].append({'id':'reconstructed-case','parts':['case-front-rim','case-wall','case-rear'],'offsetMm':[-58,0,-12],'sourceIds':['case-drawing'],'notes':'Illustrative case separates from exact native board; unmeasured case sockets/supports/clearances remain UNKNOWN.'})
        p['limitations']+=['Case-B is a hybrid: exact bare native board plus exterior dimension reconstruction. The official case archive contains2D PDF/DWG only. Shell wall/rim/back thickness, port openings, support bosses and registration are UNKNOWN; fit is not claimed.','Broad rear debug openings intentionally reveal native headers/connectors; they are not measured production cutouts. Side openings/key actuators are reference-only, and shell placement may overlap the native USB until a solid case source is available.']
    finish(dst,p)

# Enclosed watch: source-backed exterior dimensions; no invented inner PCB.
repo='ESP32-C6-Touch-AMOLED-2.06';pid='waveshare-esp32-c6-touch-amoled-2-06'
dst,sources,t=common(pid,repo);p=basics(pid,sources,t)
picture(dst,sources,repo,'*details-size*.webp','dimension-photo','dimension-photo.webp');add_all_photos(dst,sources,repo)
body=[42,50.8];screen=[33.09,40.51];depth=12.9
p['parts']=[{'name':'optical-face','role':'display-stack','color':'#101519','geometry':{'shape':'box','sizeMm':[33.3,40.7,.7],'centerMm':[0,0,-.35]},'sourceIds':['dimension-photo'],'basis':'Published optical XY window;0.7 mm stack thickness is illustrative.'}]
p['derivedParts']=[plate('front-bezel',rect(*body,11),1.2,-.6,'#191d20',[rect(*screen,9.2)],'shell'),plate('case-wall',rect(*body,11),depth-2.4,-depth/2,'#191d20',[rect(39.6,48.4,9.8)],'shell'),plate('rear-cover',rect(*body,11),1.2,-depth+.6,'#202326',role='shell'),{'id':'pcb-main','kind':'pcb-placeholder','color':'#62747c','geometry':{'sizeMm':[34,42,1.6],'centerMm':[0,0,-6.45],'labelSide':'front','labelSizeMm':[30,5]},'basis':'Hidden PCB geometry, battery, traces and supports UNKNOWN. This visible inspection placeholder is not manufacturer CAD.','sourceIds':['resources','dimension-photo']}]
p['dimensions']={'enclosure':{'outerSizeMm':body+[depth],'overallDepthIncludingActuatorsMm':13.6,'strapWidthMm':22,'wallThicknessMm':None,'sourceIds':['dimension-photo']},'display':{'visibleSizeMm':screen,'cornerRadiusMm':9.2,'sourceIds':['dimension-photo']}}
p['presentation']={'screen':{'shape':'rectangle','sizeMm':screen,'positionMm':[0,0,.008],'cornerRadiusMm':9.2},'inspectionGroups':[{'id':'exterior-optics','parts':['front-bezel','optical-face','live-display'],'offsetMm':[59,0,15],'sourceIds':['dimension-photo'],'notes':'Reconstructed exterior moves aside to reveal UNKNOWN inner PCB.'}]}
p['validation']={'display':{'width':410,'height':502},'controls':[],'ledIndices':[],'shellNames':['front-bezel','case-wall','rear-cover'],'unknownPcbInspection':[{'label':'pcb-main-unknown-front'}],'screenAspectRelativeTolerance':.001}
p['dimensionTargets']=[{'part':'front-bezel','sizeMm':body+[None],'toleranceMm':.001,'sourceId':'dimension-photo'},{'part':'screen','sizeMm':screen+[None],'toleranceMm':.001,'sourceId':'dimension-photo'}]
p['limitations']+=['Official resource dimensions PDF is named ESP32-S3; it is not promoted to an exact C6 solid assembly. Exact C6 current photographs supply this exterior reference.','12.9 mm casing body and13.6 mm overall actuator envelope are distinguished. Strap/buckle, wall thickness, unseen PCB and connectors are UNKNOWN; no fabricated production apertures or guessed3D buttons.','R9.2 optical window is published. Exterior corner radius11 mm is an illustrative reconstruction estimate.']
finish(dst,p)

# Bare 1.69 variants: retain precise published XY dimensions and render the
# board/optical separation with explicitly unknown thickness and inner layout.
for touch in [True,False]:
    repo='ESP32-C6-'+('Touch-'if touch else'')+'LCD-1.69';pid='waveshare-'+repo.lower().replace('.','-')
    dst,sources,t=common(pid,repo);p=basics(pid,sources,t);picture(dst,sources,repo,'*size*.webp','dimension-photo','dimension-photo.webp');add_all_photos(dst,sources,repo)
    pcb=[29.83 if touch else 30,37.12];screen=[27.972,32.634];glass=[33.13,41.13]if touch else[30,37.12]
    p['parts']=[{'name':'optical-stack','role':'display-stack','color':'#13191b','geometry':{'shape':'box','sizeMm':glass+[1.5],'centerMm':[0,0,.75]},'sourceIds':['dimension-photo'],'basis':'Touch cover33.13×41.13 mm is published; bare no-touch cover bounds are UNKNOWN and shown as a board-sized debug envelope.1.5 mm optical depth is illustrative.'}]
    p['derivedParts']=[{'id':'pcb-main','kind':'pcb-placeholder','color':'#38566a','geometry':{'sizeMm':pcb+[1.6],'centerMm':[0,0,-.8],'labelSide':'both','labelSizeMm':[24,5]},'basis':'Published board XY dimensions; actual stack thickness, IC heights, connector internals and hole diameters UNKNOWN. Schematic determines electrical parts, not their3D positions.','sourceIds':['dimension-photo','resources']}]
    p['dimensions']={'pcb':{'sizeMm':pcb+[None],'illustrativeThicknessMm':1.6,'sourceIds':['dimension-photo']},'display':{'visibleSizeMm':screen,'coverSizeMm':glass if touch else None,'sourceIds':['dimension-photo']}}
    p['presentation']={'screen':{'shape':'rectangle','sizeMm':screen,'positionMm':[0,0,1.508],'cornerRadiusMm':3},'inspectionGroups':[{'id':'optical-stack','parts':['optical-stack','live-display'],'offsetMm':[45,0,13],'sourceIds':['dimension-photo'],'notes':'Display separates from a visibly UNKNOWN PCB; no copied layout or hidden components.'}]}
    p['validation']={'display':{'width':240,'height':280},'controls':[],'ledIndices':[],'unknownPcbInspection':[{'label':'pcb-main-unknown-front'}],'screenAspectRelativeTolerance':.001}
    p['dimensionTargets']=[{'part':'screen','sizeMm':screen+[None],'toleranceMm':.001,'sourceId':'dimension-photo'}]
    p['limitations']+=['Published photograph is landscape. An explicit90-degree presentation choice gives27.972×32.634 mm for the240×280 portrait framebuffer; physical scan/touch orientation is not accepted from that aspect alone.','All source images remain unchanged in the reference gallery. The UNKNOWN board is a debugging placeholder, not a flat claim to reproduce photographed chips/connectors or manufacture-ready mechanics.','Glass/PCB separation, stack thickness and3 mm live-window corner radius are illustrative. BOOT/PWR remain exact workbench GPIO controls; no unmeasured3D actuator targets.']
    finish(dst,p)
