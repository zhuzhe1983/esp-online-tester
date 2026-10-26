"""Check explicit paper ROIs and retain actual browser appearance evidence.

Run after the headless capture documented in the product model reports. This
does not render synthetic reference images or certify missing internal parts.
"""
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

receipt_text = (ROOT / 'output/playwright/lane01-sticker-final-browser.txt').read_text()
browser = json.JSONDecoder().raw_decode(receipt_text.split('### Result\n', 1)[1])[0]
assert browser['cacheClearedAndDisabled']

for receipt in browser['receipts']:
    identity = receipt['id']
    assets = ROOT / 'web/platform/assets' / identity
    evidence = ROOT / 'docs/platform/evidence' / identity
    meta = json.loads((assets / 'model.json').read_text())
    plan = json.loads((assets / 'model-plan.json').read_text())
    assert receipt['nonShellMaterialUnchanged'], 'Appearance mutated a screen, key, glass or paper material'
    assert receipt['parts'] == [{'name':p['name'], 'texture':p.get('texture')} for p in meta['parts']], 'Browser metadata is stale'
    for name, expected in meta['artifacts'].items():
        assert digest(assets / name) == expected, name
    checks = []
    if identity == 'm5stack-k132-adv':
        # Independently selected source-photo points: RGB is preserved, alpha
        # removes plastic, screw heads, display and keys from the paper layer.
        samples = {
            'front-stickers.png': [(200,400,255,'ADV paper'), (525,560,255,'key legend paper'), (500,400,0,'LCD'), (201,601,0,'black keycap'), (700,500,0,'gray shell'), (305,507,0,'screw')],
            'back-stickers.png': [(580,400,255,'top paper'), (580,600,255,'center paper'), (580,800,0,'molded nameplate'), (477,510,0,'left screw'), (891,510,0,'right screw'), (300,370,0,'top gray shell')],
        }
        for filename, probes in samples.items():
            definition = next(t for t in plan['textureSources'] if t['file'] == filename)
            image = Image.open(assets / filename).convert('RGBA')
            left, top, _, _ = definition['cropXYXY']
            for x, y, alpha, label in probes:
                actual = image.getpixel((x-left, y-top))[3]
                assert actual == alpha, (filename, label, actual)
                checks.append({'texture':filename, 'sourcePixel':[x,y], 'feature':label, 'expectedAlpha':alpha, 'actualAlpha':actual})
        textured = [p for p in meta['parts'] if p.get('texture')]
        assert {p['name'] for p in textured} == {'shell-front','shell-back','stamp-white-label'}
        assert next(p for p in textured if p['name']=='shell-front')['texture']['zPlanesMm'] == [6,9.6]
        assert next(p for p in textured if p['name']=='shell-back')['texture']['zPlanesMm'] == [-9.6]
    else:
        assert not next(p for p in meta['parts'] if p['name']=='shell-back').get('texture')
        paper = next(p for p in plan['surfaces'] if p['name']=='back-paper')
        assert paper['role']=='label' and paper['side']=='back'
        assert paper['positionMm'][2] == -10.365
        checks.append({'feature':'back paper', 'separateExteriorSurface':True, 'shellTextureAbsent':True, 'coveredInternalPockets':True, 'viewerZMm':-10.365})
        fasteners = [p for p in meta['derivedParts'] if p['id'].startswith('back-fastener')]
        assert len(fasteners) == 4
        assert all(p['material'] == {'roughness':.25,'metalness':.8} for p in fasteners)
        checks.append({'feature':'rear silver fasteners','separateMaterial':True,'cadHoleCentersMm':[[-9,-21],[9,-21]],'profile':'photo-estimated; standard/thread unknown'})
    pictures = []
    grid = Image.new('RGB', (1200, 1640), 'white')
    draw = ImageDraw.Draw(grid)
    for column, appearance in enumerate(['original','gold','half-clear']):
        draw.text((column*400+10,10), appearance, fill='black')
        for row, view in enumerate(['front','back','front-oblique','back-oblique']):
            path = ROOT / f'output/playwright/{identity}-sticker-{appearance}-{view}.png'
            image = Image.open(path).convert('RGB')
            assert image.size == (800,800)
            grid.paste(image.resize((400,400), Image.Resampling.LANCZOS), (column*400,40+row*400))
            pictures.append({'appearance':appearance, 'view':view, 'captureSha256':digest(path)})
    grid.save(evidence / 'sticker-appearance-grid.webp', lossless=True)
    old = ROOT / f'output/playwright/{identity}-before-sticker-fix-gold-back.png'
    if old.exists():
        comparison = Image.new('RGB',(1600,840),'white')
        ImageDraw.Draw(comparison).text((12,12),'Preserved rejected back projection / corrected separate paper or alpha decal',fill='black')
        comparison.paste(Image.open(old).convert('RGB'),(0,40))
        comparison.paste(Image.open(ROOT / f'output/playwright/{identity}-sticker-gold-back.png').convert('RGB'),(800,40))
        comparison.save(evidence / 'sticker-before-after.webp', lossless=True)
    report = {'schemaVersion':'1', 'id':identity, 'checkedAt':browser['capturedAt'], 'browserVersion':browser['browser'], 'headless':True, 'httpCacheClearedAndDisabled':True, 'modelSha256':digest(assets/'model.json'), 'planSha256':digest(assets/'model-plan.json'), 'genericRendererSha256':digest(ROOT/'web/platform/manufacturer-model.js'), 'nonShellMaterialsUnchanged':True, 'nonShellMaterialCount':receipt['nonShellCount'], 'alphaAndSurfaceChecks':checks, 'captures':pictures, 'comparisonSha256':digest(evidence/'sticker-appearance-grid.webp'), 'result':'pass', 'scope':'Paper-versus-shell material separation, front/back/oblique inspection and source alpha probes. Existing source-CAD dimensions are checked independently. No calibrated optical properties, physical hardware measurement, internal assembly completeness or firmware compatibility claim.'}
    (evidence/'sticker-verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(identity+': paper/material isolation passed; source CAD remains partial')
