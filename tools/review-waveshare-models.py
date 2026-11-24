#!/usr/bin/env python3
"""Engineering review plots from the actual production-loader mesh dump.

These orthographic plots do not claim browser/WebGL material acceptance.
Original photos remain unchanged; the second plot overlays measured geometry
coordinates and explicitly registered reference photographs.
"""
import argparse
import json
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parents[1]


def meshes(record):
    result = []
    for mesh in record['meshes']:
        position = np.asarray(mesh['position'], dtype=float).reshape(-1, 3)
        index = np.asarray(mesh['index']).reshape(-1, 3) if mesh['index'] else np.arange(len(position)).reshape(-1, 3)
        result.append((mesh, position[index]))
    return result


def project(points, right, up):
    return np.stack([points @ right, points @ up], axis=-1)


def rasterize(polygons, depths, colors, resolution=720):
    """Opaque orthographic z-buffer; avoids mean-depth painter overlap artifacts."""
    polygons, depths, colors = np.asarray(polygons), np.asarray(depths), np.asarray(colors)
    lower = polygons.reshape(-1, 2).min(0)
    upper = polygons.reshape(-1, 2).max(0)
    padding = np.maximum(upper - lower, .01) * .12
    lower, upper = lower - padding, upper + padding
    pixels = (polygons - lower) / (upper - lower) * resolution - .5
    image = np.empty((resolution, resolution, 3)); image[:] = [228/255, 231/255, 230/255]
    zbuffer = np.full((resolution, resolution), -np.inf)
    for triangle, z, color in zip(pixels, depths, colors):
        xmin, ymin = np.maximum(np.ceil(triangle.min(0)).astype(int), 0)
        xmax, ymax = np.minimum(np.floor(triangle.max(0)).astype(int), resolution - 1)
        if xmax < xmin or ymax < ymin:
            continue
        (x0,y0), (x1,y1), (x2,y2) = triangle
        denominator = (y1-y2)*(x0-x2)+(x2-x1)*(y0-y2)
        if abs(denominator) < 1e-12:
            continue
        ys, xs = np.mgrid[ymin:ymax+1, xmin:xmax+1]
        a = ((y1-y2)*(xs-x2)+(x2-x1)*(ys-y2))/denominator
        b = ((y2-y0)*(xs-x2)+(x0-x2)*(ys-y2))/denominator
        c = 1-a-b
        depth = a*z[0]+b*z[1]+c*z[2]
        region = zbuffer[ymin:ymax+1, xmin:xmax+1]
        visible = (a >= -1e-9) & (b >= -1e-9) & (c >= -1e-9) & (depth > region)
        region[visible] = depth[visible]
        image[ymin:ymax+1, xmin:xmax+1][visible] = color
    return image, (lower[0], upper[0], lower[1], upper[1]), zbuffer


def annotation_visible(point, depth, extent, zbuffer):
    """Labels obey the same depth buffer as geometry, including an inner PCB."""
    xmin, xmax, ymin, ymax = extent
    height, width = zbuffer.shape
    x = int(np.floor((point[0]-xmin)/(xmax-xmin)*width))
    y = int(np.floor((point[1]-ymin)/(ymax-ymin)*height))
    return 0 <= x < width and 0 <= y < height and depth >= zbuffer[y, x] - .001


def review(path, output):
    record = json.loads(path.read_text())
    product_id = record['id']
    directory = ROOT / 'web/platform/assets' / product_id
    metadata = json.loads((directory / 'model.json').read_text())
    plan = json.loads((directory / 'model-plan.json').read_text())
    parts = meshes(record)
    fig, axes = plt.subplots(2, 2, figsize=(12, 10), facecolor='#fafaf6')
    views = [('Front', [1, 0, 0], [0, 1, 0]), ('Reverse', [-1, 0, 0], [0, 1, 0]),
             ('Edge', [1, 0, 0], [0, 0, 1]), ('Oblique', [.8, -.6, 0], [.3, .4, .8660254])]
    for ax, (title, right, up) in zip(axes.flat, views):
        right, up = np.asarray(right), np.asarray(up)
        view = np.cross(right, up)
        polygons, colors, depths, unknown_labels = [], [], [], []
        for mesh, triangles in parts:
            if mesh['unknown']:
                normal = np.asarray(mesh['normal']).reshape(-1, 3).mean(0)
                if np.dot(normal, view) > 0:
                    center = triangles.reshape(-1, 3).mean(0)
                    unknown_labels.append((project(center, right, up), center @ view))
                continue
            color = mesh['color']
            if mesh['texture']:
                parent = next((p for p in metadata['parts'] if mesh['name'] == p['name'] + '-photo-surface'), None)
                if parent is None or parent['role'] == 'photo-appearance':
                    continue
                # The loader splits textured top faces from their side meshes.
                # Retain that geometry, with the parent material's review color.
                color = parent['color'].removeprefix('#')
            normal = np.cross(triangles[:, 1] - triangles[:, 0], triangles[:, 2] - triangles[:, 0])
            length = np.linalg.norm(normal, axis=1)
            good = length > 1e-10
            normal[good] /= length[good, None]
            good &= normal @ view > 1e-8
            if not np.any(good):
                continue
            triangles, normal = triangles[good], normal[good]
            rgb = np.array([int(color[i:i+2], 16) for i in (0, 2, 4)]) / 255
            if mesh['screen']:
                rgb = np.array([.12, .22, .27])  # Explicit review swatch, no firmware image claim.
            light = .62 + .38 * np.maximum(0, normal @ (view * .8 + up * .4 + right * .2))
            polygons.extend(project(triangles, right, up))
            colors.extend(np.clip(rgb * light[:, None], 0, 1))
            depths.extend(triangles @ view)
        if polygons:
            rendered, extent, zbuffer = rasterize(polygons, depths, colors)
            ax.imshow(rendered, origin='lower', extent=extent, interpolation='nearest')
            for point, depth in unknown_labels:
                if annotation_visible(point, depth, extent, zbuffer):
                    ax.text(*point, 'UNKNOWN', ha='center', va='center', color='white', fontsize=8, weight='bold')
        else:
            # A retained front surface does not establish a side or reverse.
            # Keep that absence visible instead of inventing an enclosure.
            ax.text(.5, .5, 'UNKNOWN\nNo sourced surface in this view',
                    transform=ax.transAxes, ha='center', va='center', fontsize=10)
        ax.set_aspect('equal')
        ax.set_title(title)
        ax.set_xlabel('mm')
        ax.set_ylabel('mm')
        ax.set_facecolor('#e4e7e6')
    fig.suptitle(product_id + ('\nExpanded inspection; ' if record.get('inspectionExpanded') else '\n') + 'Actual loader mesh; geometry-only; screen is a review swatch', fontsize=12)
    fig.tight_layout(rect=(0, 0, 1, .94))
    output.mkdir(parents=True, exist_ok=True)
    fig.savefig(output / (product_id + '-four-views.png'), dpi=135)
    plt.close(fig)
    if record.get('inspectionExpanded'):
        print(product_id + ' expanded inspection review written; assembled-photo registration is inapplicable', flush=True)
        return
    if plan.get('photoRegistration', {}).get('review'):
        reg = plan['photoRegistration']['review']
        filename = reg['file']
        affine = np.asarray(reg['pixelFromModelXY'])
    elif product_id.endswith('zero'):
        filename = 'dimension-photo.webp'
        affine = np.array([[528/18, 0, 478], [0, -685/23.5, 505.5]])
    elif product_id.endswith('dev-kit-n8r8'):
        filename = 'dimension-photo.webp'
        affine = np.array([[0, -614/63.3, 437], [-247/25.4, 0, 322.5]])
    else:
        if not any(key in plan.get('photoRegistration', {}) for key in ['driverFront', 'front']):
            # A source photograph is not an asserted registration. Preserve the
            # geometry review without inventing a pixel-to-millimetre transform.
            print(product_id + ' geometry review written; no registered-photo overlay declared', flush=True)
            return
        filename = 'front-photo.jpg'
        reg = plan['photoRegistration']['driverFront' if 'driverFront' in plan['photoRegistration'] else 'front']
        left, top, right, bottom = reg['pcbPixelBounds']
        width, height = reg['nominalSizeMm']
        shift_y = -133 if 'driverFront' in plan['photoRegistration'] else 0
        affine = np.array([[(right-left)/width, 0, (left+right)/2], [0, -(bottom-top)/height, (top+bottom)/2 + shift_y*(bottom-top)/height]])
    fig, axes = plt.subplots(1, 2, figsize=(12, 7), facecolor='white')
    image = np.asarray(ImageOps.exif_transpose(Image.open(directory / filename)))
    for ax in axes:
        ax.imshow(image)
        ax.set_axis_off()
    axes[0].set_title('Unmodified manufacturer reference')
    axes[1].set_title('Same pixels + registered geometry outlines')
    for mesh, triangles in parts:
        if plan.get('photoRegistration', {}).get('review', {}).get('includeParts') is not None and mesh['name'] not in plan['photoRegistration']['review']['includeParts']:
            continue
        if mesh['texture'] or mesh['unknown'] or mesh['screen'] or mesh['name'] == 'optional-glass':
            continue
        # Draw XY convex hulls as footprint comparisons, not hidden package details.
        points = triangles[:, :, :2].reshape(-1, 2)
        if not len(points):
            continue
        from scipy.spatial import ConvexHull, QhullError
        try:
            hull = points[ConvexHull(points).vertices]
        except QhullError:
            continue
        pixel = np.column_stack([hull, np.ones(len(hull))]) @ affine.T
        pixel = np.vstack([pixel, pixel[0]])
        axes[1].plot(pixel[:, 0], pixel[:, 1], color='#00d6a5' if 'pcb' in mesh['name'] else '#e85f16', linewidth=.45, alpha=.7)
    fig.suptitle(product_id + '\nPhoto registration is declared construction evidence; heights and hidden geometry remain unverified', fontsize=11)
    fig.tight_layout(rect=(0, 0, 1, .94))
    fig.savefig(output / (product_id + '-photo-registration.png'), dpi=135)
    plt.close(fig)
    print(product_id + ' review plots written', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--meshes', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    arguments = parser.parse_args()
    for path in sorted(arguments.meshes.glob('waveshare-*.json')):
        review(path, arguments.output)
