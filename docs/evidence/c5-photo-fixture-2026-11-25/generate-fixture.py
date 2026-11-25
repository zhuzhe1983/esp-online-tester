#!/usr/bin/env python3
"""Generate one deterministic host JPEG/FAT32 fixture; no guest or mounting."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import sys
import PIL
from PIL import Image, features

SOURCE_SHA = "a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6"
DATE = ((2026 - 1980) << 9) | (11 << 5) | 25


def rec(data):
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def u16(data, at): return struct.unpack_from("<H", data, at)[0]
def u32(data, at): return struct.unpack_from("<I", data, at)[0]


def exclusive(path, data):
    with path.open("xb") as out: out.write(data)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source", type=Path, required=True)
    p.add_argument("--output-directory", type=Path, required=True)
    args = p.parse_args()
    source, output = args.source.absolute(), args.output_directory.absolute()
    assert not os.path.lexists(output), "existing output refused"
    assert not output.is_symlink() and all(not x.is_symlink() for x in output.parents), "symlink output refused"
    original = source.read_bytes()
    assert rec(original) == {"bytes": 67108864, "sha256": SOURCE_SHA}
    assert original[510:512] == b"\x55\xaa"
    part = original[446:462]
    assert part[4] == 0x0c
    start, sectors = u32(part, 8), u32(part, 12)
    base = start * 512
    boot = original[base:base + 512]
    bps, spc, reserved, fats, fat_sectors = u16(boot, 11), boot[13], u16(boot, 14), boot[16], u32(boot, 36)
    root, info, backup = u32(boot, 44), u16(boot, 48), u16(boot, 50)
    assert bps == 512 and spc == 1 and fats == 2 and root == 2
    assert u32(boot, 32) == sectors and u16(boot, 42) == 0 and u16(boot, 40) == 0
    first_data_sector = start + reserved + fats * fat_sectors
    clusters = (sectors - reserved - fats * fat_sectors) // spc
    fat_start = base + reserved * bps
    fat = original[fat_start:fat_start + fat_sectors * bps]
    assert fat == original[fat_start + fat_sectors * bps:fat_start + 2 * fat_sectors * bps]
    def value(c): return u32(fat, c * 4) & 0x0fffffff
    assert value(root) >= 0x0ffffff8
    def cluster_offset(c): return (first_data_sector + (c - 2) * spc) * bps
    root_offset = cluster_offset(root)
    root_data = original[root_offset:root_offset + bps * spc]
    active = [(i, root_data[i:i + 32]) for i in range(0, len(root_data), 32) if root_data[i] not in (0, 0xe5)]
    assert len(active) == 1 and active[0][0] == 0 and active[0][1][11] == 8, "nonempty root refused"
    assert root_data[32:64] == b"\0" * 32
    free = [c for c in range(2, clusters + 2) if value(c) == 0]
    assert len(free) == 127005
    palette = [(224, 48, 48), (48, 208, 80), (48, 96, 224)]
    rgb = bytes(v for y in range(320) for x in range(320) for v in palette[((x // 32) + (y // 32)) % 3])
    image = Image.frombytes("RGB", (320, 320), rgb)
    def encode():
        buffer = io.BytesIO()
        image.save(buffer, format="JPEG", quality=90, subsampling=0, optimize=False,
                   progressive=False, exif=b"", icc_profile=None)
        return buffer.getvalue()
    jpeg = encode()
    assert jpeg == encode(), "encoder repeat was not byte-identical"
    assert len(jpeg) <= 128 * 1024
    decoded = Image.open(io.BytesIO(jpeg))
    decoded.load()
    assert decoded.size == (320, 320) and not decoded.getexif() and "icc_profile" not in decoded.info
    rgba = decoded.convert("RGBA").tobytes()
    count = (len(jpeg) + bps * spc - 1) // (bps * spc)
    allocated = free[:count + 1]
    directory, file_chain = allocated[0], allocated[1:]
    assert allocated == list(range(3, 3 + count + 1))
    assert all(original[cluster_offset(c):cluster_offset(c) + bps * spc] == b"\0" * (bps * spc) for c in allocated)
    changes = []
    def add(at, data, kind):
        assert 0 <= at < len(original) and at + len(data) <= len(original)
        assert all(at + len(data) <= q[0] or q[0] + len(q[1]) <= at for q in changes)
        changes.append((at, data, kind))
    new_fat = {directory: 0x0fffffff}
    for i, c in enumerate(file_chain): new_fat[c] = file_chain[i + 1] if i + 1 < len(file_chain) else 0x0fffffff
    for c, next_cluster in sorted(new_fat.items()):
        entry = (u32(fat, c * 4) & 0xf0000000) | next_cluster
        for index in range(fats): add(fat_start + index * fat_sectors * bps + c * 4, struct.pack("<I", entry), "FAT allocation")
    next_free = free[count + 1]
    for sector in [info, backup + info]:
        at = base + sector * bps
        block = original[at:at + bps]
        assert (u32(block, 0), u32(block, 484), u32(block, 508)) == (0x41615252, 0x61417272, 0xaa550000)
        assert u32(block, 488) == len(free) and u32(block, 492) == 3
        add(at + 488, struct.pack("<II", len(free) - len(allocated), next_free), "FSInfo counters")
    def entry(name, attr, ntcase, cluster, size):
        assert len(name) == 11
        row = bytearray(32)
        row[:11], row[11], row[12] = name, attr, ntcase
        struct.pack_into("<H", row, 16, DATE)
        struct.pack_into("<H", row, 18, DATE)
        struct.pack_into("<H", row, 20, cluster >> 16)
        struct.pack_into("<H", row, 24, DATE)
        struct.pack_into("<H", row, 26, cluster & 0xffff)
        struct.pack_into("<I", row, 28, size)
        return bytes(row)
    photo_entry = entry(b"PHOTO      ", 0x10, 0x08, directory, 0)
    add(root_offset + 32, photo_entry, "root /photo entry")
    directory_data = (entry(b".          ", 0x10, 0, directory, 0)
                      + entry(b"..         ", 0x10, 0, 0, 0)
                      + entry(b"TEST    JPG", 0x20, 0x18, file_chain[0], len(jpeg)))
    directory_data += b"\0" * (bps * spc - len(directory_data))
    add(cluster_offset(directory), directory_data, "photo directory cluster")
    for i, c in enumerate(file_chain):
        data = jpeg[i * bps * spc:(i + 1) * bps * spc]
        add(cluster_offset(c), data, "JPEG payload")
    output.mkdir(exist_ok=False)
    exclusive(output / "test.jpg", jpeg)
    exclusive(output / "test-decoded.rgba", rgba)
    decoded.save(output / "test-decoded.png", format="PNG", optimize=False, compress_level=9)
    card = output / "lcd273-photo-fixture.img"
    exclusive(card, original)
    with card.open("r+b") as target:
        for at, data, _ in sorted(changes):
            target.seek(at)
            assert target.read(len(data)) == original[at:at + len(data)]
            target.seek(at)
            target.write(data)
    final = card.read_bytes()
    assert rec(source.read_bytes()) == rec(original)
    receipt = {"kind": "host-only deterministic JPEG/FAT32 fixture; guest unexecuted", "source": rec(original),
               "source_after": rec(source.read_bytes()), "card": rec(final), "jpeg": rec(jpeg), "decoded_rgba": rec(rgba),
               "decoded_png": rec((output / "test-decoded.png").read_bytes()),
               "generator": rec(Path(__file__).read_bytes()), "python": sys.version.split()[0], "pillow": PIL.__version__,
               "jpeg_codec": features.version_codec("jpg"), "libjpeg_turbo": features.version_feature("libjpeg_turbo"),
               "image": {"width": 320, "height": 320, "source_mode": "RGB", "decoded_mode": "RGBA", "row_stride": 1280,
                         "source_rgb": rec(rgb), "tile_pixels": 32, "palette": palette, "rule": "palette[((x//32)+(y//32))%3]",
                         "jpeg_quality": 90, "subsampling": "4:4:4", "progressive": False, "optimize": False,
                         "exif": False, "icc_profile": False, "repeat_encode_identical": True},
               "geometry": {"partition_start_lba": start, "partition_sectors": sectors, "bytes_per_sector": bps,
                            "sectors_per_cluster": spc, "reserved_sectors": reserved, "fat_count": fats, "fat_sectors": fat_sectors,
                            "root_cluster": root, "cluster_count": clusters, "first_data_lba": first_data_sector},
               "directory": {"path": "/photo/test.jpg", "photo_cluster": directory, "file_clusters": file_chain,
                             "root_slot": 1, "file_slot": 2, "photo_ntcase": 8, "file_ntcase": 24, "lfn_entries": 0,
                             "dotdot_cluster": 0, "fixed_fat_date": "2026-11-25 00:00:00", "original_volume_label_preserved": True},
               "free_clusters_before": len(free), "free_clusters_after": len(free) - len(allocated), "next_free": next_free,
               "patches": [{"byte_offset": at, "bytes": len(data), "kind": kind, "new_sha256": rec(data)["sha256"]}
                           for at, data, kind in sorted(changes)], "guest_executed": False}
    exclusive(output / "generation.json", (json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode())
    print(json.dumps({"card": receipt["card"], "jpeg": receipt["jpeg"], "decoded_rgba": receipt["decoded_rgba"],
                      "directory": receipt["directory"]}, indent=2))


if __name__ == "__main__": main()
