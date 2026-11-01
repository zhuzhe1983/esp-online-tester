#!/usr/bin/env python3
"""Verify source-surface partitioning independently of a product layout."""
import importlib.util, unittest
from pathlib import Path
import numpy as np
spec=importlib.util.spec_from_file_location('exporter',Path(__file__).with_name('onboard-lane-01.py'));exporter=importlib.util.module_from_spec(spec);spec.loader.exec_module(exporter)
def area(corners):
 a=np.asarray(corners).reshape(-1,3,6)[:,:,:3]
 return np.linalg.norm(np.cross(a[:,1]-a[:,0],a[:,2]-a[:,0]),axis=1).sum()/2 if len(a)else 0
class SourcePlaneClip(unittest.TestCase):
 def test_triangle_crossing_multiple_planes_keeps_area_and_surface(self):
  source=np.array([[-3.,0.,2.,0.,0.,1.],[3.,0.,2.,0.,0.,1.],[0.,6.,2.,0.,0.,1.]])
  partitions=[exporter.clip_source_triangle(source,(0,a,b))for a,b in zip(range(-3,3),range(-2,4))]
  self.assertAlmostEqual(sum(area(p)for p in partitions),area(source),places=12)
  for index,part in enumerate(partitions):
   a=np.asarray(part);self.assertTrue(np.all(a[:,0]>=index-3));self.assertTrue(np.all(a[:,0]<=index-2));np.testing.assert_allclose(a[:,2],2)
   # Original triangle half planes, independent of the clipping implementation.
   self.assertTrue(np.all(a[:,1]>=0));self.assertTrue(np.all(a[:,1]<=2*(3-np.abs(a[:,0]))+1e-12));np.testing.assert_allclose(np.linalg.norm(a[:,3:],axis=1),1)
 def test_interpolated_normals_follow_original_edges(self):
  source=np.array([[-1.,0.,0.,1.,0.,0.],[1.,0.,0.,0.,1.,0.],[0.,2.,0.,0.,0.,1.]])
  result=np.asarray(exporter.clip_source_triangle(source,(0,0.,1.)))
  at=result[np.all(np.isclose(result[:,:3],[0,0,0]),axis=1)]
  self.assertGreater(len(at),0);np.testing.assert_allclose(at[0,3:],[2**-.5,2**-.5,0])
 def test_boundary_does_not_generate_cap_or_degenerate_triangle(self):
  source=np.array([[0.,0.,0.,0.,0.,1.],[1.,0.,0.,0.,0.,1.],[0.,1.,0.,0.,0.,1.]])
  self.assertEqual(exporter.clip_source_triangle(source,(0,-1.,0.)),[])
  np.testing.assert_allclose(exporter.clip_source_triangle(source,(0,0.,1.)),source)
 def test_disjoint_clip_is_empty(self):
  source=np.array([[0.,0.,0.,0.,0.,1.],[1.,0.,0.,0.,0.,1.],[0.,1.,0.,0.,0.,1.]])
  self.assertEqual(exporter.clip_source_triangle(source,(0,2.,3.)),[])
 def test_invalid_specification_rejected(self):
  for spec in [None,{},dict(axis=0,minMm=1,maxMm=1),dict(axis=0,minMm=2,maxMm=1),dict(axis=3,minMm=0,maxMm=1),dict(axis=True,minMm=0,maxMm=1),dict(axis=0,minMm=float('nan'),maxMm=1),dict(axis=0,minMm=0,maxMm=float('inf')),dict(axis=0,minMm=0,maxMm=True),dict(axis=0,minMm=0,maxMm=1,offset=1)]:
   with self.subTest(spec=spec),self.assertRaises(ValueError):exporter.source_plane_clip(spec)
if __name__=='__main__':unittest.main()
