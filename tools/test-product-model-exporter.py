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
   plan['surfaces']=[{'name':'translucent-blade','sizeMm':[2,3],'radiusMm':0,'positionMm':[0,0,2],'side':'front','role':'propeller','sourceIds':['cad'],'notes':'Photo-supported material fixture','material':{'opacity':.38,'roughness':.25,'metalness':0}}]
   painted,_=exporter.build_model(prepared,plan,root/'painted',evidence_root=root/'painted-evidence',license_path=source/'license.txt')
   self.assertNotIn('material',painted['parts'][0])
   self.assertEqual(painted['parts'][1]['material'],{'opacity':.38,'roughness':.25,'metalness':0})
   plan['provenance']['licenseFile']='../escape.txt'
   with self.assertRaisesRegex(ValueError,'license filename'):exporter.build_model(prepared,plan,root/'bad',evidence_root=root/'bad-evidence',license_path=source/'license.txt')
   self.assertFalse((root/'escape.txt').exists())

if __name__=='__main__':unittest.main()
