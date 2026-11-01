#!/usr/bin/env python3
"""Offline safety/format checks; no third-party firmware is needed in CI."""
import importlib.util
import json
import struct
import unittest
from pathlib import Path

module = importlib.util.spec_from_file_location("profile_tool", Path(__file__).with_name("stackchan-firmware-profile.py"))
tool = importlib.util.module_from_spec(module)
module.loader.exec_module(tool)
SPEC = json.loads((tool.PROFILE / "provenance.json").read_text())
MANIFEST = json.loads((tool.PROFILE / "profile.json").read_text())


class ProfileSafety(unittest.TestCase):
    def test_elf_contains_symbols_without_loadable_code_or_ram(self):
        elf = (tool.PROFILE / "symbols.elf").read_bytes()
        self.assertEqual(elf, tool.symbol_elf(SPEC))
        self.assertEqual(len(elf), MANIFEST["symbols"]["bytes"])
        self.assertEqual(tool.sha(elf), MANIFEST["symbols"]["sha256"])
        self.assertEqual(struct.unpack_from("<I", elf, 24)[0], 0)  # no execution entry
        self.assertEqual(struct.unpack_from("<H", elf, 44)[0], 0)  # no program headers
        sections, names, symbols, _, _ = tool.object_tables(elf)
        self.assertEqual(names, ["", ".symtab", ".strtab", ".shstrtab"])
        self.assertTrue(all(section[2] == 0 for section in sections))  # no SHF_ALLOC
        self.assertEqual({s["name"]: (s["value"], s["size"]) for s in symbols if s["name"]},
                         {s["name"]: (s["address"], s["size"]) for s in SPEC["symbols"]})
        self.assertTrue(all(s["section"] == 0xfff1 for s in symbols if s["name"]))

    def test_wrong_firmware_is_rejected_before_any_address_is_read(self):
        for data in (b"", b"\xe9\x06", bytes(SPEC["firmware"]["bytes"])):
            with self.assertRaisesRegex(ValueError, "firmware size/SHA-256 mismatch"):
                tool.validate_firmware(SPEC, data)

    def test_address_windows_cannot_cross_a_segment_or_read_ram_bss(self):
        with self.assertRaisesRegex(ValueError, "unmapped"):
            tool.mapped_offset(SPEC, 0x3fcbb280, 512)
        segment = SPEC["segments"][3]
        with self.assertRaisesRegex(ValueError, "unmapped"):
            tool.mapped_offset(SPEC, segment["address"] + segment["bytes"] - 1, 2)

    def test_persistence_exception_cannot_cover_an_executable_window_or_ota_slot(self):
        self.assertEqual(MANIFEST["mutableFlashRanges"], [
            dict(offset=0x9000, bytes=0x4000, name="nvs"),
            dict(offset=0xd000, bytes=0x2000, name="otadata")])
        for window in MANIFEST["windows"]:
            self.assertGreater(window["bytes"], 0)
            self.assertLessEqual(window["offset"] + window["bytes"], MANIFEST["firmwareBytes"])
            for mutable in MANIFEST["mutableFlashRanges"]:
                self.assertTrue(window["offset"] >= mutable["offset"] + mutable["bytes"] or
                                mutable["offset"] >= window["offset"] + window["bytes"])
        spare = next(p for p in SPEC["partitions"] if p["name"] == "ota_1")
        self.assertTrue(spare["availableAllFF"])
        self.assertEqual(spare["availableBytes"], spare["bytes"])

    def test_bss_aliases_are_explicitly_a_proven_interior_reservation(self):
        reservation = SPEC["bssReservation"]
        self.assertFalse(reservation["originalLinkerBoundsRecovered"])
        self.assertEqual(reservation["end"] - reservation["start"], 512)
        self.assertLessEqual(reservation["provenContainerStartAtMost"], reservation["start"])
        self.assertGreaterEqual(reservation["provenContainerEndAtLeast"], reservation["end"])
        with self.assertRaisesRegex(ValueError, "archive hash mismatch"):
            tool.verify_controller_archive(SPEC, b"", b"!<arch>\n")


if __name__ == "__main__":
    unittest.main()
