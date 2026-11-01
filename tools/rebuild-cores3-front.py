#!/usr/bin/env python3
"""Rebuild the source-backed CoreS3 front optics and reusable head-only contract.

Run --write --scratch <fresh directory> with tools/requirements-model.txt installed;
--check only needs the Python standard library. No download or AI-generated pixels.
"""
import argparse, copy, hashlib, importlib.util, json, shutil
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web/platform/assets/m5stack-k128'
DXF_SHA = 'c97a042b573bdc2e0f7c09897420b9352ccc941a7705ce18caf59356fb17fc42'
DXF_ID = 'dimension-drawing-74afac7b92'
HEAD_NAMES = ['shell-front', 'sensor-window-support', 'front-glass']
OPTIC_IDS = ['camera-bezel', 'camera-window', 'camera-lens', 'microphone-left', 'microphone-right', 'ltr-window-1', 'ltr-window-2']
def digest(data): return hashlib.sha256(data).hexdigest()
def read(path): return json.loads(path.read_text())
def write(path, value): path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
def module(path, name):
 spec = importlib.util.spec_from_file_location(name, path); result = importlib.util.module_from_spec(spec); spec.loader.exec_module(result); return result

def entities():
 data = (ASSETS / 'source-cores3.dxf').read_bytes()
 if digest(data) != DXF_SHA: raise ValueError('CoreS3 DXF SHA-256 mismatch')
 lines = data.decode().splitlines(); result = {}; current = None; active = False
 for code, value in zip(lines[::2], lines[1::2]):
  code, value = code.strip(), value.strip()
  if code == '2' and value == 'ENTITIES': active = True; continue
  if not active: continue
  if code == '0':
   if current and '5' in current: result[current['5'][0]] = current
   if value == 'ENDSEC': break
   current = {'type': value}
  elif current is not None: current.setdefault(code, []).append(value)
 return result

def front_spec():
 e = entities()
 f = lambda handle, key: float(e[handle][key][0])
 # Dimension witness lines enclose the 54 mm square; no photo is used as a scale ruler.
 cx = (f('6D1', '13') + f('6D1', '14')) / 2
 cy = (f('6D2', '23') + f('6D2', '24')) / 2
 xy = lambda handle: [round(f(handle, '10') - cx, 6), round(f(handle, '20') - cy, 6)]
 def circle(handle, identity, color, z, depth, roughness):
  if e[handle]['type'] != 'CIRCLE': raise ValueError('DXF optical entity changed')
  return {'id': identity, 'kind': 'cylinder', 'color': color, 'material': {'roughness': roughness, 'metalness': 0}, 'geometry': {'radiusMm': round(f(handle, '40'), 6), 'depthMm': depth, 'centerMm': [*xy(handle), z]}, 'basis': 'cad-measurement', 'sourceIds': [DXF_ID, 'product-photo-86528c8744', 'product-photo-fbcb404695'], 'notes': f'DXF entity {handle} fixes optical center and radius. Front-facing visual window only; depth, optical color and roughness are uncalibrated presentation estimates, not internal sensor-package CAD.'}
 optics = [circle('10B8', 'camera-bezel', '#9b5732', 15.812, .02, .56), circle('10BA', 'camera-window', '#090d0f', 15.826, .02, .25), circle('10BB', 'camera-lens', '#20373d', 15.838, .012, .16), circle('109A', 'microphone-left', '#030505', 15.815, .02, .9), circle('10BC', 'microphone-right', '#030505', 15.815, .02, .9), circle('10BD', 'ltr-window-1', '#304749', 15.815, .02, .22), circle('10BE', 'ltr-window-2', '#304749', 15.815, .02, .22)]
 glass = {'sizeMm': [round(f('75B','10') - f('71E','10'), 6), round(f('75F','20') - f('724','20'), 6)], 'radiusMm': round(f('724','10') - f('71E','10'), 6)}
 screen_center = [round((f('734','10') + f('734','11'))/2 - cx, 6), round((f('734','20') + f('756','20'))/2 - cy, 6)]
 return {'centerDxfMm': [cx, cy], 'glass': glass, 'screenCenterMm': screen_center, 'screenWindowMm': [round(abs(f('734','10')-f('734','11')),6), round(abs(f('759','20')-f('759','21')),6)], 'optics': optics}

def configure(plan):
 plan = copy.deepcopy(plan); spec = front_spec()
 plan['derivedParts'] = [p for p in plan['derivedParts'] if p['id'] not in OPTIC_IDS] + spec['optics']
 surface = next(p for p in plan['surfaces'] if p['name'] == 'front-glass')
 surface.update(spec['glass'], sourceIds=[DXF_ID, 'product-photo-86528c8744'], notes='DXF TMP_DWG front-cover outline: 50.8 mm square with R3.4 corners. Z=15.8 mm is a display-stack estimate 0.05 mm beyond the source shell; no photographed screen pixels are drawn.')
 screen = plan['presentation']['screen']; screen['positionMm'] = [*spec['screenCenterMm'], 15.85]
 screen['sourceIds'] = [DXF_ID, 'official-docs']
 screen['basis'] = 'Centered using DXF entities 734/756/759 (41.6×31.4 mm optical window). 40.64×30.48 mm active LCD is the nominal 2-inch 4:3 display area; physical stack Z remains estimated.'
 return plan

def contract(meta):
 spec = front_spec()
 parts = [p for p in meta['parts'] if p['name'] in HEAD_NAMES]
 return {'schemaVersion':'1', 'id':'m5stack-cores3-front', 'productId':'m5stack-k128', 'unit':'mm', 'binary':'geometry.bin', 'binarySha256':meta['artifacts']['geometry.bin'], 'strideFloats':6, 'parts':parts, 'derivedParts':[p for p in meta['derivedParts'] if p['id'] in OPTIC_IDS], 'screen':meta['presentation']['screen'], 'assemblyPartNames':HEAD_NAMES+OPTIC_IDS+['live-display'], 'sourceCad':{'sourceId':'manufacturer-cad-c6394d7bc8', 'file':'source.stl', 'sha256':meta['provenance']['sourceSha256'], 'components':{'shell-front':0,'sensor-window-support':2}, 'matrixRows':meta['sourceTransform']['matrixRows']}, 'anchors':{'rearMatingPlane':{'positionMm':[0,0,.25],'normal':[0,0,-1]}, 'shellFrontPlane':{'positionMm':[0,0,15.75],'normal':[0,0,1]}, 'glassFront':{'positionMm':[0,0,15.8],'normal':[0,0,1]}}, 'headEnvelopeMm':[54,54,15.5], 'dxfCoordinates':{k:v for k,v in spec.items() if k!='optics'}, 'sourceIds':['manufacturer-cad-c6394d7bc8',DXF_ID,'product-photo-86528c8744','product-photo-fbcb404695','official-docs'], 'excludedParts':[p['name'] for p in meta['parts'] if p['name'] not in HEAD_NAMES]+[p['id'] for p in meta['derivedParts'] if p['id'] not in OPTIC_IDS], 'integration':'Choose the host mating-plane transform from its own CAD and photographs. Transform selected vertices/normals and derived centers rigidly, carry all assemblyPartNames with the host head joint, and use the centered screen contract. Never reuse DinBase, mounting feet, host PCB placeholders or inferred mating offsets. Manufacturer source parts use hostGlobal^-1 * desiredWorld * sourceCad.matrixRows as their per-part transform.', 'limitations':['Only front visual apertures are represented; no invented internal camera/microphone/LTR packages or PCB layout.', 'Lens colors, material response and thin overlay depth are approximate.', 'The glass and active LCD Z stack are estimated; DXF provides planar outline and aperture centers.', 'No physical unit or camera/microphone/proximity firmware behavior is verified by this contract.']}

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--write',action='store_true');p.add_argument('--scratch');args=p.parse_args()
 if args.check == args.write: p.error('Choose exactly one of --check or --write')
 meta=read(ASSETS/'model.json');plan=read(ASSETS/'model-plan.json');configured=configure(plan)
 if args.check:
  if configured != plan: raise ValueError('Plan does not match the pinned CoreS3 DXF')
  for key in ['presentation','derivedParts']:
   if meta[key] != plan[key]: raise ValueError(f'Model {key} differs from source plan')
  if contract(meta) != read(ASSETS/'front-assembly.json'): raise ValueError('Reusable front contract is stale')
  if digest((ASSETS/'geometry.bin').read_bytes()) != meta['artifacts']['geometry.bin']: raise ValueError('Geometry SHA-256 mismatch')
  print('PASS CoreS3 DXF optical geometry, centered LCD and head-only assembly contract');return
 if not args.scratch: p.error('--write requires a fresh --scratch directory')
 rebuild=module(ROOT/'tools/rebuild-product-model.py','core_rebuild');product=rebuild.load_product(ROOT,'m5stack-k128');product['plan']=configured
 scratch=Path(args.scratch).resolve();folder,_=rebuild.hydrate(product,scratch);exporter=rebuild.exporter(ROOT);exporter.TEMP=scratch/'sources';exporter.REV=meta['provenance']['commit'];out=scratch/'front-output'
 if out.exists(): raise ValueError('Scratch output exists; choose a fresh directory')
 actual,_=exporter.build_model(read(folder/'prepared.json'),configured,out,evidence_root=scratch/'evidence',license_path=folder/product['byId']['cad-license']['archiveName'])
 actual['artifacts']['source-cores3.dxf']=DXF_SHA
 write(ASSETS/'model-plan.json',configured);write(ASSETS/'model.json',actual);shutil.copyfile(out/'geometry.bin',ASSETS/'geometry.bin');write(ASSETS/'front-assembly.json',contract(actual))
 print('Updated CoreS3 optical geometry and head-only front-assembly.json from pinned DXF')
if __name__=='__main__': main()
