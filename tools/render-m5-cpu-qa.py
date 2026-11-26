#!/usr/bin/env python3
"""Blender CPU render of export-m5-cpu-models.mjs output; no source edits.
blender -b -t 2 --python tools/render-m5-cpu-qa.py -- --repo REPO --input EXPORT --out OUTPUT
"""
import argparse, json, math, hashlib, sys, pathlib, re
import bpy
import numpy as np
from mathutils import Vector, Matrix
from PIL import Image, ImageDraw, ImageFont
args=sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else []
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);p.add_argument('--input',required=True);p.add_argument('--out',required=True);p.add_argument('--ids');p.add_argument('--size',type=int,default=400);p.add_argument('--samples',type=int,default=16);p.add_argument('--only-view',choices=['front','back','oblique','rear-oblique','rear-side']);p.add_argument('--back-oblique',action='store_true');p.add_argument('--oblique-eye',help='Comma-separated studio camera XYZ; overrides the oblique direction only');p.add_argument('--appearance',choices=['original','gold','purple'],default='original');p.add_argument('--views',choices=['default','rear','front','oblique'],default='default');a=p.parse_args(args)
oblique_eye=tuple(float(v)for v in a.oblique_eye.split(','))if a.oblique_eye else (1.7,1.1,-3 if a.back_oblique else 3)
if len(oblique_eye)!=3 or not all(math.isfinite(v)for v in oblique_eye)or math.hypot(oblique_eye[0],oblique_eye[2])<1e-9:p.error('--oblique-eye requires finite XYZ with a nonzero X/Z direction')
repo=pathlib.Path(a.repo).resolve();src=pathlib.Path(a.input).resolve();out=pathlib.Path(a.out).resolve();out.mkdir(parents=True,exist_ok=True)
ids=a.ids.split(',') if a.ids else [x['id'] for x in json.loads((src/'export-receipt.json').read_text())['products'] if not x['errors']]
def digest(file):return hashlib.sha256(pathlib.Path(file).read_bytes()).hexdigest()
def text_image(m,i):
 spec=m['map'];w,h=spec['size'];im=Image.new('RGBA',(w,h),(0,0,0,0));d=ImageDraw.Draw(im)
 for t in spec['text']:
  size=int(re.search(r'(\d+)px',t['font']).group(1));font_path='/usr/share/fonts/truetype/dejavu/DejaVuSansMono'+('-Bold' if 'bold' in t['font'] else '')+'.ttf';font=ImageFont.truetype(font_path,size)
  bbox=d.textbbox((0,0),t['text'],font=font);x=t['x'];y=t['y']
  if t['textAlign']=='center':x-=(bbox[2]-bbox[0])/2
  if t['textBaseline']=='middle':y-=(bbox[3]-bbox[1])/2+bbox[1]
  d.text((x,y),t['text'],font=font,fill=t['fillStyle'])
 file=out/('derived-text-%s.png'%i);im.save(file);return file

def material(m,i):
 mat=bpy.data.materials.new('source-'+str(i));mat.use_nodes=True;nt=mat.node_tree;nt.nodes.clear();output=nt.nodes.new('ShaderNodeOutputMaterial');transparent=nt.nodes.new('ShaderNodeBsdfTransparent');mix=nt.nodes.new('ShaderNodeMixShader');nt.links.new(transparent.outputs[0],mix.inputs[1]);nt.links.new(mix.outputs[0],output.inputs['Surface'])
 color=nt.nodes.new('ShaderNodeRGB');color.outputs[0].default_value=(*m['color'],1);color_out=color.outputs[0];alpha_out=None
 spec=m['map']
 if spec and spec.get('constant') is not None:color.outputs[0].default_value=tuple(v/255 for v in spec['constant'])
 elif spec and (spec.get('file') or spec.get('text')):
  file=repo/spec['file'] if spec.get('file') else text_image(m,i)
  if spec.get('sha256') and digest(file)!=spec['sha256']:raise ValueError('Texture changed '+str(file))
  image=bpy.data.images.load(str(file),check_existing=True);image.colorspace_settings.name='sRGB';image.alpha_mode='STRAIGHT';tex=nt.nodes.new('ShaderNodeTexImage');tex.image=image;tex.interpolation='Linear';tex.extension='REPEAT' if spec.get('wrapS')==1000 or spec.get('wrapT')==1000 else 'EXTEND'
  mul=nt.nodes.new('ShaderNodeMixRGB');mul.blend_type='MULTIPLY';mul.inputs[0].default_value=1;nt.links.new(color_out,mul.inputs[1]);nt.links.new(tex.outputs['Color'],mul.inputs[2]);color_out=mul.outputs[0];alpha_out=tex.outputs['Alpha']
 if m['unlit']:
  shader=nt.nodes.new('ShaderNodeEmission');nt.links.new(color_out,shader.inputs['Color']);shader.inputs['Strength'].default_value=1
 else:
  shader=nt.nodes.new('ShaderNodeBsdfPrincipled');nt.links.new(color_out,shader.inputs['Base Color']);shader.inputs['Roughness'].default_value=m['roughness'];shader.inputs['Metallic'].default_value=m['metalness']
  if 'Emission Color' in shader.inputs:shader.inputs['Emission Color'].default_value=(*m['emission'],1);shader.inputs['Emission Strength'].default_value=m['emissiveIntensity']
 nt.links.new(shader.outputs[0],mix.inputs[2])
 if alpha_out is None:
  val=nt.nodes.new('ShaderNodeValue');val.outputs[0].default_value=1;alpha_out=val.outputs[0]
 if m['alphaTest']>0:
  clip=nt.nodes.new('ShaderNodeMath');clip.operation='GREATER_THAN';clip.inputs[1].default_value=m['alphaTest'];nt.links.new(alpha_out,clip.inputs[0]);alpha_out=clip.outputs[0]
 opacity=nt.nodes.new('ShaderNodeMath');opacity.operation='MULTIPLY';opacity.inputs[1].default_value=m['opacity'];nt.links.new(alpha_out,opacity.inputs[0]);alpha_out=opacity.outputs[0]
 if not m['doubleSided']:
  geom=nt.nodes.new('ShaderNodeNewGeometry');front=nt.nodes.new('ShaderNodeMath');front.operation='SUBTRACT';front.inputs[0].default_value=1;nt.links.new(geom.outputs['Backfacing'],front.inputs[1]);both=nt.nodes.new('ShaderNodeMath');both.operation='MULTIPLY';nt.links.new(alpha_out,both.inputs[0]);nt.links.new(front.outputs[0],both.inputs[1]);alpha_out=both.outputs[0]
 nt.links.new(alpha_out,mix.inputs[0]);return mat

def look(camera,eye,target):
 camera.location=eye;forward=(target-eye).normalized();right=forward.cross(Vector((0,1,0))).normalized();up=right.cross(forward).normalized();camera.rotation_euler=Matrix((right,up,-forward)).transposed().to_euler();return right,up,forward

def new_light(name,location,energy,size,target):
 data=bpy.data.lights.new(name,'AREA');data.energy=energy;data.shape='DISK';data.size=size;obj=bpy.data.objects.new(name,data);bpy.context.collection.objects.link(obj);obj.location=location;obj.rotation_euler=(target-location).to_track_quat('-Z','Y').to_euler();return obj
rows=[]
for identity in ids:
 meta=json.loads((src/(identity+'.json')).read_text());raw=(src/meta['geometryFile']).read_bytes();assert hashlib.sha256(raw).hexdigest()==meta['geometrySha256']
 bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
 for old in list(bpy.data.materials):bpy.data.materials.remove(old)
 scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=a.samples;scene.cycles.use_denoising=False;scene.cycles.max_bounces=5;scene.cycles.transparent_max_bounces=8;scene.render.threads_mode='FIXED';scene.render.threads=2;scene.render.resolution_x=a.size;scene.render.resolution_y=a.size;scene.render.resolution_percentage=100;scene.render.image_settings.file_format='PNG';scene.render.film_transparent=False
 scene.view_settings.view_transform='Standard';scene.view_settings.look='Medium High Contrast' if 'Medium High Contrast' in [i.identifier for i in scene.view_settings.bl_rna.properties['look'].enum_items] else 'None';scene.view_settings.exposure=0;scene.view_settings.gamma=1
 world=bpy.data.worlds.new('neutral studio');scene.world=world;world.use_nodes=True;world.node_tree.nodes['Background'].inputs['Color'].default_value=(.78,.80,.83,1);world.node_tree.nodes['Background'].inputs['Strength'].default_value=.65
 mats=[material({**m,**(meta.get('appearanceVariants',{}).get(a.appearance,[{}]*len(meta['materials']))[i])},identity+'-'+str(i)) for i,m in enumerate(meta['materials'])]
 positions=[]
 for m in meta['meshes']:
  positions.append(np.frombuffer(raw,dtype='<f4',count=m['vertices']*3,offset=m['positionOffset']).reshape(-1,3))
 all_positions=np.vstack(positions);lo=all_positions.min(axis=0);hi=all_positions.max(axis=0);center=(lo+hi)/2;scale=1/float(max(hi-lo))
 for m,pos in zip(meta['meshes'],positions):
  count=m['vertices'];indices=np.frombuffer(raw,dtype='<u4',count=m['triangles']*3,offset=m['indexOffset']);triangles=len(indices)//3;mesh=bpy.data.meshes.new(m['name']);mesh.vertices.add(count);mesh.vertices.foreach_set('co',((pos-center)*scale).ravel());mesh.loops.add(len(indices));mesh.loops.foreach_set('vertex_index',indices);mesh.polygons.add(triangles);mesh.polygons.foreach_set('loop_start',np.arange(triangles,dtype=np.int32)*3);mesh.polygons.foreach_set('loop_total',np.full(triangles,3,dtype=np.int32));mesh.update();mesh.materials.append(mats[m['material']]);obj=bpy.data.objects.new(m['name'],mesh);bpy.context.collection.objects.link(obj)
  if m['uvOffset'] is not None:
   uv=np.frombuffer(raw,dtype='<f4',count=count*2,offset=m['uvOffset']).reshape(-1,2).copy();spec=meta['materials'][m['material']]['map']
   if spec:
    mat=spec['matrix'];u=uv[:,0].copy();v=uv[:,1].copy();uv[:,0]=mat[0]*u+mat[3]*v+mat[6];uv[:,1]=mat[1]*u+mat[4]*v+mat[7]
    if not spec['flipY']:uv[:,1]=1-uv[:,1]
   layer=mesh.uv_layers.new(name='source-uv');layer.data.foreach_set('uv',uv[indices].ravel())
  if m['normalOffset'] is not None:
   normal=np.frombuffer(raw,dtype='<f4',count=count*3,offset=m['normalOffset']).reshape(-1,3);mesh.polygons.foreach_set('use_smooth',np.ones(triangles,dtype=np.bool_));mesh.normals_split_custom_set(normal[indices].tolist())
 data=bpy.data.cameras.new('QA camera');cam=bpy.data.objects.new('QA camera',data);bpy.context.collection.objects.link(cam);scene.camera=cam;data.type='ORTHO';target=Vector((0,0,0));corners=[Vector(((x-center[0])*scale,(y-center[1])*scale,(z-center[2])*scale)) for x in[lo[0],hi[0]] for y in[lo[1],hi[1]] for z in[lo[2],hi[2]]]
 views=[]
 views_to_render=[('back',Vector((0,0,-3))),('rear-oblique',Vector(oblique_eye if a.oblique_eye else (1.7,1.1,-3))),('rear-side',Vector((3,.1,-.35)))] if a.views=='rear' else [('front',Vector((0,0,3))),('back',Vector((0,0,-3))),('oblique',Vector(oblique_eye))]
 if a.views in ['front','oblique']:views_to_render=[(a.views,Vector((0,0,3) if a.views=='front' else oblique_eye))]
 if a.only_view:views_to_render=[(view,eye)for view,eye in views_to_render if view==a.only_view]
 if not views_to_render:raise ValueError('Requested view is not in selected view set')
 for view,eye in views_to_render:
  right,up,forward=look(cam,eye,target);xs=[c.dot(right) for c in corners];ys=[c.dot(up) for c in corners];data.ortho_scale=max(max(xs)-min(xs),max(ys)-min(ys))*1.18
  key=new_light('key',target-forward*2.5-right*1.6+up*2,140,3,target);fill=new_light('fill',target-forward*1.8+right*2-up*.2,60,3,target)
  file=out/(identity+'-'+view+'.png');scene.render.filepath=str(file);bpy.ops.render.render(write_still=True);views.append({'view':view,'file':file.name,'sha256':digest(file),'cameraEye':list(eye),'orthoScale':data.ortho_scale});bpy.data.objects.remove(key,do_unlink=True);bpy.data.objects.remove(fill,do_unlink=True)
 row={'id':identity,'name':meta['name'],'appearance':a.appearance,'samples':a.samples,'imageSize':a.size,'sourceModelSha256':meta['sourceModelSha256'],'geometrySha256':meta['geometrySha256'],'sourceBoundsMm':[lo.tolist(),hi.tolist()],'studioUniformScale':scale,'views':views,'references':meta['references']};rows.append(row);(out/'render-receipt.json').write_text(json.dumps({'renderer':bpy.app.version_string,'engine':'Cycles CPU','threads':2,'samples':a.samples,'scope':'Actual 3D CPU raster images from production model meshes/UV/alpha/materials; not a browser screenshot or calibrated photograph overlay. Canvas text uses a documented DejaVu Sans Mono substitute. Blank screen is not firmware output.','products':rows},indent=2)+'\n');print('CPU_QA_RENDERED',identity,flush=True)
