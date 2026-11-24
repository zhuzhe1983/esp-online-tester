#!/usr/bin/env python3
"""Compare actual production-loader footprints with unchanged manufacturer photos.

The supplied registration is a manual image projection, not a 3D fit measurement.
Source board photographs and their embedded lighting remain unmodified.
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
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--meshes', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
args = p.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
manifest = json.loads((ROOT/'docs/research/waveshare-s3-industrial-2026-11-24/batch-03/manifest.json').read_text())


def planar_homography(model, pixels):
    """Four corresponding points; no source pixel warp or geometry scaling."""
    rows, target = [], []
    for (x, y), (u, v) in zip(model, pixels):
        rows.extend([[x,y,1,0,0,0,-u*x,-u*y], [0,0,0,x,y,1,-v*x,-v*y]])
        target.extend([u,v])
    return np.append(np.linalg.solve(rows, target),1).reshape(3,3)


records = []
for entry in manifest['items']:
    for mid in entry['modelIds']:
        directory = ROOT/'web/platform/assets'/mid
        plan = json.loads((directory/'model-plan.json').read_text())
        record = json.loads((args.meshes/(mid+'.json')).read_text())
        sources = {x['id']: x for x in plan['sources']}
        sid = 'dimension-photo'
        if '8di-8ro' in mid:
            matrix = np.array([[670/175,0,480],[0,-346/90,305],[0,0,1]])
            names = {'case-back-flange','front-cap','front-cheek--75','front-cheek-75','rj45-back','usb-open-envelope','input','power','rs485','relay-left','relay-right'}
            note = 'Published 175 x 90 mm family enclosure. Separate source dimension arrows have slightly different pixel scales. Local connector positions, terminal pitch/depth and wall thickness are estimates. The PoE variant does not inherit the pictured common-network sticker.'
        elif 'relay-6ch' in mid:
            matrix = planar_homography([[-56.5,32.5],[56.5,32.5],[56.5,-32.5],[-56.5,-32.5]],[[205,310],[550,130],[714,306],[382,531]])
            names = {'front-cap'}
            note = 'Oblique reference: manually selected front-cap corners only. This plane projection does not measure the 145 x 90 x 40 mm enclosure or transfer a flattened label onto its unpictured walls. Full dimensions use the manufacturer arrows; depth and fit remain unknown.'
        else:
            reg = plan.get('photoRegistration',{}).get('pcb-front-photo')
            if reg:
                sid = reg['sourceId']; left,top,right,bottom = reg['sourcePixelBounds']; width,height = reg['nominalSizeMm']
            else:
                # N8R8 has the same published outline, but its own SiP image is
                # retained independently. This comparison covers the outline only.
                left,top,right,bottom,width,height = 149,142,399,468,18,23.5
            matrix = np.array([[(right-left)/width,0,(right+left)/2],[0,-(bottom-top)/height,(bottom+top)/2],[0,0,1]])
            if 'eth-' in mid or mid=='waveshare-s3-eth-mechanical':
                names = {'pcb-main','esp32-s3','w5500','magnetics','camera-ffc','ceramic-antenna','rj45-back','rj45-top','rj45-left','rj45-right'}
                note = 'Exact main-board front: 21 x 72.8 mm outline and photographed front packages. USB, RGB and TF are on the reverse. Camera/PoE/header packages use their separately retained exact-variant photos; they are not measured using this front-board projection.'
            elif 'tiny' in mid:
                names = {'pcb-main'} if 'n8r8' in mid else {'pcb-main','soc-package','ffc-8pin','rgb-led','ceramic-antenna'}
                note = '18 x 23.5 mm main-board outline. FH4R2 and PICO1-N8R8 use distinct package envelopes and exact SKU photos. Only the common outline is compared against the FH4R2 dimension drawing for N8R8. Adapter and cable are separate kit contents; no guessed installed cable route.'
            else:
                names = {'pcb-main','wroom-shield','ffc-18pin','ffc-40pin','usb-open-envelope','boot-button','reset-button'}
                note = '66.3 x 25.4 mm driver-board footprint. This plane comparison covers the controller board only; the separate kit panel is dimensioned from its own retained manufacturer PDF. Wall/connector/package heights and FPC mating remain uncalibrated.'
        source = sources[sid]
        image = Image.open(directory/source['file'])
        fig, axes = plt.subplots(1,2,figsize=(13,8))
        for axis in axes:
            axis.imshow(image); axis.axis('off')
        axes[0].set_title('Unmodified manufacturer reference')
        axes[1].set_title('Production-loader footprint projection')
        projected = []
        for mesh in record['meshes']:
            if mesh['name'] not in names: continue
            points = np.unique(np.asarray(mesh['position']).reshape(-1,3)[:,:2],axis=0)
            if len(points)<3: continue
            try: hull = points[ConvexHull(points).vertices]
            except QhullError: continue
            pixel = np.column_stack([hull,np.ones(len(hull))]) @ matrix.T
            pixel = pixel[:,:2] / pixel[:,2:3]
            pixel = np.vstack([pixel,pixel[0]])
            axes[1].plot(pixel[:,0],pixel[:,1],linewidth=.85,alpha=.85)
            projected.append(mesh['name'])
        fig.suptitle(mid,fontsize=11,y=.98)
        fig.text(.5,.94,textwrap.fill(note,width=125),fontsize=9,ha='center',va='top')
        fig.tight_layout(rect=(0,0,1,.86))
        filename=mid+'-source-overlay.png'
        fig.savefig(args.output/filename,dpi=120); plt.close(fig)
        records.append({'model':mid,'modelSha256':hashlib.sha256((directory/'model.json').read_bytes()).hexdigest(),'sourceId':sid,'sourceSha256':source['sha256'],'homogeneousPixelFromModelXY':matrix.tolist(),'overlay':filename,'projectedParts':projected,'notes':note,'scope':'Manual planar projection and convex footprints only; depth, hidden structure, materials, optics and hardware fit are not certified.'})
(args.output/'photo-registration-checks.json').write_text(json.dumps({'records':records},indent=2)+'\n')
print(f'{len(records)} unchanged-photo overlays written.')
