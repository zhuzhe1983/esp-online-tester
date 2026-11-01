#!/usr/bin/env python3
"""Reproduce/verify the exact StackChan factory symbol-only VHCI profile.

No network access, firmware code output, or guessed address discovery. The optional
controller archive check replays the documented object/relocation evidence.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROFILE = ROOT / "web/platform/assets/firmware-profiles/stackchan-factory-1.5.1"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def mapped_offset(spec, address, size):
    for segment in spec["segments"]:
        offset = address - segment["address"]
        if offset >= 0 and offset + size <= segment["bytes"]:
            return segment["offset"] + offset
    raise ValueError(f"unmapped address/extent: {address:#x}+{size:#x}")


def image_bytes(spec, firmware, address, size):
    offset = mapped_offset(spec, address, size)
    return firmware[offset:offset + size]


def symbol_elf(spec):
    """ELF32 Xtensa, absolute symbols only; no program/load/alloc sections."""
    strings, symbols = bytearray(b"\0"), bytearray(16)
    for symbol in spec["symbols"]:
        name_offset = len(strings)
        strings.extend(symbol["name"].encode("ascii") + b"\0")
        info = 0x12 if symbol["size"] else 0x10  # GLOBAL FUNC / NOTYPE
        symbols.extend(struct.pack("<IIIBBH", name_offset, symbol["address"],
                                   symbol["size"], info, 0, 0xfff1))  # SHN_ABS
    names = b"\0.symtab\0.strtab\0.shstrtab\0"
    offsets = [52, 52 + len(symbols), 52 + len(symbols) + len(strings)]
    payload = symbols + strings + names
    padding = bytes((-len(payload) - 52) % 4)
    header = b"\x7fELF\x01\x01\x01" + bytes(9) + struct.pack(
        "<HHIIIIIHHHHHH", 1, 94, 1, 0, 0, 52 + len(payload) + len(padding),
        0, 52, 0, 0, 40, 4, 3)
    sections = bytes(40)
    for values in [(1, 2, 0, 0, offsets[0], len(symbols), 2, 1, 4, 16),
                   (9, 3, 0, 0, offsets[1], len(strings), 0, 0, 1, 0),
                   (17, 3, 0, 0, offsets[2], len(names), 0, 0, 1, 0)]:
        sections += struct.pack("<10I", *values)
    return header + payload + padding + sections


def validate_firmware(spec, firmware):
    expected = spec["firmware"]
    require(len(firmware) == expected["bytes"] and sha(firmware) == expected["sha256"],
            "firmware size/SHA-256 mismatch; no profile may be applied")
    cursor = expected["appOffset"] + 24
    require(firmware[expected["appOffset"]:expected["appOffset"] + 2] == b"\xe9\x06",
            "unexpected ESP image header")
    for segment in spec["segments"]:
        address, size = struct.unpack_from("<II", firmware, cursor)
        require((address, size, cursor + 8) ==
                (segment["address"], segment["bytes"], segment["offset"]),
                "ESP image segment mapping differs")
        cursor += 8 + size
    for index, partition in enumerate(spec["partitions"]):
        record = struct.unpack_from("<HBBII16sI", firmware, 0x8000 + index * 32)
        label = record[5].split(b"\0")[0].decode("ascii")
        require(record[:5] == (0x50aa, partition["type"], partition["subtype"],
                              partition["offset"], partition["bytes"])
                and label == partition["name"] and record[6] == partition["flags"],
                "partition table differs")
    spare = next(p for p in spec["partitions"] if p["name"] == "ota_1")
    require(all(b == 255 for b in firmware[spare["offset"]:spare["offset"] + spare["bytes"]]),
            "alternate OTA slot must be entirely erased")
    for window in spec["symbolWindows"]:
        if window["symbol"].endswith(":__func__"):
            require(image_bytes(spec, firmware, window["address"], window["bytes"]) ==
                    window["symbol"].split(":")[0].encode() + b"\0", "function-name anchor differs")
    for call in spec["callSites"]:
        instruction = int.from_bytes(image_bytes(spec, firmware, call["address"], 3), "little")
        require(instruction & 63 == 0x25, "expected Xtensa CALL8 opcode")
        displacement = instruction >> 6
        if displacement & (1 << 17):
            displacement -= 1 << 18
        require((call["address"] & ~3) + 4 + displacement * 4 == call["target"],
                "controller call target differs")


def manifest(spec, firmware, elf):
    validate_firmware(spec, firmware)
    windows = []

    def add(address, size, name):
        offset = mapped_offset(spec, address, size)
        windows.append(dict(offset=offset, bytes=size,
                            sha256=sha(firmware[offset:offset + size]), symbol=name))

    for symbol in spec["symbols"]:
        if symbol["size"]:
            add(symbol["address"], symbol["size"], symbol["name"])
    for anchor in spec["bssAnchors"]:
        add(anchor["functionAddress"], anchor["functionSize"], anchor["function"])
        add(anchor["literalAddress"], 4, anchor["symbol"] + ":literal")
    for window in spec["symbolWindows"]:
        add(window["address"], window["bytes"], window["symbol"])
    for function in spec["controllerApiFunctions"]:
        add(function["functionAddress"], function["functionSize"], function["function"])
    for call in spec["callSites"]:
        add(call["address"], 3, "CALL8:" + hex(call["target"]))
    return dict(schemaVersion=1, id=spec["id"], board=spec["board"],
                firmwareSha256=spec["firmware"]["sha256"], firmwareBytes=len(firmware),
                format="merged", symbols=dict(file="symbols.elf", sha256=sha(elf), bytes=len(elf)),
                windows=windows, mutableFlashRanges=spec["mutableFlashRanges"])


def archive_members(data):
    require(data[:8] == b"!<arch>\n", "not an ar archive")
    cursor, long_names = 8, b""
    while cursor < len(data):
        header = data[cursor:cursor + 60]
        require(len(header) == 60 and header[58:] == b"`\n", "invalid ar member")
        size, name = int(header[48:58]), header[:16].decode("ascii").strip()
        body = data[cursor + 60:cursor + 60 + size]
        cursor += 60 + size + size % 2
        if name == "//":
            long_names = body
        elif name.startswith("/") and name[1:].isdigit():
            yield long_names[int(name[1:]):].split(b"/\n")[0].decode("ascii").rstrip("/"), body
        else:
            yield name.rstrip("/"), body


def object_tables(data):
    require(data[:6] == b"\x7fELF\x01\x01", "expected ELF32 little-endian archive member")
    section_offset = struct.unpack_from("<I", data, 32)[0]
    entry_size, count, name_index = struct.unpack_from("<HHH", data, 46)
    sections = [struct.unpack_from("<10I", data, section_offset + i * entry_size) for i in range(count)]

    def contents(section):
        return data[section[4]:section[4] + section[5]]

    def name(strings, offset):
        return strings[offset:].split(b"\0")[0].decode("ascii")

    names = contents(sections[name_index])
    section_names = [name(names, s[0]) for s in sections]
    symtab = next(s for s in sections if s[1] == 2)
    strings = contents(sections[symtab[6]])
    symbols = []
    for offset in range(symtab[4], symtab[4] + symtab[5], symtab[9]):
        index, value, size, info, other, section = struct.unpack_from("<IIIBBH", data, offset)
        symbols.append(dict(name=name(strings, index), value=value, size=size, section=section, info=info))
    relocations = {}
    for section in sections:
        if section[1] == 4:  # SHT_RELA
            relocations[section[7]] = [struct.unpack_from("<IIi", data, offset)
                                      for offset in range(section[4], section[4] + section[5], section[9])]
    return sections, section_names, symbols, relocations, contents


def verify_controller_archive(spec, firmware, data):
    source = next(s for s in spec["sources"] if s["id"] == "controller-library")
    require(len(data) == source["bytes"] and sha(data) == source["sha256"], "controller archive hash mismatch")
    members = dict(archive_members(data))
    for anchor in spec["bssAnchors"] + spec["controllerApiFunctions"]:
        sections, names, symbols, relocations, contents = object_tables(members[anchor["object"]])
        function = next(s for s in symbols if s["name"] == anchor["function"])
        start, size = function["value"], function["size"]
        require(size == anchor["functionSize"], "controller function extent differs")
        code = contents(sections[function["section"]])[start:start + size]
        linked = image_bytes(spec, firmware, anchor["functionAddress"], size)
        relocs = relocations[function["section"]]
        mask = bytearray(b"\1" * size)
        for offset, info, _ in relocs:
            width = 6 if info & 255 == 11 else (4 if info & 255 == 1 else 3)
            for index in range(max(0, offset - start), min(size, offset - start + width)):
                mask[index] = 0
        require(all(not keep or a == b for keep, a, b in zip(mask, code, linked)),
                "controller function non-relocated bytes differ: " + anchor["function"])
        if "relocationOffset" not in anchor:
            continue
        offset, info, addend = next(r for r in relocs if r[0] == anchor["relocationOffset"] and r[1] & 255 == 20)
        target = symbols[info >> 8]
        require(target["section"] == function["section"], "L32R must reference its object literal")
        literal_relocation = next(r for r in relocs if r[0] == target["value"] + addend and r[1] & 255 == 1)
        bss_symbol = symbols[literal_relocation[1] >> 8]
        require(names[bss_symbol["section"]] == anchor["section"]
                and sections[bss_symbol["section"]][1] == 8, "literal must address the documented NOBITS section")
        require((bss_symbol["name"] or names[bss_symbol["section"]]) == anchor["symbol"], "BSS symbol differs")
        require((bss_symbol["size"] or sections[bss_symbol["section"]][5]) == anchor["symbolSize"], "BSS extent differs")
        load = anchor["functionAddress"] + offset - start
        require(load == anchor["loadAddress"], "load offset differs")
        instruction = image_bytes(spec, firmware, load, 3)
        require(instruction[0] & 15 == 1, "expected L32R opcode")
        immediate = int.from_bytes(instruction[1:], "little", signed=True)
        literal = ((load + 3) & ~3) + immediate * 4
        require(literal == anchor["literalAddress"], "linked literal address differs")
        value = int.from_bytes(image_bytes(spec, firmware, literal, 4), "little")
        require(value == anchor["symbolAddress"] + literal_relocation[2], "linked BSS address differs")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--firmware", type=Path, required=True, help="unmodified official merged BIN")
    parser.add_argument("--controller-lib", type=Path, help="exact IDF 5.5.4 S3 libbtdm_app.a; verify BSS relocations")
    parser.add_argument("--write", action="store_true", help="regenerate profile.json and symbols.elf after verification")
    args = parser.parse_args()
    spec = json.loads((PROFILE / "provenance.json").read_text())
    firmware = args.firmware.read_bytes()
    elf = symbol_elf(spec)
    profile = manifest(spec, firmware, elf)
    if args.controller_lib:
        verify_controller_archive(spec, firmware, args.controller_lib.read_bytes())
    artifacts = {"symbols.elf": elf, "profile.json": (json.dumps(profile, indent=2) + "\n").encode()}
    for name, data in artifacts.items():
        if args.write:
            (PROFILE / name).write_bytes(data)
        else:
            require((PROFILE / name).read_bytes() == data, "generated artifact differs: " + name)
    print(json.dumps(dict(ok=True, firmwareSha256=sha(firmware), symbolElfSha256=sha(elf),
                          windows=len(profile["windows"]), controllerRelocationsVerified=bool(args.controller_lib))))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, StopIteration, KeyError, struct.error) as error:
        raise SystemExit("profile verification failed: " + str(error)) from error
