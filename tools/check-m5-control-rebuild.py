#!/usr/bin/env python3
"""Exercise all five public offline rebuild dispatch paths against immutable native inputs."""
import argparse,hashlib,importlib.util,json,pathlib,sys,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
IDS=['m5stack-k049-b','m5stack-k049','m5stack-k049-c','m5stack-k048','m5stack-k146']
def load(p,name):
 spec=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source-root',required=True,type=pathlib.Path);p.add_argument('--plan-root',default=ROOT,type=pathlib.Path);p.add_argument('--out',required=True,type=pathlib.Path);a=p.parse_args();root=a.source_root.resolve();own=a.plan_root.resolve();rows=[];sys.path.insert(0,str(root/'tools'))
 with tempfile.TemporaryDirectory(prefix='m5-control-public-rebuild-')as tmp:
  stage=pathlib.Path(tmp);(stage/'tools').symlink_to(own/'tools',target_is_directory=True);(stage/'docs').symlink_to(root/'docs',target_is_directory=True);assets=stage/'web/platform/assets';assets.mkdir(parents=True)
  for sku in IDS:
   source=root/'web/platform/assets'/sku;local=own/'web/platform/assets'/sku;folder=assets/sku;folder.mkdir()
   for f in source.iterdir():
    if f.is_file()and not(local/f.name).is_file():(folder/f.name).symlink_to(f)
   for name in ['model.json','model-plan.json','control-base-model.json.gz']:(folder/name).symlink_to(local/name)
   output=stage/'output'
   if sku in ['m5stack-k049','m5stack-k049-c']:
    name='build-m5-classic-expansion-models.py'if sku=='m5stack-k049'else'build-m5-legacy-five-models.py';module=load(own/'tools'/name,'public_'+sku.replace('-','_'));module.ROOT=stage
    if hasattr(module,'ASSETS'):module.ASSETS=assets
    module.build(sku,output);destination=output/sku
   else:
    module=load(own/'tools/rebuild-product-model.py','public_'+sku.replace('-','_'));product=module.load_product(stage,sku);comparison=module.build(product,stage/('scratch-'+sku));assert comparison['passed'];destination=stage/('scratch-'+sku)/'rebuilt'/sku
   expected=json.loads((local/'model.json').read_text());assert(destination/'model.json').read_bytes()==(local/'model.json').read_bytes();assert(destination/expected['binary']).read_bytes()==(source/expected['binary']).read_bytes();rows.append({'productId':sku,'publicBuilder':module.__file__.split('/')[-1],'modelSha256':sha(local/'model.json'),'nativeGeometryByteIdentical':True,'committedModelByteIdentical':True})
  builder=load(own/'tools/rebuild-m5-control-models.py','control_bad_inputs');victim='m5stack-k146';copy=stage/'bad-plan';folder=copy/'web/platform/assets'/victim;folder.mkdir(parents=True)
  import shutil
  for name in ['model.json','model-plan.json','control-base-model.json.gz']:shutil.copyfile(own/'web/platform/assets'/victim/name,folder/name)
  plan=json.loads((folder/'model-plan.json').read_text());tests=[]
  for kind in ['snapshot-hash','native-geometry-hash','partition-overlap']:
   bad=json.loads(json.dumps(plan))
   if kind=='snapshot-hash':bad['controlSurfaceRecipe']['baseModel']['sha256']='0'*64
   elif kind=='native-geometry-hash':bad['controlSurfaceRecipe']['baseGeometry']['sha256']='0'*64
   else:next(iter(bad['controlSurfaceRecipe']['nativePartitions'].values()))[0]['triangleRanges'][0][0]=1
   (folder/'model-plan.json').write_text(json.dumps(bad))
   try:builder.build(victim,root,copy,stage/('rejected-'+kind))
   except ValueError as e:tests.append({'kind':kind,'rejected':True,'error':str(e)})
   else:raise AssertionError('Tampered input accepted: '+kind)
  a.out.parent.mkdir(parents=True,exist_ok=True);a.out.write_text(json.dumps({'products':rows,'tamperNegatives':tests,'scope':'Actual generic/classic/legacy public dispatchers; offline fixed-input reconstruction only. No geometry retessellation, download, device or factory acceptance.'},indent=2)+'\n');print('5 public rebuilds and 3 tamper negatives passed')
if __name__=='__main__':main()
