#!/usr/bin/env python3
"""Exercise raw open STL preservation and third-party provenance during export."""
import hashlib,importlib.util,json,tempfile,unittest
from pathlib import Path
import numpy as np
import trimesh

spec=importlib.util.spec_from_file_location('exporter',Path(__file__).with_name('onboard-lane-01.py'));exporter=importlib.util.module_from_spec(spec);spec.loader.exec_module(exporter)

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()

class SourceMeshTest(unittest.TestCase):
 def test_open_preclassified_mesh_keeps_triangles_and_author_license(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);source=root/'sources'/'test';source.mkdir(parents=True)
   mesh=trimesh.Trimesh(vertices=[[0,0,0],[1,0,0],[0,1,0],[0,0,1]],faces=[[0,2,1],[0,1,3],[1,2,3]],process=False)
   mesh.export(source/'open.stl');(source/'license.txt').write_text('Specific model author license\n');(source/'components.json').write_text('[]')
   records=[{'id':sid,'archiveName':name,'url':'https://example.org/'+name,'sha256':sha(source/name),'bytes':(source/name).stat().st_size,'status':'fetched'}for sid,name in [('cad','open.stl'),('cad-license','license.txt')]]
   prepared={'id':'test','sources':records};plan={'sourceId':'cad','preserveMesh':True,'matrixRows':np.eye(4).tolist(),'assemblyNotes':'Retain native coordinates','parts':[{'name':'open-board','sourceComponent':0,'role':'pcb'}],'provenance':{'repository':'https://example.org/models','sourceLicense':'Specific model author license','licenseFile':'LICENSE-Author.txt'},'limitations':[]}
   exporter.TEMP=root/'sources';output=root/'output';meta,_=exporter.build_model(prepared,plan,output,evidence_root=root/'evidence',license_path=source/'license.txt')
   self.assertEqual(meta['parts'][0]['vertexCount'],9)
   vertices=np.frombuffer((output/'geometry.bin').read_bytes(),dtype='<f4').reshape(-1,6)[:,:3]
   np.testing.assert_allclose(vertices,mesh.triangles.reshape(-1,3))
   self.assertEqual(meta['provenance']['repository'],'https://example.org/models');self.assertEqual(meta['provenance']['sourceLicense'],'Specific model author license')
   self.assertEqual(meta['artifacts']['LICENSE-Author.txt'],sha(source/'license.txt'));self.assertEqual(meta['artifacts']['source.stl'],sha(source/'open.stl'))
   self.assertNotIn('supportingAssets',meta,'Legacy plans keep their metadata contract unchanged')
   plan['surfaces']=[{'name':'translucent-blade','sizeMm':[2,3],'radiusMm':0,'positionMm':[0,0,2],'side':'front','role':'propeller','sourceIds':['cad'],'notes':'Photo-supported material fixture','material':{'opacity':.38,'roughness':.25,'metalness':0}}]
   painted,_=exporter.build_model(prepared,plan,root/'painted',evidence_root=root/'painted-evidence',license_path=source/'license.txt')
   self.assertNotIn('material',painted['parts'][0])
   self.assertEqual(painted['parts'][1]['material'],{'opacity':.38,'roughness':.25,'metalness':0})
   plan['provenance']['licenseFile']='../escape.txt'
   with self.assertRaisesRegex(ValueError,'license filename'):exporter.build_model(prepared,plan,root/'bad',evidence_root=root/'bad-evidence',license_path=source/'license.txt')
   self.assertFalse((root/'escape.txt').exists())

 def test_supporting_assets_keep_original_bytes_and_reject_unsafe_destinations(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);source=root/'sources'/'test';source.mkdir(parents=True)
   trimesh.Trimesh(vertices=[[0,0,0],[1,0,0],[0,1,0]],faces=[[0,1,2]],process=False).export(source/'open.stl')
   for name,data in [('license.txt',b'Main license\n'),('third-party.txt',b'Author attribution\nCC-BY-SA exception\n'),('components.json',b'[]')]: (source/name).write_bytes(data)
   records=[{'id':sid,'archiveName':name,'url':'https://example.org/'+name,'sha256':sha(source/name),'bytes':(source/name).stat().st_size,'status':'fetched'}for sid,name in [('cad','open.stl'),('cad-license','license.txt'),('third-party','third-party.txt')]]
   prepared={'id':'test','sources':records};plan={'sourceId':'cad','preserveMesh':True,'matrixRows':np.eye(4).tolist(),'assemblyNotes':'Native geometry','parts':[{'name':'part','sourceComponent':0}],'limitations':[],'supportingAssets':[{'sourceId':'third-party','file':'LICENSE-Upstream.txt'}]}
   exporter.TEMP=root/'sources';output=root/'output';meta,_=exporter.build_model(prepared,plan,output,evidence_root=root/'evidence',license_path=source/'license.txt')
   self.assertEqual((output/'LICENSE-Upstream.txt').read_bytes(),(source/'third-party.txt').read_bytes())
   self.assertEqual(meta['artifacts']['LICENSE-Upstream.txt'],records[-1]['sha256']);self.assertEqual(meta['supportingAssets'],[{**plan['supportingAssets'][0],'sha256':records[-1]['sha256']}])
   for bad in ['../escape.txt','/absolute.txt','sub/license.txt','geometry.bin','source.stl','reference-photo.webp','LICENSE-M5Stack.txt']:
    plan['supportingAssets'][0]['file']=bad
    with self.assertRaisesRegex(ValueError,'supporting asset filename'):exporter.build_model(prepared,plan,root/'bad',evidence_root=root/'bad-evidence',license_path=source/'license.txt')
   plan['supportingAssets'][0]['file']='LICENSE-Upstream.txt';plan['supportingAssets'].append(dict(plan['supportingAssets'][0]))
   with self.assertRaisesRegex(ValueError,'supporting asset filename'):exporter.build_model(prepared,plan,root/'duplicate',evidence_root=root/'duplicate-evidence',license_path=source/'license.txt')
   plan['supportingAssets'].pop();(source/'third-party.txt').write_bytes(b'Changed attribution')
   with self.assertRaisesRegex(ValueError,'source changed'):exporter.build_model(prepared,plan,root/'changed',evidence_root=root/'changed-evidence',license_path=source/'license.txt')
   (source/'third-party.txt').write_bytes((output/'LICENSE-Upstream.txt').read_bytes());outside=root/'protected.txt';outside.write_bytes(b'keep');linked=root/'linked';linked.mkdir();(linked/'LICENSE-Upstream.txt').symlink_to(outside)
   with self.assertRaisesRegex(ValueError,'symlink'):exporter.build_model(prepared,plan,linked,evidence_root=root/'linked-evidence',license_path=source/'license.txt')
   self.assertEqual(outside.read_bytes(),b'keep')
   self.assertFalse((root/'escape.txt').exists())

if __name__=='__main__':unittest.main()
