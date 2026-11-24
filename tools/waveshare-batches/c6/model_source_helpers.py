"""Prepare reviewed source plans; source images are retained byte-for-byte."""
from pathlib import Path
import copy, datetime, gzip, hashlib, json, math, shutil
from PIL import Image

ROOT=Path(__file__).resolve().parents[3]; A=ROOT/'web/platform/assets'
def configure(sources):
    global R,S
    R=Path(sources);S=R/'c6-sources'
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
        import zipfile
        archives=[];entries=[]
        for arc in sorted((S/repo/'files/official-resources').glob('*.zip')):
            rec=json.loads(Path(str(arc)+'.receipt.json').read_text())
            with zipfile.ZipFile(arc)as z:
                members=[{'path':i.filename,'bytes':i.file_size,'crc32':i.CRC}for i in z.infolist()]
            archives.append({'source':rec,'entries':members});entries.extend(members)
        t={'sha':None,'tree':entries,'method':'derived-ZIP-central-directory','archives':archives}
        if archives:
            raw=(json.dumps(t,indent=2)+'\n').encode();out=dst/'archive-inventory.json';out.write_bytes(raw)
            sources.append({'id':'archive-inventory','file':out.name,'url':archives[0]['source']['url'],'bytes':len(raw),'sha256':sha(raw),'fetchedAt':archives[0]['source']['fetchedAt'],'captureKind':'derived-archive-member-inventory','role':'reference-only','rights':RIGHTS,'notes':'Derived full ZIP central directories; not Git trees or original HTTP response bytes.'})
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
    return {'schemaVersion':1,'id':pid,'method':'dimension-photo-reconstruction','sources':sources,'matrixRows':[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]],'sourceTopologyCounts':{},'parts':[],'derivedParts':[],'dimensions':{},'presentation':{},'dimensionTargets':[],'validation':{'controls':[],'ledIndices':[]},'sourceSearch':{'status':'no-matching-CAD-in-recorded-scope','inspected':['overview','resources','product']+(['repository-tree']if t['sha']else['archive-inventory']if t['tree']else[]),'repositoryCommit':t['sha'],'repositoryTreeTruncated':False,'repositoryTreeEntries':len(t['tree']),'result':'Canonical resource links and any retained pinned repository or ZIP directory were inspected for CAD/STEP/mechanical files. Search scope is the retained evidence only, not a global absence claim.'},'materialBasis':'Published product photographs and explicit reconstruction estimates; unseen structures remain UNKNOWN.','assemblyNotes':'Front points toward +Z. Mechanical reconstruction is in millimetres; uncertain depth and hidden structures are not manufacturer measurements.','limitations':['This model is a dimension/photo reconstruction, not manufacturer CAD.','Unmeasured depths, corner radii and connector contact geometry are explicit illustrative estimates; no fit, drop-in enclosure or hardware acceptance claim.','Source photographs are unchanged. Any display artwork shown in them is reference only; the live screen receives the simulator framebuffer.'],'referencePhotos':[]}
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
    for i,f in enumerate(sorted(p for p in (S/repo/'images').iterdir()if p.suffix.lower()in['.webp','.jpg','.png'])):
        if any(k in f.name.lower()for k in ['size','0.96inch_lcd']):continue
        sources.append(source(dst,f,'photo-'+str(i),'photo-'+str(i)+f.suffix))
