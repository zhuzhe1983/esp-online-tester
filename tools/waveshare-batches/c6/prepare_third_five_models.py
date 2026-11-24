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
        arc=S/repo/'files/official-resources/ESP32-C6-LCD-1.3-Structure.zip'
        rec=json.loads(Path(str(arc)+'.receipt.json').read_text())
        with zipfile.ZipFile(arc)as z:entries=[{'path':i.filename,'bytes':i.file_size,'crc32':i.CRC}for i in z.infolist()]
        t={'sha':None,'tree':entries,'method':'derived-ZIP-central-directory','source':rec}
        raw=(json.dumps(t,indent=2)+'\n').encode();out=dst/'archive-inventory.json';out.write_bytes(raw)
        sources.append({'id':'archive-inventory','file':out.name,'url':rec['url'],'bytes':len(raw),'sha256':sha(raw),'fetchedAt':rec['fetchedAt'],'captureKind':'derived-archive-member-inventory','role':'reference-only','rights':RIGHTS,'notes':'Derived ZIP central directory; not a Git tree or HTTP response.'})
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


def add_all_photos(dst,sources,repo):
    for i,f in enumerate(sorted((S/repo/'images').glob('*.webp'))):
        if any(k in f.name.lower()for k in ['size','0.96inch_lcd']):continue
        sources.append(source(dst,f,'photo-'+str(i),'photo-'+str(i)+'.webp'))

def cad(repo,pid,arc,count,pcb,screen,shape,mat,names,control=None):
    dst,sources,t=common(pid,repo);p=basics(pid,sources,t);adir=S/repo/'files/official-resources'
    sources.append(source(dst,adir/arc,'official-archive','structure'+Path(arc).suffix,'cad-container'))
    rec=json.loads((adir/'source.stp.receipt.json').read_text());s=source(dst,adir/'source.stp','official-cad','source.stp','cad-source',compress=True)
    s.update(origin='rar-member'if arc.endswith('.rar')else'zip-member',containerSourceId='official-archive',archive={'entry':rec['archiveMember'],'containerSha256':rec['containerSha256']});sources.append(s)
    picture(dst,sources,repo,'*Product_Size*.webp'if'LCD'in repo else'*ProductSize*.webp','dimension-photo','dimension-photo.webp');add_all_photos(dst,sources,repo)
    top=json.loads((adir/'topology.json').read_text());colors=json.loads((adir/'topology-names.json').read_text());assert len(top)==count
    p['method']='manufacturer-cad';p['matrixRows']=mat;p['sourceTopologyCounts']={'source.stp.gz':count};p['sourceCoverage']={'source.stp.gz':{'includedIndices':list(range(count)),'omittedIndices':[],'grouping':'Every native topology preserved; rigid source-coordinate transform only, no scaling.'}}
    for i in range(count):
        rgb=colors[i].get('color');color='#'+''.join(f'{round(255*(12.92*x if x<=.0031308 else 1.055*x**(1/2.4)-.055)):02x}'for x in rgb)if rgb else'#a9a9a9'
        part={'name':names.get(i,f'native-{i:03d}'),'sourceFile':'source.stp.gz','sourceComponent':i,'sourceTopology':'solid-or-shell','color':color,'role':'component'}
        if i in([190,191,192]if'LCD'in repo else[0,1,401,402]):part['role']='display-stack'
        if control and i==control:part['controlId']='boot'
        p['parts'].append(part)
    p['sourceSearch']={'status':'official-archive-native-STEP-found','inspected':['overview','resources','product','official-archive'],'sourceArchive':arc,'archiveMember':rec['archiveMember'],'result':'Exact product STEP is extracted from the retained original official archive; container and member byte hashes are independently checked. No Git revision is invented for ZIP-only material.'}
    p['materialBasis']='Original native linear RGB converted to sRGB; native shapes without supplied color use explicit neutral grey.'
    p['assemblyNotes']='Unscaled native solids/shells; global rigid coordinate transform puts screen toward+Z. Debug separation is not operating spacing.'
    p['limitations']=['Native manufacturer CAD revision may differ from current production. No hardware fit or browser WebGL acceptance.','The emissive face is projected above the native cover solely to show the live framebuffer; that rendering offset is not glass thickness.','Uncolored native components use neutral grey; no fabricated silkscreen or hidden battery.']
    p['presentation']={'screen':{'shape':shape,'sizeMm':screen[:2],'positionMm':screen[2:]},'inspectionGroups':[{'id':'native-optics','parts':[names[i]for i in names if i in([190,191,192]if'LCD'in repo else[0,1,401,402])]+['live-display'],'offsetMm':[50,0,15],'sourceIds':['official-cad'],'notes':'Whole source optical components moved together only for inspection.'}]}
    p['validation']={'display':{'width':240 if'LCD'in repo else 466,'height':240 if'LCD'in repo else 466},'controls':[{'id':'boot','type':'button'}]if control else[],'controlViewNormals':{'boot':[-1,0,0]}if control else{},'ledIndices':[],'screenAspectRelativeTolerance':.001}
    p['dimensionTargets']=[{'part':'screen','sizeMm':screen[:2]+[None],'toleranceMm':.001,'sourceId':'official-cad'}]
    p['dimensions']={'display':{'sourceEmissiveSizeMm':screen[:2],'sourceIds':['official-cad']},'pcb':{'sourceComponent':pcb,'sourceBoundsMm':top[pcb]['bounds'],'sourceSizeMm':top[pcb]['size'],'sourceIds':['official-cad']}}
    return dst,p

for header in[False,True]:
    repo='ESP32-C6-LCD-1.3';pid='waveshare-esp32-c6-lcd-1-3'+('-m'if header else'')
    dst,p=cad(repo,pid,'ESP32-C6-LCD-1.3-Structure.zip',193,0,[23.4,23.4,0,1.58,5.4580036],'rectangle',[[-1,0,0,-3.85735635959],[0,1,0,-27.67775897],[0,0,-1,0],[0,0,0,1]],{0:'pcb-main',24:'usb-shell',25:'tf-socket-shell',43:'boot-key',58:'reset-key',190:'optical-backplate',191:'lcd-cover',192:'lcd-flex'},43)
    p['dimensions']['display'].update(sourceEmissiveFace='component191 face10 z-5.3500036;23.4 mm square',nativeCoverSizeMm=[26.16,29.22],sourceCoverFrontZmm=-5.4500036)
    p['limitations']+=['Official photograph states26.25×29.42 mm cover /30 mm board length, while retained native cover26.16×29.22 mm and PCB26.3067×29.2925 mm differ. Native CAD is retained without scaling and the revision discrepancy remains open.','BOOT target is native switch43, matched against the unmodified rear resource photograph. The other native switch58 is RESET and is not treated as a GPIO input.']
    holes=[r for r in json.loads((S/repo/'files/official-resources/hole-cylinders.json').read_text())if .39<r[0]<.41];assert len(holes)==12
    p['dimensions']['headers']={'present':header,'nativeHoleCenters':holes,'pitchMm':2.54,'pinsPerRow':6,'pinLengthMm':None,'sourceIds':['official-cad','product']}
    if header:
        for x in sorted(set(r[1]for r in holes)):
            row=[r for r in holes if r[1]==x];yc=sum(r[2]for r in row)/6
            p['parts'].append({'name':'header-body-'+str(x),'color':'#24282a','geometry':{'shape':'box','sizeMm':[2.54,15.24,2.5],'centerMm':[x,yc,1.25]},'sourceIds':['official-cad','product'],'basis':'Exact12 native through-hole centers;2.5 mm plastic thickness is an explicitly estimated presoldered header envelope.'})
            for j,(_,x,y,z)in enumerate(row):p['parts'].append({'name':f'header-pin-{x}-{j}','color':'#b9a76e','geometry':{'shape':'box','sizeMm':[.64,.64,9],'centerMm':[x,y,3.2]},'sourceIds':['official-cad','product'],'basis':'Exact native hole center;0.64 mm section and9 mm length are UNKNOWN illustrative manufacturing dimensions.'})
        p['limitations'].append('SKU33644-M adds exactly positioned illustrative header pins; the official STEP is bare. Added body, section, length and finish are not manufacturer CAD.')
    finish(dst,p)

repo='ESP32-C6-Touch-AMOLED-1.32';pid='waveshare-esp32-c6-touch-amoled-1-32'
dst,p=cad(repo,pid,'ESP32-C6-Touch-AMOLED-1.32-3dFile.rar',408,2,[33.55,33.55,0,0,1.308],'circle',[[1,0,0,0],[0,0,-1,0],[0,1,0,0],[0,0,0,1]],{0:'front-cover',1:'optical-stack-flex',2:'pcb-main',64:'usb-shell',393:'side-key-1',394:'side-key-2',395:'display-connector',399:'speaker-connector',400:'battery-connector',401:'front-frame',402:'optical-support'})
p['dimensions']['display'].update(nativeEmissiveFace='component1 face147 Y-0.05 diameter33.55',viewingDiameterMm=33.75,coverOuterDiameterMm=36.4,sourceViewingFace='component0 face9 Y1.27',frontTrimDiameterMm=34.3)
p['limitations']+=['Published33.75 mm viewing and native33.55 mm emissive diameters are not conflated. The physical cover is36.4 mm, trim34.3 mm.','Native switch positions are preserved. Exact photographic BOOT/PWR-to-topology registration is unverified, so the3D switches remain unbound while source-backed GPIO9/GPIO1 controls remain available in the workbench.']
finish(dst,p)

for repo,pid,body,depth,radius,screen,sy,pixels,tol in[
('ESP32-C6-GEEK','waveshare-esp32-c6-geek',[24.5,61],9,2,[14.86,24.91],-8.445,[135,240],.061),
('ESP32-C6-Touch-AMOLED-1.8','waveshare-esp32-c6-touch-amoled-1-8',[37.6,45.2],15,6,[28.7,34.94],0,[368,448],.001),
('ESP32-C6-Touch-LCD-1.83','waveshare-esp32-c6-touch-lcd-1-83',[37.2,44.2],17.7,6,[29.9,35.3],0,[240,284],.003),
]:
    dst,sources,t=common(pid,repo);p=basics(pid,sources,t);picture(dst,sources,repo,'*size*.webp','dimension-photo','dimension-photo.webp');add_all_photos(dst,sources,repo)
    outline=rect(*body,radius);hole=[[x,y+sy]for x,y in rect(screen[0]+.05,screen[1]+.05,.5)]
    color='#e9e8e3'if'GEEK'in repo else'#1b2023'
    p['parts']=[{'name':'optical-face','role':'display-stack','color':'#10161a','geometry':{'shape':'box','sizeMm':[screen[0]+.5,screen[1]+.5,.7],'centerMm':[0,sy,-.35]},'sourceIds':['dimension-photo'],'basis':'Published visible XY aperture,0.7 mm optical depth explicitly illustrative.'}]
    p['derivedParts']=[plate('front-bezel',outline,1.2,-.6,color,[hole],'shell'),plate('case-wall',outline,depth-2.4,-depth/2,color,[rect(body[0]-2.4,body[1]-2.4,max(.3,radius-1.2))],'shell'),plate('rear-cover',outline,1.2,-depth+.6,color,role='shell'),{'id':'pcb-main','kind':'pcb-placeholder','color':'#667982','geometry':{'sizeMm':[body[0]-6,body[1]-8,1.6],'centerMm':[0,0,-depth/2],'labelSide':'front','labelSizeMm':[body[0]-9,5]},'basis':'Hidden PCB placement, dimensions, components and traces UNKNOWN. Debug envelope only; not an inferred board assembly.','sourceIds':['resources','dimension-photo']}]
    p['dimensions']={'enclosure':{'outerSizeMm':body+[depth],'cornerRadiusMm':None,'illustrativeCornerRadiusMm':radius,'sourceIds':['dimension-photo'],'wallThicknessMm':None},'display':{'visibleSizeMm':screen,'centerYmm':sy,'sourceIds':['dimension-photo']}}
    p['presentation']={'screen':{'shape':'rectangle','sizeMm':screen,'positionMm':[0,sy,.008]},'inspectionGroups':[{'id':'exterior-optics','parts':['front-bezel','optical-face','live-display'],'offsetMm':[55,0,12],'notes':'Debug separation exposes UNKNOWN inside, not assembly instructions.'}]}
    p['validation']={'display':{'width':pixels[0],'height':pixels[1]},'controls':[],'ledIndices':[],'shellNames':['front-bezel','case-wall','rear-cover'],'unknownPcbInspection':[{'label':'pcb-main-unknown-front'}],'screenAspectRelativeTolerance':min(tol,.001)}
    if tol>.001:
        p['validation']['screenAspectSource']={'sourceId':'dimension-photo','sizeMm':screen,'pixelResolution':pixels,'status':'documented-source-conflict','reason':'Published physical dimensions and pixel resolution differ from a square-pixel aspect. Preserve both sources; no physical pixel calibration is claimed.'}
    p['dimensionTargets']=[{'part':'front-bezel','sizeMm':body+[None],'toleranceMm':.001,'sourceId':'dimension-photo'},{'part':'screen','sizeMm':screen+[None],'toleranceMm':.001,'sourceId':'dimension-photo'}]
    p['limitations']+=['Measured exterior dimensions are retained; unsupplied corner radius, wall thickness and internal support geometry are explicit estimates.','Connector openings, rear labels, key travel and hidden battery position remain reference-only. BOOT/PWR are workbench controls; no approximate exterior actuator is promoted to a precise3D target.','Public gallery original URLs returned403; unchanged official documentation images supply exterior reference.']
    if'GEEK'in repo:
        p['limitations']+=['The complete61 mm silhouette includes the removable16 mm USB protective cap. Cap/USB mechanical fit and uncovered plug contact geometry remain UNKNOWN.','Official24.91×14.86 mm screen and135×240 portrait BSP have a6.06% square-pixel aspect discrepancy; published pixel size0.1101×0.1035 mm is also unequal. Physical dimensions are preserved, with an explicit tolerance, not silently stretched to invented square-pixel dimensions.','V1/V2 SD routing differs; outward shape is shared from current product photographs, not verified native CAD for either revision.']
        p['dimensions']['enclosure']['usbCapLengthMm']=16
    if'1.83'in repo:p['limitations'].append('17.7 mm casing body depth is published separately from19 mm overall including actuators; exact actuator geometry is not fabricated. Screen dimension rounding differs from square-pixel aspect by0.23%, explicitly tolerated.')
    if'1.8'in repo:p['limitations'].append('Official dimension image rear label identifies V1 SH8601/FT3168. Current overview confirms common board-level interface pins, not identical revision mechanics. V2 reuses this exterior only as a dimension/photo reference with unverified revision fit; V2 label artwork is not fabricated.')
    finish(dst,p)
