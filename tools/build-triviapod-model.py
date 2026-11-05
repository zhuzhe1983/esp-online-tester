#!/usr/bin/env python3
"""Export the pinned official TriviaPOD display model (requires numpy).

Offline: python tools/build-triviapod-model.py --output /tmp/triviapod-rebuild
The retained gzip contains the exact glTF decoded from window.GLTF_B64 in the
official website asset. No downloaded JavaScript is executed. This is a display
model, not production CAD or evidence of the physical product's dimensions.
"""
import argparse
import base64
import gzip
import hashlib
import json
import shutil
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web/platform/assets/m5stack-triviapod'
SOURCE_SHA = 'd2870244aa86264b14f44932bb47d78a7f0b0b5ecf5c3e2844b28a7dc672d07a'
ROTATION = np.array([[0, -1, 0], [0, 0, -1], [1, 0, 0]], dtype=float)
NAMES = ['shell', 'button-blue', 'screen-glass', 'button-green', 'power-button', 'usb-c']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(source_dir, output):
    raw = gzip.decompress((source_dir / 'source.gltf.gz').read_bytes())
    assert sha(raw) == SOURCE_SHA, 'Official source changed; review before rebuilding.'
    gltf = json.loads(raw)
    assert gltf['asset']['version'] == '2.0'
    assert len(gltf['nodes']) == len(gltf['meshes']) == 2 and len(gltf['materials']) == 6
    assert not gltf.get('images') and not gltf.get('textures') and not gltf.get('animations')
    buffer = base64.b64decode(gltf['buffers'][0]['uri'].split(',', 1)[1], validate=True)
    assert len(buffer) == gltf['buffers'][0]['byteLength']

    def accessor(index):
        spec = gltf['accessors'][index]
        assert not spec.get('sparse')
        view = gltf['bufferViews'][spec['bufferView']]
        assert view['buffer'] == 0
        dtype = np.dtype({5123: '<u2', 5125: '<u4', 5126: '<f4'}[spec['componentType']])
        columns = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4}[spec['type']]
        offset = view.get('byteOffset', 0) + spec.get('byteOffset', 0)
        stride = view.get('byteStride', columns * dtype.itemsize)
        end = spec.get('byteOffset', 0) + (spec['count'] - 1) * stride + columns * dtype.itemsize
        assert end <= view['byteLength']
        return np.ndarray((spec['count'], columns), dtype=dtype, buffer=buffer,
                          offset=offset, strides=(stride, dtype.itemsize))

    def node_matrix(node):
        assert not node.get('children') and not node.get('skin')
        if 'matrix' in node:
            return np.array(node['matrix']).reshape(4, 4).T
        x, y, z, w = node.get('rotation', [0, 0, 0, 1])
        norm = np.linalg.norm([x, y, z, w])
        x, y, z, w = np.array([x, y, z, w]) / norm
        result = np.eye(4)
        result[:3, :3] = np.array([
            [1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w)],
            [2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w)],
            [2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y)],
        ]) @ np.diag(node.get('scale', [1, 1, 1]))
        result[:3, 3] = node.get('translation', [0, 0, 0])
        return result

    def srgb_hex(linear):
        c = np.array(linear[:3])
        c = np.where(c <= .0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - .055)
        return '#' + ''.join(f'{round(float(v)*255):02x}' for v in c)

    transform = np.eye(4)
    transform[:3, :3] = ROTATION
    transform[:3, 3] = [0, 0, -7.75]
    parts, buffers, offset = [], [], 0
    for ni in gltf['scenes'][gltf.get('scene', 0)]['nodes']:
        node = gltf['nodes'][ni]
        matrix = transform @ node_matrix(node)
        assert np.linalg.det(matrix[:3, :3]) > 0
        normal_matrix = np.linalg.inv(matrix[:3, :3]).T
        for pi, primitive in enumerate(gltf['meshes'][node['mesh']]['primitives']):
            assert primitive.get('mode', 4) == 4 and not primitive.get('targets')
            attrs = primitive['attributes']
            if 'COLOR_0' in attrs:
                # All source vertex colors are white; retain the material colors.
                spec = gltf['accessors'][attrs['COLOR_0']]
                assert spec.get('normalized') and np.all(accessor(attrs['COLOR_0']) == 65535)
            vertices = accessor(attrs['POSITION']) @ matrix[:3, :3].T + matrix[:3, 3]
            normals = accessor(attrs['NORMAL']) @ normal_matrix.T
            normals /= np.linalg.norm(normals, axis=1)[:, None]
            indices = accessor(primitive['indices']).reshape(-1)
            assert len(indices) % 3 == 0 and indices.max() < len(vertices)
            data = np.concatenate((vertices[indices], normals[indices]), axis=1).astype('<f4')
            assert np.isfinite(data).all()
            material_index = primitive['material']
            material = gltf['materials'][material_index]
            pbr = material['pbrMetallicRoughness']
            name = NAMES[material_index]
            parts.append({
                'name': name, 'role': 'shell' if name == 'shell' else 'source-part',
                'sourceNode': ni, 'sourceMesh': node['mesh'], 'sourcePrimitive': pi,
                'sourceMaterial': material, 'sourceNodeTransform': node_matrix(node).tolist(),
                'byteOffset': offset, 'vertexCount': len(indices),
                'color': '#d5d5d5' if name == 'shell' else srgb_hex(pbr.get('baseColorFactor', [1, 1, 1, 1])),
                'material': {'metalness': pbr.get('metallicFactor', 1), 'roughness': pbr.get('roughnessFactor', 1)},
                'boundsMm': [data[:, :3].min(axis=0).tolist(), data[:, :3].max(axis=0).tolist()],
            })
            buffers.append(data.tobytes())
            offset += data.nbytes
    assert len(parts) == 6 and sum(p['vertexCount'] // 3 for p in parts) == 81532
    output.mkdir(parents=True, exist_ok=True)
    for name in ['source.gltf.gz', 'source-receipts.json', 'SOURCE-RIGHTS.txt',
                 *[f'reference-photo-{i:02d}.webp' for i in range(5, 9)]]:
        if (source_dir / name).resolve() != (output / name).resolve():
            shutil.copyfile(source_dir / name, output / name)
    (output / 'geometry.bin').write_bytes(b''.join(buffers))
    metadata = {
        'schemaVersion': '1', 'id': 'm5stack-triviapod', 'unit': 'mm',
        'binary': 'geometry.bin', 'strideFloats': 6, 'parts': parts,
        'sourceTransform': {
            'matrixRows': transform.tolist(), 'scale': 1,
            'note': 'Apply each original glTF node transform first, then (-Y,-Z,X-7.75). Numeric authoring units are displayed as mm without scaling, inferred from the StopWatch-size enclosure. glTF normally uses metres, but this website model has a 51.947-unit diameter and its viewer normalizes the size. TriviaPOD official physical dimensions remain WIP; this is not a verified metric CAD model.',
        },
        'omittedComponents': [],
        'derivedParts': [{
            'id': 'pcb-main', 'kind': 'pcb-placeholder', 'color': '#686d69', 'label': 'UNKNOWN',
            'basis': 'estimated', 'sourceIds': [],
            'geometry': {'shape': 'disc', 'sizeMm': [44, 44, 1], 'centerMm': [0, 0, 0], 'labelSide': 'both'},
            'notes': 'Blank neutral placeholder only: outline, thickness, location and color are unverified, estimated to fit inside the display enclosure. Not an electronics reconstruction. Read UNKNOWN from the back in transparent-shell mode.',
        }],
        'provenance': {
            'sourceUrl': 'https://triviapod.com/assets/models/TriviaPod3D.gltf.js',
            'sourceSha256': 'fd222026f75612a56fba1eaa3dfe81a9c61c79133f0026c54effaa4ed8f889b3',
            'decodedSha256': SOURCE_SHA,
            'decode': 'Parse the window.GLTF_B64 string literal as data; strict base64 decode. Store exact decoded bytes in gzip with mtime=0.',
            'sourceKind': 'Official promotional glTF, not production CAD.',
            'sourceLicense': 'No explicit reusable license found; see SOURCE-RIGHTS.txt. Not relicensed under the project MIT license.',
            'sources': json.loads((source_dir / 'source-receipts.json').read_text()),
        },
        'limitations': [
            'Only the official exterior display meshes are known. No PCB layout, board assembly, internal battery or wiring geometry was supplied.',
            'Model units are inferred for display; actual product dimensions and weight are still WIP on the official page.',
            'All six source primitive meshes and their original node assembly transforms are retained. White identity vertex colors and unused UVs are omitted.',
            'Original source material definitions are retained in sourceMaterial. The white shell uses a #d5d5d5 display albedo to avoid clipping its relief under the platform lights; this is not measured plastic color. Other base colors, roughness and metalness are retained. The viewer approximates clearcoat and screen transmission with its standard opaque materials; no game screenshot or simulated TriviaPOD framebuffer is added.',
            'TriviaPOD is an independent catalog product with a visual-only model and no implemented board runtime. Its 32 MB flash and proprietary Etermax firmware do not inherit the StopWatch runtime, default firmware, pins or expansion interfaces.',
        ],
    }
    metadata['artifacts'] = {p.name: sha(p.read_bytes()) for p in sorted(output.iterdir())
                             if p.name in ['source.gltf.gz', 'geometry.bin', 'SOURCE-RIGHTS.txt', 'source-receipts.json']
                             or p.name.startswith('reference-photo-')}
    (output / 'model.json').write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'parts': len(parts), 'triangles': 81532, 'bytes': offset,
                      'geometrySha256': sha((output / 'geometry.bin').read_bytes())}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sources', type=Path, default=ASSETS)
    parser.add_argument('--output', type=Path, default=ASSETS)
    args = parser.parse_args()
    build(args.sources, args.output)
