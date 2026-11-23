#!/usr/bin/env python3
"""Derive EE02/03/04/05 preview plans from retained official KiCad layouts.

No network and no package-library lookup. Source PCB XY, outline, thickness and
circular drills are preserved. Unavailable package bodies are explicitly simple
estimates; F.Fab/F.SilkS envelopes do not certify physical connector fit.
Re-run this before tools/build-seeed-models.py when changing PCB-derived plans.
"""
import argparse, hashlib, json, math, re
from pathlib import Path
import numpy as np

ROOT=Path(__file__).resolve().parents[1]
ASSETS=ROOT/'web/platform/assets'
IDENTITY=np.eye(4).tolist()

def write(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def parse(text):
    stack=[];out=[]
    for match in re.finditer(r'\(|\)|"(?:\\.|[^"\\])*"|[^\s()]+',text):
        token=match.group()
        if token=='(':stack.append([])
        elif token==')':
            value=stack.pop()
            if stack:stack[-1].append(value)
            else:out.append(value)
        else:stack[-1].append(json.loads(token,strict=False) if token.startswith('"') else token)
    assert not stack and len(out)==1
    return out[0]
def elems(e,key):return [x for x in e[1:] if isinstance(x,list) and x and x[0]==key]
def one(e,key,default=None):
    found=elems(e,key);return found[0][1:] if found else default

def xy(e,key):return np.array(one(e,key)[:2],float)
def arc_points(edge,tolerance=.015):
    a,m,b=(xy(edge,k) for k in ('start','mid','end'))
    center=np.linalg.solve(2*np.array([m-a,b-a]),np.array([m@m-a@a,b@b-a@a]))
    radius=np.linalg.norm(a-center);angles=[math.atan2(*(point-center)[::-1]) for point in (a,m,b)]
    aa,mm,bb=angles;end=(bb-aa)%(2*math.pi);mid=(mm-aa)%(2*math.pi)
    sweep=end if mid<=end else end-2*math.pi
    step=2*math.acos(max(-1,min(1,1-tolerance/radius)));count=max(2,math.ceil(abs(sweep)/step))
    pts=[center+radius*np.array([math.cos(t),math.sin(t)]) for t in np.linspace(aa,aa+sweep,count+1)]
    pts[0]=a;pts[-1]=b;return pts

def outline(tree):
    edges=[e for e in tree[1:] if isinstance(e,list) and e and e[0] in ('gr_line','gr_arc','gr_circle') and one(e,'layer')==['Edge.Cuts']]
    curves=[];holes=[]
    for edge in edges:
        if edge[0]=='gr_line':curves.append([xy(edge,'start'),xy(edge,'end')])
        elif edge[0]=='gr_arc':curves.append(arc_points(edge))
        else:
            center=xy(edge,'center');holes.append((center,float(np.linalg.norm(xy(edge,'end')-center)),'Edge.Cuts circle'))
    chain=curves.pop(0);gaps=[]
    while curves and np.linalg.norm(chain[-1]-chain[0])>.005:
        candidates=[(np.linalg.norm(chain[-1]-p[index]),i,index) for i,p in enumerate(curves) for index in (0,-1)]
        gap,i,index=min(candidates);assert gap<=.005,('Disconnected board outline',gap)
        points=curves.pop(i);points=points if index==0 else points[::-1]
        gaps.append(float(gap));chain.extend(points[1:])
    gap=float(np.linalg.norm(chain[-1]-chain[0]));assert gap<=.005
    gaps.append(gap);chain.pop()
    points=np.array(chain);bounds=[points.min(0),points.max(0)]
    return points,holes,bounds,max(gaps),[[point.tolist() for point in curve] for curve in curves]

def inside(point,poly):
    x,y=point;odd=False
    for a,b in zip(poly,np.roll(poly,-1,axis=0)):
        if (a[1]>y)!=(b[1]>y) and x<(b[0]-a[0])*(y-a[1])/(b[1]-a[1])+a[0]:odd=not odd
    return odd

def distance_to_edges(point,poly):
    a=poly;b=np.roll(poly,-1,axis=0);ab=b-a
    t=np.clip(np.sum((point-a)*ab,axis=1)/np.sum(ab*ab,axis=1),0,1)
    return float(np.linalg.norm(point-a-t[:,None]*ab,axis=1).min())

def graphics_points(f,layer):
    points=[]
    for g in f[1:]:
        if not isinstance(g,list) or not g or not g[0].startswith('fp_') or g[0]=='fp_text' or one(g,'layer')!=[layer]:continue
        for key in ('start','end','mid','center'):
            if one(g,key):points.append(xy(g,key))
        if g[0]=='fp_circle':
            c=xy(g,'center');r=np.linalg.norm(xy(g,'end')-c);points.extend([c-r,c+r])
        if g[0]=='fp_poly':points.extend(np.array(p[1:3],float) for p in one(g,'pts',[]) if p[0]=='xy')
    return points

def derive(board):
    pid='seeed-xiao-epaper-'+board;p=ASSETS/pid
    sources=json.loads((p/'source-manifest.json').read_text())
    source=next(s for s in sources if s['id']=='pcb-layout');data=(p/source['file']).read_bytes()
    assert hashlib.sha256(data).hexdigest()==source.get('retained',source)['sha256']
    tree=parse(data.decode());poly,holes,bounds,maxgap,unmodeled_edges=outline(tree);origin=(bounds[0]+bounds[1])/2
    thickness=float(next(g[1] for g in one(tree,'general') if g[0]=='thickness'))
    def view(point):return np.array([origin[1]-point[1],origin[0]-point[0]])
    def local(f,point):
        a=one(f,'at');x,y=float(a[0]),float(a[1]);theta=math.radians(float(a[2]) if len(a)>2 else 0);u,v=point
        return np.array([x+math.cos(theta)*u+math.sin(theta)*v,y-math.sin(theta)*u+math.cos(theta)*v])
    def orient(f):
        a=one(f,'at');return math.radians(float(a[2]) if len(a)>2 else 0)-math.pi/2
    def box(f,lo,hi,height,z=None):
        lo,hi=np.array(lo,float),np.array(hi,float);center=view(local(f,(lo+hi)/2))
        return {'sizeMm':[*(hi-lo).tolist(),height],'centerMm':[float(center[0]),float(center[1]),thickness/2+height/2 if z is None else z],'rotationZRadians':orient(f)}
    parts=[];derived=[];omitted=[];packages=[];pads={'front':[],'back':[]};drill_omitted=[]
    controls={'SW3':'key1','SW4':'key2','SW5':'key3'} if board!='ee05' else {'K2':'key1','K3':'key2','K4':'key3'}
    footprints=elems(tree,'footprint')
    for f in footprints:
        props={q[1]:q[2] for q in elems(f,'property')};ref=props.get('Reference');value=props.get('Value','')
        if ref is None:
            ref=next((q[2] for q in elems(f,'fp_text') if q[1]=='reference'),'unnamed')
        center=np.array(one(f,'at')[:2],float);attrs=one(f,'attr',[])
        if np.any(center<bounds[0]-.5) or np.any(center>bounds[1]+.5):
            omitted.append({'reference':ref,'value':value,'reason':'Unplaced footprint outside manufacturer board outline; no relocation.','sourceAtMm':center.tolist()});continue
        # SMD copper pads remain visible even where the manufacturer marks a component DNP.
        for pad in elems(f,'pad'):
            at=one(pad,'at',[0,0]);size=np.array(one(pad,'size',[0,0]),float);point=local(f,np.array(at[:2],float));v=view(point)
            if np.any(size<=0):continue
            side='back' if one(f,'layer')==['B.Cu'] else 'front'
            if pad[2]!='np_thru_hole':
                angle=math.radians(float(at[2]) if len(at)>2 else 0)-math.pi/2
                pads[side].append({'sizeMm':[float(size[0]),float(size[1]),.035],'centerMm':[float(v[0]),float(v[1]),(.8+.0175)*(1 if side=='front' else -1)],'rotationZRadians':angle})
            drill=one(pad,'drill')
            if drill:
                if len(drill)==1 and re.fullmatch(r'[0-9.]+',drill[0]):
                    radius=float(drill[0])/2
                    if inside(point,poly) and distance_to_edges(point,poly)>radius+.005:
                        if not any(np.linalg.norm(point-c)<r+radius+.002 for c,r,_ in holes):holes.append((point,radius,ref+':pad'+pad[1]))
                    else:drill_omitted.append({'reference':ref,'pad':pad[1],'reason':'edge/castellation intersects outline; not modeled as a full circular hole'})
                else:drill_omitted.append({'reference':ref,'pad':pad[1],'reason':'noncircular drill retained in source but omitted from simplified preview'})
        if 'dnp' in attrs:
            omitted.append({'reference':ref,'value':value,'reason':'Manufacturer footprint is marked DNP; copper pads retained, no populated body.'});continue
        if ref.startswith(('H','TP','FID')) or 'LOGO' in f[1].upper():continue
        if ref=='U3':
            v=view(local(f,[.075,0]));module=[17.8,20.95,1.0]
            derived.append({'id':'xiao-module-pcb-unknown','kind':'pcb-placeholder','color':'#263632','geometry':{'sizeMm':module,'centerMm':[float(v[0]),float(v[1]),1.5],'labelSide':'both','labelSizeMm':[14,3.5]},'basis':'manufacturer-footprint-envelope-internal-UNKNOWN','sourceIds':['pcb-layout'],'notes':'U3 manufacturer F.Fab gives the XIAO carrier envelope and XY. Separate module PCB layout/package CAD is unavailable here; internal devices are UNKNOWN, not reconstructed from the parent schematic.'})
            # Visible USB mouth is an explicitly estimated open frame; it cannot accept electrical connections.
            usb=view(local(f,[8,0]));M=[[1,0,0,float(usb[0])],[0,0,1,float(usb[1])],[0,-1,0,3.6],[0,0,0,1]]
            parts.append({'name':'xiao-usb-mouth-estimate','role':'component','color':'#adb5b6','material':{'metalness':.75,'roughness':.35},'matrixRows':M,'geometry':{'shape':'ring','outerSizeMm':[8.9,3.2],'innerSizeMm':[8.1,2.4],'zRangeMm':[-3.65,3.65]},'basis':'photo-based-package-estimate','sourceIds':['pcb-layout',sources[2]['id']],'notes':'Approximate open Type-C shell only; hidden depth, mating plane and contact geometry are not mechanically verified.'})
            continue
        if ref in controls or ref in (['SW2'] if board!='ee05' else ['K1']):
            if board=='ee05':body=box(f,[-2.35,-1.05],[2.35,1.2],1.88);cap=box(f,[-1,-2.2],[1,-1.05],1.05,1.85)
            else:body=box(f,[-2.25,0],[2.2,2.8],4.5);cap=box(f,[-1.15,2.8],[1.15,5.5],2.2,3.05)
            parts.append({'name':'button-'+ref.lower()+'-housing','role':'component','color':'#a6aeac','geometry':{'shape':'boxes','boxes':[body]},'basis':'manufacturer-footprint-XY-height-estimate','sourceIds':['pcb-layout'],'footprintReference':ref})
            rec={'name':'button-'+ref.lower()+'-cap','role':'button','color':'#343b38','geometry':{'shape':'boxes','boxes':[cap]},'basis':'manufacturer-footprint-XY-height-estimate','sourceIds':['pcb-layout'],'footprintReference':ref,'notes':'Side actuator inferred from retained footprint graphics and official front/annotated photographs. Height is approximate.'}
            if ref in controls:
                rec['controlId']=controls[ref];rec['physicalNet']=next((n[-1] for pad in elems(f,'pad') if (n:=one(pad,'net')) and ('BUTON' in n[-1] or 'BOTTON' in n[-1])),None)
                assert rec['physicalNet'] and rec['physicalNet'].endswith(rec['controlId'][-1])
            else:rec['notes']+=' Reset has no GPIO button binding; use the platform whole-device reset.'
            parts.append(rec);continue
        points=graphics_points(f,'F.Fab');basis='F.Fab'
        if not points:points=graphics_points(f,'F.SilkS');basis='F.SilkS'
        if not points:continue
        points=np.array(points);lo,hi=points.min(0),points.max(0);size=hi-lo
        if min(size)<.05 or max(size)>40:continue
        description=props.get('Description','');height=.55;color='#404744'
        if ref.startswith('C'):height=.65;color='#a8946d'
        elif ref.startswith('R'):height=.35;color='#343835'
        elif ref.startswith('L'):height=1.6;color='#4c504b'
        elif ref.startswith('U'):height=1.0;color='#333a38'
        elif ref.startswith(('J','P')):height=2.0;color='#d4d1bb' if 'FPC' in description else '#d9ddd5'
        elif ref.startswith('SW'):height=2.5;color='#3f4641'
        elif ref.startswith('D'):height=.65;color='#58624f'
        elif ref.startswith(('Y','X')):height=.8;color='#b8c0bb'
        height_basis='explicit package-height estimate'
        if props.get('Height') and re.fullmatch(r'[0-9.]+',props['Height']):height=float(props['Height']);height_basis='manufacturer footprint Height property'
        elif (m:=re.search(r'(?:[x*]H|\*H|×H|W[0-9.]+\*H)([0-9.]+)mm',description)):
            height=float(m.group(1));height_basis='manufacturer footprint Description height'
        if not 0<height<10:height=.8;height_basis='explicit package-height estimate'
        body=box(f,lo,hi,height)
        parts.append({'name':'component-'+ref.lower(),'role':'component','color':color,'geometry':{'shape':'boxes','boxes':[body]},'basis':'manufacturer-footprint-XY-with-simplified-package','sourceIds':['pcb-layout'],'footprintReference':ref,'footprintValue':value,'packageGeometry':{'xyBasis':basis+' bounding envelope','heightBasis':height_basis,'heightMm':height},'notes':'Rectangular preview body, not the unavailable manufacturer STEP package. Exact pins and mechanical insertion are not represented.'})
        packages.append({'reference':ref,'xyBasis':basis,'heightMm':height,'heightBasis':height_basis})
    boardpart={'name':'pcb-main','role':'pcb','color':'#19231f','geometry':{'shape':'plate','outlineMm':[view(v).tolist() for v in poly],'holes':[{'centerMm':view(c).tolist(),'radiusMm':r,'source':label} for c,r,label in holes],'thicknessMm':thickness},'basis':'manufacturer-KiCad-Edge.Cuts-drills-thickness','sourceIds':['pcb-layout'],'notes':'Source outline in millimetres; native arcs sampled to 0.015 mm sagitta. Endpoint joins differ by at most '+str(maxgap)+' mm. No whole-board scaling.'}
    parts.insert(0,boardpart)
    for side,boxes in pads.items():
        if boxes:parts.insert(1,{'name':'pcb-'+side+'-pads','role':'component','color':'#adb4a3','geometry':{'shape':'boxes','boxes':boxes},'basis':'manufacturer-pad-XY-rectangular-preview','sourceIds':['pcb-layout'],'notes':'Copper-pad positions, dimensions and source angles retained; rounded pads are shown as thin rectangles and solder fillets are not modeled.'})
    ext=(bounds[1]-bounds[0])[::-1];nominal=None if board=='ee05' else [81,44,7] if board=='ee04' else [80,43,7]
    photos=[{'file':s['file'],'sourceId':s['id'],'sourceSha256':s['sha256'],'view':'official-annotated-reference' if any(k in s['file'] for k in ('overview','buttons','pin')) else 'official-product-front'} for s in sources if s.get('kind') in ('official-reference-photo','product-photo')]
    limitations=['Preview only: no mechanical-fit, electrical connector insertion, WebGL/browser or hardware firmware validation is implied.',
      'This SKU is a bare controller board with no integrated display. No screen plane, enclosure, battery or external antenna placement is invented.',
      'Manufacturer PCB outline, circular holes, thickness, footprint XY and source angle are retained. Copper tracks, vias and silk-screen artwork are not rendered.',
      'Component packages use simplified F.Fab/F.SilkS envelopes with explicitly reported height estimates; external connector mating geometry and hidden contacts remain unverified.',
      'The parent PCB provides only the XIAO module footprint. Its separate internal PCB layout is UNKNOWN; an open USB shell estimate is drawn from photographs.',
      'DNP parts and unplaced off-board footprints are listed and not populated. Noncircular/edge-intersecting drill omissions are separately reported.',
      'No calibrated photograph overlay is claimed. Official front and annotated photographs support component-order/orientation review; source/measured nominal differences are retained.']
    if unmodeled_edges:limitations.append('Open internal Edge.Cuts segments are retained in source and listed in pcbDerivation; unspecified slot/router width is not invented in the preview substrate.')
    if board=='ee04':limitations.append('Official Wiki links a GrabCAD 3D model, but its public download returned HTTP 403. This fallback uses the available official PCB, not a claim that no CAD was published.')
    groups=[{'id':'module','parts':['xiao-module-pcb-unknown','xiao-usb-mouth-estimate'],'offsetMm':[0,0,24],'sourceIds':['pcb-layout'],'notes':'Viewer inspection offset only; does not claim physical fasteners.'},{'id':'board','parts':[q['name'] for q in parts if q['name']!='xiao-usb-mouth-estimate'],'offsetMm':[0,0,0],'sourceIds':['pcb-layout'],'notes':'Source board remains at its manufacturer XY scale.'}]
    groups=[group for group in groups if any(group['offsetMm'])]
    plan={'schemaVersion':'1','id':pid,'method':'manufacturer-pcb-and-photo-derived','matrixRows':IDENTITY,'assemblyNotes':'KiCad XY has a downward second axis. The explicit front-view mapping is displayX=boardCenterY-KiCadY, displayY=boardCenterX-KiCadX; source distances and millimetres are preserved. Package Z dimensions are separate labeled estimates.','sources':sources,'parts':parts,'derivedParts':derived,'dimensions':{'width':float(ext[0]+3),'height':float(ext[1]+3),'depth':7,'unit':'mm','basis':'PCB extents plus visible estimated package projections','scope':'Preview envelope; exact board outline is separately checked.'},'presentation':{'bodySizeMm':[float(ext[0]+3),float(ext[1]+3),7],'screen':None,'inspectionGroups':groups},'referencePhotos':photos,'omittedComponents':omitted,'limitations':limitations,'materialBasis':'Official multi-angle product photographs: dark PCB, silver exposed metal, dark actuators, cream FPC/battery connectors. Colors are approximate, not calibrated.','dimensionChecks':[{'part':'pcb-main','basis':'manufacturer-KiCad-Edge.Cuts','measuredSizeMm':[float(ext[0]),float(ext[1]),thickness],'officialPhotoNominalEnvelopeMm':nominal,'nominalMinusPcbXYMm':[float(nominal[0]-ext[0]),float(nominal[1]-ext[1])] if nominal else None,'notes':'Product-photo dimensions may describe package projections and different revision rounding. The PCB is not stretched to match the nominal product envelope.' if nominal else 'No reliable manufacturer overall envelope/thickness was found for EE05. Only the source PCB outline and thickness are asserted; the 7 mm preview depth is an explicit package-envelope estimate.','sourceIds':['pcb-layout',sources[2]['id']]}],'pcbDerivation':{'script':'tools/derive-seeed-pcb-plans.py','sourceSha256':source.get('retained',source)['sha256'],'sourceCoordinatesOriginMm':origin.tolist(),'nativeArcSagittaToleranceMm':.015,'maxJoinedEndpointGapMm':maxgap,'embeddedStepCount':data.count(b'(embedded_files'),'circularHolesIncluded':len(holes),'unmodeledDrills':drill_omitted,'unmodeledOpenInternalEdgeCuts':unmodeled_edges,'footprintCount':len(footprints),'simplifiedPackageCount':len(packages),'packageHeightAssumptions':packages},'reviewViews':{'front':{'position':[0,0,200],'span':float(max(ext)*1.4)},'back':{'position':[0,0,-200],'span':float(max(ext)*1.4)},'side':{'position':[200,0,0],'span':float(max(ext)*1.4)}}}
    assert len(parts)<=512
    write(p/'model-plan.json',plan)
    print(json.dumps({'id':pid,'parts':len(parts),'pcbXYMm':ext.tolist(),'drills':len(holes),'omittedFootprints':len(omitted),'sharedResetBound':False}),flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('boards',nargs='*',choices=['ee02','ee03','ee04','ee05']);args=parser.parse_args()
    for board in args.boards or ['ee02','ee03','ee04','ee05']:derive(board)
