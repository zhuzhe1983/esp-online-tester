#!/usr/bin/env python3
"""Compare part-local oriented position triangles, excluding ordering and normals.

This is weaker than a byte-identical rebuild: it says nothing about normal
vectors, triangle order, metadata identity, materials or physical acceptance.
Usage: python compare-native-position-triangles.py ORIGINAL_DIR REBUILT_DIR
"""
import hashlib
import json
import sys
from pathlib import Path


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical_positions(raw):
    assert len(raw) % 72 == 0, "expected triangles of three position/normal vertices"
    triangles = []
    for offset in range(0, len(raw), 72):
        tri = b"".join(raw[offset + j:offset + j + 12] for j in (0, 24, 48))
        # Preserve winding. Only cyclic changes of the first vertex are allowed.
        triangles.append(min(tri, tri[12:] + tri[:12], tri[24:] + tri[:24]))
    return digest(b"".join(sorted(triangles)))


original, rebuilt = map(Path, sys.argv[1:3])
metadata = json.loads((original / "model.json").read_text())
a, b = [(path / "geometry.bin").read_bytes() for path in (original, rebuilt)]
result = {"method": "SHA-256 of sorted oriented position triangles with canonical cyclic start; exact float32 position bytes, excluding normal vectors", "originalSha256": digest(a), "rebuiltSha256": digest(b), "byteIdentical": a == b, "parts": []}
for part in metadata["parts"]:
    start = part["byteOffset"]
    end = start + part["vertexCount"] * 24
    x, y = [canonical_positions(raw[start:end]) for raw in (a, b)]
    result["parts"].append({"name": part["name"], "vertexCount": part["vertexCount"], "originalPositionSha256": x, "rebuiltPositionSha256": y, "orientedPositionsMatch": x == y})
result["allOrientedPositionsMatch"] = all(p["orientedPositionsMatch"] for p in result["parts"])
print(json.dumps(result, indent=2))
