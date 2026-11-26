"""Decode retained M5Stack model images without claiming rendered acceptance.

Usage: python tools/audit-m5stack-images.py /absolute/receipt.json
"""
import hashlib
import json
from pathlib import Path
import sys

from PIL import Image


root = Path(__file__).resolve().parents[1]
platform = root / 'web/platform'
products = json.loads((platform / 'catalog/products.json').read_text())['items']
directories = set()
for product in products:
    if product.get('manufacturer') != 'M5Stack':
        continue
    model_url = product.get('model', {}).get('url')
    if model_url and model_url.startswith('./assets/'):
        directories.add((platform / model_url).resolve().parent)
    own_assets = platform / 'assets' / product['id']
    if own_assets.is_dir():
        directories.add(own_assets.resolve())

files = sorted({p for d in directories for p in d.rglob('*')
                if p.is_file() and p.suffix.lower() in
                {'.png', '.jpg', '.jpeg', '.webp', '.gif'}})
results = []
for filename in files:
    row = {'file': filename.relative_to(root).as_posix(),
           'sha256': hashlib.sha256(filename.read_bytes()).hexdigest()}
    try:
        with Image.open(filename) as image:
            image.verify()
        with Image.open(filename) as image:
            image.load()
            row.update(width=image.width, height=image.height, mode=image.mode,
                       frames=getattr(image, 'n_frames', 1), decoded=True)
            if 'A' in image.getbands():
                alpha = image.getchannel('A').histogram()
                row['visibleAlphaPixels'] = sum(alpha[1:])
    except Exception as error:
        row.update(decoded=False, error=str(error))
    results.append(row)

failed = sum(not row['decoded'] for row in results)
receipt = {'schemaVersion': 1,
           'scope': 'Pillow verify and full first-frame decode of retained images '
                    'in catalog model directories and per-product M5Stack asset '
                    'directories. No WebGL upload, UV alignment, photograph '
                    'accuracy or real-hardware acceptance.',
           'directories': len(directories), 'images': len(results),
           'failed': failed, 'results': results}
Path(sys.argv[1]).write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({key: receipt[key] for key in ['directories', 'images', 'failed']}))
raise SystemExit(1 if failed else 0)
