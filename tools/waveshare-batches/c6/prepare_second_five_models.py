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

# Native source topology, including the original unscaled PCB and complete optics.
def native(repo,pid,step,topdir,count,omit,names,screen,pixels,boot,pcb_dim):
    dst,sources,t=common(pid,repo);p=basics(pid,sources,t)
    src=S/repo/'files'/step;td=S/repo/'files'/topdir
    sources.append(source(dst,src,'official-cad','source.stp','cad-source',compress=True))
    picture(dst,sources,repo,'*Size*.webp'if repo.startswith('ESP32-C3')else'product_5-*.webp','dimension-photo','dimension-photo.webp')
    for n,id in [(2,'front-photo'),(3,'back-photo'),(4,'angle-photo')]:picture(dst,sources,repo,f'*-{n}.jpg',id,id+'.jpg')
    topology=json.loads((td/'topology.json').read_text());colors={x['index']:x.get('color')for x in json.loads((td/'topology-names.json').read_text())};assert len(topology)==count
    p['method']='manufacturer-cad';p['sourceTopologyCounts']={'source.stp.gz':count};p['sourceCoverage']={'source.stp.gz':{'includedIndices':[i for i in range(count)if i not in omit],'omittedIndices':omit,'grouping':'Every retained native topology is independently represented. No scaling, invented fillet or source-coordinate flattening.'}}
    def color(i):
        rgb=colors.get(i)
        if rgb:return '#'+''.join(f'{round(255*(12.92*x if x<=.0031308 else 1.055*x**(1/2.4)-.055)):02x}'for x in rgb)
        return '#121515'if i==0 else'#a9a9a9'
    for i in range(count):
        if i in omit:continue
        part={'name':names.get(i,f'native-{i:03d}'),'sourceFile':'source.stp.gz','sourceComponent':i,'sourceTopology':'solid-or-shell','role':'component','color':color(i)}
        if i==0:part['role']='display-stack'
        if i==boot:part['controlId']='boot'
        p['parts'].append(part)
    p['sourceSearch'].update(status='official-fixed-commit-STEP-found',result='Exact product native STEP found in the complete pinned official tree. Every selected topology retains original millimetre coordinates; omitted indices are enumerated per SKU.')
    p['materialBasis']='Native OCCT linear RGB converted to sRGB where present. Uncoloured optical component0 uses source-photo black; remaining uncoloured topology uses neutral grey, without pretending the source supplied a material.'
    p['assemblyNotes']='Native source coordinates preserved exactly; front is +Z. Optical flex tails keep their real source poses. Debug separation is not assembly spacing.'
    p['limitations']=['Native CAD is a manufacturer mechanical export, not an electrical schematic or production-revision guarantee.','No hardware-fit or browser WebGL acceptance claim. Unspecified CAD material uses a stated fallback, never a fabricated silkscreen or texture.','Live framebuffer is projected onto the native front viewing face; this rendering offset is not glass thickness.']
    p['presentation']={'screen':{'shape':'rectangle','sizeMm':screen[:2],'positionMm':[0,0,screen[2]],'basis':'Native optical face plus0.008 mm presentation offset; exact face indices and viewing/emissive distinction recorded in dimensions.'},'inspectionGroups':[{'id':'native-optics','parts':[names.get(0,'native-000')]+([names[1]]if 1 in names and 1 not in omit and 'optic'in names[1]else[])+(['front-frame']if'front-frame'in names.values()else[])+['live-display'],'offsetMm':[45,0,12],'sourceIds':['official-cad'],'notes':'Debug separation of entire native optical components, including flex; no implied operating distance.'}]}
    p['validation']={'display':{'width':pixels[0],'height':pixels[1]},'controls':[{'id':'boot','type':'button'}],'controlViewNormals':{'boot':[-1,0,0]},'ledIndices':[],'screenAspectRelativeTolerance':.001}
    p['dimensionTargets']=[{'part':'pcb-main','sizeMm':pcb_dim,'toleranceMm':.05,'sourceId':'dimension-photo','scope':'Published nominal dimensions versus unscaled original topology.'},{'part':'screen','sizeMm':screen[:2]+[None],'toleranceMm':.001,'sourceId':'official-cad'}]
    return dst,p

repo='ESP32-C3-LCD-1.47'
for header in [False,True]:
    pid='waveshare-esp32-c3-lcd-1-47'+('-m'if header else'')
    dst,p=native(repo,pid,'schematic/ESP32-C3-LCD-1.47.stp','schematic',184,[],{0:'cover-and-flex',1:'lcd-optical-stack',2:'pcb-main',108:'usb-shell',169:'reset-key',170:'boot-key',181:'tf-socket'},[17.39,32.35,1.408],[172,320],170,[20.32,36.37,.8])
    p['dimensions']={'display':{'activeSizeMm':[17.39,32.35],'lcmSizeMm':[19.39,36.28],'nativeActiveFace':'component0 face44 z1.400','nativeOuterBoundsIncludeFlex':True,'sourceIds':['official-cad','dimension-photo']},'pcb':{'nominalSizeMm':[20.32,36.37,.8],'nativeMeasuredSizeMm':[20.33725676,36.38725514,.7999996]},'headers':{'present':header,'countPerRow':9,'pitchMm':2.54,'rowCentersXmm':[-8.89,8.89],'pinCentersYmm':[-7.86964+2.54*i for i in range(9)],'sourceIds':['official-cad','back-photo'],'exactPinLengthMm':None}}
    p['limitations']+=['Published resolution172×320 conflicts with pinned BSP180×320 plus30-column gap. Runtime exposes the advertised172-pixel physical centre crop with34-column gap; side4 pixels from each BSP edge are outside the declared visible screen.','The photographed rear view is rotated180 degrees relative to the native USB-down front convention; native negative-X side switch is BOOT9.']
    if header:
        for x in [-8.89,8.89]:
            p['parts'].append({'name':'header-body-'+str(x),'role':'component','color':'#23262a','geometry':{'shape':'box','sizeMm':[2.54,22.86,2.5],'centerMm':[x,2.29036,-4.9]},'sourceIds':['official-cad','back-photo','product'],'basis':'Exact native through-hole centres and2.54 mm pitch;2.5 mm plastic thickness is an explicit estimate for presoldered SKU34664, not supplied CAD.'})
            for i in range(9):p['parts'].append({'name':f'header-pin-{x}-{i}','role':'component','color':'#bdac77','geometry':{'shape':'box','sizeMm':[.64,.64,9],'centerMm':[x,-7.86964+2.54*i,-7]},'sourceIds':['official-cad','product'],'basis':'Hole centre exact; pin section0.64 mm and9 mm length are illustrative UNKNOWN manufacturing dimensions, not measured.'})
        p['limitations'].append('The official STEP is the bare-board assembly. SKU34664-M adds source-positioned schematic header geometry; exact header body, section, length and plating remain UNKNOWN estimates. This subset is not manufacturer CAD for the added pins.')
    finish(dst,p)

repo='ESP32-C6-Touch-AMOLED-1.64'
for header in [False,True]:
    pid='waveshare-esp32-c6-touch-amoled-1-64'+('-m'if header else'')
    omit=[]if header else[1,2]
    dst,p=native(repo,pid,'hardware/dimensions/esp32-c6-touch-amoled-1_64.stp','hardware/dimensions',264,omit,{0:'cover-and-flex',1:'header-positive-x',2:'header-negative-x',3:'tf-socket',4:'pcb-main',184:'boot-key',185:'power-key',209:'usb-shell',257:'battery-connector',258:'front-frame'},[21.84,35.57,1.458],[280,456],184,[28.6,43.5,1.6])
    p['dimensions']={'display':{'nativeEmissiveSizeMm':[21.84,35.57],'nativeEmissiveFace':'component0 face143 z0.800','viewingSizeMm':[22.34,36.06],'nativeViewingFace':'component0 face37 z1.450','coverSizeMm':[26.74,41.62],'sourceIds':['official-cad','dimension-photo']},'outerFrame':{'sizeMm':[29.12,44,None],'sourceIds':['dimension-photo']},'headers':{'present':header,'nativeIndices':[1,2],'pitchMm':2.54,'sourceIds':['official-cad','dimension-photo']}}
    p['limitations']+=['The retained native assembly includes both presoldered rows; bare SKU34055 omits only native topologies1 and2. SKU34056-M retains all264 components.','The dimension image states22.34×36.06 mm viewing aperture. Framebuffer uses the independently extracted21.84×35.57 mm emissive rectangle, projected through that cover; the two dimensions are not conflated.','Physical PWR is retained visually but unbound: its analog/PMIC behaviour is outside the digital board subset. BOOT9 is the source-backed control.']
    finish(dst,p)

# Dimensioned enclosures: unchanged reference photographs, explicit UNKNOWN interiors.
for repo,pid,body,depth,radius,screen,sy,pixels in [
('ESP32-C6-LCD-0.85','waveshare-esp32-c6-lcd-0-85',[46,46],22.5,5.8,[16.21,16.21],0,[128,128]),
('ESP32-C6-ePaper-1.54','waveshare-esp32-c6-epaper-1-54',[39.8,53],16.9,4.5,[27.8,27.8],1.7,[200,200]),
('ESP32-C6-Touch-AMOLED-2.16','waveshare-esp32-c6-touch-amoled-2-16',[46,46],22.5,5.8,[38.99,38.99],0,[480,480])]:
    dst,sources,t=common(pid,repo);p=basics(pid,sources,t)
    picture(dst,sources,repo,'*details-size-*.webp'if'0.85'in repo else'*ProductSize-*.webp','dimension-photo','dimension-photo.webp')
    for n,id in [(2,'front-photo'),(3,'back-photo'),(4,'angle-photo')]:picture(dst,sources,repo,f'*-{n}.jpg',id,id+'.jpg')
    aperture=[x+.05 for x in screen];outline=rect(*body,radius);hole=[[x,y+sy]for x,y in rect(*aperture,.2)]
    p['parts']=[{'name':'optical-face','role':'display-stack','color':'#e0e0dc'if'ePaper'in repo else'#0d1214','geometry':{'shape':'box','sizeMm':[screen[0]+.5,screen[1]+.5,.7],'centerMm':[0,sy,-.35]},'sourceIds':['dimension-photo'],'basis':'Published screen aperture; optical thickness0.7 mm is an explicit estimate.'}]
    p['derivedParts']=[plate('front-bezel',outline,1.2,-.6,'#eeeae5',[hole],'shell'),plate('case-wall',outline,depth-2.4,-depth/2,'#f2efea',[rect(body[0]-2.4,body[1]-2.4,max(.5,radius-1.2))],'shell'),plate('rear-cover',outline,1.2,-depth+.6,'#eeeae5',role='shell'),{'id':'pcb-main','kind':'pcb-placeholder','color':'#667982','geometry':{'sizeMm':[body[0]-7,body[1]-8,1.6],'centerMm':[0,0,-depth/2],'labelSide':'front','labelSizeMm':[body[0]-12,6]},'basis':'Hidden PCB, placement, dimensions, components and traces are UNKNOWN; this debug envelope is deliberately not a claimed reconstruction.','sourceIds':['resources','dimension-photo']}]
    p['dimensions']={'enclosure':{'outerSizeMm':body+[depth],'cornerRadiusMm':radius,'sourceIds':['dimension-photo'],'wallThicknessMm':None},'display':{'activeOrViewingSizeMm':screen,'centerYmm':sy,'sourceIds':['dimension-photo'],'basis':'Dimensioned visible aperture; active-vs-viewing distinction not separately established.'}}
    p['presentation']={'screen':{'shape':'rectangle','sizeMm':screen,'positionMm':[0,sy,.008]},'inspectionGroups':[{'id':'exterior-optics','parts':['front-bezel','optical-face','live-display'],'offsetMm':[60,0,15],'notes':'Debug separation reveals UNKNOWN interior; not disassembly or engineering-fit instructions.'}]}
    p['validation']={'display':{'width':pixels[0],'height':pixels[1]},'controls':[],'ledIndices':[],'shellNames':['front-bezel','case-wall','rear-cover'],'unknownPcbInspection':[{'label':'pcb-main-unknown-front'}]}
    p['dimensionTargets']=[{'part':'front-bezel','sizeMm':body+[None],'toleranceMm':.001,'sourceId':'dimension-photo'},{'part':'screen','sizeMm':screen+[None],'toleranceMm':.001,'sourceId':'dimension-photo'}]
    p['limitations']+=['Separate battery is included in the base SKU and omitted in -EN. This shared exterior does not fabricate an internal battery location.','Rear label, connector openings, actuator travel and internal supports remain reference-only because no matching native assembly was found. The visual rear cover is an approximate envelope, not a claim that the product has no ports.','BOOT remains a runtime keyboard/UI control. No imprecisely located exterior actuator is exposed as an exact3D input target.']
    if'0.85'in repo or'2.16'in repo:
        outer=32 if'0.85'in repo else 43.3;inner=26 if'0.85'in repo else 38.99
        trim=plate('front-optical-trim',rect(outer,outer,3),.006,.003,'#9c9e9d'if'0.85'in repo else'#111515',[rect(inner,inner)],'display-stack')
        trim['basis']='Dimensioned outer/inner trim XY;0.006 mm is a rendering layer, not measured thickness. Undimensioned corner radius3 mm is an explicit photograph-based estimate.'
        p['derivedParts'].append(trim);p['presentation']['inspectionGroups'][0]['parts'].append('front-optical-trim')
    if'0.85'in repo:
        p['dimensions']['enclosure']['heightBasis']='46 mm from34 mm mounting pitch plus6 mm end offsets and square dimensioned product; no independent inner PCB measurement.'
        p['limitations'].append('32 mm outer and26 mm inner front trim follows the dimension drawing as a thin visual layer. Its depth profile and light-ring optics remain unknown; the trim is not a calibrated LED diffuser.')
    if'2.16'in repo:
        archive=S/repo/'files/official-resources/ESP32-C6-Touch-AMOLED-2.16-Struct.zip';sources.append(source(dst,archive,'official-structure-archive','structure.zip',compress=True))
        import io,zipfile
        z=zipfile.ZipFile(archive);members=[n for n in z.namelist()if not n.endswith('/')]
        p['sourceSearch'].update(status='official-2d-structure-archive-no-solid-assembly',result='Official C6 structure ZIP was fetched and inspected. It contains only S3-named DXF/PDF, no solid STEP or C6-specific board assembly; exact C6 dimension photograph is the geometric basis.',archiveMembers=members)
        p['limitations'].append('Official C6-linked archive internally names ESP32-S3-Touch-AMOLED-2.16-Dxf.dxf/pdf. Those original2D files are retained as evidence and are not relabelled or claimed as a C6 native solid assembly.')
    finish(dst,p)
