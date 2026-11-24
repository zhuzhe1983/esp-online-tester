#!/usr/bin/env python3
"""Replay canonical Waveshare gzip patches in a private Git index/object store.

The main worktree, index, objects and refs are read-only inputs. Patches are
streamed one at a time; expanded diffs are never retained. A full run also binds
the resulting Git blobs to the CI Rust/catalog freezes and official SKU census.

Example (run from the repository root):
  python3 docs/platform/evidence/waveshare-final-20261124/verify-chain.py \
    --manifest DELIVERY/manifest.json --patch-dir DELIVERY \
    --catalog-freeze docs/platform/evidence/waveshare-final-20261124/catalog-freeze.json \
    --output DELIVERY/chain-verification

Use --max-batch 29 only for an explicitly partial preflight. Its successful exit
does not certify completion of the final 149-family delivery.
"""
from __future__ import annotations

import argparse
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tempfile
import time
from retained_evidence import read_bytes

BASE = "9e0f09c3f5507e0c50be1bb69d5653a40b42a3d4"
BASE_TREE = "967d1e2f9115771d409dbec02e09bb6c409b8f75"
CATALOGS = {
    "web/platform/catalog/products.json",
    "web/platform/catalog/firmwares.json",
    "web/platform/catalog/firmware-watchlist.json",
    "web/platform/knowledge/index.json",
}
INVENTORY = "docs/platform/research/waveshare-20261124/waveshare-complete-inventory.json"
CHUNK = 1024 * 1024


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read_hash(stream):
    digest = hashlib.sha256()
    size = 0
    for block in iter(lambda: stream.read(CHUNK), b""):
        digest.update(block)
        size += len(block)
    return size, digest.hexdigest()


def write_json(path, value):
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    temp.replace(path)


def load_freeze(path):
    raw = read_bytes(path)
    value = json.loads(raw)
    lines = value["sha256sum"]
    require(sha256(lines.encode()) == value["manifestSha256"], "Freeze manifest digest mismatch")
    result = {}
    for line in lines.splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        require(match is not None, "Invalid sha256sum freeze line")
        digest, name = match.groups()
        require(name not in result, "Duplicate path in freeze")
        result[name] = digest
    require(len(result) == value["fileCount"], "Freeze file count mismatch")
    canonical = "".join(f"{result[name]}  {name}\n" for name in sorted(result))
    require(canonical == lines, "Freeze must be sorted and LF terminated")
    return result, {"fileSha256": sha256(raw), "manifestSha256": value["manifestSha256"], "fileCount": len(result)}


class Git:
    def __init__(self, repo, output, temporary, main_objects):
        self.repo = repo
        self.output = output
        self.counter = 0
        self.env = dict(os.environ)
        self.env.update(
            GIT_INDEX_FILE=str(temporary / "index"),
            GIT_OBJECT_DIRECTORY=str(temporary / "objects"),
            GIT_ALTERNATE_OBJECT_DIRECTORIES=str(main_objects),
            GIT_OPTIONAL_LOCKS="0",
            GIT_NO_LAZY_FETCH="1",
            GIT_NO_REPLACE_OBJECTS="1",
        )
        (temporary / "objects").mkdir()
        (output / "raw").mkdir()

    def spawn(self, label, arguments, stdin=False, pipe_stdout=False):
        self.counter += 1
        stem = f"{self.counter:03d}-{label}"
        stdout_path = self.output / "raw" / (stem + ".stdout")
        stderr_path = self.output / "raw" / (stem + ".stderr")
        stdout = subprocess.PIPE if pipe_stdout else stdout_path.open("wb")
        stderr = stderr_path.open("wb")
        argv = ["git", "-c", "gc.auto=0", *arguments]
        started = time.monotonic()
        process = subprocess.Popen(
            argv, cwd=self.repo, env=self.env,
            stdin=subprocess.PIPE if stdin else subprocess.DEVNULL,
            stdout=stdout, stderr=stderr,
        )
        return process, {
            "command": argv,
            "cwd": "${REPOSITORY}",
            "environment": {
                "GIT_INDEX_FILE": "${PRIVATE_INDEX}",
                "GIT_OBJECT_DIRECTORY": "${PRIVATE_OBJECTS}",
                "GIT_ALTERNATE_OBJECT_DIRECTORIES": "${MAIN_OBJECTS}",
                "GIT_OPTIONAL_LOCKS": "0",
                "GIT_NO_LAZY_FETCH": "1",
                "GIT_NO_REPLACE_OBJECTS": "1",
            },
            "stdout": str(stdout_path.relative_to(self.output)),
            "stderr": str(stderr_path.relative_to(self.output)),
            "started": started,
        }, stdout, stderr

    def finish(self, execution, **details):
        process, record, stdout, stderr = execution
        status = process.wait()
        if stdout != subprocess.PIPE:
            stdout.close()
        stderr.close()
        record["exitCode"] = status
        record["elapsedSeconds"] = round(time.monotonic() - record.pop("started"), 3)
        record.update(details)
        with (self.output / "commands.jsonl").open("a") as log:
            log.write(json.dumps(record, ensure_ascii=False) + "\n")
        require(status == 0, "Git command failed: " + " ".join(record["command"]) + "; see " + record["stderr"])
        return record

    def run(self, label, *arguments):
        execution = self.spawn(label, list(arguments))
        record = self.finish(execution)
        return (self.output / record["stdout"]).read_bytes()

    def replay(self, batch, patch_path):
        # Hold one descriptor across the compressed check and replay. No raw
        # patch file is created, and only the current chunk is held in Python.
        with patch_path.open("rb") as compressed:
            compressed_size, compressed_sha = read_hash(compressed)
            require(compressed_size == batch["gzipBytes"], "Compressed byte count mismatch")
            require(compressed_sha == batch["gzipSha256"], "Compressed SHA-256 mismatch")
            compressed.seek(0)
            execution = self.spawn(
                f"batch-{batch['number']:03d}-apply",
                ["apply", "--cached", "--binary", "--whitespace=nowarn", "-"], stdin=True,
            )
            process = execution[0]
            expanded_sha = hashlib.sha256()
            expanded_size = 0
            failure = None
            try:
                with gzip.GzipFile(fileobj=compressed, mode="rb") as expanded:
                    for block in iter(lambda: expanded.read(CHUNK), b""):
                        expanded_sha.update(block)
                        expanded_size += len(block)
                        process.stdin.write(block)
                process.stdin.close()
            except BaseException as exc:
                failure = exc
                try:
                    process.stdin.close()
                except BrokenPipeError:
                    pass
            self.finish(
                execution, stdinSource=batch["gzipFilename"] + " (gzip stream)",
                stdinBytes=expanded_size, stdinSha256=expanded_sha.hexdigest(),
            )
            if failure:
                raise failure
        require(expanded_size == batch["bytes"], "Expanded byte count mismatch")
        require(expanded_sha.hexdigest() == batch["sha256"], "Expanded SHA-256 mismatch")
        tree = self.run(f"batch-{batch['number']:03d}-tree", "write-tree").decode().strip()
        require(tree == batch["tree"], f"Resulting tree differs for batch {batch['number']:03d}")
        return {
            "number": batch["number"], "baseCommit": batch["baseCommit"],
            "commit": batch["commit"], "tree": tree,
            "gzipBytes": compressed_size, "gzipSha256": compressed_sha,
            "diffBytes": expanded_size, "diffSha256": expanded_sha.hexdigest(),
            "applyExitCode": 0, "treeMatched": True,
        }

    def entries(self, label, tree):
        result = {}
        for row in self.run(label, "ls-tree", "-r", "-z", "--full-tree", tree).split(b"\0"):
            if row:
                info, name = row.split(b"\t", 1)
                mode, kind, oid = info.decode().split(" ")
                result[name.decode()] = {"mode": mode, "kind": kind, "oid": oid}
        return result

    def blobs(self, entries, wanted, retain):
        execution = self.spawn("checked-blobs", ["cat-file", "--batch"], stdin=True, pipe_stdout=True)
        process = execution[0]
        digests = {}
        retained = {}
        headers = self.output / execution[1]["stdout"]
        total = 0
        try:
            with headers.open("wb") as header_log:
                for name in sorted(wanted):
                    require(name in entries and entries[name]["kind"] == "blob", "Required Git blob missing: " + name)
                    oid = entries[name]["oid"]
                    process.stdin.write((oid + "\n").encode())
                    process.stdin.flush()
                    header = process.stdout.readline()
                    header_log.write(header)
                    fields = header.decode().strip().split(" ")
                    require(len(fields) == 3 and fields[:2] == [oid, "blob"], "Invalid cat-file blob header")
                    length = int(fields[2])
                    remaining = length
                    digest = hashlib.sha256()
                    kept = bytearray() if name in retain else None
                    while remaining:
                        block = process.stdout.read(min(CHUNK, remaining))
                        require(block, "Unexpected end of Git blob")
                        digest.update(block)
                        if kept is not None:
                            kept.extend(block)
                        remaining -= len(block)
                    require(process.stdout.read(1) == b"\n", "Missing cat-file blob delimiter")
                    digests[name] = {"oid": oid, "bytes": length, "sha256": digest.hexdigest()}
                    if kept is not None:
                        retained[name] = bytes(kept)
                    total += length
            process.stdin.close()
            process.stdout.close()
            self.finish(execution, stdoutPolicy="Raw object headers retained; blob bytes streamed into hashes, not copied to logs", blobCount=len(digests), blobBytes=total)
        except BaseException:
            process.kill()
            process.wait()
            execution[3].close()
            raise
        return digests, retained


def check_coverage(manifest, selected, products, inventory, complete):
    included = [x for x in inventory["items"] if x["scope"] == "included-controller-or-controller-kit"]
    families = {x["listingId"]: x for x in included}
    require(len(families) == len(included) == 149, "Official inventory must contain 149 controller families")
    raw_skus = [str(v["sku"]) for x in included for v in x["variants"]]
    require(len(raw_skus) == len(set(raw_skus)) == 384, "Official inventory must have 384 unique raw SKU rows")
    require("33355" in raw_skus, "Accessory exclusion must be explicit")
    listing_ids = [name for batch in selected for name in batch["listings"]]
    profile_ids = [name for batch in selected for name in batch["productIds"]]
    require(len(listing_ids) == len(set(listing_ids)), "A listing appears in multiple batches")
    require(len(profile_ids) == len(set(profile_ids)), "A profile appears in multiple batches")
    require(set(listing_ids) <= set(families), "Batch contains a listing outside official controller scope")
    expected_skus = {
        str(v["sku"]) for name in listing_ids for v in families[name]["variants"] if str(v["sku"]) != "33355"
    }
    waveshare = [p for p in products["items"] if p.get("manufacturer") == "Waveshare"]
    actual_ids = [p["id"] for p in waveshare]
    require(len(actual_ids) == len(set(actual_ids)), "Duplicate Waveshare product ID")
    require(set(actual_ids) == set(profile_ids), "Catalog profiles differ from ordered batch scope")
    require(all(p.get("sku") for p in waveshare), "Every profile must identify an official SKU")
    actual_skus = {str(p["sku"]) for p in waveshare}
    require(actual_skus == expected_skus, "Catalog SKU coverage differs from official included families")
    require("33355" not in actual_skus, "Accessory-only SKU 33355 must not be a controller profile")
    if complete:
        require(len(selected) == 30, "Complete delivery must contain 30 incremental batches")
        require(set(listing_ids) == set(families), "Final delivery does not cover all 149 families")
        require((len(waveshare), len(actual_skus), len(listing_ids)) == (407, 383, 149), "Final profile/SKU/family totals differ")
        require(set(manifest["productIds"]) == set(actual_ids), "Final manifest product IDs differ from its tree")
        require(set(manifest["listings"]) == set(listing_ids), "Final manifest listing IDs differ from its batches")
        require(manifest.get("profileCount") == 407 and manifest.get("skuCount") == 383, "Final manifest totals differ")
    return {
        "profileCount": len(waveshare), "controllerSkuCount": len(actual_skus),
        "includedFamilyCount": len(listing_ids), "officialFamilyCount": len(families),
        "officialRawSkuCount": len(raw_skus), "excludedAccessorySkus": ["33355"],
        "missingFamilies": sorted(set(families) - set(listing_ids)),
        "missingControllerSkus": sorted(set(raw_skus) - {"33355"} - actual_skus),
        "profileIds": sorted(actual_ids), "controllerSkus": sorted(actual_skus),
        "familyIds": sorted(listing_ids), "complete": complete,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[4])
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--patch-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rust-freeze", type=Path, default=Path(__file__).with_name("rust-source-freeze.json"))
    parser.add_argument("--catalog-freeze", type=Path)
    parser.add_argument("--inventory", default=INVENTORY, help="Repository-relative official inventory path in the resulting tree")
    parser.add_argument("--max-batch", type=int, help="Partial preflight only; does not certify full delivery")
    args = parser.parse_args()
    repo = args.repo.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    require(not any(output.iterdir()), "Output directory must be empty to preserve prior evidence")
    summary = {
        "schemaVersion": 1, "status": "running", "fullDeliveryVerification": args.max_batch is None,
        "verifierSha256": sha256(Path(__file__).read_bytes()),
    }
    started = time.monotonic()
    try:
        manifest_raw = args.manifest.read_bytes()
        manifest = json.loads(manifest_raw)
        if "expected" in manifest:
            manifest = manifest["expected"]
        require(manifest["baseCommit"] == BASE and manifest["baseTree"] == BASE_TREE, "Unexpected chain base")
        all_batches = manifest["batches"]
        require([b["number"] for b in all_batches] == list(range(1, len(all_batches) + 1)), "Batch numbers are not contiguous")
        if args.max_batch is None:
            require(manifest.get("complete") is True, "Incomplete manifest: use --max-batch for a partial preflight")
            require(args.catalog_freeze is not None, "A full verification requires the tested catalog freeze")
            selected = all_batches
            require(selected and manifest["latestCommit"] == selected[-1]["commit"], "Final manifest latestCommit differs from its last batch")
        else:
            require(1 <= args.max_batch <= len(all_batches), "Invalid --max-batch")
            selected = all_batches[:args.max_batch]
        parent = BASE
        for batch in selected:
            require(batch["baseCommit"] == parent, "Commit parent chain is discontinuous")
            require(batch["gzipFilename"] == f"waveshare-{batch['number']:03d}.diff.gz", "Unexpected gzip filename")
            expected_families = 4 if batch["number"] == 30 else 5
            require(len(batch["listings"]) == expected_families, "Expected five families per batch and four in final batch 030")
            parent = batch["commit"]
        rust, rust_receipt = load_freeze(args.rust_freeze)
        require(len(rust) == 855, "Expected exactly 855 frozen Rust/Cargo files")
        catalog, catalog_receipt = ({}, None) if args.catalog_freeze is None else load_freeze(args.catalog_freeze)
        if catalog:
            require(set(catalog) == CATALOGS, "Catalog freeze must identify exactly the four central catalogs")
        main_objects = Path(subprocess.check_output(["git", "rev-parse", "--path-format=absolute", "--git-path", "objects"], cwd=repo, text=True).strip())
        summary.update(baseCommit=BASE, baseTree=BASE_TREE, manifestSha256=sha256(manifest_raw), batchLimit=args.max_batch, rustFreeze=rust_receipt, catalogFreeze=catalog_receipt)
        batches = []
        # Temporary objects are on the output filesystem, not the system tmpfs.
        with tempfile.TemporaryDirectory(prefix=".private-git-", dir=output) as temp:
            git = Git(repo, output, Path(temp), main_objects)
            actual_base = git.run("base-tree", "rev-parse", BASE + "^{tree}").decode().strip()
            require(actual_base == BASE_TREE, "Source repository base tree differs")
            git.run("read-base", "read-tree", BASE)
            for batch in selected:
                receipt = git.replay(batch, args.patch_dir / batch["gzipFilename"])
                batches.append(receipt)
                with (output / "batches.jsonl").open("a") as log:
                    log.write(json.dumps(receipt) + "\n")
                print(json.dumps(receipt), flush=True)
            final_tree = batches[-1]["tree"]
            entries = git.entries("result-tree", final_tree)
            base_entries = git.entries("base-entries", BASE_TREE)
            actual_rust = {p for p, e in entries.items() if e["kind"] == "blob" and (p.endswith(".rs") or PurePosixPath(p).name in {"Cargo.toml", "Cargo.lock"})}
            require(actual_rust == set(rust), "Resulting tree Rust/Cargo file set differs from the tested freeze")
            golden = {p: e for p, e in base_entries.items() if p.startswith("tests/golden/")}
            final_golden = {p: e for p, e in entries.items() if p.startswith("tests/golden/")}
            require(len(golden) == 92 and final_golden == golden, "Golden file paths/modes/objects differ from base")
            digests, retained = git.blobs(entries, set(rust) | CATALOGS | {args.inventory}, {"web/platform/catalog/products.json", args.inventory})
            for name, digest in rust.items():
                require(digests[name]["sha256"] == digest, "Rust/Cargo blob differs from tested freeze: " + name)
            for name, digest in catalog.items():
                require(digests[name]["sha256"] == digest, "Catalog blob differs from tested freeze: " + name)
            scope = check_coverage(manifest, selected, json.loads(retained["web/platform/catalog/products.json"]), json.loads(retained[args.inventory]), args.max_batch is None)
            write_json(output / "checked-blobs.json", digests)
            write_json(output / "golden-objects.json", golden)
            write_json(output / "coverage.json", scope)
            summary.update(
                status="passed", batchCount=len(batches), finalCommit=selected[-1]["commit"], finalTree=final_tree,
                gzipBytes=sum(b["gzipBytes"] for b in batches), diffBytes=sum(b["diffBytes"] for b in batches),
                rustCargoFilesMatched=len(rust), goldenObjectsUnchanged=len(golden), catalogFilesMatched=len(catalog),
                profileCount=scope["profileCount"], controllerSkuCount=scope["controllerSkuCount"], includedFamilyCount=scope["includedFamilyCount"],
                officialInventory={"path": args.inventory, **digests[args.inventory]},
                expandedDiffFilesRetained=0, privateIndexAndObjectStore=True, patchInputChunkBytes=CHUNK,
            )
    except BaseException as exc:
        summary.update(status="failed", error=type(exc).__name__ + ": " + str(exc))
        raise
    finally:
        summary["elapsedSeconds"] = round(time.monotonic() - started, 3)
        summary["checkedAt"] = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")
        write_json(output / "summary.json", summary)
        print(json.dumps(summary, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.CalledProcessError) as exc:
        print("Stopped: " + str(exc), file=sys.stderr)
        sys.exit(1)
