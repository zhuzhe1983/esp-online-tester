#!/usr/bin/env python3
"""Rebuild Seeed product previews from retained, hash-pinned CAD and model plans.

Usage: python tools/build-seeed-models.py [product-id ...] --output /new/directory
Dependencies: tools/requirements-model.txt. This command does not use the network.
Manufacturer CAD is never stretched to match a catalogue size. Optional source
assemblies, duplicate instances and unplaced geometry are listed in each plan.
Photo-derived frames and UNKNOWN boards are explicitly distinct from CAD.
Button control IDs are copied from each model plan; binding changes never move
or resize the source geometry.
"""
import argparse
import functools
import hashlib
import json
import math
import re
import shutil
from pathlib import Path

import numpy as np
import trimesh

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web/platform/assets'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def source_path(product_id, entry, root=None):
    """Shared retained sources avoid duplicating large family CAD in every SKU."""
    root = ASSETS if root is None else root
    owner = entry.get('sharedAssetProductId', product_id)
    name = entry['file']
    assert isinstance(owner, str) and re.fullmatch(r'seeed-[a-z0-9]+(?:-[a-z0-9]+)*', owner)
    assert not entry.get('sharedAssetProductId') or owner != product_id, 'Shared sources must name a different asset owner'
    assert isinstance(name, str) and Path(name).name == name and '\\' not in name and name not in ('.', '..')
    path = root / owner / name
    assert path.parent.resolve().parent == root.resolve() and path.resolve().parent == path.parent.resolve(), 'Source must remain in its fixed asset directory'
    return path


@functools.lru_cache(maxsize=2)
def load_step(path, selected, deflection, angle):
    """Tessellate selected native topology; skipping small packages does not move it."""
    from OCP.STEPControl import STEPControl_Reader
    from OCP.IFSelect import IFSelect_RetDone
    from OCP.TopExp import TopExp_Explorer
    from OCP.TopAbs import TopAbs_SOLID, TopAbs_SHELL, TopAbs_FACE, TopAbs_REVERSED
    from OCP.TopoDS import TopoDS
    from OCP.BRepMesh import BRepMesh_IncrementalMesh
    from OCP.BRep import BRep_Tool
    from OCP.TopLoc import TopLoc_Location
    reader = STEPControl_Reader()
    assert reader.ReadFile(str(path)) == IFSelect_RetDone
    reader.TransferRoots()
    root, shapes, result = reader.OneShape(), [], {}
    for kind, avoid in [(TopAbs_SOLID, None), (TopAbs_SHELL, TopAbs_SOLID), (TopAbs_FACE, TopAbs_SHELL)]:
        explorer = TopExp_Explorer(root, kind) if avoid is None else TopExp_Explorer(root, kind, avoid)
        while explorer.More():
            shapes.append((explorer.Current(), kind))
            explorer.Next()
    assert all(0 <= index < len(shapes) for index in selected), 'Source topology changed'
    for index in sorted(selected):
        shape, kind = shapes[index]
        BRepMesh_IncrementalMesh(shape, deflection, False, angle, False).Perform()
        vertices, faces, face_shapes = [], [], []
        if kind == TopAbs_FACE:
            face_shapes.append(shape)
        else:
            explorer = TopExp_Explorer(shape, TopAbs_FACE)
            while explorer.More():
                face_shapes.append(explorer.Current())
                explorer.Next()
        for item in face_shapes:
            face, location = TopoDS.Face_s(item), TopLoc_Location()
            triangulation = BRep_Tool.Triangulation_s(face, location)
            if not triangulation:
                continue
            start = len(vertices)
            for number in range(1, triangulation.NbNodes() + 1):
                point = triangulation.Node(number).Transformed(location.Transformation())
                vertices.append([point.X(), point.Y(), point.Z()])
            for number in range(1, triangulation.NbTriangles() + 1):
                triangle = [value + start - 1 for value in triangulation.Triangle(number).Get()]
                if face.Orientation() == TopAbs_REVERSED:
                    triangle.reverse()
                faces.append(triangle)
        mesh = trimesh.Trimesh(vertices=vertices, faces=faces, process=True)
        mesh.update_faces(mesh.nondegenerate_faces(height=1e-10))
        mesh.remove_unreferenced_vertices()
        result[index] = mesh
    return result


def rigid(value):
    matrix = np.asarray(value, float)
    assert matrix.shape == (4, 4) and np.isfinite(matrix).all(), 'Invalid source transform'
    rotation = matrix[:3, :3]
    assert np.allclose(rotation.T @ rotation, np.eye(3), atol=1e-7)
    assert np.isclose(np.linalg.det(rotation), 1), 'Only rigid, right-handed transforms are allowed'
    assert np.allclose(matrix[3], [0, 0, 0, 1])
    return matrix


def procedural_mesh(item):
    """Simple, dimensioned display/frame geometry; no generated photographic art."""
    shape = item['shape']
    if shape == 'boxes':
        meshes = []
        for box in item['boxes']:
            mesh = trimesh.creation.box(extents=box['sizeMm'])
            angle = box.get('rotationZRadians', 0)
            rotation = trimesh.transformations.rotation_matrix(angle, [0, 0, 1])
            mesh.apply_transform(rotation)
            mesh.apply_translation(box['centerMm'])
            meshes.append(mesh)
        assert meshes, 'Empty procedural box group'
        return trimesh.util.concatenate(meshes)
    if shape == 'plate':
        # The outline and drilled holes come from retained manufacturer PCB Edge.Cuts.
        # This creates a substrate, not an assertion that missing package CAD is known.
        from OCP.BRepBuilderAPI import BRepBuilderAPI_MakePolygon, BRepBuilderAPI_MakeEdge, BRepBuilderAPI_MakeWire, BRepBuilderAPI_MakeFace
        from OCP.BRepPrimAPI import BRepPrimAPI_MakePrism
        from OCP.BRepMesh import BRepMesh_IncrementalMesh
        from OCP.BRep import BRep_Tool
        from OCP.gp import gp_Pnt, gp_Vec, gp_Dir, gp_Ax2, gp_Circ
        from OCP.TopExp import TopExp_Explorer
        from OCP.TopAbs import TopAbs_FACE, TopAbs_REVERSED
        from OCP.TopoDS import TopoDS
        from OCP.TopLoc import TopLoc_Location
        points = np.asarray(item['outlineMm'], float)
        assert len(points) >= 3 and points.shape[1] == 2 and np.isfinite(points).all()
        if np.sum(points[:, 0] * np.roll(points[:, 1], -1) - np.roll(points[:, 0], -1) * points[:, 1]) < 0:
            points = points[::-1]
        thickness = item['thicknessMm']
        assert 0 < thickness <= 10
        polygon = BRepBuilderAPI_MakePolygon()
        for x, y in points:
            polygon.Add(gp_Pnt(float(x), float(y), -thickness / 2))
        polygon.Close()
        face = BRepBuilderAPI_MakeFace(polygon.Wire())
        for hole in item.get('holes', []):
            x, y = hole['centerMm']
            circle = gp_Circ(gp_Ax2(gp_Pnt(x, y, -thickness / 2), gp_Dir(0, 0, 1)), hole['radiusMm'])
            wire = BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(circle).Edge()).Wire()
            face.Add(TopoDS.Wire_s(wire.Reversed()))
        assert face.IsDone(), 'Invalid PCB outline or hole'
        solid = BRepPrimAPI_MakePrism(face.Face(), gp_Vec(0, 0, thickness)).Shape()
        BRepMesh_IncrementalMesh(solid, .025, False, .25, False).Perform()
        explorer = TopExp_Explorer(solid, TopAbs_FACE)
        vertices, faces = [], []
        while explorer.More():
            f, location = TopoDS.Face_s(explorer.Current()), TopLoc_Location()
            triangulation = BRep_Tool.Triangulation_s(f, location)
            if triangulation:
                start = len(vertices)
                for index in range(1, triangulation.NbNodes() + 1):
                    point = triangulation.Node(index).Transformed(location.Transformation())
                    vertices.append([point.X(), point.Y(), point.Z()])
                for index in range(1, triangulation.NbTriangles() + 1):
                    triangle = [n + start - 1 for n in triangulation.Triangle(index).Get()]
                    faces.append(triangle[::-1] if f.Orientation() == TopAbs_REVERSED else triangle)
            explorer.Next()
        result = trimesh.Trimesh(vertices=vertices, faces=faces, process=True)
        assert result.is_watertight, 'PCB substrate must be a closed solid'
        return result
    if shape == 'box':
        mesh = trimesh.creation.box(extents=item['sizeMm'])
        mesh.apply_translation(item.get('centerMm', [0, 0, 0]))
        return mesh
    if shape == 'ring':
        # Four connected-sided trapezoids, with independently stated front/back
        # outer and inner rectangles. This makes an actual aperture, not a decal.
        outer = item['outerSizeMm']
        inner = item['innerSizeMm']
        z0, z1 = item['zRangeMm']
        inset = item.get('frontInsetMm', 0)
        center = np.asarray(item.get('centerMm', [0, 0, 0]), float)
        def ring(w, h, z):
            return [[-w/2, -h/2, z], [w/2, -h/2, z], [w/2, h/2, z], [-w/2, h/2, z]]
        aperture_offset = np.asarray([*item.get('innerCenterOffsetMm', [0, 0]), 0], float)
        vertices = np.concatenate([np.asarray(ring(*outer, z0)), np.asarray(ring(*inner, z0)) + aperture_offset,
                                   np.asarray(ring(outer[0]-2*inset, outer[1]-2*inset, z1)),
                                   np.asarray(ring(inner[0]+2*inset, inner[1]+2*inset, z1)) + aperture_offset]) + center
        faces = []
        def quad(a, b, c, d):
            faces.extend([[a, b, c], [a, c, d]])
        for i in range(4):
            j = (i + 1) % 4
            quad(i, j, j+8, i+8)
            quad(i+4, i+12, j+12, j+4)
            quad(i+8, j+8, j+12, i+12)
            quad(i, i+4, j+4, j)
        return trimesh.Trimesh(vertices=vertices, faces=faces, process=False)
    raise ValueError('Unsupported procedural shape: ' + shape)


def build(product_id, output):
    source = ASSETS / product_id
    plan = json.loads((source / 'model-plan.json').read_text())
    destination = output / product_id
    destination.mkdir(parents=True, exist_ok=True)
    for entry in plan['sources']:
        if entry.get('file'):
            path = source_path(product_id, entry)
            # Original HTTP identity remains separate from a documented retained-body redaction.
            retained = entry.get('retained', entry)
            assert path.is_file() and digest(path) == retained['sha256'], f'Source hash mismatch: {path}'
            target = source_path(product_id, entry, output)
            if path.resolve() != target.resolve():
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, target)
    cache = {}
    tessellation = plan.get('tessellation', {'deflectionMm': .12, 'angleRadians': .35})
    records, buffers, checks, inventory = [], [], [], []
    offset = 0
    all_bounds = []
    for item in plan['parts']:
        file = item.get('file')
        matrix = rigid(item.get('matrixRows', plan['matrixRows']))
        rotation = matrix[:3, :3]
        if file:
            owner = item.get('sharedAssetProductId', product_id)
            key = (owner, file)
            assert any(entry.get('file') == file and entry.get('sharedAssetProductId', product_id) == owner for entry in plan['sources']), 'Unpinned CAD input'
            if key not in cache:
                selected = {index for part in plan['parts'] if part.get('file') == file and part.get('sharedAssetProductId', product_id) == owner for index in part['components']}
                # Independently identified family SKUs can share identical source
                # tessellation; their own metadata and acceptance are regenerated.
                cache[key] = load_step(source_path(product_id, item), tuple(sorted(selected)), tessellation['deflectionMm'], tessellation['angleRadians'])
            meshes = [(index, cache[key][index]) for index in item['components']]
        else:
            meshes = [(None, procedural_mesh(item['geometry']))]
        arrays = []
        for index, mesh in meshes:
            assert mesh is not None and len(mesh.faces), f'Empty source component {file}:{index}'
            vertices = mesh.vertices @ rotation.T + matrix[:3, 3]
            normals = mesh.face_normals @ rotation.T
            arrays.append(np.concatenate([vertices[mesh.faces].reshape(-1, 3), np.repeat(normals, 3, axis=0)], axis=1))
            entry = {'sourceFile': file, 'component': index, 'triangles': len(mesh.faces), 'part': item['name']}
            if item.get('sharedAssetProductId'):
                entry['sharedAssetProductId'] = item['sharedAssetProductId']
            inventory.append(entry)
        original = np.concatenate(arrays)
        data = original.astype('<f4')
        error = float(np.abs(data[:, :3] - original[:, :3]).max())
        assert error <= .05, 'Float export exceeded source tolerance'
        bounds = [data[:, :3].min(0).tolist(), data[:, :3].max(0).tolist()]
        record = {k: v for k, v in item.items() if k not in ['file', 'components', 'matrixRows', 'geometry']}
        record.update(sourceFile=file, sourceComponents=item.get('components', []), byteOffset=offset,
                      vertexCount=len(data), boundsMm=bounds)
        records.append(record)
        buffers.append(data.tobytes())
        all_bounds.append(bounds)
        checks.append({'part': item['name'], 'maxFloat32ExportErrorMm': error, 'toleranceMm': .05, 'pass': True})
        offset += data.nbytes
    assert len(records) <= 512, 'Too many model parts for the platform loader'
    assert 0 < offset <= 64 * 1024 * 1024, 'Geometry exceeds the platform limit'
    binary = b''.join(buffers)
    (destination / 'geometry.bin').write_bytes(binary)
    notice = ('Seeed Studio product models and reference photographs\n\n'
              'Manufacturer material is retained with its original rights. Official public downloads\n'
              'do not supply a separate model/photo redistribution licence unless a source record\n'
              'explicitly identifies one. No MIT relicensing of manufacturer material is claimed.\n'
              'See source-manifest.json and model-plan.json for original URLs, UTC retrieval times,\n'
              'byte counts, SHA-256 identities, selected variants and omitted source geometry.\n'
              'Repository-authored display planes and explicitly marked photo-derived/debug geometry\n'
              'are separate from the manufacturer CAD. These are debugging previews, not production CAD.\n')
    (destination / 'SOURCE-RIGHTS.txt').write_text(notice)
    for name in ['model-plan.json', 'source-manifest.json']:
        if (source / name).exists() and source.resolve() != destination.resolve():
            shutil.copyfile(source / name, destination / name)
    artifacts = {entry['file']: entry.get('retained', entry)['sha256'] for entry in plan['sources'] if entry.get('file') and not entry.get('sharedAssetProductId')}
    shared = {str(source_path(product_id, entry).relative_to(ASSETS)): entry.get('retained', entry)['sha256'] for entry in plan['sources'] if entry.get('file') and entry.get('sharedAssetProductId')}
    artifacts.update({'geometry.bin': digest(destination / 'geometry.bin'),
                      'SOURCE-RIGHTS.txt': digest(destination / 'SOURCE-RIGHTS.txt')})
    for name in ['model-plan.json', 'source-manifest.json']:
        if (destination / name).exists():
            artifacts[name] = digest(destination / name)
    has_cad = any(item.get('file') for item in plan['parts'])
    has_pcb = any(item.get('geometry', {}).get('shape') == 'plate' for item in plan['parts'])
    metadata = {'schemaVersion': '1', 'id': product_id, 'unit': 'mm', 'binary': 'geometry.bin',
                'strideFloats': 6, 'parts': records, 'derivedParts': plan.get('derivedParts', []),
                'dimensions': plan['dimensions'], 'presentation': plan['presentation'],
                'sourceTransform': {'matrixRows': plan['matrixRows'], 'scale': 1, 'note': plan['assemblyNotes']},
                'referencePhotos': plan.get('referencePhotos', []), 'reviewViews': plan.get('reviewViews', {}),
                'omittedComponents': plan.get('omittedComponents', []),
                'provenance': {'kind': plan['method'], 'sources': plan['sources'],
                               'generator': 'tools/build-seeed-models.py',
                               'tessellation': {**tessellation, 'engine': 'OpenCascade' if has_cad else 'OpenCascade PCB extrusion and trimesh package estimates' if has_pcb else 'trimesh procedural geometry', 'units': 'mm', 'selection': 'Selected native solids plus orphan shells/faces; no nonuniform scaling or assembly relocation.' if has_cad else 'Manufacturer PCB outline, drills, thickness and footprint XY preserved. Package heights and explicitly named external panel layouts are estimates; no official 3D assembly is claimed.' if has_pcb else 'Explicitly estimated boxes and aperture frames; no manufacturer CAD.'},
                               'materialBasis': plan.get('materialBasis', 'Source CAD colors where available, otherwise explicitly approximate visual materials.'),
                               'licenseFile': 'SOURCE-RIGHTS.txt'},
                'limitations': plan['limitations'], 'artifacts': artifacts}
    if shared:
        metadata['sharedArtifacts'] = shared
    if plan.get('sourceIdentity'):
        metadata['provenance']['sourceIdentity'] = plan['sourceIdentity']
    write(destination / 'model.json', metadata)
    bounds = np.asarray(all_bounds)
    low, high = bounds[:, 0].min(0), bounds[:, 1].max(0)
    report = {'productId': product_id, 'modelSha256': digest(destination / 'model.json'),
              'geometrySha256': digest(destination / 'geometry.bin'), 'geometryBytes': offset,
              'exportChecks': checks, 'sourceInventory': inventory,
              'exportedBoundsMm': [low.tolist(), high.tolist()], 'exportedExtentsMm': (high-low).tolist(),
              'dimensionChecks': plan.get('dimensionChecks', []),
              'verification': {'sourceScale': 'preserved' if has_cad else 'manufacturer-PCB-XY-and-thickness-preserved' if has_pcb else 'not-applicable-procedural-estimate', 'float32Conversion': 'passed',
                               'photoOverlay': 'not-measured', 'hardwareFit': 'not-tested',
                               'firmware': 'outside-model-check'},
              'limitations': plan['limitations']}
    if shared:
        report['sharedSourceOwners'] = sorted({entry['sharedAssetProductId'] for entry in plan['sources'] if entry.get('sharedAssetProductId')})
        report['sourceIdentity'] = plan.get('sourceIdentity', {})
    write(destination / 'build-check.json', report)
    print(json.dumps({'id': product_id, 'parts': len(records), 'triangles': len(binary)//72,
                      'geometryBytes': offset, 'modelSha256': report['modelSha256']}), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ids', nargs='*')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    ids = args.ids or sorted(path.parent.name for path in ASSETS.glob('seeed-*/model-plan.json'))
    for product_id in ids:
        assert product_id.startswith('seeed-') and '/' not in product_id
        build(product_id, args.output)


if __name__ == '__main__':
    main()
