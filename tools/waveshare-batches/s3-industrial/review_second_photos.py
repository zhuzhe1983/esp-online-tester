#!/usr/bin/env python3
"""Overlay actual production-loader footprints on retained manufacturer photos.

This is a source-bounded projection check, not optical/physical fit or WebGL
acceptance. Texture pixels are never changed. Port heights remain estimates.
"""
from pathlib import Path
import argparse, hashlib, json, textwrap
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
from PIL import Image
from scipy.spatial import ConvexHull, QhullError

ROOT = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--meshes', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
manifest = json.loads((ROOT/'docs/research/waveshare-s3-industrial-2026-11-24/batch-02/manifest.json').read_text())
records = []
for entry in manifest['items']:
    for mid in entry['modelIds']:
        directory=ROOT/'web/platform/assets'/mid
        plan=json.loads((directory/'model-plan.json').read_text())
        model_bytes=(directory/'model.json').read_bytes()
        record=json.loads((args.meshes/(mid+'.json')).read_text())
        source=next(s for s in plan['sources'] if s['id']=='dimension-photo')
        if 'audio' in mid:
            affine=np.array([[306/58,0,317],[0,-306/58,247]])
            names={'native-case-base','pcb-main','native-upper-pmma'}
            note='Published 58 mm diameter registers the native footprint. The 49.7 mm full native stack differs by 0.9 mm from the photo callouts (43.7 + 5.1 = 48.8 mm); retained unscaled. Interior and optical behavior are not accepted at 1:1 fidelity.'
        elif '8di' in mid:
            affine=np.array([[670/175,0,480],[0,-346/90,305]])
            names={'case-back-flange','front-cap','front-cheek--75','front-cheek-75','ethernet-jack-back','ethernet-jack-side--7.4','ethernet-jack-side-7.4','antenna-sma-envelope','usb-open-envelope'}
            names.update(x['name'] for x in record['meshes'] if x['name'].startswith('terminal-'))
            note='Exact DO or RO-C/PoE photo. Published 175 x 90 mm envelope; source mounting and body pixel scales differ. Port, panel and terminal alignment remains estimated; hollow insertion cavities and distinct terminal counts are retained.'
        else:
            affine=np.array([[130/23.3,0,412],[0,-295/47.3,345.5]])
            names={'front-cap'}
            note='Published 23.3 x 47.3 mm face against an oblique source photograph; horizontal foreshortening applies only to this photo projection. The U connector is independently checked against its exact variant image; CAD is not rescaled.'
        image=Image.open(directory/source['file'])
        fig,axes=plt.subplots(1,2,figsize=(13,8))
        for axis in axes:axis.imshow(image);axis.axis('off')
        axes[0].set_title('Unmodified manufacturer reference')
        axes[1].set_title('Actual model footprint projection')
        projected=[]
        for mesh in record['meshes']:
            if mesh['name'] not in names:continue
            points=np.unique(np.asarray(mesh['position']).reshape(-1,3)[:,:2],axis=0)
            if len(points)<3:continue
            try:hull=points[ConvexHull(points).vertices]
            except QhullError:continue
            pixel=np.column_stack([hull,np.ones(len(hull))])@affine.T
            pixel=np.vstack([pixel,pixel[0]])
            axes[1].plot(pixel[:,0],pixel[:,1],linewidth=.8,alpha=.85,label=mesh['name'])
            projected.append({'part':mesh['name'],'projection':'convex XY envelope only; internal holes excluded'})
        fig.suptitle(mid, fontsize=11, y=.98)
        fig.text(.5, .94, textwrap.fill(note, width=125), fontsize=9, ha='center', va='top')
        fig.tight_layout(rect=(0,0,1,.86))
        filename=mid+'-source-overlay.png'
        fig.savefig(args.output/filename,dpi=120)
        plt.close(fig)
        records.append({'model':mid,'modelSha256':hashlib.sha256(model_bytes).hexdigest(),'sourceId':source['id'],'sourceSha256':source['sha256'],'pixelFromModelXY':affine.tolist(),'overlay':filename,'projectedParts':projected,'notes':note,'scope':'Approximate footprint registration; hole topology/hidden structure/material/optical behavior not certified.'})
(args.output/'photo-registration-checks.json').write_text(json.dumps({'records':records},indent=2)+'\n')
