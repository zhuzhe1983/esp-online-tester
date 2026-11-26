#!/usr/bin/env python3
"""Reproduce five sourced cap mappings offline without retessellating frozen native geometry."""
import argparse,copy,gzip,hashlib,json,pathlib,shutil,struct
IDS=['m5stack-k049-b','m5stack-k049','m5stack-k049-c','m5stack-k048','m5stack-k146']
ROOT=pathlib.Path(__file__).resolve().parents[1]
def digest(b):return hashlib.sha256(b).hexdigest()
def write(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def build(sku,source_root,plan_root,output):
 source=source_root/'web/platform/assets'/sku;plans=plan_root/'web/platform/assets'/sku;out=output/sku
 if out.resolve()in [source.resolve(),plans.resolve()]:raise ValueError('Separate output required')
 out.mkdir(parents=True,exist_ok=True);plan=json.loads((plans/'model-plan.json').read_text());recipe=plan['controlSurfaceRecipe'];base=recipe['baseModel'];frozen=(plans/base['file']).read_bytes()
 if digest(frozen)!=base['sha256']:raise ValueError('Frozen metadata changed')
 raw=gzip.decompress(frozen)
 if digest(raw)!=base['expandedSha256']:raise ValueError('Expanded native metadata changed')
 model=json.loads(raw);original=copy.deepcopy(model)
 if model['id']!=sku or recipe['baseGeometry']['file']!=model['binary']:raise ValueError('Native product identity changed')
 for name,expected in original['artifacts'].items():
  if pathlib.Path(name).name!=name:raise ValueError('Unsafe artifact filename')
  payload=(source/name).read_bytes()
  if digest(payload)!=expected:raise ValueError('Original artifact changed: '+name)
  (out/name).write_bytes(payload)
 binary=(out/model['binary']).read_bytes()
 if digest(binary)!=recipe['baseGeometry']['sha256']:raise ValueError('Native geometry changed')
 native=gzip.decompress(binary)if model.get('binaryCompression')=='gzip'else binary;parts=[]
 for part in original['parts']:
  ranges=recipe['nativePartitions'].get(part['name'])
  if not ranges:parts.append(part);continue
  expected_start=0
  for item in ranges:
   if len(item['triangleRanges'])!=1:raise ValueError('Each partition must be contiguous')
   start,end=item['triangleRanges'][0]
   if not all(isinstance(x,int)for x in [start,end])or start!=expected_start or end<=start or end>part['vertexCount']//3:raise ValueError('Partition gap, overlap or invalid range')
   new=copy.deepcopy(part);new.update({k:v for k,v in item.items()if k!='triangleRanges'});new['byteOffset']=part['byteOffset']+start*72;new['vertexCount']=(end-start)*3
   values=list(struct.iter_unpack('<6f',native[new['byteOffset']:new['byteOffset']+new['vertexCount']*24]));new['boundsMm']=[[min(v[i]for v in values)for i in range(3)],[max(v[i]for v in values)for i in range(3)]];new['nativeTriangleRanges']=item['triangleRanges'];parts.append(new);expected_start=end
  if expected_start!=part['vertexCount']//3:raise ValueError('Incomplete native partition')
 model['parts']=parts
 for key in ['presentation','derivedParts','limitations']:model[key]=copy.deepcopy(plan[key])
 # Expose a retained exact-SKU side view used for the actuator derivation.
 for photo in plan.get('photos',[]):
  if any(x['sourceId']==photo['sourceId']for x in model['referencePhotos']):continue
  record=next(x for x in model['provenance']['sources']if x['id']==photo['sourceId']);name='reference-'+record['archiveName'];payload=(source/name).read_bytes()
  if digest(payload)!=record['sha256']:raise ValueError('Actuator reference photo changed')
  (out/name).write_bytes(payload);model['artifacts'][name]=record['sha256'];model['referencePhotos'].append({**photo,'file':name,'sourceSha256':record['sha256']})
 (out/base['file']).write_bytes(frozen);model['artifacts'][base['file']]=base['sha256'];model['provenance']['controlSurfaceRebuild']={'recipe':'model-plan.json#controlSurfaceRecipe','builder':'tools/rebuild-m5-control-models.py','baseModelSha256':base['expandedSha256'],'nativeGeometryPolicy':'Exact frozen native bytes; original source receipts unchanged. Metadata partitions do not reorder or recompute vertices/normals.'};write(out/'model.json',model);shutil.copyfile(plans/'model-plan.json',out/'model-plan.json')
 return {'productId':sku,'baseModelSha256':base['expandedSha256'],'modelSha256':digest((out/'model.json').read_bytes()),'planSha256':digest((plans/'model-plan.json').read_bytes()),'geometryFile':model['binary'],'geometrySha256':digest(binary),'originalGeometryByteIdentical':True,'allOriginalArtifactsPreserved':True,'nativeParts':len(parts),'newDerivedCaps':sum(x['id'].startswith('rotary-cap-')for x in model['derivedParts']),'visibleGeometryDelta':recipe['visibleGeometryDelta']}
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source-root',type=pathlib.Path,default=ROOT);p.add_argument('--plan-root',type=pathlib.Path,default=ROOT);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--ids',default=','.join(IDS));a=p.parse_args();rows=[]
 for sku in a.ids.split(','):
  if sku not in IDS:raise ValueError('Unknown SKU')
  rows.append(build(sku,a.source_root.resolve(),a.plan_root.resolve(),a.output.resolve()))
 write(a.output/'control-rebuild-receipt.json',{'products':rows});print(json.dumps(rows,indent=2))
if __name__=='__main__':main()
