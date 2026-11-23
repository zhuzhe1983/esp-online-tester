#!/usr/bin/env python3
"""Check DinMeter assembled PCB clearance in exported model-mm geometry.

Requires numpy, trimesh, rtree and the manifold3d intersection backend.
Run from any directory. Pass --baseline to record both model revisions.
"""

import argparse
import hashlib
import itertools
import json
from pathlib import Path
import subprocess

import numpy as np
import trimesh


ROOT = Path(__file__).resolve().parents[1]
PRODUCTS = ("m5stack-k134", "m5stack-k134-v11")


def model_file(sku, name, ref):
    path = Path("web/platform/assets") / sku / name
    if ref:
        return subprocess.check_output(["git", "show", f"{ref}:{path}"], cwd=ROOT)
    return (ROOT / path).read_bytes()


def surface_samples(bounds, spacing=0.25):
    axes = [np.linspace(low, high, int(np.ceil((high - low) / spacing)) + 1) for low, high in bounds.T]
    faces = []
    for axis in range(3):
        face_axes = axes.copy()
        face_axes[axis] = bounds[:, axis]
        faces.extend(itertools.product(*face_axes))
    return np.unique(faces, axis=0)


def measure(sku, ref=None):
    metadata_bytes = model_file(sku, "model.json", ref)
    metadata = json.loads(metadata_bytes)
    payload = model_file(sku, metadata["binary"], ref)
    mesh_sha = hashlib.sha256(payload).hexdigest()
    assert mesh_sha == metadata["artifacts"][metadata["binary"]]
    board = next(part for part in metadata["derivedParts"] if part["id"] == "pcb-main")
    shape = board.get("geometry", {})
    size = shape.get("sizeMm", board.get("dimensionsMm"))
    center = shape.get("centerMm", board.get("centerMm"))
    assert board["kind"] == "pcb-placeholder" and board["label"] == "UNKNOWN"
    assert shape.get("shape", "box") == "box" and not any(shape.get("rotation", []))
    pcb = trimesh.creation.box(size, transform=trimesh.transformations.translation_matrix(center))
    samples = surface_samples(pcb.bounds)
    measured = {}
    for part in metadata["parts"]:
        if part["role"] != "shell":
            continue
        values = np.frombuffer(payload, dtype="<f4", count=part["vertexCount"] * 6, offset=part["byteOffset"])
        vertices = values.reshape(-1, 6)[:, :3]
        mesh = trimesh.Trimesh(vertices=vertices, faces=np.arange(len(vertices)).reshape(-1, 3))
        mesh.fix_normals(multibody=True)
        assert mesh.is_watertight, part["name"]
        overlap = trimesh.boolean.intersection([pcb, mesh], engine="manifold")
        measured[part["name"]] = {
            "overlapMm3": round(overlap.volume, 6) if len(overlap.faces) else 0,
            "sampledMinGapMm": round(-float(trimesh.proximity.signed_distance(mesh, samples).max()), 6),
        }
    return {
        "modelSha256": hashlib.sha256(metadata_bytes).hexdigest(),
        "geometrySha256": mesh_sha,
        "sourceSha256": metadata["provenance"]["sourceSha256"],
        "boxSizeMm": size,
        "boxCenterMm": center,
        "boundsMm": np.round(pcb.bounds, 6).tolist(),
        "surfaceSamples": len(samples),
        "shells": measured,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", metavar="GIT_REF")
    args = parser.parse_args()
    report = {}
    for sku in PRODUCTS:
        current = measure(sku)
        assert all(mesh["overlapMm3"] == 0 and mesh["sampledMinGapMm"] > 0 for mesh in current["shells"].values()), sku
        report[sku] = {"current": current}
        if args.baseline:
            before = measure(sku, args.baseline)
            assert before["geometrySha256"] == current["geometrySha256"], sku
            report[sku]["baseline"] = before
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
