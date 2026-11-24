#!/usr/bin/env python3
"""Regression: a label inside a PCB must not print over an opaque screen."""
import importlib.util
from pathlib import Path

module_path = Path(__file__).with_name('review-waveshare-models.py')
spec = importlib.util.spec_from_file_location('waveshare_review', module_path)
review = importlib.util.module_from_spec(spec)
spec.loader.exec_module(review)

triangles = [[[-1, -1], [1, -1], [1, 1]], [[-1, -1], [1, 1], [-1, 1]]]
_, extent, depth = review.rasterize(triangles, [[0, 0, 0], [0, 0, 0]], [[0, 0, 0]] * 2, resolution=96)
assert not review.annotation_visible([0, 0], -5.735, extent, depth), 'Hidden inner PCB label leaked through the display'
assert review.annotation_visible([0, 0], .015, extent, depth), 'An exposed label above its support plane disappeared'
assert not review.annotation_visible([5, 0], 1, extent, depth), 'Off-image annotation was accepted'
print('Review annotation occlusion: 3 assertions passed')
