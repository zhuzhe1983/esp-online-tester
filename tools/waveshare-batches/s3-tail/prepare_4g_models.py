"""Retain the manufacturer's shared 4G PCBA reference, with kit omissions explicit."""
from pathlib import Path
import argparse, json, sys, collections
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'c6'))
import model_source_helpers as h

ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument('--sources', type=Path, required=True, help='Inventory directory')
ap.add_argument('--prefetch', type=Path, required=True, help='Industrial source directory')
a = ap.parse_args(); S = a.sources / 's3-tail-4g-sources'
top = json.loads((S / 'cad-topology.json').read_text())
assert len(top) == 742
native = next((S / 'members/esp32-s3-a-sim7670x-4g-pcba').glob('*.stp'))
transform = [[0, 1, 0, -55], [-1, 0, 0, 16.14], [0, 0, 1, 0], [0, 0, 0, 1]]

def retained(dst, path, id, file, role='reference-only', compress=False):
    rp = Path(str(path)+'.source.json')
    if not rp.exists(): rp = Path(str(path)+'.receipt.json')
    r = json.loads(rp.read_text())
    r.pop('containerFile', None)
    r['url'] = r.get('url', r.get('requestedUrl'))
    r['resolvedUrl'] = r.get('resolvedUrl', r.get('finalUrl', r['url']))
    return h.source(dst, path, id, file, role, receipt=r, compress=compress)

for modem, sku, crop in [('a7670e', '26749', (133, 765, 761, 932)),
                         ('sim7670g', '26750', (119, 751, 788, 958))]:
    listing = 'esp32-s3-'+modem+'-4g'; pid = 'waveshare-'+listing
    dst = h.A / pid; dst.mkdir(parents=True, exist_ok=True)
    pre = a.prefetch / listing; sources = []
    for name in ['overview', 'resources']:
        sources.append(retained(dst, pre/(name+'.html'), name, name+'.html', compress=True))
    store = json.loads((a.sources/'products'/(listing+'.json')).read_text())
    assert store['singleSku'] == sku
    selection = {k: store[k] for k in ['listingId', 'url', 'source', 'singleSku', 'partNumber', 'documentationLinks']}
    selection.update(captureKind='derived-public-storefront-fields-not-original-HTTP-body',
        packageIncludes=['Controller board', 'Acrylic case and screw pack', 'GNSS ceramic antenna',
                         'OV5640 camera', '2-pin screw terminal', '40-pin straight header',
                         'USB-A to USB-C cable 1 m', 'FPC antenna', 'Screwdriver', 'Double-sided tape, 4 pieces']
                        + (['8 ohm 2 W 2030 speaker'] if modem == 'a7670e' else []),
        packageExcludes=['18650 battery', 'Solar panel'],
        packageBasis='Exact listing public package-content and battery/solar exclusion statements; raw storefront body is not published.')
    raw = (json.dumps(selection, ensure_ascii=False, indent=2)+'\n').encode()
    (dst/'sku-selection.json').write_bytes(raw)
    sources.append({'id':'product', 'file':'sku-selection.json', 'url':store['url'], 'resolvedUrl':store['url'],
        'bytes':len(raw), 'sha256':h.sha(raw), 'fetchedAt':store['source']['fetchedAt'],
        'captureKind':selection['captureKind'], 'role':'reference-only', 'rights':h.RIGHTS})
    for i, photo in enumerate(sorted((pre/'photos').glob('*.webp'))):
        is_size = '-size-' in photo.name
        sources.append(retained(dst, photo, 'dimension-photo' if is_size else 'photo-'+str(i),
                                'dimension-photo.webp' if is_size else 'photo-'+str(i)+'.webp'))
    sources.append(retained(dst, S/'esp32-s3-a-sim7670x-4g-pcba.zip', 'cad-archive', 'source-archive.zip', 'cad-container'))
    cad = retained(dst, native, 'official-cad', 'source.stp', 'cad-source', compress=True)
    cad.update(origin='zip-member', containerSourceId='cad-archive',
               archive={'entry':cad['archiveMember'], 'containerSha256':cad['containerSha256']})
    sources.append(cad)
    p = h.basics(pid, sources, {'sha':None, 'tree':[]})
    p.update(method='manufacturer-cad', matrixRows=transform,
        sourceTopologyCounts={'source.stp.gz':742},
        sourceCoverage={'source.stp.gz':{'includedIndices':[i for i in range(742) if i != 4],
            'omittedIndices':[4], 'grouping':'741 native solids/open shells retained. Only the source battery is omitted because it is explicitly not included in either sale kit. Native source coordinates and dimensions are unchanged.'}},
        omittedSourceComponents=[{'sourceFile':'source.stp.gz', 'componentIndex':4,
            'reason':'The shared manufacturer assembly contains an 18650 battery; exact SKU package statements explicitly exclude it. The original source bytes still retain it for inspection.', 'sourceIds':['product','official-cad']}])
    names = {0:'wifi-u-fl',1:'usb-c-shell',2:'sim-socket',3:'battery-holder',5:'dip-switch',
             6:'power-slide-switch',7:'modem-shield',8:'tf-socket',9:'camera-fpc',10:'camera-reference',
             11:'pcb-main',410:'rgb-led'}
    missing = {0:'#b1b4b6',1:'#b8bbbd',2:'#aaaeb1',3:'#24282a',5:'#24282a',6:'#24282a',
               7:'#b3b5b6',8:'#a8abad',9:'#dcdacb',10:'#202629'}
    groups = collections.OrderedDict()
    for item in top:
        i = item['index']
        if i == 4: continue
        rgb = item.get('color')
        color = '#'+''.join(f'{round(255*(12.92*x if x<=.0031308 else 1.055*x**(1/2.4)-.055)):02x}' for x in rgb) if rgb else missing.get(i,'#999da0')
        key = (names.get(i), color) if i in names else (None, color)
        groups.setdefault(key, []).append(i)
    for (name, color), indices in groups.items():
        part = {'name':name or 'native-material-'+color[1:], 'sourceFile':'source.stp.gz',
                'sourceComponents':indices, 'sourceTopology':'solid-or-shell', 'color':color,
                'role':'component'}
        if name == 'rgb-led': part['ledIndex'] = 0
        p['parts'].append(part)
    p['assemblyNotes'] = 'Proper rigid rotation only: (X,Y,Z) becomes (Y-55,16.14-X,Z). Original STEP millimetres and every relative assembly position are retained. Source STEP header is 2024-03-08, before the documented December 2025 V2 revision; this is a qualified shared PCBA reference, not a verified V2 layout.'
    p['sourceTopologyTypes'] = dict(collections.Counter(x['topology'] for x in top))
    p['materialBasis'] = 'Native linear colors converted to sRGB. Missing native colors use photo-informed black holder/camera, silver shield/socket and ivory FPC; these are uncalibrated appearances. No fabricated silkscreen, modem identity decal or photograph texture.'
    p['dimensions'] = {'pcb':{'publishedSizeMm':[110,30.44], 'nativeSizeMm':top[11]['size'], 'sourceIds':['dimension-photo','official-cad']},
        'kit':{'publishedSizeMm':[125,31.6,34], 'rendered':False, 'sourceIds':['dimension-photo','product'],
               'status':'unknown-case-not-rendered', 'notes':'Published outer kit envelope includes acrylic/support accessories. No matching case CAD or exact fastening geometry was found in the linked shared PCBA archive.'}}
    p['dimensionTargets'] = [{'part':'pcb-main','sizeMm':[110,30.44,1.59841946], 'toleranceMm':.05,
        'sourceId':'dimension-photo', 'notes':'Published bare PCB X/Y; thickness is a native CAD measurement, not a dimension printed on the photo.'}]
    p['validation'] = {'controls':[], 'ledIndices':[0]}
    p['presentation'] = {'inspectionGroups':[{'id':'camera-reference', 'parts':['camera-reference'],
        'offsetMm':[0,40,20], 'sourceIds':['official-cad','product'],
        'notes':'Inspect the native camera independently. Kit specifies OV5640; exact source camera housing/sensor identity and V2 placement remain unverified.'}]}
    x0,x1,y0,y1 = crop
    p['photoRegistration'] = {'review':{'file':'dimension-photo.webp',
        'pixelFromModelXY':[[(x1-x0)/110,0,(x1+x0)/2],[0,-(y1-y0)/30.44,(y1+y0)/2]],
        'includeParts':['pcb-main'], 'basis':'Exact model dimension photo front PCB silhouette registered to the published 110 x 30.44 mm outline. Reference kit picture is not used as a scale target; retained native board contour is not rescaled.'}}
    p['sourceSearch'].update(status='official-shared-PCBA-STEP-found', inspected=['overview','resources','product','cad-archive'],
        result='Both exact manufacturer resource pages link this shared original PCBA archive. All 738 solids and 4 open shells enumerated; 741 parts rendered after documented exclusion of the unbundled battery. No exact acrylic kit case or current V2 PCB replacement found in that archive.')
    p['limitations'] = [
        'This is the shared manufacturer PCBA reference from a STEP header dated 2024-03-08; the current V2 pin assignment is independently sourced. Exact V2 layout and modem-specific mechanical/BOM differences are unknown.',
        'SKU '+sku+' includes an acrylic case/support and accessories. The picker explicitly labels their absence. No full 125 x 31.6 x 34 mm kit, screw/standoff fit, battery, loose antenna, speaker or cable assembly is claimed.',
        'The native battery is omitted because the sale kit excludes it. Battery holder and all other 741 native components, including all four open shells, remain. Source assembly bytes are preserved unchanged.',
        'Camera reference is retained from manufacturer CAD; OV5640 is a package fact, not proof of exact lens/housing CAD identity. No camera image or USB/cellular behavior is synthesized.',
        'Native RGB body 410 is associated with the one documented GPIO38 chain output using the original photo. Current V2 spatial placement is uncalibrated; numeric chain output is independently tested.',
        'BOOT is available in the workbench. Switch actuator separation, unknown acrylic openings and connector mating positions are unverified, so no 3D button targets or connected cables are invented.',
        'Native material colors are not calibrated physical color. No hardware fit, WebGL browser, physical optical or full original application acceptance claim.']
    h.finish(dst,p)
