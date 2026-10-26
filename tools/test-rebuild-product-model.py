#!/usr/bin/env python3
"""Small offline tests for exact-source reconstruction boundaries; no CAD import."""
import importlib.util,io,json,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
spec=importlib.util.spec_from_file_location('reproduce',Path(__file__).with_name('rebuild-product-model.py'));r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
class RebuildInputs(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)/'repo';self.assets=self.root/'web/platform/assets/test-device';self.assets.mkdir(parents=True);self.scratch=Path(self.temp.name)/'scratch';self.data={'source.stl':b'pinned CAD input','LICENSE-M5Stack.txt':b'pinned license'}
  self.sources=[{'id':sid,'status':'fetched','sha256':r.digest(data),'bytes':len(data),'archiveName':archive,'url':'https://example.org/'+archive,'fetchedAt':'2026-01-01T00:00:00Z'}for sid,archive,data in [('cad','cad.stl',self.data['source.stl']),('cad-license','license.txt',self.data['LICENSE-M5Stack.txt'])]]
  self.plan={'sourceId':'cad','parts':[],'photos':[]};self.meta={'artifacts':{name:r.digest(data)for name,data in self.data.items()},'provenance':{'sources':self.sources,'commit':'a'*40}}
  for name,data in self.data.items():(self.assets/name).write_bytes(data)
  (self.assets/'model-plan.json').write_text(json.dumps(self.plan));self.save_meta();self.product=r.load_product(self.root,'test-device')
 def save_meta(self):(self.assets/'model.json').write_text(json.dumps(self.meta))
 def test_offline_reuse_keeps_original_time_and_records_new_acquisition(self):
  with patch.object(r,'fetch_exact',side_effect=AssertionError('network forbidden')):
   folder,receipt=r.hydrate(self.product,self.scratch)
   self.assertEqual((folder/'cad.stl').read_bytes(),self.data['source.stl']);self.assertEqual(receipt['sources'][0]['originalFetchedAt'],'2026-01-01T00:00:00Z');self.assertEqual(receipt['sources'][0]['method'],'committed-asset')
   _,cached=r.hydrate(self.product,self.scratch);self.assertTrue(all(s['method']=='verified-scratch-cache'for s in cached['sources']))
 def test_changed_local_source_fails_even_if_network_allowed(self):
  (self.assets/'source.stl').write_bytes(b'changed CAD data')
  with patch.object(r,'fetch_exact',side_effect=AssertionError('must not fetch replacement')):
   with self.assertRaisesRegex(ValueError,'changed'):r.hydrate(self.product,self.scratch,True)
 def test_missing_source_requires_explicit_network(self):
  (self.assets/'source.stl').unlink()
  with self.assertRaisesRegex(ValueError,'--allow-network'):r.hydrate(self.product,self.scratch)
 def test_source_receipt_conflict_fails(self):
  audit=self.root/'docs/platform/evidence/test-device/source-audit.json';audit.parent.mkdir(parents=True);audit.write_text(json.dumps({'sources':[{**self.sources[0],'sha256':'0'*64}]}))
  with self.assertRaisesRegex(ValueError,'disagreement'):r.load_product(self.root,'test-device')
 def test_scratch_never_targets_catalog_or_evidence(self):
  for child in ['web/platform/assets/new','docs/platform/evidence/new','.butler/new','tools/new']:
   with self.assertRaisesRegex(ValueError,'Scratch'):r.hydrate(self.product,self.root/child)
 def test_symlink_escape_fails(self):
  self.scratch.mkdir();(self.scratch/'sources').symlink_to(self.assets,target_is_directory=True)
  with self.assertRaisesRegex(ValueError,'escapes'):r.hydrate(self.product,self.scratch)
 def test_asset_symlink_fails(self):
  (self.assets/'source.stl').unlink();external=Path(self.temp.name)/'external';external.write_bytes(self.data['source.stl']);(self.assets/'source.stl').symlink_to(external)
  with self.assertRaisesRegex(ValueError,'escapes'):r.hydrate(self.product,self.scratch)
 def test_remote_hash_drift_is_never_saved(self):
  class Response(io.BytesIO):
   headers={}
   def geturl(self):return 'https://example.org/cad.stl'
  class Opener:
   def open(self,*args,**kwargs):return Response(b'x'*len(self.data))
  opener=Opener();opener.data=self.data['source.stl'];destination=Path(self.temp.name)/'download'
  with patch.object(r,'build_opener',return_value=opener):
   with self.assertRaisesRegex(ValueError,'changed'):r.fetch_exact(self.sources[0],destination)
  self.assertFalse(destination.exists())
 def test_network_only_https_without_credentials(self):
  for url in ['http://example.org/file','https://user:secret@example.org/file','file:///tmp/file']:
   with self.assertRaises(ValueError):r.secure_url(url)
 def test_comparison_detects_real_bytes_changed(self):
  output=Path(self.temp.name)/'build';output.mkdir();(output/'geometry.bin').write_bytes(b'tampered');expected={'artifacts':{'geometry.bin':r.digest(b'original')}};actual={'artifacts':dict(expected['artifacts'])};(output/'model.json').write_text(json.dumps(actual));result=r.compare_build(expected,actual,output);self.assertFalse(result['passed']);self.assertIn('artifact:geometry.bin',result['mismatches'])
if __name__=='__main__':unittest.main()
