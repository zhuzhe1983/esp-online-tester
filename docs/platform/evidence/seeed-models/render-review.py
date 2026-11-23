#!/usr/bin/env python3
"""Software-only mesh review with synthetic display colors; never a firmware screenshot.

Requires numpy, trimesh, Pillow and a C compiler. Orthographic z-buffer rendering verifies gross
orientation, model bounds and plane placement; it is not a WebGL/material test
or a calibrated photo-overlay measurement. Run with optional product IDs.
"""
import pathlib,json,sys,math,ctypes,tempfile,subprocess,argparse,hashlib
import numpy as np,trimesh
from PIL import Image,ImageDraw
base=pathlib.Path(__file__).resolve().parents[4]/'web/platform/assets'
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('ids',nargs='*');parser.add_argument('--output',type=pathlib.Path,default=pathlib.Path(__file__).resolve().parent);args=parser.parse_args()
out=args.output;out.mkdir(parents=True,exist_ok=True)

# Compile this small, auditable CPU rasterizer in a temporary directory. It uses
# interpolated per-pixel depth rather than sorting whole triangles by their
# average depth, which would create false occlusion on large CAD faces.
_temporary=tempfile.TemporaryDirectory(prefix='seeed-review-')
_c=pathlib.Path(_temporary.name)/'raster.c'
_c.write_text(r'''
#include <math.h>
#include <stdint.h>
void raster(const double *v, const uint8_t *colors, int count, int width, int height,
            double *depth, uint8_t *rgb) {
  for (int t=0;t<count;t++) {
    const double *p=v+9*t;
    double x0=p[0],y0=p[1],z0=p[2],x1=p[3],y1=p[4],z1=p[5],x2=p[6],y2=p[7],z2=p[8];
    double den=(y1-y2)*(x0-x2)+(x2-x1)*(y0-y2);
    if (fabs(den)<1e-12) continue;
    int xa=(int)floor(fmin(x0,fmin(x1,x2))),xb=(int)ceil(fmax(x0,fmax(x1,x2)));
    int ya=(int)floor(fmin(y0,fmin(y1,y2))),yb=(int)ceil(fmax(y0,fmax(y1,y2)));
    if (xa<0)xa=0;if(ya<0)ya=0;if(xb>=width)xb=width-1;if(yb>=height)yb=height-1;
    for(int y=ya;y<=yb;y++) for(int x=xa;x<=xb;x++) {
      double px=x+.5,py=y+.5;
      double a=((y1-y2)*(px-x2)+(x2-x1)*(py-y2))/den;
      double b=((y2-y0)*(px-x2)+(x0-x2)*(py-y2))/den,c=1-a-b;
      if(a<-.0000001||b<-.0000001||c<-.0000001)continue;
      double z=a*z0+b*z1+c*z2;int i=y*width+x;
      if(z>=depth[i]){depth[i]=z;rgb[3*i]=colors[3*t];rgb[3*i+1]=colors[3*t+1];rgb[3*i+2]=colors[3*t+2];}
    }
  }
}
''')
_so=pathlib.Path(_temporary.name)/'raster.so'
subprocess.run(['cc','-O2','-shared','-fPIC',str(_c),'-o',str(_so),'-lm'],check=True)
_library=ctypes.CDLL(str(_so));_raster=_library.raster
_raster.argtypes=[ctypes.POINTER(ctypes.c_double),ctypes.POINTER(ctypes.c_uint8),ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.POINTER(ctypes.c_double),ctypes.POINTER(ctypes.c_uint8)]
_raster.restype=None

def raster(v,c,N):
 v=np.ascontiguousarray(v,dtype=np.float64);c=np.ascontiguousarray(c,dtype=np.uint8)
 depth=np.full((N,N),-1e30,dtype=np.float64);pixels=np.empty((N,N,3),dtype=np.uint8);pixels[:]=[238,241,237]
 _raster(v.ctypes.data_as(ctypes.POINTER(ctypes.c_double)),c.ctypes.data_as(ctypes.POINTER(ctypes.c_uint8)),len(v),N,N,depth.ctypes.data_as(ctypes.POINTER(ctypes.c_double)),pixels.ctypes.data_as(ctypes.POINTER(ctypes.c_uint8)))
 return Image.fromarray(pixels),depth

def Rxyz(v):
 x,y,z=v;rx=np.array([[1,0,0],[0,math.cos(x),-math.sin(x)],[0,math.sin(x),math.cos(x)]]);ry=np.array([[math.cos(y),0,math.sin(y)],[0,1,0],[-math.sin(y),0,math.cos(y)]]);rz=np.array([[math.cos(z),-math.sin(z),0],[math.sin(z),math.cos(z),0],[0,0,1]])
 return rx@ry@rz

def color(s):return np.array([int(s[j:j+2],16)for j in [1,3,5]])

def do(pid):
 p=base/pid
 if not(p/'model.json').exists():return
 m=json.loads((p/'model.json').read_text());data=np.fromfile(p/'geometry.bin',dtype='<f4').reshape(-1,6)
 triangles=[];normals=[];colors=[];labels=[]
 def add(v,n,c):triangles.append(v);normals.append(n);colors.append(np.broadcast_to(color(c),(len(v),3)))
 for part in m['parts']:
  a=data[part['byteOffset']//24:part['byteOffset']//24+part['vertexCount']].reshape(-1,3,6);add(a[:,:,:3],a[:,0,3:],part['color'])
 for part in m.get('derivedParts',[]):
  g=part.get('geometry',{});shape=part['kind'];mesh=None
  if shape in ['box','pcb-placeholder']:mesh=trimesh.creation.box(g['sizeMm'])
  if shape=='cylinder':mesh=trimesh.creation.cylinder(g['radiusMm'],g['depthMm'],sections=24)
  if mesh is None:continue
  R=Rxyz(g.get('rotation',[0,0,0]));v=mesh.vertices@R.T+g.get('centerMm',[0,0,0]);add(v[mesh.faces],mesh.face_normals@R.T,part.get('color','#70776e'))
  if shape=='pcb-placeholder':labels.append((g['centerMm'],g['sizeMm']))
 if m['presentation'].get('screen'):
  s=m['presentation']['screen'];w,h=s['sizeMm'];center=np.array(s['positionMm']);R=Rxyz(s.get('rotation',[0,0,0]));palette=['#e66555','#f1ba49','#7fae72','#5d9aba','#826aad','#ca648c']
  if s.get('shape')=='circle':
   for i in range(96):
    a,b=i*2*math.pi/96,(i+1)*2*math.pi/96;v=np.array([[0,0,0],[w/2*math.cos(a),h/2*math.sin(a),0],[w/2*math.cos(b),h/2*math.sin(b),0]]);add((v@R.T+center)[None],(np.array([0,0,1])@R.T)[None],palette[(i//16)%6])
  else:
   for x in range(6):
    for y in range(8):
     x0,x1=(x/6-.5)*w,((x+1)/6-.5)*w;y0,y1=(y/8-.5)*h,((y+1)/8-.5)*h
     v=np.array([[x0,y0,0],[x1,y0,0],[x1,y1,0],[x0,y1,0]])@R.T+center;c=palette[x]if y%2==0 else '#e4ebdd'
     add(v[np.array([[0,1,2],[0,2,3]])],np.broadcast_to(np.array([0,0,1])@R.T,(2,3)),c)
 allv=np.concatenate(triangles);alln=np.concatenate(normals);allc=np.concatenate(colors);N=560;span=max(m['dimensions']['width'],m['dimensions']['height'])*1.30
 sheet=Image.new('RGB',(N*3,N+45),'#eef1ed');draw=ImageDraw.Draw(sheet);views=[('front',np.eye(3)),('back',np.diag([-1,1,-1])),('angle',Rxyz([.17,-.42,0]))]
 review=[]
 for vi,(name,R)in enumerate(views):
  v=allv@R.T;n=alln@R.T;mask=n[:,2]>.0001;v=v[mask];c=allc[mask];n=n[mask]
  pixels=np.stack([(v[:,:,0]/span+.5)*N,(.5-v[:,:,1]/span)*N],axis=2);shade=np.clip(.60+.35*n[:,2]+.08*n[:,1]-.05*n[:,0],.38,1);c=(c*shade[:,None]).astype('uint8')
  xymin=pixels.reshape(-1,2).min(0);xymax=pixels.reshape(-1,2).max(0)
  clipped=bool((xymin<0).any() or (xymax>N).any());assert not clipped,(pid,name,'mesh clipped',xymin,xymax)
  review.append({'view':name,'projectedPixelBounds':[xymin.tolist(),xymax.tolist()],'viewportPixels':[N,N],'clipped':clipped,'visibleTriangles':len(v)})
  projected=np.concatenate([pixels,v[:,:,2,None]],axis=2)
  im,depth=raster(projected,c,N);d=ImageDraw.Draw(im)
  if name in ['front','back']:
   for c1,size in labels:
    q=np.array(c1)@R.T;xx=(q[0]/span+.5)*N;yy=(.5-q[1]/span)*N
    # Annotate a placeholder only when the CPU depth buffer exposes its face.
    # The actual viewer creates its own UNKNOWN text on the physical board face.
    ix,iy=int(xx),int(yy)
    if 0<=ix<N and 0<=iy<N and abs(depth[iy,ix]-(q[2]+size[2]/2))<.05:d.text((xx-20,yy-3),'UNKNOWN',fill='#eaf0e7')
  d.text((12,12),name,fill='#344335');sheet.paste(im,(vi*N,35))
 label='synthetic framebuffer' if m['presentation'].get('screen') else 'no integrated display'
 draw.text((12,8),pid+' | software mesh review | '+label+' | not firmware / not calibrated photo overlay',fill='#344335')
 path=out/(pid+'-review.png');sheet.save(path,optimize=True)
 receipt={'productId':pid,'modelSha256':hashlib.sha256((p/'model.json').read_bytes()).hexdigest(),'reviewSha256':hashlib.sha256(path.read_bytes()).hexdigest(),'softwareViews':review,'screenMode':label,'scope':'CPU orthographic mesh rendering only; no WebGL/browser, firmware or calibrated photo-overlay pass'}
 (out/(pid+'-render.json')).write_text(json.dumps(receipt,indent=2)+'\n');print(pid,flush=True)
for pid in args.ids or [p.parent.name for p in sorted(base.glob('seeed-*/model.json'))]:do(pid)
