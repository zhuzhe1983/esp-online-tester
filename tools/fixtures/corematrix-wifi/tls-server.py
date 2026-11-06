#!/usr/bin/env python3
"""Run an OpenSSL TLS fixture with a finite, byte-checkable HTTP response.

Only caller-supplied test certificates/keys and bind addresses are accepted.
The temporary HTTP file is removed when the child server exits (Ctrl-C to stop).
"""
import argparse
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bind", required=True, help="Host IPv4 address reachable by guest NAT")
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--cert", required=True, type=Path)
    parser.add_argument("--key", required=True, type=Path)
    parser.add_argument("--openssl", default="openssl", help="OpenSSL executable (qualified with 3.6.4)")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="corematrix-tls-") as directory:
        response = (b"HTTP/1.0 200 OK\r\nContent-Length: 4096\r\n"
                    b"Connection: close\r\n\r\n" + bytes(range(256)) * 16)
        (Path(directory) / "c61-wifi").write_bytes(response)
        command = [args.openssl, "s_server", "-accept", f"{args.bind}:{args.port}",
                   "-cert", str(args.cert.resolve()), "-key", str(args.key.resolve()),
                   "-HTTP", "-http_server_binmode", "-tls1_2", "-groups", "P-256",
                   "-cipher", "ECDHE-ECDSA-AES128-GCM-SHA256", "-state"]
        return subprocess.call(command, cwd=directory)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
