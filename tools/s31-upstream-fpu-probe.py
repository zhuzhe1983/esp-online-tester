#!/usr/bin/env python3
"""Observe the fixed Plus2.0.68 lazy-FPU handler in a caller-supplied reference.

No image/register edits. Temporary debug breakpoints are removed before stepping;
raw UART/reference logs stay in the caller's ignored output directory. The JSON
retains only relevant architectural observations, not full register dumps.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import time
import urllib.request


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--firmware-url', required=True)
    ap.add_argument('--sha256', required=True)
    ap.add_argument('--runtime', type=Path, required=True)
    ap.add_argument('--runtime-sha256', required=True)
    ap.add_argument('--output', type=Path, required=True)
    ap.add_argument('--port', type=int, default=23331)
    args = ap.parse_args()
    runtime = args.runtime.resolve()
    if hashlib.sha256(runtime.read_bytes()).hexdigest() != args.runtime_sha256:
        raise ValueError('reference runtime SHA-256 mismatch')
    with urllib.request.urlopen(args.firmware_url, timeout=45) as response:
        data = response.read(32 * 1024 * 1024 + 1)
        if len(data) > 32 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != args.sha256:
            raise ValueError('firmware size or SHA-256 mismatch')
        source = dict(url=args.firmware_url, finalUrl=response.url,
                      fetchedAt=datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
                      mediaType=response.headers.get_content_type(), status=response.status,
                      bytes=len(data), sha256=args.sha256)
    args.output.mkdir(parents=True, exist_ok=True)
    receipt = dict(source=source, runtimeSha256=args.runtime_sha256,
                   scope='Fixed Plus2.0.68 exception path; upstream emulator behavior, not silicon.',
                   firmwareStorage='RAM and stdin pipe only', observations=[])
    process = None
    connection = None
    try:
        with (args.output / 'stdout.log').open('wb') as stdout, (args.output / 'stderr.log').open('wb') as stderr:
            process = subprocess.Popen([str(runtime), '--chip', 'esp32s31', '--firmware', '/dev/stdin',
                                        '--gdb', str(args.port), '--gdb-halt', '--net', 'user,restrict=yes',
                                        '--timeout', '40s', '--log-color', 'never'],
                                       stdin=subprocess.PIPE, stdout=stdout, stderr=stderr)
            process.stdin.write(data)
            process.stdin.close()
            for _ in range(100):
                try:
                    connection = socket.create_connection(('127.0.0.1', args.port), timeout=.5)
                    break
                except OSError:
                    if process.poll() is not None:
                        raise RuntimeError('reference exited before debugger')
                    time.sleep(.05)
            if connection is None:
                raise RuntimeError('reference debugger not ready')
            connection.settimeout(30)

            def receive_exact(count):
                result = b''
                while len(result) < count:
                    part = connection.recv(count - len(result))
                    if not part:
                        raise EOFError('debugger closed')
                    result += part
                return result

            def command(request):
                payload = request.encode()
                connection.sendall(b'$' + payload + b'#' + f'{sum(payload) % 256:02x}'.encode())
                while receive_exact(1) != b'$':
                    pass
                body = b''
                while True:
                    char = receive_exact(1)
                    if char == b'#':
                        break
                    body += char
                    if len(body) > 65536:
                        raise ValueError('oversized debugger packet')
                if int(receive_exact(2), 16) != sum(body) % 256:
                    raise ValueError('debugger checksum mismatch')
                connection.sendall(b'+')
                return body.decode()

            def registers():
                raw = bytes.fromhex(command('g'))
                return [int.from_bytes(raw[i:i + 4], 'little') for i in range(0, len(raw), 4)]

            def check_pc(expected, label):
                regs = registers()
                if len(regs) < 33 or regs[32] != expected:
                    raise ValueError(f'{label}: guest did not reach expected PC')
                receipt['observations'].append(dict(event=label, pc=hex(regs[32])))
                return regs

            def breakpoint(address, enable, width):
                if command(f'{"Z" if enable else "z"}0,{address:x},{width}') != 'OK':
                    raise ValueError('reference rejected temporary breakpoint')

            command('qSupported:multiprocess+;qXfer:features:read+')
            xml = command('qXfer:features:read:target.xml:0,4000')
            if 'name="f15" bitsize="32" type="ieee_single" regnum="48"' not in xml:
                raise ValueError('reference FPU debug register mapping changed')
            command('?')
            breakpoint(0x4006ba84, True, 4)
            command('c')
            check_pc(0x4006ba84, 'before original FLW')
            breakpoint(0x4006ba84, False, 4)
            if command('m4006ba84,4') != '87a7c753':
                raise ValueError('fixed firmware FLW signature mismatch')
            # Actual guest csrrw a0,EXT_ILL,zero, not a guessed GDB CSR index.
            if command('m2f00017e,4') != '7315007f':
                raise ValueError('fixed firmware EXT_ILL read signature mismatch')
            command('s')
            check_pc(0x2f000000, 'actual architectural exception vector')
            breakpoint(0x2f000182, True, 2)
            command('c')
            regs = check_pc(0x2f000182, 'after actual guest EXT_ILL read')
            receipt['extIllReadByGuest'] = regs[10]
            if regs[10] != 1:
                raise ValueError('reference guest did not read EXT_ILL FPU reason')
            breakpoint(0x2f000182, False, 2)
            breakpoint(0x4006ba84, True, 4)
            command('c')
            check_pc(0x4006ba84, 'guest handler returned to original FLW')
            breakpoint(0x4006ba84, False, 4)
            if command('m4006ba84,4') != '87a7c753':
                raise ValueError('original instruction not preserved')
            command('s')
            check_pc(0x4006ba88, 'original FLW retired')
            receipt['loadedF15'] = hex(int.from_bytes(bytes.fromhex(command('p30')), 'little'))
            if receipt['loadedF15'] != '0x3f800000':
                raise ValueError('unexpected actual FLW result')
            command('D')
            receipt['passed'] = True
    except Exception as error:
        receipt['error'] = str(error)
        raise
    finally:
        if connection is not None:
            connection.close()
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        receipt['exitCode'] = process.returncode if process else None
        for name in ['stdout.log', 'stderr.log']:
            path = args.output / name
            if path.exists():
                receipt[name + 'Sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        (args.output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')


if __name__ == '__main__':
    main()
