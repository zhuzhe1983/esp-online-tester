#!/usr/bin/env python3
"""Build Waveshare previews from retained, hash-pinned CAD and explicit model plans.

Requires cadquery-ocp 7.8.1.1.post1 and numpy 2.3.5. No network, source image
editing, inferred package placement, or nonuniform CAD scaling occurs here.
"""
import argparse
import gzip
import hashlib
import importlib.metadata
import io
import json
from pathlib import Path
import shutil
import tempfile
import zipfile

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web/platform/assets'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write(path, data):
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n')


def rigid(value):
    matrix = np.asarray(value, dtype=float)
    assert matrix.shape == (4, 4) and np.isfinite(matrix).all()
    rotation = matrix[:3, :3]
    assert np.allclose(rotation.T @ rotation, np.eye(3), atol=1e-8)
    assert np.isclose(np.linalg.det(rotation), 1)
    assert np.allclose(matrix[3], [0, 0, 0, 1])
    return matrix


def topology(path):
    from OCP.STEPCAFControl import STEPCAFControl_Reader
    from OCP.TDocStd import TDocStd_Document
    from OCP.TCollection import TCollection_ExtendedString
    from OCP.TopExp import TopExp_Explorer
    from OCP.TopAbs import TopAbs_SOLID, TopAbs_SHELL, TopAbs_FACE
    from OCP.IFSelect import IFSelect_RetDone
    doc = TDocStd_Document(TCollection_ExtendedString('XmlXCAF'))
    reader = STEPCAFControl_Reader()
    reader.SetColorMode(True)
    reader.SetNameMode(True)
    assert reader.ReadFile(str(path)) == IFSelect_RetDone and reader.Transfer(doc)
    root, shapes = reader.Reader().OneShape(), []
    for kind, avoid in [(TopAbs_SOLID, None), (TopAbs_SHELL, TopAbs_SOLID), (TopAbs_FACE, TopAbs_SHELL)]:
        explorer = TopExp_Explorer(root, kind) if avoid is None else TopExp_Explorer(root, kind, avoid)
        while explorer.More():
            shapes.append(explorer.Current())
            explorer.Next()
    return shapes


def bounds(shape):
    from OCP.BRepBndLib import BRepBndLib
    from OCP.Bnd import Bnd_Box
    box = Bnd_Box()
    BRepBndLib.AddOptimal_s(shape, box, False, False)
    return np.asarray(box.Get()).reshape(2, 3)


def face(points, holes=(), z=0):
    from OCP.BRepBuilderAPI import BRepBuilderAPI_MakePolygon, BRepBuilderAPI_MakeFace
    from OCP.TopoDS import TopoDS
    from OCP.gp import gp_Pnt
    def wire(values, clockwise=False):
        values = np.asarray(values, float)
        assert values.ndim == 2 and values.shape[1] == 2 and len(values) >= 3
        area = np.sum(values[:, 0] * np.roll(values[:, 1], -1) - np.roll(values[:, 0], -1) * values[:, 1])
        if (area < 0) != clockwise:
            values = values[::-1]
        polygon = BRepBuilderAPI_MakePolygon()
        for x, y in values:
            polygon.Add(gp_Pnt(float(x), float(y), z))
        polygon.Close()
        return polygon.Wire()
    builder = BRepBuilderAPI_MakeFace(wire(points))
    for hole in holes:
        builder.Add(TopoDS.Wire_s(wire(hole, True)))
    assert builder.IsDone()
    return builder.Face()


def procedural(item):
    from OCP.BRepPrimAPI import BRepPrimAPI_MakeBox, BRepPrimAPI_MakeCylinder
    from OCP.BRepAlgoAPI import BRepAlgoAPI_Cut
    from OCP.gp import gp_Pnt, gp_Ax2, gp_Dir
    kind = item['shape']
    if kind == 'box':
        size, center = np.asarray(item['sizeMm']), np.asarray(item.get('centerMm', [0, 0, 0]))
        assert np.isfinite(size).all() and (size > 0).all()
        return BRepPrimAPI_MakeBox(gp_Pnt(*(center - size / 2)), *size).Shape()
    if kind == 'cylinder':
        x, y, z = item.get('centerMm', [0, 0, 0])
        radius, depth = item['radiusMm'], item['depthMm']
        return BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(x, y, z - depth / 2), gp_Dir(0, 0, 1)), radius, depth).Shape()
    if kind == 'open-usb-shell':
        # A visibly open approximate metal envelope, not an electrical connector.
        size, center = np.asarray(item['sizeMm']), np.asarray(item['centerMm'])
        wall = item['wallMm']
        outer = BRepPrimAPI_MakeBox(gp_Pnt(*(center - size / 2)), *size).Shape()
        inner_size = size - [2 * wall, -2, 2 * wall]
        inner = BRepPrimAPI_MakeBox(gp_Pnt(*(center - inner_size / 2)), *inner_size).Shape()
        cut = BRepAlgoAPI_Cut(outer, inner)
        assert cut.IsDone()
        return cut.Shape()
    if kind == 'photo-face':
        surface = face(item['outlineMm'], item.get('holesMm', []), item.get('zMm', 0))
        for polygon in item.get('excludePolygonsMm', []):
            cut = BRepAlgoAPI_Cut(surface, face(polygon, z=item.get('zMm', 0)))
            assert cut.IsDone()
            surface = cut.Shape()
        return surface.Reversed() if item.get('back') else surface
    raise ValueError('Unsupported explicit geometry: ' + kind)


def tessellate(shape, deflection, angle):
    from OCP.BRepMesh import BRepMesh_IncrementalMesh
    from OCP.TopExp import TopExp_Explorer
    from OCP.TopAbs import TopAbs_FACE, TopAbs_REVERSED
    from OCP.TopoDS import TopoDS
    from OCP.TopLoc import TopLoc_Location
    from OCP.BRep import BRep_Tool
    BRepMesh_IncrementalMesh(shape, deflection, False, angle, False).Perform()
    faces = [shape] if shape.ShapeType() == TopAbs_FACE else []
    if not faces:
        explorer = TopExp_Explorer(shape, TopAbs_FACE)
        while explorer.More():
            faces.append(explorer.Current())
            explorer.Next()
    arrays = []
    for raw in faces:
        f, location = TopoDS.Face_s(raw), TopLoc_Location()
        triangles = BRep_Tool.Triangulation_s(f, location)
        if triangles is None:
            continue
        vertices = []
        for number in range(1, triangles.NbNodes() + 1):
            p = triangles.Node(number).Transformed(location.Transformation())
            vertices.append([p.X(), p.Y(), p.Z()])
        vertices = np.asarray(vertices)
        indices = np.asarray([triangles.Triangle(i).Get() for i in range(1, triangles.NbTriangles() + 1)]) - 1
        if f.Orientation() == TopAbs_REVERSED:
            indices = indices[:, ::-1]
        data = vertices[indices]
        normal = np.cross(data[:, 1] - data[:, 0], data[:, 2] - data[:, 0])
        length = np.linalg.norm(normal, axis=1)
        keep = length > 1e-12
        if np.any(keep):
            arrays.append(np.concatenate([data[keep].reshape(-1, 3), np.repeat(normal[keep] / length[keep, None], 3, axis=0)], axis=1))
    assert arrays, 'Empty selected source topology'
    return np.concatenate(arrays)


def build(product_id, destination):
    source = ASSETS / product_id
    plan = json.loads((source / 'model-plan.json').read_text())
    assert plan['id'] == product_id and product_id.startswith('waveshare-')
    destination.mkdir(parents=True, exist_ok=True)
    artifacts, original_sources = {}, {}
    for entry in plan['sources']:
        filename = entry['file']
        assert Path(filename).name == filename and filename not in ('.', '..')
        data = (source / filename).read_bytes()
        retained = entry.get('retained', entry)
        assert sha(data) == retained['sha256'] and len(data) == retained['bytes'], filename
        original = gzip.decompress(data) if entry.get('retained', {}).get('encoding') == 'gzip' else data
        assert sha(original) == entry['sha256'] and len(original) == entry['bytes'], 'Original source identity: ' + filename
        original_sources[entry['id']] = original
        if source.resolve() != destination.resolve():
            (destination / filename).write_bytes(data)
        artifacts[filename] = sha(data)
    for entry in plan['sources']:
        if entry.get('origin') not in ('zip-member', 'rar-member'):
            continue
        container = original_sources[entry['containerSourceId']]
        assert sha(container) == entry['archive']['containerSha256']
        if entry['origin'] == 'zip-member':
            with zipfile.ZipFile(io.BytesIO(container)) as archive:
                assert archive.namelist().count(entry['archive']['entry']) == 1, 'Ambiguous original ZIP member'
                assert archive.read(entry['archive']['entry']) == original_sources[entry['id']], 'ZIP member/source mismatch'
        else:
            from waveshare_source_archives import rar_members
            member = entry['archive']['entry']
            assert rar_members(container, {member})[member] == original_sources[entry['id']], 'RAR member/source mismatch'
    records, buffers, checks, offset = [], [], [], 0
    source_selections = {}
    settings = plan.get('tessellation', {'deflectionMm': .05, 'angleRadians': .25})
    cache = {}
    with tempfile.TemporaryDirectory(prefix='waveshare-cad-') as temporary:
        for item in plan.get('parts', []):
            matrix = rigid(item.get('matrixRows', plan.get('matrixRows', np.eye(4).tolist())))
            if 'sourceFile' in item:
                name = item['sourceFile']
                assert name in artifacts, 'CAD source is not retained and pinned'
                if name not in cache:
                    path = source / name
                    if name.endswith('.gz'):
                        data = gzip.decompress(path.read_bytes())
                        entry = next(s for s in plan['sources'] if s['file'] == name)
                        assert sha(data) == entry['sha256'] and len(data) == entry['bytes']
                        path = Path(temporary) / name.removesuffix('.gz')
                        path.write_bytes(data)
                    cache[name] = topology(path)
                    assert len(cache[name]) == plan['sourceTopologyCounts'][name], 'CAD topology identity changed'
                assert ('sourceComponent' in item) != ('sourceComponents' in item), 'Select one source index or an explicit group'
                indices = item.get('sourceComponents', [item.get('sourceComponent')])
                assert isinstance(indices, list) and 0 < len(indices) <= 4096
                assert all(type(i) is int and 0 <= i < len(cache[name]) for i in indices)
                assert len(set(indices)) == len(indices), 'Duplicate source component within a draw group'
                source_selections.setdefault(name, []).extend(indices)
                shapes = [cache[name][index] for index in indices]
            else:
                shapes = [procedural(item['geometry'])]
            arrays, component_checks = [], []
            for component_number, shape in enumerate(shapes):
                original = tessellate(shape, **{'deflection': settings['deflectionMm'], 'angle': settings['angleRadians']})
                original[:, :3] = original[:, :3] @ matrix[:3, :3].T + matrix[:3, 3]
                original[:, 3:] = original[:, 3:] @ matrix[:3, :3].T
                arrays.append(original)
                if 'sourceFile' in item:
                    raw = bounds(shape)
                    corners = np.asarray([[x, y, z] for x in raw[:, 0] for y in raw[:, 1] for z in raw[:, 2]])
                    corners = corners @ matrix[:3, :3].T + matrix[:3, 3]
                    expected = np.asarray([corners.min(0), corners.max(0)])
                    positions = original[:, :3].astype('<f4')
                    exported = np.asarray([positions.min(0), positions.max(0)])
                    deviation = float(np.abs(expected - exported).max())
                    assert deviation <= .05, f'Native bounds exceed declared export tolerance: {item["name"]} component {indices[component_number]} {deviation}'
                    component_checks.append({'sourceComponent': indices[component_number], 'sourceBoundsMm': raw.tolist(),
                                             'maxSourceBoundsDeviationMm': deviation, 'triangles': len(original) // 3})
            original = np.concatenate(arrays)
            data = original.astype('<f4')
            error = float(np.abs(original[:, :3] - data[:, :3]).max())
            assert error < .001 and np.isfinite(data).all()
            exported_bounds = np.asarray([data[:, :3].min(0), data[:, :3].max(0)])
            part = {k: v for k, v in item.items() if k not in ['geometry', 'matrixRows']}
            part.update(byteOffset=offset, vertexCount=len(data), boundsMm=exported_bounds.tolist())
            records.append(part)
            buffers.append(data.tobytes())
            check = {'name': item['name'], 'maxFloat32ErrorMm': error, 'triangles': len(data) // 3}
            if 'sourceFile' in item:
                component_bounds = np.asarray([entry['sourceBoundsMm'] for entry in component_checks])
                raw = [component_bounds[:, 0].min(0).tolist(), component_bounds[:, 1].max(0).tolist()]
                check.update(sourceBoundsMm=raw, maxSourceBoundsDeviationMm=max(entry['maxSourceBoundsDeviationMm'] for entry in component_checks))
                if 'sourceComponents' in item:
                    check['componentChecks'] = component_checks
            checks.append(check)
            offset += data.nbytes
    for name, coverage in plan.get('sourceCoverage', {}).items():
        selected = source_selections.get(name, [])
        assert len(selected) == len(set(selected)), 'Source component reused across draw groups'
        assert sorted(selected) == coverage['includedIndices'], 'Source coverage differs from plan'
        omitted = coverage.get('omittedIndices', [])
        assert not set(selected).intersection(omitted) and len(omitted) == len(set(omitted))
        assert sorted(selected + omitted) == list(range(plan['sourceTopologyCounts'][name])), 'Incomplete source topology accounting'
    assert len(records) <= 512 and 0 < offset <= 64 * 1024 * 1024
    binary = b''.join(buffers)
    compressed = gzip.compress(binary, compresslevel=9, mtime=0)
    (destination / 'geometry.bin.gz').write_bytes(compressed)
    artifacts['geometry.bin.gz'] = sha(compressed)
    for filename in ['model-plan.json', 'source-manifest.json', 'SOURCE-RIGHTS.txt']:
        if source.resolve() != destination.resolve():
            shutil.copyfile(source / filename, destination / filename)
        artifacts[filename] = sha((source / filename).read_bytes())
    metadata = {k: plan[k] for k in ['id', 'dimensions', 'presentation', 'limitations', 'referencePhotos', 'method', 'sourceSearch', 'photoRegistration', 'omittedSourceComponents', 'sourceCoverage'] if k in plan}
    metadata.update(schemaVersion='1', unit='mm', strideFloats=6, binary='geometry.bin.gz', binaryCompression='gzip',
                    parts=records, derivedParts=plan.get('derivedParts', []), artifacts=artifacts,
                    provenance={'kind': plan['method'], 'sources': plan['sources'], 'generator': 'tools/build-waveshare-models.py',
                                'generatorSha256': sha(Path(__file__).read_bytes()),
                                'tessellation': settings, 'licenseFile': 'SOURCE-RIGHTS.txt', 'materialBasis': plan['materialBasis']},
                    sourceTransform={'matrixRows': plan.get('matrixRows', np.eye(4).tolist()), 'scale': 1, 'note': plan['assemblyNotes']})
    write(destination / 'model.json', metadata)
    check = {'id': product_id, 'modelSha256': sha((destination / 'model.json').read_bytes()),
             'geometry': {'rawBytes': len(binary), 'rawSha256': sha(binary), 'gzipBytes': len(compressed), 'gzipSha256': sha(compressed)},
             'generatorSha256': sha(Path(__file__).read_bytes()),
             'engine': {name: importlib.metadata.version(name) for name in ['cadquery-ocp', 'numpy']},
             'checks': checks, 'dimensionTargets': plan.get('dimensionTargets', []),
             'scope': 'Hash-pinned source export and finite/range/bounds checks. Photo, browser, hardware fit and firmware acceptance are separate.'}
    write(destination / 'build-check.json', check)
    print(json.dumps({'id': product_id, 'parts': len(records), 'gzipBytes': len(compressed), 'rawBytes': len(binary), 'status': 'built'}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ids', nargs='*')
    parser.add_argument('--output', type=Path, required=True)
    arguments = parser.parse_args()
    selected = arguments.ids or sorted(p.parent.name for p in ASSETS.glob('waveshare-*/model-plan.json'))
    for product_id in selected:
        build(product_id, arguments.output / product_id)
