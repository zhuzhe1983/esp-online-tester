#!/usr/bin/env python3
"""Independent read-only FAT32 traversal/diff and host JPEG decode verification."""
import argparse
from collections import Counter
import hashlib
import io
import json
from pathlib import Path
import PIL
from PIL import Image

ORIGINAL_SHA = "a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6"


def digest(data): return hashlib.sha256(data).hexdigest()
def integer(data, at, size): return int.from_bytes(data[at:at + size], "little")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["source", "card", "jpeg", "rgba", "report"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    original, card = args.source.read_bytes(), args.card.read_bytes()
    assert digest(original) == ORIGINAL_SHA and len(original) == len(card) == 67108864
    assert args.source.resolve() != args.card.resolve()
    assert card[:512] == original[:512]
    first = integer(card, 454, 4)
    base = first * 512
    bpb = card[base:base + 512]
    assert bpb == original[base:base + 512] and bpb[510:] == b"\x55\xaa"
    sector_bytes = integer(bpb, 11, 2)
    sectors_per_cluster = bpb[13]
    reserved, copies = integer(bpb, 14, 2), bpb[16]
    fat_sectors, volume_sectors = integer(bpb, 36, 4), integer(bpb, 32, 4)
    root = integer(bpb, 44, 4)
    cluster_bytes = sector_bytes * sectors_per_cluster
    first_data = first + reserved + copies * fat_sectors
    count = (volume_sectors - reserved - copies * fat_sectors) // sectors_per_cluster
    assert sector_bytes == 512 and copies == 2 and root == 2 and count >= 65525
    fats = [card[base + (reserved + i * fat_sectors) * sector_bytes:base + (reserved + (i + 1) * fat_sectors) * sector_bytes] for i in range(copies)]
    assert fats[0] == fats[1]
    original_fat = original[base + reserved * sector_bytes:base + (reserved + fat_sectors) * sector_bytes]
    def fat(c, old=False): return integer(original_fat if old else fats[0], c * 4, 4) & 0x0fffffff
    def offset(c): return (first_data + (c - 2) * sectors_per_cluster) * sector_bytes
    def chain(start):
        out = []
        while True:
            assert 2 <= start < count + 2 and start not in out and len(out) <= count
            out.append(start)
            following = fat(start)
            if following >= 0x0ffffff8: return out
            assert following != 0x0ffffff7
            start = following
    def contents(chain_list): return b"".join(card[offset(c):offset(c) + cluster_bytes] for c in chain_list)
    def entries(data):
        rows = []
        for at in range(0, len(data), 32):
            row = data[at:at + 32]
            if row[0] == 0: break
            assert row[0] != 0xe5 and row[11] != 0x0f and all(v < 128 for v in row[:11])
            body = row[:8].decode("ascii").rstrip(" ")
            ext = row[8:11].decode("ascii").rstrip(" ")
            if row[12] & 8: body = body.lower()
            if row[12] & 16: ext = ext.lower()
            name = body + ("." + ext if ext else "")
            rows.append({"name": name, "raw_name": row[:11].decode("ascii"), "attr": row[11], "ntcase": row[12],
                         "cluster": (integer(row, 20, 2) << 16) | integer(row, 26, 2),
                         "bytes": integer(row, 28, 4), "slot": at // 32})
        return rows
    root_chain = chain(root)
    assert root_chain == [2] and fat(2) == fat(2, True)
    root_data = contents(root_chain)
    original_root = original[offset(root):offset(root) + cluster_bytes]
    old_root_rows, root_rows = entries(original_root), entries(root_data)
    assert len(old_root_rows) == 1 and old_root_rows[0]["attr"] == 8
    assert len(root_rows) == 2 and root_rows[0] == old_root_rows[0]
    assert root_data[:32] == original_root[:32] and root_data[64:] == original_root[64:]
    directory = root_rows[1]
    assert (directory["name"], directory["raw_name"], directory["attr"], directory["ntcase"], directory["bytes"]) == ("photo", "PHOTO      ", 16, 8, 0)
    directory_chain = chain(directory["cluster"])
    assert directory_chain == [3]
    photo_rows = entries(contents(directory_chain))
    assert len(photo_rows) == 3 and [r["name"] for r in photo_rows] == [".", "..", "test.jpg"]
    assert photo_rows[0]["cluster"] == 3 and photo_rows[1]["cluster"] == 0
    file = photo_rows[2]
    assert (file["attr"], file["ntcase"], file["raw_name"]) == (32, 24, "TEST    JPG")
    file_chain = chain(file["cluster"])
    assert file_chain == list(range(4, 15)) and len(file_chain) == (file["bytes"] + cluster_bytes - 1) // cluster_bytes
    all_file_data = contents(file_chain)
    jpeg = all_file_data[:file["bytes"]]
    assert jpeg == args.jpeg.read_bytes() and file["bytes"] == 5200
    assert all_file_data[file["bytes"]:] == b"\0" * (len(all_file_data) - file["bytes"])
    allocated = directory_chain + file_chain
    assert all(fat(c, True) == 0 for c in allocated)
    assert [c for c in range(2, count + 2) if fat(c, True) != 0] == [2]
    assert [c for c in range(2, count + 2) if fat(c) != 0] == [2] + allocated
    changed_fat = [c for c in range(2, count + 2) if fat(c) != fat(c, True)]
    assert changed_fat == allocated
    old_free = sum(fat(c, True) == 0 for c in range(2, count + 2))
    new_free = sum(fat(c) == 0 for c in range(2, count + 2))
    fsinfo_sectors = [integer(bpb, 48, 2), integer(bpb, 50, 2) + integer(bpb, 48, 2)]
    allowed = [(offset(root) + 32, offset(root) + 64)]
    for sector in fsinfo_sectors:
        at = base + sector * sector_bytes
        info = card[at:at + sector_bytes]
        assert (integer(info, 0, 4), integer(info, 484, 4), integer(info, 508, 4)) == (0x41615252, 0x61417272, 0xaa550000)
        assert integer(info, 488, 4) == new_free and integer(info, 492, 4) == 15
        assert info[:488] == original[at:at + 488] and info[496:] == original[at + 496:at + sector_bytes]
        allowed.append((at + 488, at + 496))
    for i in range(copies):
        for c in allocated:
            at = base + (reserved + i * fat_sectors) * sector_bytes + c * 4
            allowed.append((at, at + 4))
    for c in allocated: allowed.append((offset(c), offset(c) + cluster_bytes))
    changed_sectors, changed_bytes = [], 0
    for at in range(0, len(card), sector_bytes):
        before, after = original[at:at + sector_bytes], card[at:at + sector_bytes]
        if before == after: continue
        changed_sectors.append(at // sector_bytes)
        for n, (a, b) in enumerate(zip(before, after)):
            if a != b:
                position = at + n
                assert any(low <= position < high for low, high in allowed), "unexplained image byte modification"
                changed_bytes += 1
    assert old_free - new_free == len(allocated)
    decoded = Image.open(io.BytesIO(jpeg))
    decoded.load()
    assert decoded.size == (320, 320) and not decoded.getexif() and not decoded.info.get("exif") and not decoded.info.get("icc_profile")
    assert not decoded.info.get("progressive")
    rgba = decoded.convert("RGBA").tobytes()
    assert rgba == args.rgba.read_bytes() and len(rgba) == 320 * 320 * 4 and set(rgba[3::4]) == {255}
    palette = [(224, 48, 48), (48, 208, 80), (48, 96, 224)]
    samples = []
    for i, expected in enumerate(palette):
        point = (16 + 32 * i, 16)
        value = decoded.getpixel(point)
        assert all(abs(a - b) <= 3 for a, b in zip(value, expected))
        samples.append({"point": point, "decoded_rgb": value, "source_rgb": expected})
    marker, sof = 2, None
    markers = []
    while marker < len(jpeg):
        assert jpeg[marker] == 255
        code = jpeg[marker + 1]
        markers.append(hex(code))
        assert code not in (0xe1, 0xe2, 0xfe), "EXIF/ICC/comment metadata refused"
        length = int.from_bytes(jpeg[marker + 2:marker + 4], "big")
        if code == 0xc0: sof = jpeg[marker + 4:marker + 2 + length]
        if code == 0xda: break
        marker += 2 + length
    assert sof and sof[0] == 8 and sof[5] == 3 and [sof[i] for i in (7, 10, 13)] == [0x11] * 3
    assert jpeg.endswith(b"\xff\xd9")
    assert digest(args.source.read_bytes()) == ORIGINAL_SHA
    report = {"all_passed": True, "verification": "independent FAT32 parser, chain/directory traversal, full-image bounded diff and host decode; inputs opened read-only",
              "source_sha256_before_after": ORIGINAL_SHA, "card": {"bytes": len(card), "sha256": digest(card)},
              "jpeg": {"bytes": len(jpeg), "sha256": digest(jpeg)}, "decoded_rgba": {"bytes": len(rgba), "sha256": digest(rgba)},
              "root_entries": root_rows, "photo_entries": photo_rows, "root_chain": root_chain, "photo_chain": directory_chain,
              "jpeg_chain": file_chain, "fat_changed_entries_only": changed_fat, "mirrored_fats_equal": True,
              "free_clusters_before": old_free, "free_clusters_after": new_free, "next_free": 15,
              "changed_sectors": changed_sectors, "changed_bytes": changed_bytes, "mbr_boot_and_existing_data_preserved": True,
              "dimensions": [320, 320], "jpeg_pre_scan_markers": markers, "baseline_8bit_444": True, "exif": False,
              "decoded_sample_pixels": samples, "pillow": PIL.__version__,
              "verifier": {"bytes": Path(__file__).stat().st_size, "sha256": digest(Path(__file__).read_bytes())},
              "guest_executed": False, "target_jpeg_decoder_or_frame_verified": False}
    with args.report.open("x") as target: target.write(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"all_passed": True, "changed_sectors": changed_sectors, "changed_bytes": changed_bytes,
                      "card_sha256": report["card"]["sha256"], "decoded_sample_pixels": samples}, indent=2))


if __name__ == "__main__": main()
