#!/usr/bin/env python3
"""Retain measured large-panel evidence without copying manufacturer firmware."""
import argparse
import datetime
import gzip
import hashlib
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MODELS = ['waveshare-esp32-s3-touch-lcd-5', 'waveshare-esp32-s3-lcd-5',
          'waveshare-esp32-s3-touch-lcd-5b', 'waveshare-esp32-s3-lcd-5b',
          'waveshare-esp32-s3-touch-lcd-4-3b', 'waveshare-esp32-s3-touch-lcd-4',
          'waveshare-esp32-s3-lcd-4']
LISTINGS = ['esp32-s3-touch-lcd-5', 'esp32-s3-touch-lcd-4.3b', 'esp32-s3-touch-lcd-4']


def read(path):
    return json.loads(path.read_text())


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    cache, out = args.cache, args.output
    out.mkdir(parents=True, exist_ok=True)
    stamp = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
    for name in ['large-rgb-tests.log', 'large-rgb-tail-regression.log',
                 'large-rgb-clippy-final.log', 'large-rgb-native-build.log',
                 'large-catalog-final.log', 'large-knowledge-watchlist-final.log',
                 'large-model-build.log', 'large-model-review.log', 'large-expanded.log']:
        shutil.copy2(cache / name, out / name)
    runs = read(cache / 'large-native-results.json')
    assert len(runs) == 5 and all(r['exit'] == 0 for r in runs)
    save(out / 'original-run-index.json', runs)
    for run in runs:
        target = out / 'original-runs' / run['board']
        target.mkdir(parents=True, exist_ok=True)
        for source in (cache / 'large-native' / run['board']).iterdir():
            if source.name == 'frame.rgb565':
                (target / (source.name + '.gz')).write_bytes(gzip.compress(source.read_bytes(), mtime=0))
            elif source.suffix in ['.txt', '.png', '.json', '.log']:
                shutil.copy2(source, target / source.name)
    aliases = []
    for listing in LISTINGS[:2]:
        fragment = read(ROOT / 'tools/waveshare-batches/root-s3-five' / (listing + '.json'))
        for firmware in fragment['firmwares']:
            products = [p for p in fragment['products'] if p['id'] in firmware['productIds']]
            board = products[0]['runtime']['board']
            run = next(r for r in runs if r['board'] == board)
            assert run['firmwareSha256'] == firmware['source']['sha256']
            aliases.append(dict(board=board, productIds=firmware['productIds'],
                                pinnedSource=firmware['source'], executedOriginal=run['source'],
                                bytes=run['firmwareBytes'], byteIdentityVerified=True,
                                purpose='Pinned README describes an integrated LVGL example; FactoryOnly in the filename alone does not establish preinstalled factory purpose.'))
    save(out / 'pinned-original-byte-identity.json', aliases)
    models = []
    source_checks = []
    for model_id in MODELS:
        asset = ROOT / 'web/platform/assets' / model_id
        loader = read(cache / 'large-review' / model_id / 'loader.json')['results'][0]
        built = read(asset / 'build-check.json')
        model_hash = digest(asset / 'model.json')
        assert loader['modelSha256'] == built['modelSha256'] == model_hash
        for source in read(asset / 'source-manifest.json')['sources']:
            path = asset / source['file']
            content = path.read_bytes()
            retained = source.get('retained')
            if retained:
                assert len(content) == retained['bytes'] and digest(path) == retained['sha256']
                if retained.get('encoding') == 'gzip':
                    content = gzip.decompress(content)
            assert len(content) == source['bytes'] and hashlib.sha256(content).hexdigest() == source['sha256']
            source_checks.append(dict(model=model_id, sourceId=source['id'], bytes=len(content), sha256=source['sha256'], verified=True))
        shutil.copytree(cache / 'large-review' / model_id, out / 'models' / model_id, dirs_exist_ok=True)
        expanded = cache / 'large-expanded' / model_id
        if expanded.exists():
            assert read(expanded / 'loader.json')['results'][0]['modelSha256'] == model_hash
            shutil.copytree(expanded, out / 'expanded' / model_id, dirs_exist_ok=True)
        models.append(dict(id=model_id, modelSha256=model_hash, geometry=built['geometry'],
                           productionLoaderVerified=True, viewsAndOverlayReviewed=True,
                           expandedViewsReviewed=expanded.exists(), browserWebglVerified=False,
                           physicalFitVerified=False))
    for listing in LISTINGS:
        folder = ROOT / 'docs/platform/research/waveshare-s3-large-20261124' / listing
        for source in read(folder / 'source-manifest.json')['sources']:
            if 'retained' not in source:
                continue
            path = folder / source['retained']
            assert path.stat().st_size == source['retainedBytes'] and digest(path) == source['retainedSha256']
            content = gzip.decompress(path.read_bytes())
            receipt = source['receipt']
            assert len(content) == receipt['bytes'] and hashlib.sha256(content).hexdigest() == receipt['sha256']
            source_checks.append(dict(listing=listing, sourceId=source['id'], bytes=len(content), sha256=receipt['sha256'], verified=True))
    save(out / 'source-hash-checks.json', dict(checkedAt=stamp, sources=source_checks))
    prior = read(cache / 'optical-validation-metadata-refresh.json')
    save(out / 'model-metadata-history.json', dict(previousStage=prior, finalModels=models,
        finalChange='After recording the optical-window evidence, control verification normals were set to the sourced rear PCB view. Geometry and presentation were not changed; model-plan/model/build-check hashes were refreshed, then all seven production loaders and review renders were rerun.'))
    source_paths = ['esp32s3/src/board/waveshare_ch422_rgb.rs', 'esp32s3/tests/waveshare_large_rgb.rs',
                    'esp32s3/examples/waveshare_source_attempt.rs', 'esp-soc/src/devices/waveshare_ch422g.rs',
                    'esp-soc/src/devices/pcf85063.rs', 'esp32s3/src/periph.rs', 'esp-periph/src/i2c/gt911.rs']
    save(out / 'acceptance.json', dict(checkedAt=stamp, listings=LISTINGS, saleSkus=8, profiles=8,
        models=models, runtimeSourceHashes=[dict(path=p, sha256=digest(ROOT / p)) for p in source_paths],
        lx7IntegrationTests=4, priorTailLx7RegressionTests=5, originalBoundedRuns=5,
        originalRunScope='CH422G five selectors only; Touch4 V4 original runs and runtime tests have independent evidence.',
        hardwareValidated=False, browserWebglValidated=False, fullApplicationValidated=False,
        firmwarePolicy='All catalog images remain trial and remote-links-only. No third-party BIN copied into this evidence.'))
    save(out / 'retained-files.json', dict(checkedAt=stamp, files=[dict(path=str(p.relative_to(out)), bytes=p.stat().st_size, sha256=digest(p))
        for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'retained-files.json']))
    print(json.dumps(dict(models=len(models), sourceHashes=len(source_checks), originalRuns=len(runs), output=str(out))))


if __name__ == '__main__':
    main()
