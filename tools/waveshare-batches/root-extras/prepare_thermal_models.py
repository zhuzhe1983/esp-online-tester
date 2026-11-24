#!/usr/bin/env python3
"""Photo/dimension models of the exact 45/90 degree Thermal ESP32 modules.

No sensor CAD, confidential datasheet, synthetic thermal image or sibling PCB.
All source photographs are copied unchanged; unmeasured Z is an estimate.
"""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
RIGHTS = 'Original Waveshare photographs retain manufacturer rights. Public access is not a separate redistribution licence. Approximate geometry is reconstructed from published images/dimensions.'


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def rectangle(w, h):
    return [[-w/2, -h/2], [w/2, -h/2], [w/2, h/2], [-w/2, h/2]]


def circle(radius, center=(0, 0), n=64):
    return [[center[0]+radius*math.cos(i*2*math.pi/n), center[1]+radius*math.sin(i*2*math.pi/n)] for i in range(n)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, required=True)
    args = parser.parse_args()
    src = args.source_root
    for angle in [45, 90]:
        pid = f'waveshare-thermal-{angle}-camera-esp32-module'
        out = ROOT/'web/platform/assets'/pid
        out.mkdir(parents=True, exist_ok=True)
        sources = []
        for sid, name, dest in [
            ('dimension-photo', 'Thermal-45-Camera-ESP32-Module-details-size.jpg', 'dimension-photo.jpg'),
            ('reverse-photo', 'Thermal-45-Camera-ESP32-Module-details-intro.jpg', 'reverse-photo.jpg'),
            ('optics-photo', 'Thermal-45-Camera-ESP32-Module-details-1.jpg', 'optics-photo.jpg'),
            ('assembly-photo', 'thermal-45-camera-esp32-module-1.jpg', 'assembly-photo.jpg'),
            ('overview', 'overview.html', 'overview.html.gz'),
        ]:
            body = (src/name).read_bytes()
            receipt = json.loads((src/(name+'.receipt.json')).read_text())
            digest = hashlib.sha256(body).hexdigest()
            assert len(body) == receipt['bytes'] and digest == receipt['sha256']
            stored = gzip.compress(body, mtime=0) if dest.endswith('.gz') else body
            (out/dest).write_bytes(stored)
            row = dict(id=sid, url=receipt['requestedUrl'], finalUrl=receipt['finalUrl'],
                       fetchedAt=receipt['fetchedAt'], bytes=len(body), sha256=digest,
                       file=dest, role='photo-source' if name.endswith('.jpg') else 'reference-only', rights=RIGHTS)
            if dest.endswith('.gz'):
                row['retained'] = dict(encoding='gzip', bytes=len(stored), sha256=hashlib.sha256(stored).hexdigest())
            else:
                row['imagePixels'] = list(Image.open(src/name).size)
            sources.append(row)

        def xy(x, y):
            return [(x-475.5)*29/505, (332.5-y)*29/505]
        outline = [xy(x,y) for x,y in [(223,175),(561,175),(561,80),(728,80),(728,585),(223,585)]]
        holes = [circle(1.1, xy(x,y)) for x,y in [(249,561),(706,561),(706,104)]]
        def plate(name, contour, depth, z, color, holes=None, basis='', center=None):
            return dict(id=name, kind='plate', color=color,
                        geometry=dict(outlineMm=contour, holesMm=holes or [], depthMm=depth, centerMm=center or [0,0,z]),
                        sourceIds=['dimension-photo','optics-photo'], basis=basis)
        derived = [plate('pcb-main', outline, 1.6, -.8, '#263c58', holes,
                         'Published carrier envelope29 x29mm; observed L-shaped silhouette and three holes. Thickness1.6 and hole diameter2.2 are estimates, not drawing annotations.')]
        # Exact original source pixels on the observed carrier faces. These do
        # not assert component heights or replace a live image/framebuffer.
        for side, texture, z, uv in [
            ('front','dimension-photo.jpg',.012,[505/29/960,0,475.5/960,0,505/29/639,1-332.5/639]),
            ('back','reverse-photo.jpg',-1.612,[-452/29/960,0,486/960,0,452/29/800,1-322/800]),
        ]:
            derived.append(dict(id='carrier-'+side+'-photo', kind='photo-surface', texture=texture,
                                geometry=dict(outlineMm=outline, holesMm=holes, centerMm=[0,0,z], side=side, uvFromMm=uv),
                                sourceIds=['dimension-photo' if side=='front' else 'reverse-photo'],
                                basis='Unchanged exact-board photograph clipped to the observed L-shaped carrier; annotations outside it are excluded. Perspective, heights and hidden copper remain unverified.'))
        # The WROOM PCB is a distinct part beneath the carrier; its antenna
        # extends into the photographed cut-out. Do not fill the cut-out with
        # a fictitious rectangular carrier board.
        module_outline = [[-13.3,-11.4],[5,-11.4],[5,14.5],[-13.3,14.5]]
        derived.append(plate('module-pcb-envelope', module_outline, .8, -2.1, '#19341f',
                             basis='Distinct WROOM PCB outline estimated from reverse photograph; XY registration ±1mm, thickness and solder standoff unknown.'))
        derived.append(dict(id='module-reverse-photo', kind='photo-surface', texture='reverse-photo.jpg',
                            geometry=dict(outlineMm=module_outline, centerMm=[0,0,-2.512], side='back',
                                          uvFromMm=[-452/29/960,0,486/960,0,452/29/800,1-322/800]),
                            sourceIds=['reverse-photo'], basis='Unchanged original module pixels; shield height and hidden internals not reconstructed.'))
        # The separate sensor carrier is visible in the assembly photo. Its
        # hidden underside is explicitly UNKNOWN, rather than a made-up PCB.
        sensor_outline = rectangle(20,11)
        derived.append(dict(id='sensor-daughter-pcb', kind='pcb-placeholder', color='#254c2a',
                            geometry=dict(shape='polygon', outlineMm=sensor_outline, holesMm=[], depthMm=.8,
                                          sizeMm=[20,11,.8], centerMm=[-1,0,3.0], labelSide='back', labelSizeMm=[12,2.2]),
                            sourceIds=['assembly-photo','optics-photo'],
                            basis='Separate observed sensor daughterboard;20 x11 x0.8mm and Z3 are visual estimates, XY±2mm/Z unknown. Its hidden layout is UNKNOWN. No sensor schematic or footprint is fabricated.'))
        # Both the square sensor base and barrel have true optical openings.
        # Their unmeasured diameter/depth are deliberately marked estimates.
        lens_center = [-5.5,0]
        aperture = 3.2 if angle == 45 else 1.2
        derived.append(plate('lens-square-base', rectangle(11,11), 1.2, 0, '#16191a', [circle(aperture)],
                             f'Observed{angle}degree optics;11mm square and1.2mm depth are approximate visual envelopes, not a calibrated optical or mounting dimension.', [*lens_center,4.0]))
        derived.append(plate('lens-open-barrel', circle(4.8), 4.0, 0, '#101414', [circle(aperture)],
                             f'Open barrel outer radius4.8 and aperture radius{aperture}mm are estimates from the original comparison photograph; depth4mm unmeasured. No invented thread or solid lens cap.', [*lens_center,6.6]))
        derived.append(plate('optical-surface', circle(aperture*.98), .12, 0, '#36573b', [],
                             'Separate recessed optical plane based only on visible green reflection. No calibrated material, sensor pixels, temperature or synthetic thermal image.', [*lens_center,4.66]))
        parts = []
        def box(name, role, size, center, color, **kwargs):
            return dict(name=name, role=role, color=color, geometry=dict(shape='box',sizeMm=size,centerMm=center),
                        sourceIds=['reverse-photo'], basis='Photo-positioned visible feature; all heights and package dimensions are explicit estimates, ±1mm XY with no fit tolerance.', **kwargs)
        parts.append(box('boot-cap','control',[3,3,1.4],[12.64,8.47,-2.5],'#202223',controlId='boot'))
        parts.append(box('reset-cap','component',[3,3,1.4],[12.64,-8.34,-2.5],'#202223'))
        parts.append(box('rgb-window','led',[2,2,.9],[7.5,8.47,-2.1],'#d5d8cb',ledIndex=0))
        parts.append(dict(name='usb-open-envelope',role='connector',color='#a8afb0',
                          geometry=dict(shape='open-usb-shell',sizeMm=[8.95,6.4,3.2],centerMm=[0,0,0],wallMm=.3),
                          matrixRows=[[0,-1,0,11.94],[1,0,0,.128],[0,0,1,-3.2],[0,0,0,1]],
                          sourceIds=['reverse-photo','dimension-photo'],
                          basis='Open USB envelope rotated so its mouth faces the photographed right edge. Dimensions8.95 x6.4 x3.2 and wall0.3mm are estimates; no contact or mating claim.'))
        limits = [
            'Only29 x29mm carrier size and26 x26mm hole spacing are manufacturer dimension labels; thickness, module, sensor, optics and connector geometry are approximate photo envelopes.',
            'The carrier is L-shaped with three observed holes; the separate WROOM PCB antenna occupies the cut-out. No fourth mounting hole or filled rectangular carrier is invented.',
            'No native CAD, PCB layout or Gerber was found in either complete linked ZIP inventory. SDK object files are not mechanical CAD.',
            'The linked MI0802 PDF is marked confidential/NDA. It is not redistributed or used to manufacture undocumented sensor internals.',
            'Sensor daughterboard underside, flex path, spacers, hidden solder, lens thread, standoff and optical stack depth remain UNKNOWN/unmodeled.',
            '45 and90 degree optics differ in the photographed aperture; aperture diameters are estimates, not focal or field-of-view calibration.',
            'There is no screen. The thermal sensor does not receive a fabricated framebuffer, temperature, ID or ready interrupt.',
            'BOOT is an actual input and the discrete RGB has three active-low channels; RESET geometry is visual only.',
            'Source geometry and CPU rays are separate from browser WebGL, hardware fit, radiometric calibration and physical acceptance.'
        ]
        plan = dict(id=pid,method='dimension-photo-reconstruction',sources=sources,parts=parts,derivedParts=derived,
                    presentation=dict(inspectionGroups=[dict(id='carrier-assembly',parts=['pcb-main','carrier-front-photo','carrier-back-photo','module-pcb-envelope','module-reverse-photo','boot-cap','reset-cap','rgb-window','usb-open-envelope'],offsetMm=[34,0,-14],sourceIds=['optics-photo'],notes='Debug separation moves the supporting carrier away from the stationary sensor underside UNKNOWN label; not a demonstrated physical opening motion.')]),
                    sourceSearch=dict(status='no-official-cad-located',inspected=['overview'],scope='Both linked original complete Thermal-ESP32-Demo and Thermal-ESP32-Web-Demo ZIP inventories retained in docs/platform/research/waveshare-thermal-20261124; no STEP/STP/IGES/KiCad/Gerber members.'),
                    dimensionTargets=[dict(part='pcb-main',sizeMm=[29,29,None],toleranceMm=.001,sourceId='dimension-photo')],
                    validation=dict(controls=[dict(id='boot',type='button')],controlViewNormals=dict(boot=[0,0,-1]),ledIndices=[0],shellNames=[],unknownPcbInspection=[dict(label='sensor-daughter-pcb-unknown-back')]),
                    referencePhotos=[dict(sourceId=s['id'],file=s['file'],views='Original manufacturer image, unchanged.') for s in sources if 'imagePixels' in s],
                    photoRegistration=dict(sourceImagesUnchanged=True,frontPixelEnvelope=[223,80,728,585],carrierMm=[29,29],holesPhotoPixels=[[249,561],[706,561],[706,104]],review=dict(file='dimension-photo.jpg',pixelFromModelXY=[[505/29,0,475.5],[0,-505/29,332.5]],includeParts=['pcb-main'],basis='Original manufacturer carrier dimension image, observed L outline and three holes; no hidden geometry or Z verification.')),
                    limitations=limits,materialBasis='Exact unmodified carrier/module photographs and neutral estimated optical/package colors. No radiometric or material calibration.',
                    assemblyNotes='Native millimetres, carrier front+Z; photo-registered estimates only, no sibling CAD scaling.')
        save(out/'model-plan.json',plan)
        save(out/'source-manifest.json',dict(schemaVersion=1,id=pid,sources=sources,limitations=limits,search=plan['sourceSearch']))
        (out/'SOURCE-RIGHTS.txt').write_text(RIGHTS+'\n')
        print(pid)


if __name__ == '__main__':
    main()
