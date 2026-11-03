#!/usr/bin/env python3
"""Rebuild the retained StampFly assembly and check its source/visual contract.

Uses the committed multi-source plan and original photo crops, with no download
unless --allow-network is explicit. All output goes to a fresh scratch folder.
"""
import argparse,importlib.util,json,struct
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]

def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);loaded=importlib.util.module_from_spec(spec);spec.loader.exec_module(loaded);return loaded

def verify(product):
 plan=product['plan'];meta=product['metadata'];sources=product['byId']
 if plan.get('preserveMesh') is not True:raise ValueError('StampFly pre-split sources must preserve their original open triangles')
 if meta['provenance']['repository']!='https://github.com/M5Fly-kanazawa/stampfly_ecosystem':raise ValueError('Do not relabel the linked simulator source as M5Stack production CAD')
 for source in sources.values():
  if source['id']=='ecosystem-pcb-stl':
   path=next(p for p in (product['assets']/name for name,h in meta['artifacts'].items() if h==source['sha256']) if p.exists())
   count=struct.unpack_from('<I',path.read_bytes(),80)[0]
   if count!=1036:raise ValueError('The pinned PCB has1036 triangles; automatic hole repair would silently add four')
   selected=sum(b-a for part in plan['parts'] if part.get('sourceId')==source['id'] for a,b in part.get('sourceFaceRanges',[]))
   omitted=sum(b-a for part in plan['omittedComponents'] if part.get('sourceId')==source['id'] for a,b in part.get('sourceFaceRanges',[]))
   if selected+omitted!=count:raise ValueError('PCB source triangles must be accounted for exactly')
 for key in ['fl','fr','rl','rr']:
  for side in ['front','back']:
   names={s['name'] for s in plan['surfaces'] if s['name'].startswith('propeller-'+key+'-') and s['side']==side}
   if names!={f'propeller-{key}-blade-{i}-{side}' for i in range(4)}:raise ValueError('Official K138 photos show four blades per rotor')
 pcb=next(p for p in meta['parts'] if p['name']=='pcb-main')
 if pcb['role']!='pcb' or pcb['color']!='#141816' or pcb['texture']['zPlanesMm']!=[5.752010345458984]:raise ValueError('PCB photo must stay on the black source board top plane')
 marker=next(p for p in meta['derivedParts'] if p['id']=='pcb-unknown-marker')
 back=next(p for p in meta['parts'] if p['name']=='pcb-back')
 if back['role']!='pcb' or back['texture']['zPlanesMm']!=[4.25201035]:raise ValueError('Back photo belongs on the original carrier underside')
 if marker['geometry']['labelSide']!='front':raise ValueError('UNKNOWN belongs on the undocumented inward daughterboard face')
 if any(p['material']['opacity']!=.38 for p in meta['parts'] if p['role']=='propeller'):raise ValueError('Propeller blades must retain their photo-derived translucent material')
 return {'sourcePcbTriangles':count,'rotors':4,'bladesPerRotor':4,'sourceScale':meta['sourceTransform']['scale']}

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--scratch',type=Path,required=True);parser.add_argument('--allow-network',action='store_true');args=parser.parse_args()
 rebuild=module('rebuild_product_model',ROOT/'tools/rebuild-product-model.py');product=rebuild.load_product(ROOT,'m5stack-k138');contract=verify(product);result=rebuild.build(product,args.scratch,args.allow_network)
 print(json.dumps({'passed':result['passed'],'verifiedArtifacts':len(result['verifiedArtifacts']),'contract':contract},indent=2))
if __name__=='__main__':main()
