#!/usr/bin/env python3
"""Fetch a caller-selected official image into RAM, verify it, run the real S31 ROM.

No firmware file is created. The diagnostic output contains public source receipts,
image layout and guest output, never firmware bytes. This is not a product launcher.
"""
import argparse
import datetime
import hashlib
import json
import pathlib
import struct
import subprocess
import urllib.request


def image(data, offset):
    if data[offset] != 0xE9:
        raise ValueError(f"no ESP image at {offset:#x}")
    count = data[offset + 1]
    if not 1 <= count <= 16:
        raise ValueError("invalid segment count")
    pos = offset + 24
    checksum = 0xEF
    segments = []
    for _ in range(count):
        address, size = struct.unpack_from("<II", data, pos)
        pos += 8
        if pos + size > len(data):
            raise ValueError("image segment exceeds input")
        segments.append({"address": f"0x{address:08x}", "bytes": size})
        for byte in data[pos:pos + size]:
            checksum ^= byte
        pos += size
    checksum_pos = pos | 15
    end = checksum_pos + 1
    checksum_ok = data[checksum_pos] == checksum
    digest_ok = None
    if data[offset + 23]:
        digest_ok = hashlib.sha256(data[offset:end]).digest() == data[end:end + 32]
    if not checksum_ok or digest_ok is False:
        raise ValueError("ESP image checksum/digest failed")
    return {"offset": hex(offset), "entry": hex(struct.unpack_from("<I", data, offset + 4)[0]),
            "chipId": struct.unpack_from("<H", data, offset + 12)[0], "segments": segments,
            "checksumValid": checksum_ok, "appendedDigestValid": digest_ok}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--firmware-url", required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--rom", required=True)
    parser.add_argument("--runner", default="target/release/examples/bringup")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    with urllib.request.urlopen(args.firmware_url, timeout=60) as response:
        data = response.read(256 * 1024 * 1024 + 1)
        if len(data) > 256 * 1024 * 1024:
            raise ValueError("firmware exceeds 256 MiB")
        receipt = {"url": args.firmware_url, "finalUrl": response.url, "status": response.status,
                   "fetchedAt": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                   "mediaType": response.headers.get_content_type(), "bytes": len(data),
                   "sha256": hashlib.sha256(data).hexdigest()}
    if receipt["sha256"] != args.sha256:
        raise ValueError("firmware SHA-256 does not match caller's expected value")
    partitions = []
    for pos in range(0x8000, 0x9000, 32):
        if data[pos:pos + 2] != b"\xaa\x50":
            break
        _, kind, subtype, offset, size, label, flags = struct.unpack_from("<HBBII16sI", data, pos)
        partitions.append({"type": kind, "subtype": subtype, "offset": hex(offset), "bytes": size,
                           "label": label.split(b"\0")[0].decode("ascii"), "flags": flags})
    images = [image(data, 0x2000)]
    images.extend(image(data, int(p["offset"], 16)) for p in partitions
                  if p["type"] == 0 and data[int(p["offset"], 16)] == 0xE9)
    if not partitions or any(x["chipId"] != 32 for x in images):
        raise ValueError("input does not contain the expected S31 merged image layout")
    command = [args.runner, "--rom", "--usb-host", "--strap=0x12", "--flash=/dev/stdin", args.rom, "", "200000"]
    result = subprocess.run(command, input=data, capture_output=True, timeout=120, check=True)
    report = {"source": receipt, "romSha256": hashlib.sha256(pathlib.Path(args.rom).read_bytes()).hexdigest(),
              "partitions": partitions, "images": images,
              "developmentInputs": {"strap": "0x12", "usbHost": True, "norIdentity": None, "otpImage": None},
              "uart": result.stdout.decode("utf-8", errors="replace"),
              "diagnostic": result.stderr.decode("utf-8", errors="replace"),
              "storage": "firmware bytes only in host/guest RAM and subprocess pipe; not written to disk"}
    pathlib.Path(args.output).write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(report["diagnostic"])


if __name__ == "__main__":
    main()
