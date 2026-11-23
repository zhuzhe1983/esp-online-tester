#!/usr/bin/env python3
"""Meaningful offline-source boundary checks for Seeed model reconstruction."""
import contextlib,hashlib,importlib.util,io,json,tempfile,unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('seeed_builder',Path(__file__).with_name('build-seeed-models.py'))
builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)

class SourceBoundaries(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='seeed-build-test-');self.root=Path(self.temp.name);self.original=builder.ASSETS;builder.ASSETS=self.root/'assets'
  self.pid='seeed-test-kit';self.owner='seeed-test-board';self.p=builder.ASSETS/self.pid;self.shared=builder.ASSETS/self.owner;self.p.mkdir(parents=True);self.shared.mkdir()
  self.body=b'fixed manufacturer PCB source\n';(self.shared/'source.kicad_pcb').write_bytes(self.body)
  self.source={'id':'pcb','file':'source.kicad_pcb','sharedAssetProductId':self.owner,'sha256':hashlib.sha256(self.body).hexdigest(),'bytes':len(self.body)}
  self.plan={'id':self.pid,'method':'test-fixture','matrixRows':builder.np.eye(4).tolist(),'sources':[self.source],'parts':[{'name':'board','role':'pcb','color':'#123456','geometry':{'shape':'box','sizeMm':[10,20,1]}}],'presentation':{'screen':None},'dimensions':{'width':10,'height':20,'depth':1},'assemblyNotes':'test fixture','limitations':['No device acceptance.'],'sourceIdentity':{'sourceBoard':self.owner,'targetKit':self.pid,'notes':'Source sharing is not compatibility inheritance.'}}
  self.save()
 def tearDown(self):builder.ASSETS=self.original;self.temp.cleanup()
 def save(self):(self.p/'model-plan.json').write_text(json.dumps(self.plan))
 def build(self):
  with contextlib.redirect_stdout(io.StringIO()):builder.build(self.pid,self.root/'out')
 def test_shared_source_is_copied_once_and_hash_pinned(self):
  self.build();out=self.root/'out';model=json.loads((out/self.pid/'model.json').read_text());report=json.loads((out/self.pid/'build-check.json').read_text())
  self.assertEqual((out/self.owner/'source.kicad_pcb').read_bytes(),self.body)
  self.assertFalse((out/self.pid/'source.kicad_pcb').exists())
  self.assertEqual(model['sharedArtifacts'][self.owner+'/source.kicad_pcb'],self.source['sha256'])
  self.assertEqual(report['sharedSourceOwners'],[self.owner]);self.assertIsNone(model['presentation']['screen'])
 def test_changed_shared_source_fails_before_export(self):
  (self.shared/'source.kicad_pcb').write_bytes(self.body+b'mutation')
  with self.assertRaisesRegex(AssertionError,'Source hash mismatch'):self.build()
  self.assertFalse((self.root/'out'/self.pid/'geometry.bin').exists())
 def test_original_and_retained_identity_are_not_conflated(self):
  retained=b'privacy-normalized fixed PCB\n';(self.shared/'source.kicad_pcb').write_bytes(retained)
  self.source['retained']={'sha256':hashlib.sha256(retained).hexdigest(),'bytes':len(retained),'transformation':'test redaction'};self.save();self.build()
  model=json.loads((self.root/'out'/self.pid/'model.json').read_text());record=model['provenance']['sources'][0]
  self.assertEqual(record['sha256'],hashlib.sha256(self.body).hexdigest());self.assertEqual(model['sharedArtifacts'][self.owner+'/source.kicad_pcb'],hashlib.sha256(retained).hexdigest())
 def test_owner_path_traversal_self_and_symlink_escape_fail(self):
  for owner in ['../seeed-other','seeed-other/nested','seeed-other\\nested','seeed-other..',self.pid,'https://example.com']:
   with self.subTest(owner=owner),self.assertRaises(AssertionError):builder.source_path(self.pid,{**self.source,'sharedAssetProductId':owner})
  outside=self.root/'outside';outside.mkdir();(builder.ASSETS/'seeed-escape').symlink_to(outside,target_is_directory=True)
  with self.assertRaises(AssertionError):builder.source_path(self.pid,{**self.source,'sharedAssetProductId':'seeed-escape'})
  for file in ['../source.kicad_pcb','nested/source.kicad_pcb','C:\\source','..']:
   with self.subTest(file=file),self.assertRaises(AssertionError):builder.source_path(self.pid,{**self.source,'file':file})

if __name__=='__main__':unittest.main()
