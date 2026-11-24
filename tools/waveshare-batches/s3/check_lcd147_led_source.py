#!/usr/bin/env python3
"""Reproduce the LCD1.47 LED1 XDE name -> native topology binding, offline.

Uses the original retained STEP and the same topology ordering as the geometry
exporter. No name is inferred from package size or a photo annotation.
"""
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile

from OCP.BRepBndLib import BRepBndLib
from OCP.Bnd import Bnd_Box
from OCP.IFSelect import IFSelect_RetDone
from OCP.STEPCAFControl import STEPCAFControl_Reader
from OCP.TCollection import TCollection_ExtendedString
from OCP.TDataStd import TDataStd_Name
from OCP.TDF import TDF_Label, TDF_LabelSequence
from OCP.TDocStd import TDocStd_Document
from OCP.TopLoc import TopLoc_Location
from OCP.XCAFDoc import XCAFDoc_DocumentTool

ROOT = Path(__file__).resolve().parents[3]
ASSET = ROOT / 'web/platform/assets/waveshare-esp32-s3-lcd-1-47'
spec = importlib.util.spec_from_file_location('source_builder', ROOT / 'tools/build-waveshare-models.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)

plan = json.loads((ASSET / 'model-plan.json').read_text())
source = next(s for s in plan['sources'] if s['id'] == 'official-cad')
raw = gzip.decompress((ASSET / source['file']).read_bytes())
assert hashlib.sha256(raw).hexdigest() == source['sha256']
assert len(raw) == source['bytes']
assert source['sha256'] == 'e5c0fb1c4493ad3b98c1dce8f2dc415960cf980fe7f02103e1f462c11dda2550'


def label_name(label):
    attr = TDataStd_Name()
    return attr.Get().ToExtString() if label.FindAttribute(TDataStd_Name.GetID_s(), attr) else ''


with tempfile.TemporaryDirectory(prefix='waveshare-lcd147-led-') as temporary:
    path = Path(temporary) / 'original.stp'
    path.write_bytes(raw)
    shapes = builder.topology(path)
    assert len(shapes) == 158
    native_bounds = [builder.bounds(shape) for shape in shapes]
    document = TDocStd_Document(TCollection_ExtendedString('BinXCAF'))
    reader = STEPCAFControl_Reader()
    assert reader.ReadFile(str(path)) == IFSelect_RetDone and reader.Transfer(document)
    tool = XCAFDoc_DocumentTool.ShapeTool_s(document.Main())
    roots = TDF_LabelSequence()
    tool.GetFreeShapes(roots)
    observations = []

    def visit(label, parent_location, parent_names):
        referred = TDF_Label()
        target = referred if tool.GetReferredShape_s(label, referred) else label
        location = parent_location.Multiplied(tool.GetLocation_s(label))
        name, target_name = label_name(label), label_name(target)
        names = parent_names + [name or target_name]
        if name == 'LED1' and target_name == 'LED1_ASM':
            shape = tool.GetShape_s(target).Moved(location)
            box = Bnd_Box()
            BRepBndLib.Add_s(shape, box)
            values = box.Get()
            bounds = [list(values[:3]), list(values[3:])]
            matches = []
            for index, original in enumerate(native_bounds):
                delta = max(abs(bounds[i][j] - original[i, j]) for i in range(2) for j in range(3))
                if delta < .001:
                    matches.append({'sourceComponent': index, 'maxBoundsDeltaMm': delta})
            observations.append({'path': names, 'labelName': name, 'referredName': target_name,
                                 'globalBoundsMm': bounds, 'topologyMatches': matches})
        children = TDF_LabelSequence()
        if tool.GetComponents_s(target, children):
            for index in range(1, children.Length() + 1):
                visit(children.Value(index), location, names)

    for index in range(1, roots.Length() + 1):
        visit(roots.Value(index), TopLoc_Location(), [])

assert len(observations) == 1
assert [m['sourceComponent'] for m in observations[0]['topologyMatches']] == [2]
led_parts = [part for part in plan['parts'] if part.get('ledIndex') == 0]
assert len(led_parts) == 1 and led_parts[0]['sourceComponent'] == 2
assert not any(part.get('sourceComponent') == 56 and 'ledIndex' in part for part in plan['parts'])
print(json.dumps({'sourceSha256': source['sha256'], 'status': 'pass', 'matches': observations,
                  'electricalSource': 'Retained board-schematic.pdf: LED1 WS2812B-0807; RGB_IO/GPIO38.',
                  'scope': 'Native source identity/placement and declared output binding only; acrylic diffusion and optical appearance are unvalidated.'}, indent=2))
