#!/usr/bin/env python3
"""Record native component bounds, names and source colors without editing CAD."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import numpy as np
from OCP.STEPCAFControl import STEPCAFControl_Reader
from OCP.TDocStd import TDocStd_Document
from OCP.TCollection import TCollection_ExtendedString
from OCP.TopExp import TopExp_Explorer
from OCP.TopAbs import TopAbs_SOLID, TopAbs_SHELL, TopAbs_FACE
from OCP.XCAFDoc import XCAFDoc_DocumentTool, XCAFDoc_ColorType
from OCP.Quantity import Quantity_Color
from OCP.IFSelect import IFSelect_RetDone

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('builder', ROOT / 'tools/build-waveshare-models.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('files', type=Path, nargs='+')
    args = parser.parse_args()
    for path in args.files:
        output = path.with_name(path.name + '.inspection.json')
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if output.exists():
            old = json.loads(output.read_text())
            if old['sourceSha256'] == digest:
                print(path.name, 'cached', len(old['components']), flush=True)
                continue
        doc = TDocStd_Document(TCollection_ExtendedString('XmlXCAF'))
        reader = STEPCAFControl_Reader()
        reader.SetColorMode(True)
        reader.SetNameMode(True)
        assert reader.ReadFile(str(path)) == IFSelect_RetDone and reader.Transfer(doc)
        root = reader.Reader().OneShape()
        colors = XCAFDoc_DocumentTool.ColorTool_s(doc.Main())
        records = []
        for kind, avoid in [(TopAbs_SOLID, None), (TopAbs_SHELL, TopAbs_SOLID), (TopAbs_FACE, TopAbs_SHELL)]:
            explorer = TopExp_Explorer(root, kind) if avoid is None else TopExp_Explorer(root, kind, avoid)
            while explorer.More():
                shape = explorer.Current()
                bounds = builder.bounds(shape)
                record = {'index': len(records), 'bounds': bounds.tolist(),
                          'size': (bounds[1] - bounds[0]).tolist(), 'center': bounds.mean(axis=0).tolist()}
                entity = reader.Reader().WS().TransferReader().EntityFromShapeResult(shape, 1)
                if entity is not None and hasattr(entity, 'Name'):
                    record['sourceName'] = entity.Name().ToCString()
                color = Quantity_Color()
                if colors.GetColor(shape, XCAFDoc_ColorType.XCAFDoc_ColorSurf, color) or colors.GetColor(shape, XCAFDoc_ColorType.XCAFDoc_ColorGen, color):
                    record['sourceColorLinear'] = [color.Red(), color.Green(), color.Blue()]
                records.append(record)
                explorer.Next()
        result = {'sourceSha256': digest, 'nativeComponents': len(records), 'scale': 1,
                  'bounds': [np.min([r['bounds'][0] for r in records], axis=0).tolist(),
                             np.max([r['bounds'][1] for r in records], axis=0).tolist()], 'components': records}
        output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
        largest = sorted(records, key=lambda r: np.prod(sorted(r['size'])[-2:]), reverse=True)[:7]
        print(json.dumps({'file': path.name, 'count': len(records), 'bounds': result['bounds'],
                          'largest': [{k: r[k] for k in ['index', 'size', 'sourceName'] if k in r} for r in largest]}, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    main()
