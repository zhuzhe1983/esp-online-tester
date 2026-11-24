"""Retain exact V4 runtime sources; audit and run the two original factory images.

Caller supplies all local inputs. Firmware stays remote-only. This is bounded
native ROM evidence, not factory acceptance, physical testing or a WASM claim.
"""
from pathlib import Path
import argparse, datetime, gzip, hashlib, importlib.util, json, struct, subprocess, time, zipfile

def digest(data): return hashlib.sha256(data).hexdigest()
def sha(path): return digest(path.read_bytes())
def write_json(path, value): path.write_text(json.dumps(value, indent=2) + '\n')

ap = argparse.ArgumentParser(description=__doc__)
for field in ['source-cache', 'runtime-cache', 'research', 'evidence', 'runner', 'rom', 'sd']:
    ap.add_argument('--' + field, type=Path, required=True)
ap.add_argument('--guest-ms', type=int, default=5000)
a = ap.parse_args()
assert 0 < a.guest_ms <= 5000
a.research.mkdir(parents=True, exist_ok=True); a.evidence.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('audit', root / 'tools/audit-factory-firmware-sources.py')
audit = importlib.util.module_from_spec(spec); spec.loader.exec_module(audit)
sources = []

def retain(path, label):
    receipt = json.loads(path.with_name(path.name + '.receipt.json').read_text())
    raw = path.read_bytes(); assert len(raw) == receipt['bytes'] and digest(raw) == receipt['sha256']
    out = a.research / (label + '.gz'); out.write_bytes(gzip.compress(raw, mtime=0))
    receipt.pop('containerFile', None)
    receipt.update(retainedFile=out.name, retainedSha256=sha(out), retainedBytes=out.stat().st_size,
                   captureKind='original-source-bytes', label=label)
    sources.append(receipt)

def retain_member(archive, member, label):
    receipt = json.loads(archive.with_name(archive.name + '.receipt.json').read_text())
    assert sha(archive) == receipt['sha256']
    with zipfile.ZipFile(archive) as z: raw = z.read(member)
    receipt.update(archiveMember=member, archiveFormat='zip', containerSha256=receipt['sha256'],
                   containerBytes=receipt['bytes'], bytes=len(raw), sha256=digest(raw))
    out = a.research / (label + '.gz'); out.write_bytes(gzip.compress(raw, mtime=0))
    receipt.update(retainedFile=out.name, retainedSha256=sha(out), retainedBytes=out.stat().st_size,
                   captureKind='original-source-bytes', label=label)
    sources.append(receipt)

selected = {
    'hardware__schematics__ESP32-S3-Touch-LCD-4 V4.0.pdf': 'v4-schematic.pdf',
    'hardware__HARDWARE_REFERENCE.md': 'v4-hardware-reference.md',
    'examples__arduino__libraries__WS_CH32_IO__src__WS_CH32_IO.cpp': 'arduino-ch32.cpp',
    'examples__arduino__libraries__WS_CH32_IO__src__WS_CH32_IO.h': 'arduino-ch32.h',
    'examples__arduino__09_LVGL_Widgets__09_LVGL_Widgets.ino': 'arduino-lvgl.ino',
    'examples__esp-idf__02_SD_Test__main__sd_card_example_main.c': 'idf-sd-example.c',
    'examples__esp-idf__06_lvgl_demo_v8__main__main.c': 'idf-lvgl-main.c',
    'examples__esp-idf__06_lvgl_demo_v8__main__idf_component.yml': 'idf-lvgl-component.yml',
    'firmware__README.md': 'factory-readme.md',
    'firmware__manifest.json': 'factory-manifest.json',
    'firmware__artifacts.json': 'factory-artifacts.json',
}
for name, label in selected.items(): retain(a.source_cache / name, label)
retain(a.runtime_cache / 'README.md', 'ioexpander-readme.md')
bsp = a.runtime_cache / 'members/cfab2ac9-0b63-4778-a6f4-6a2d557ed716/esp32_s3_touch_lcd_4'
for name, label in [('esp32_s3_touch_lcd_4.c', 'bsp3.c'),
                    ('include/bsp/esp32_s3_touch_lcd_4.h', 'bsp3.h'),
                    ('include/bsp/display.h', 'bsp3-display.h'), ('include/bsp/touch.h', 'bsp3-touch.h')]:
    retain(bsp / name, label)
helper = a.runtime_cache / 'members/017b8d7c-18a9-4945-b5dd-4d5da52b43b4/custom_io_expander_ch32v003'
for name, label in [('custom_io_expander_ch32v003.c', 'ch32-driver1.0.1.c'),
                    ('include/custom_io_expander_ch32v003.h', 'ch32-driver1.0.1.h')]:
    retain(helper / name, label)
for folder, version in [('headers', '5.4.1'), ('headers551', '5.5.1')]:
    for name in ['gpio_sig_map.h', 'spi_ll.h', 'lcd_cam_reg.h']:
        retain(a.runtime_cache / folder / name, 'idf' + version + '-' + name)
for archive, member, label in [
    ('cfab2ac9-0b63-4778-a6f4-6a2d557ed716.zip','esp32_s3_touch_lcd_4/LICENSE','bsp3-LICENSE'),
    ('cfab2ac9-0b63-4778-a6f4-6a2d557ed716.zip','esp32_s3_touch_lcd_4/idf_component.yml','bsp3-component.yml'),
    ('017b8d7c-18a9-4945-b5dd-4d5da52b43b4.zip','custom_io_expander_ch32v003/license.txt','ch32-driver1.0.1-license.txt'),
    ('017b8d7c-18a9-4945-b5dd-4d5da52b43b4.zip','custom_io_expander_ch32v003/idf_component.yml','ch32-driver1.0.1-component.yml')]:
    retain_member(a.runtime_cache / archive, member, label)
write_json(a.research / 'manifest.json', {
    'schemaVersion': '1', 'recordedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'scope': 'Exact V4 runtime sources. Original selected source bytes retained gzip with HTTP and ZIP-member receipts; firmware binaries remote-only.',
    'sources': sources,
})

artifacts = {x['path']: x for x in json.loads((a.source_cache / 'firmware__artifacts.json').read_text())['artifacts']}
images = []
for board, name in [('waveshare-s3-touch-lcd4-v4', 'ESP32-S3-Touch-LCD-4_FactoryOnly_20251122.bin'),
                    ('waveshare-s3-lcd4-v4', 'ESP32-S3-LCD-4_FactoryOnly_20251218.bin')]:
    p = a.source_cache / ('firmware__' + name); b = p.read_bytes()
    r = json.loads(p.with_name(p.name + '.receipt.json').read_text()); official = artifacts['firmware/' + name]
    assert len(b) == r['bytes'] == official['size'] and digest(b) == r['sha256'] == official['sha256']
    assert audit.image(b, 0) == 9; assert len(b) <= 16 << 20
    parts = []; md5 = False
    for at in range(0x8000, 0x9000, 32):
        magic = struct.unpack_from('<H', b, at)[0]
        if magic == 0x50aa:
            _, kind, sub, off, size, label, flags = struct.unpack_from('<HBBII16sI', b, at)
            part = {'type': kind, 'subtype': sub, 'offset': off, 'bytes': size,
                    'label': label.split(b'\0')[0].decode(), 'flags': flags}
            assert off + size <= 16 << 20
            if kind == 0 and off < len(b) and b[off] == 0xe9:
                assert audit.image(b, off) == 9
                part.update(imageChipId=9, imageChecksumsValid=True)
                desc = b.find(b'\x32\x54\xcd\xab', off, min(off + 4096, len(b)))
                if desc >= 0:
                    part['appDescription'] = {key: b[desc + start:desc + end].split(b'\0')[0].decode(errors='replace')
                                             for key, start, end in [('version',16,48),('project',48,80),('time',80,96),('date',96,112),('idf',112,144)]}
            parts.append(part)
        elif magic == 0xebeb:
            assert b[at + 16:at + 32] == hashlib.md5(b[0x8000:at]).digest(); md5 = True; break
        elif magic == 0xffff: break
        else: raise ValueError((board, hex(magic)))
    assert md5 and any(x.get('imageChipId') == 9 for x in parts)
    r.update(board=board, role='factory', filename=name, result='source-layout-checksums-pass',
             executionVerified=False, roleBasis='Pinned original firmware manifest and README distinguish exact touch and non-touch factory recovery images.',
             layout={'loadOffset':0, 'chipId':9, 'bootloaderChecksumAndDigestValid':True,
                     'partitionTableOffset':0x8000, 'partitionTableMd5Valid':md5, 'partitions':parts,
                     'fitsFlashMiB':16, 'headerFlashSizeCode':b[3] >> 4})
    images.append(r)
write_json(a.evidence / 'original-image-verification.json', images)
write_json(a.evidence / 'native-run-inputs.json', {
    'binarySha256':sha(a.runner), 'romSha256':sha(a.rom), 'initialSdSha256':sha(a.sd), 'initialSdBytes':a.sd.stat().st_size,
    'runtimeSourceSha256':sha(root / 'esp32s3/src/board/waveshare_touch4_v4.rs'),
    'guestTestSourceSha256':sha(root / 'esp32s3/tests/waveshare_touch4_v4_guest.rs'),
    'scope':'Unmodified exact-model original factory images; native ROM with 16 MiB Flash, 8 MiB PSRAM and prepared empty 64 MiB FAT32. No full factory/physical/browser acceptance.',
})
runs = []
for r in images:
    board = r['board']; out = a.evidence / board; out.mkdir(exist_ok=True)
    fw = a.source_cache / ('firmware__' + r['filename'])
    cmd = [str(a.runner.resolve()), board, str(a.rom.resolve()), str(fw.resolve()), '16', '8',
           str(a.guest_ms), str(out.resolve()), str(a.sd.resolve())]
    start = time.monotonic(); p = subprocess.run(cmd, capture_output=True, text=True, timeout=240)
    row = {'board':board, 'role':'factory', 'firmwareSha256':r['sha256'], 'flashMiB':16, 'psramMiB':8,
           'guestBoundMs':a.guest_ms, 'defaultFat32MiB':64, 'exitCode':p.returncode,
           'hostSeconds':round(time.monotonic() - start, 3), 'stdout':p.stdout, 'stderr':p.stderr,
           'state':json.loads((out / 'state.json').read_text()) if (out / 'state.json').exists() else None}
    uart = out / 'uart.txt'
    if uart.exists():
        raw = uart.read_bytes(); (out / 'uart.original.gz').write_bytes(gzip.compress(raw, mtime=0))
        row['uartNormalization'] = {'originalBytes':len(raw), 'originalSha256':digest(raw),
                                    'rule':'Original retained gzip; text view only normalizes CRLF/CR and trailing whitespace.'}
        uart.write_bytes(b'\n'.join(x.rstrip() for x in raw.replace(b'\r\n',b'\n').replace(b'\r',b'').split(b'\n')).rstrip() + b'\n')
    frame = out / 'frame.rgb565'
    if frame.exists():
        row['rawFrame'] = {'bytes':frame.stat().st_size, 'sha256':sha(frame), 'retainedAs':'frame.png'}
        frame.unlink()
    row['artifacts'] = {f.name:{'bytes':f.stat().st_size, 'sha256':sha(f)} for f in sorted(out.iterdir()) if f.is_file()}
    runs.append(row); write_json(a.evidence / 'native-runs.json', runs); print(board, row['state'], flush=True)
print('retained source checks:', len(sources), flush=True)
