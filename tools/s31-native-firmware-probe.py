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
    parser.add_argument("--upstream-fpu-memory-reason", action="store_true", help="explicit official-emulator0.51.0 S31 reason-bit compatibility,not silicon verified")
    parser.add_argument("--guest-exceptions", type=int, default=0, help="bounded execution of architectural guest handlers; unknown bus faults still stop, maximum1024")
    parser.add_argument("--rom-spi0-shadow", action="store_true", help="explicit inferred legacy SPI0 config shadows; strict mode remains default")
    parser.add_argument("--systimer-from-cpu", action="store_true")
    parser.add_argument("--rtc-timer-from-cpu", action="store_true", help="explicit RTC_TIMER-only one-interpreter-cycle-per-instruction timing approximation")
    parser.add_argument("--rtc-calibration-from-cpu", action="store_true", help="explicit nominal CPU/XTAL clock conversion for TIMG0 measurement; not physical timing")
    parser.add_argument("--bbpll-hz", type=int, help="explicit stable BBPLL output frequency; no modeled analog lock")
    parser.add_argument("--nor-sfdp", help="explicit hex-offset:hex-bytes SFDP interval; no inferred device parameters")
    parser.add_argument("--nor-jedec", help="explicit six-hex-digit development NOR identity; not product hardware identification")
    parser.add_argument("--analog-bias", help="explicit four-hex-digit BIAS register0/register1 seed; no silicon reset claim")
    parser.add_argument("--analog-bod", help="explicit two-hex-digit BOD register5 seed; no measured voltage threshold")
    parser.add_argument("--analog-dig", help="explicit eight-hex-digit DIG registers5,7,13,14 seed")
    parser.add_argument("--analog-perif-glitch", help="explicit two-hex-digit PERIF register10 seed; no voltage/glitch reset claim")
    parser.add_argument("--peripheral-pad", action="append", default=[], help="explicit unresolved peripheral pad sample pin:0|1; repeat for each pin, not measured board wiring")
    parser.add_argument("--psram-mr", help="explicit AP-compatible MR0,1,2,3,4,8 responses,12hex; not identified product die")
    parser.add_argument("--psram-bytes", type=int, help="explicit separate PSRAM backing bytes; no inferred density")
    parser.add_argument("--watch-pc1", help="optional diagnostic retirement watch for the actual second-hart entry address; no execution change")
    parser.add_argument("--steps", type=int, default=200000, help="bounded instruction budget, maximum 100000000")
    args = parser.parse_args()
    if not 1 <= args.steps <= 100000000:
        parser.error("--steps must be between 1 and 100000000")
    if not 0 <= args.guest_exceptions <= 1024:
        parser.error("--guest-exceptions must be between0 and1024")
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
    command = [args.runner, "--rom", "--usb-host", "--strap=0x12", "--flash=/dev/stdin"]
    command.append("--guest-exceptions=" + str(args.guest_exceptions))
    if args.upstream_fpu_memory_reason:
        command.append("--upstream-fpu-memory-reason")
    if args.watch_pc1:
        command.append("--watch-pc1=" + args.watch_pc1)
    if args.rom_spi0_shadow:
        command.append("--rom-spi0-shadow")
    if args.systimer_from_cpu: command.append("--systimer-from-cpu")
    if args.rtc_timer_from_cpu:
        command.append("--rtc-timer-from-cpu")
    if args.rtc_calibration_from_cpu:
        command.append("--rtc-calibration-from-cpu")
    if args.bbpll_hz is not None:
        command.append("--bbpll-hz=" + str(args.bbpll_hz))
    if args.psram_mr is not None:
        command.append("--psram-mr=" + args.psram_mr)
    if args.psram_bytes is not None:
        command.append("--psram-bytes=" + str(args.psram_bytes))
    if args.nor_sfdp:
        command.append("--nor-sfdp=" + args.nor_sfdp)
    if args.nor_jedec:
        command.append("--nor-jedec=" + args.nor_jedec)
    if args.analog_bias:
        command.append("--analog-bias=" + args.analog_bias)
    if args.analog_dig:
        command.append("--analog-dig=" + args.analog_dig)
    if args.analog_bod:
        command.append("--analog-bod=" + args.analog_bod)
    if args.analog_perif_glitch:
        command.append("--analog-perif-glitch=" + args.analog_perif_glitch)
    command.extend("--peripheral-pad=" + value for value in args.peripheral_pad)
    # Observe retirement at the parsed application entry without changing execution.
    if len(images) > 1:
        command.append("--watch-pc=" + images[1]["entry"])
    command.extend([args.rom, "", str(args.steps)])
    runner_sha = hashlib.sha256(pathlib.Path(args.runner).read_bytes()).hexdigest()
    result = subprocess.run(command, input=data, capture_output=True, timeout=120, check=True)
    report = {"runnerSha256": runner_sha, "stepBudget": args.steps, "guestExceptionLimit": args.guest_exceptions, "source": receipt, "romSha256": hashlib.sha256(pathlib.Path(args.rom).read_bytes()).hexdigest(),
              "partitions": partitions, "images": images,
              "developmentInputs": {"upstreamFpuMemoryReason": args.upstream_fpu_memory_reason, "strap": "0x12", "usbHost": True, "norIdentity": args.nor_jedec, "sfdpInterval": args.nor_sfdp, "bbpllInputHz": args.bbpll_hz, "systimerFromCpuCycles": args.systimer_from_cpu, "rtcTimerFromCpuCycles": args.rtc_timer_from_cpu, "rtcCalibrationFromCpuCycles": args.rtc_calibration_from_cpu, "otpImage": None, "spi0LegacyShadows": args.rom_spi0_shadow, "analogBiasSeed": args.analog_bias, "analogPerifGlitchSeed": args.analog_perif_glitch, "analogDigSeed": args.analog_dig, "analogBodSeed": args.analog_bod, "unresolvedPeripheralPads": args.peripheral_pad, "psramModeResponses": args.psram_mr, "psramBackingBytes": args.psram_bytes},
              "uart": result.stdout.decode("utf-8", errors="replace"),
              "diagnostic": result.stderr.decode("utf-8", errors="replace"),
              "storage": "firmware bytes only in host/guest RAM and subprocess pipe; not written to disk"}
    pathlib.Path(args.output).write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(report["diagnostic"])


if __name__ == "__main__":
    main()
