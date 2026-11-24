"""Fifth C6/C3/H2 group: retain native solids, explicit revision conflicts and UNKNOWNs."""
from pathlib import Path
import argparse, json
import model_source_helpers as h

ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--sources',type=Path,required=True);args=ap.parse_args();h.configure(args.sources)
S=args.sources/'c6-sources'

def cad(dst,sources,repo,pattern):
    files=list((S/repo/'files/archive-members').rglob(pattern));assert len(files)==1,files
    f=files[0];rec=json.loads(Path(str(f)+'.receipt.json').read_text())
    container=Path(rec['containerFile']) if rec.get('containerFile') else next(p for p in (S/repo/'files/official-resources').iterdir() if p.name==Path(rec['url']).name)
    sources.append(h.source(dst,container,'official-archive','source-archive'+container.suffix,'cad-container'))
    row=h.source(dst,f,'official-cad','source.stp','cad-source',compress=True);row.update(origin=rec.get('archiveFormat','zip')+'-member',containerSourceId='official-archive',archive={'entry':rec['archiveMember'],'containerSha256':rec['containerSha256']});row.pop('containerFile',None);sources.append(row)
    return f,json.loads(f.with_suffix('.topology.json').read_text()),json.loads(f.with_suffix('.colors.json').read_text())

def native(p,top,colors,names,optics):
    groups={}
    for i in range(len(top)):
        rgb=colors[i].get('color')
        color='#'+''.join(f'{round(255*(12.92*x if x<=.0031308 else 1.055*x**(1/2.4)-.055)):02x}'for x in rgb)if rgb else('#151a1d'if i in optics else'#a9a9a9')
        key=(names.get(i),color)if i in names else(None,color);groups.setdefault(key,[]).append(i)
    for(name,color),indices in groups.items():
        p['parts'].append({'name':name or'native-material-'+color[1:],'sourceFile':'source.stp.gz','sourceComponents':indices,'sourceTopology':'solid-or-shell','color':color,'role':'display-stack'if any(i in optics for i in indices)else'component'})
    p['method']='manufacturer-cad';p['sourceTopologyCounts']={'source.stp.gz':len(top)}
    p['sourceCoverage']={'source.stp.gz':{'includedIndices':list(range(len(top))),'omittedIndices':[],'grouping':'Every native component is retained without scaling; only identical source materials are grouped for rendering.'}}
    p['materialBasis']='Native linear source colors converted to sRGB. Missing optical materials use an explicitly illustrative dark color; other missing materials are neutral grey. No generated silkscreen or photo texture.'
    p['sourceSearch'].update(status='official-native-STEP-found',result='Exact original manufacturer-linked native STEP retained with every component. Variant/revision qualifications are recorded separately.')
    p['limitations']=['Every native source component is preserved at original millimetre scale. Native material colors are not calibrated hardware color measurements.','No physical fit, enclosure clearance, optical calibration, touch registration or browser WebGL acceptance is claimed.','Known workbench GPIO controls are independent of unverified physical actuator registration; no guessed3D click targets.']

def header(p,xs,ys,pcb_rear):
    # Actual 2.54-mm pitch and row positions come from the retained drawing;
    # the unmeasured plastic and pin cross-section/height remain estimates.
    for side,x in enumerate(xs):
        p['parts'].append({'name':f'header-{side}-illustrative-body','role':'header','color':'#17191b','geometry':{'shape':'box','sizeMm':[2.4,max(ys)-min(ys)+2.4,2.5],'centerMm':[x,(max(ys)+min(ys))/2,pcb_rear-1.25]},'basis':'Published pin count/pitch and drawing registration;2.4-mm body width and2.5-mm body height are explicit estimates.','sourceIds':['dimension-photo']})
        for j,y in enumerate(ys):p['parts'].append({'name':f'header-{side}-pin-{j}-estimate','role':'header','color':'#c6c5b6','geometry':{'shape':'box','sizeMm':[.64,.64,7.5],'centerMm':[x,y,pcb_rear-3]},'basis':'Published2.54-mm pitch;0.64-mm square pins and7.5-mm length are illustrative, not measured.','sourceIds':['dimension-photo']})
    p['limitations'].append('Pre-soldered header option uses source pitch/count with explicit estimated body/pin dimensions and registration. These derived headers are not manufacturer CAD and must not be used as a fit template.')

for touch in [False,True]:
    repo='ESP32-C6-LCD-1.9';pid='waveshare-esp32-c6-'+('touch-'if touch else'')+'lcd-1-9'
    dst,sources,t=h.common(pid,repo);p=h.basics(pid,sources,t)
    h.picture(dst,sources,repo,'*ProductSize-'+('2'if touch else'1')+'-*.webp','dimension-photo','dimension-photo.webp');h.add_all_photos(dst,sources,repo)
    f,top,colors=cad(dst,sources,repo,'*Touch*.stp'if touch else'ESP32-S3-LCD*.stp')
    names=({0:'native-lcd-glass',1:'native-touch-cover',279:'native-outer-cover',3:'pcb-main',271:'header-right',272:'header-left',2:'usb-shell',274:'tf-socket'}if touch else{0:'pcb-main',275:'native-optical-backing',276:'native-lcd-glass',266:'header-right',267:'header-left',268:'usb-shell',274:'tf-socket'})
    optics=[0,1,279]if touch else[275,276];native(p,top,colors,names,optics)
    p['matrixRows']=[[1,0,0,0],[0,0,-1,0],[0,1,0,0],[0,0,0,1]]
    p['assemblyNotes']='Proper rigid90-degree rotation about+X only: native(X,Y,Z) becomes(X,-Z,Y), preserving native dimensions. Front+Z;USB toward-Y. No rescaling.'
    size=[22.7,42.72];window=[23.1,43.12]if touch else size
    # Both original native assemblies contain the same22.7×42.72 active LCD
    # face. Touch cover23.1×43.12 is a distinct larger optical opening.
    p['dimensions']={'display':{'activeAreaMm':size,'activeFace':{'component':0 if touch else 276,'face':11,'nativeBoundsMm':[[-11.35,1.35 if touch else 2.6,-21.36],[11.35,1.35 if touch else 2.6,21.36]]},'coverViewingWindowMm':window,'sourceIds':['official-cad','dimension-photo']},'pcb':{'nativeSizeMm':top[3 if touch else 0]['size'],'sourceIds':['official-cad']}}
    p['presentation']={'screen':{'shape':'rectangle','sizeMm':size,'positionMm':[0,0,2.908 if touch else 2.658],'basis':'Unscaled22.7×42.72 native LCD active face projected above outer cover. Touch23.1×43.12 opening is not substituted for active area.'},'inspectionGroups':[{'id':'native-optics','parts':[names[i]for i in optics]+['live-display'],'offsetMm':[46,0,14],'sourceIds':['official-cad'],'notes':'Move entire native optical stack for inspection; not operating spacing.'}]}
    p['validation']={'display':{'width':170,'height':320},'controls':[],'ledIndices':[],'screenAspectRelativeTolerance':.001}
    p['dimensionTargets']=[{'part':'screen','sizeMm':size+[None],'toleranceMm':.001,'sourceId':'official-cad'}]
    p['sourceSearch']['archiveMember']=f.name
    p['limitations']+=['Official C6 resources link an archive whose native STEP filename identifies ESP32-S3. Published C6 dimensions match the retained major mechanical envelope; exact electrical revision/layout identity is UNVERIFIED. Do not infer C6 electronics from the S3-named assembly.','Touch source native total depth17.35 mm differs from published11.80 mm. Native headers and other structures remain unscaled; depth/measurement scope conflict is retained.'if touch else'Native front backing total50 mm and cover49.72 mm are distinct source geometries. Published26×51 mm PCB is approximately matched; no replacement scale is fitted.']
    h.finish(dst,p)

for case in [False,True]:
    repo='ESP32-C3-LCD-0.71';pid='waveshare-esp32-c3-lcd-0-71'+('-case'if case else'')
    dst,sources,t=h.common(pid,repo);p=h.basics(pid,sources,t);h.picture(dst,sources,repo,'*introduction-size-*.webp','dimension-photo','dimension-photo.webp');h.add_all_photos(dst,sources,repo)
    f,top,colors=cad(dst,sources,repo,'*.stp');names={0:'pcb-main',21:'usb-shell',22:'native-component-22',23:'native-component-23',27:'native-lcd-glass',28:'native-display-flex'};native(p,top,colors,names,[27,28])
    p['assemblyNotes']='Original native millimetres and coordinates retained. Front+Z; USB toward-Y. Source optical face27/4 provides18 mm diameter and exact center; no recentering or scaling.'
    p['dimensions']={'display':{'activeDiameterMm':18,'coverSizeMm':[20.12,22.3],'nativeActiveFace':{'component':27,'face':4,'centerMm':[4.318522791385,10.13146285187,2.2899968]},'sourceIds':['official-cad','dimension-photo']},'pcb':{'nativeSizeMm':top[0]['size'],'sourceIds':['official-cad']}}
    p['presentation']={'screen':{'shape':'circle','sizeMm':[18,18],'positionMm':[4.318522791385,10.13146285187,2.3179968]},'inspectionGroups':[{'id':'native-optics','parts':['native-lcd-glass','native-display-flex','live-display'],'offsetMm':[32,0,10],'sourceIds':['official-cad'],'notes':'Native glass/flex moves intact to expose the29-component assembly.'}]}
    p['validation']={'display':{'width':160,'height':160},'controls':[],'ledIndices':[],'screenAspectRelativeTolerance':.001};p['dimensionTargets']=[{'part':'screen','sizeMm':[18,18,None],'toleranceMm':.001,'sourceId':'official-cad'}]
    p['limitations'].append('Native29-component assembly omits several rear IC/packages visible in the current official photograph. Missing source geometry is not filled in or represented as a complete production PCB layout.')
    p['limitations'].append('GC9D01 physical160×160 digital framebuffer uses a circular18-mm physical aperture. Corner pixels remain digital memory, not visible glass.')
    if case:
        p['limitations']+=['SKU29512 includes a protective case, but the retained official CAD contains the bare board only. Exact case dimensions, solid geometry and apertures are UNKNOWN and are not rendered. This model is explicitly the bare assembly inside the unknown case.','Official storefront gallery image URLs returned HTTP403 in the recorded attempt; existing successful official overview images remain retained. This is not evidence that no case photo exists.']
        p['dimensions']['protectiveCase']={'outerSizeMm':None,'sourceIds':['product'],'status':'unknown-not-rendered'}
    h.finish(dst,p)

for m in [False,True]:
    repo='ESP32-C6-LCD-1.47';pid='waveshare-esp32-c6-lcd-1-47'+('-m'if m else'')
    dst,sources,t=h.common(pid,repo);p=h.basics(pid,sources,t);h.picture(dst,sources,repo,'*Dimensions-*.webp','dimension-photo','dimension-photo.webp');h.add_all_photos(dst,sources,repo)
    f,top,colors=cad(dst,sources,repo,'*.stp');names={0:'native-lcd-glass',1:'native-optical-backing',2:'pcb-main',108:'usb-shell',181:'tf-socket'};native(p,top,colors,names,[0,1])
    p['assemblyNotes']='Original native millimetres and coordinates retained, front+Z andUSB toward-Y. All184 native components preserved.'
    size=[17.39,32.35];p['dimensions']={'display':{'activeAreaMm':size,'coverSizeMm':[19.39,36.28],'nativeActiveFace':{'component':0,'face':44,'zMm':1.4},'sourceIds':['official-cad','dimension-photo']},'pcb':{'nativeSizeMm':top[2]['size'],'sourceIds':['official-cad']}}
    p['presentation']={'screen':{'shape':'rectangle','sizeMm':size,'positionMm':[0,0,1.458],'cornerRadiusMm':2.6,'basis':'Native face0/44 and published LCD AA dimensions; corner radius2.6 is an explicit illustrative estimate.'},'inspectionGroups':[{'id':'native-optics','parts':['native-lcd-glass','native-optical-backing','live-display'],'offsetMm':[36,0,13],'sourceIds':['official-cad'],'notes':'Native optical parts move intact; no fabricated rear electronics.'}]}
    p['validation']={'display':{'width':172,'height':320},'controls':[],'ledIndices':[],'screenAspectRelativeTolerance':.001};p['dimensionTargets']=[{'part':'screen','sizeMm':size+[None],'toleranceMm':.001,'sourceId':'official-cad'}]
    if m:header(p,[-8.89,8.89],[-7.8836+i*2.54 for i in range(9)],-3.65)
    h.finish(dst,p)

for repo,height in [('ESP32-C6-Zero-B',25),('ESP32-H2-Zero',23.5)]:
    for m in [False,True]:
        pid='waveshare-'+repo.lower()+('-m'if m else'');dst,sources,t=h.common(pid,repo);p=h.basics(pid,sources,t);h.picture(dst,sources,repo,'*details-size-*.webp','dimension-photo','dimension-photo.webp');h.add_all_photos(dst,sources,repo)
        p['parts']=[{'name':'usb-photo-envelope','role':'connector','color':'#babfc4','geometry':{'shape':'open-usb-shell','sizeMm':[9.2,7.5,3.5],'centerMm':[0,height/2-1.75,1.75],'wallMm':.3},'basis':'Approximate open USB-C metal envelope from original front photograph. Width9.2 mm is image-scaled against18-mm board (uncertainty±0.4 mm); depth, height, wall and registration are unmeasured illustrative estimates. Contacts omitted.','sourceIds':['dimension-photo']}]
        p['derivedParts']=[{'id':'pcb-main','kind':'pcb-placeholder','color':'#244a78','geometry':{'sizeMm':[18,height,1.6],'centerMm':[0,0,-.8],'labelSide':'both','labelSizeMm':[15,3]},'basis':'Official18-mm width and published board length;1.6-mm thickness estimated. Unverified3D component heights, connector internals, traces and unmeasured geometry visibly UNKNOWN.','sourceIds':['dimension-photo','resources']}]
        p['dimensions']={'pcb':{'sizeMm':[18,height,None],'illustrativeThicknessMm':1.6,'rowSpacingMm':15.24,'headerPitchMm':2.54,'sourceIds':['dimension-photo']}}
        p['validation']={'controls':[],'ledIndices':[]};p['dimensionTargets']=[{'part':'pcb-main','sizeMm':[18,height,None],'toleranceMm':.001,'sourceId':'dimension-photo'}]
        p['limitations']+=['USB envelope is an explicitly approximate photo-derived structure, not an exact connector model. No exact solid assembly was exposed by the retained official resource/repository scope. Published XY is retained and unseen structures remain visibly UNKNOWN.','BOOT and RESET workbench inputs and RGB output do not establish physical3D switch/LED positions. No invented battery, TF socket or display.']
        if m:header(p,[-7.62,7.62],[-10.16+i*2.54 for i in range(9)],-1.6)
        h.finish(dst,p)
