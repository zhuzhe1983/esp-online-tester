"""Run exact original images for a bounded interval; never manufacture acceptance."""
from pathlib import Path
import argparse, gzip, hashlib, json, subprocess, time

FIRST = [
    ('waveshare-s3-touch-lcd28-v1', 'touch28-v1', 8, True),
    ('waveshare-s3-touch-lcd28-v2', 'touch28-v2', 8, True),
    ('waveshare-s3-lcd28', 'lcd28', 8, True),
    ('waveshare-s3-touch-lcd169', 'touch169', 8, False),
    ('waveshare-s3-touch-lcd7', 'touch7', 8, True),
    ('waveshare-s3-lcd7', 'lcd7', 8, True),
    ('waveshare-s3-touch-lcd7-external', 'touch7', 8, True),
    ('waveshare-s3-touch-lcd128', 'touch128', 2, False),
]

SECOND = [("waveshare-s3-lcd128", "lcd128", 2, False), ("waveshare-s3-touch-lcd128", "touch128", 2, False), ("waveshare-s3-geek", "geek", 2, True), ("waveshare-s3-touch-lcd43", "touch43", 8, True), ("waveshare-s3-lcd43", "lcd43", 8, True)]

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('sources', 'binary', 'rom', 'sd', 'evidence'):
        ap.add_argument('--'+name, type=Path, required=True)
    ap.add_argument('--group', choices=['first', 'second'], default='first')
    ap.add_argument('--guest-ms', type=int, default=5000)
    a = ap.parse_args()
    source = a.sources / 's3-tail-sources'
    manifest = source / 'factory-tail-verified.json'
    rows = {r['key']: r for r in json.loads(manifest.read_text())}
    a.evidence.mkdir(parents=True, exist_ok=True)
    provenance = {
        'binarySha256': digest(a.binary), 'romSha256': digest(a.rom),
        'firmwareAuditSha256': digest(manifest),
        'initialSdSha256': digest(a.sd), 'initialSdBytes': a.sd.stat().st_size,
        'scope': 'Native execution only. No hardware, WebGL, continuous operation or full application acceptance.',
    }
    (a.evidence / 'native-run-inputs.json').write_text(json.dumps(provenance, indent=2)+'\n')
    runs = []
    for board, key, psram, has_sd in (FIRST if a.group == "first" else SECOND):
        r = rows[key]
        firmware = source / r['repo'] / 'files' / r['path']
        assert digest(firmware) == r['sha256'], firmware
        out = a.evidence / board
        out.mkdir(exist_ok=True)
        for name in ('state.json', 'uart.txt', 'board.txt', 'frame.png', 'frame.rgb565', 'uart.original.gz'):
            (out / name).unlink(missing_ok=True)
        cmd = [str(a.binary.resolve()), board, str(a.rom.resolve()),
               str(firmware.resolve()), '16', str(psram), str(a.guest_ms), str(out.resolve())]
        if has_sd:
            cmd.append(str(a.sd.resolve()))
        started = time.monotonic()
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=240)
        row = {'board': board, 'firmwareKey': key, 'firmwareSha256': r['sha256'],
               'flashMiB': 16, 'psramMiB': psram, 'guestBoundMs': a.guest_ms,
               'defaultFat32MiB': 64 if has_sd else None, 'exitCode': p.returncode,
               'hostSeconds': round(time.monotonic()-started, 3), 'stdout': p.stdout, 'stderr': p.stderr,
               'state': json.loads((out/'state.json').read_text()) if (out/'state.json').exists() else None}
        uart = out / 'uart.txt'
        if uart.exists():
            raw = uart.read_bytes()
            (out / 'uart.original.gz').write_bytes(gzip.compress(raw, mtime=0))
            row['uartNormalization'] = {'originalBytes': len(raw), 'originalSha256': hashlib.sha256(raw).hexdigest(),
                'rule': 'Original bytes retained as uart.original.gz; text view converts CRLF to LF, removes residual CR and strips line-end whitespace.'}
            uart.write_bytes(b'\n'.join(line.rstrip() for line in raw.replace(b'\r\n', b'\n').replace(b'\r', b'').split(b'\n')).rstrip()+b'\n')
        (out / 'frame.rgb565').unlink(missing_ok=True)
        row['artifacts'] = {f.name: {'bytes': f.stat().st_size, 'sha256': digest(f)}
                            for f in sorted(out.iterdir()) if f.is_file()}
        runs.append(row)
        (a.evidence / 'native-runs.json').write_text(json.dumps(runs, indent=2)+'\n')
        print(board, row['state'], flush=True)

if __name__ == '__main__':
    main()
