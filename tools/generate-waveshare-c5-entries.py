#!/usr/bin/env python3
"""Generate reviewed C5 catalog fragments without editing any shared catalog.

The central patch owns measured identity/capacity/firmware data. A small explicit
fields file owns board-specific prose, wiring, controls and source aliases. No
chip-family fallback, model completion claim or factory-success inference exists.
"""
import argparse
import copy
import datetime
import gzip
import hashlib
import json
from pathlib import Path
import re


def read(path):
    return json.loads(path.read_text())


def sha(data):
    return hashlib.sha256(data).hexdigest()


def utc(value):
    return datetime.datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(
        datetime.timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def generate(root, patch, fields, timestamp, existing_root=None):
    assert patch["chip"] == "esp32c5"
    pid, board = patch["productId"], patch["profile"]
    assert fields["productId"] == pid
    manifest_path = root / patch["sourceReceipt"]
    manifest = read(manifest_path)
    sources, assets, omitted = [], [], []
    aliases = fields["sourceAliases"]
    for number, row in enumerate(manifest["sources"]):
        original = row["source"]
        if original["bytes"] == 0:
            omitted.append({"id": row["id"], "reason": "Zero-byte original retained in source manifest; not a knowledge fact source."})
            continue
        sid = aliases.get(row["id"], "source-" + str(number))
        if row.get("archive"):
            archive = manifest_path.parent / row["archive"]
            compressed = archive.read_bytes()
            assert sha(compressed) == row["archiveSha256"], str(archive)
            body = gzip.decompress(compressed)
            assert len(body) == original["bytes"] and sha(body) == original["sha256"], str(archive)
            original_name = row["archive"].removesuffix(".gz")
            retention = "Original retrieved content retained in " + str(archive.relative_to(root))
        else:
            assert row.get("retention") == "local-only-no-redistribution"
            original_name = Path(row["id"]).name
            retention = "Original downloaded document retained locally; no redistributed copy. Receipt and cited pages are in the source manifest."
        original_name = "source-" + re.sub(r"[^a-zA-Z0-9._-]", "_", original_name)
        stamp = utc(original.get("fetchedAtUTC") or original.get("fetchedAt") or original["capturedAtUTC"])
        source_url = original.get("url") or original["sourceUrl"]
        assert stamp <= timestamp, "updatedAt must follow original source retrieval"
        sources.append({"id": sid, "title": row["id"], "url": source_url,
            "kind": "manufacturer-schematic" if sid == "schematic" else "primary-technical-source",
            "status": "fetched", "resolvedUrl": original.get("finalUrl", source_url),
            "fetchedAt": stamp, "bytes": original["bytes"], "sha256": original["sha256"],
            "archiveName": original_name,
            "notes": retention + "; captureKind=" + original.get("captureKind", "recorded source receipt") + ". Hashes describe original bytes, not compressed archives."})
        assets.append({"id": sid, "sourceId": sid,
            "role": "schematic" if sid == "schematic" else "source-document", "url": source_url})

    factory = patch["downloadedFirmware"]
    assert factory["simulationVerified"] is False and factory["status"] == "reference"
    factory_name = Path(factory["path"]).name
    factory_version = re.search(r"FactoryOnly_[0-9]{8}", factory_name).group()
    sources.append({"id": "factory", "title": "Unmodified " + factory_name,
        "url": factory["url"], "kind": "manufacturer-firmware", "status": "fetched",
        "resolvedUrl": factory.get("finalUrl", factory["url"]), "fetchedAt": utc(factory["fetchedAtUTC"]),
        "bytes": factory["bytes"], "sha256": factory["sha256"], "archiveName": factory_name,
        "notes": "Downloaded original image verified by hash/checksum/digest. Binary retained outside Git; no local public mirror or successful factory-boot claim. " + factory["validation"]["note"]})
    assets.append({"id": "factory", "sourceId": "factory", "role": "firmware-download", "url": factory["url"]})
    source_map = {s["id"]: s for s in sources}
    assert len(source_map) == len(sources), "Source aliases must be unique"
    def public(sid):
        s = source_map[sid]
        return {"id": sid, "title": s["title"], "url": s["url"], "checkedAt": s["fetchedAt"][:10]}

    product = {"id": pid, "name": patch["name"], "manufacturer": "Waveshare",
        "sku": patch["skus"]["base"], "hardwareRevision": fields.get("hardwareRevision", "unspecified"),
        "chip": "esp32c5", "summary": fields["summary"], "knowledgeUrl": "./knowledge/" + pid + ".json",
        "family": {"id": board, "name": fields["familyName"]},
        "runtime": {"board": board, "flashMb": patch["flashMB"], "psramMb": patch["psramMB"],
            "status": "partial", "touch": patch["hasTouch"]},
        "displayStatus": "known", "display": fields["display"], "controls": fields["controls"],
        "interfaces": fields["interfaces"], "sources": [public(s) for s in fields["publicSourceIds"]],
        "limitations": fields["limitations"],
        "chronology": {"date": None, "basis": "unknown", "sourceId": None, "sourceUrl": None,
            "confidence": "unknown", "notes": "固件／文档日期不等同于产品发布日期。"},
        "defaultFirmwareId": board + "-factory"}
    assert [product["display"]["width"], product["display"]["height"]] == patch["resolution"]
    assert [c["id"] for c in product["controls"]] == patch["buttonIds"]
    assert any(i["type"] == "microSD" for i in product["interfaces"]) == patch["hasTfSlot"]
    for interface in product["interfaces"]:
        assert isinstance(interface["external"], bool) and isinstance(interface["pins"], list)
    facts = copy.deepcopy(fields["facts"])
    facts.append({"id": "factory", "label": "原厂镜像及独立布局", "basis": "official", "sourceIds": ["factory"],
        "value": {"bytes": factory["bytes"], "sha256": factory["sha256"], "loadOffset": factory["loadOffset"],
            "bootloaderOffset": factory["bootloaderOffset"], "partitionTableOffset": factory["partitionTableOffset"],
            "applicationOffsets": factory["applicationOffsets"], "chipId": factory["validation"]["chipId"],
            "minimumRevision": factory["validation"]["minRevision"], "maximumRevision": factory["validation"]["maxRevision"],
            "imageFlashMiB": factory["validation"]["imageFlashMB"]}})
    unknowns = [{"id": "release-date", "label": "产品发布日期", "value": None, "basis": "unknown", "sourceIds": ["overview"]},
        {"id": "uiflow", "label": "精确型号的 UIFlow 镜像", "value": None, "basis": "unknown", "sourceIds": ["readme", "resources"],
            "notes": patch["officialUiflowInvestigation"]["result"] + "; " + patch["officialUiflowInvestigation"]["scope"]}]
    for fact in facts + unknowns:
        assert all(sid in source_map for sid in fact["sourceIds"]), fact["id"]
    knowledge = {"schemaVersion": "1", "id": pid, "kind": "product", "catalogId": pid,
        "title": product["name"], "summary": product["summary"], "summarySourceIds": fields["summarySourceIds"],
        "updatedAt": timestamp, "hardwareRevision": product["hardwareRevision"], "facts": facts,
        "revisions": [], "sources": sources, "assets": assets, "unknowns": unknowns,
        "collectionNotes": fields["limitations"] + patch["dataConflicts"] + fields.get("collectionNotes", []) +
            ["原生证据：" + patch["runtimeEvidence"], "源资料收据：" + patch["sourceReceipt"]]}
    # Preserve separately reviewed geometry already integrated by the model lane.
    # Missing models create no negative/pending statement and no invented binding.
    preserved = []
    if existing_root:
        catalog = existing_root / "web/platform/catalog/products.json"
        old_product = next((p for p in read(catalog)["items"] if p["id"] == pid), None)
        if old_product:
            for key in ("model", "dimensions"):
                if key in old_product:
                    product[key] = old_product[key]
                    preserved.append("product." + key)
        old_path = existing_root / "web/platform/knowledge" / (pid + ".json")
        if old_path.exists():
            old = read(old_path)
            for key in ("facts", "sources", "assets"):
                rows = [x for x in old[key] if x["id"].startswith("model-")]
                knowledge[key].extend(rows)
                if rows:
                    preserved.append("knowledge." + key + ":model-*")

    diagnostic = patch["projectDiagnostic"]
    evidence = read(root / patch["runtimeEvidence"])
    app = root / diagnostic["files"]["app"]
    assert len(app.read_bytes()) == diagnostic["appBytes"] and sha(app.read_bytes()) == diagnostic["appSha256"]
    assert evidence["board"] == board and evidence["inputs"]["app"]["sha256"] == diagnostic["appSha256"]
    assert evidence["execution"]["exceptions"] == 0 and evidence["execution"]["resets"] == 0
    binary_url = "./firmware/" + board + "-probe.bin"
    firmware_entries = [
        {"id": board + "-factory", "name": fields["shortName"] + " · 官方出厂镜像（启动支持开发中）",
            "productIds": [pid], "version": factory_version, "status": "reference", "source": None,
            "role": "factory", "publisher": "Waveshare", "publisherGroup": "官方",
            "simulationVerified": False, "hardwareValidated": False, "sources": [public("factory")],
            "limitations": fields["factoryLimitations"]},
        {"id": board + "-sram-diagnostic", "name": fields["diagnosticTitle"],
            "productIds": [pid], "version": evidence["recordCreatedAtUTC"][:10].replace("-", ""),
            "status": "trial", "source": {"url": binary_url, "sha256": diagnostic["appSha256"], "bootMode": "app-direct"},
            "role": "example", "publisher": "esp-online-tester", "publisherGroup": "项目诊断",
            "simulationVerified": True, "hardwareValidated": False,
            "sources": [{"title": "本项目原创诊断；native 验证范围见单独证据", "url": "https://github.com/zhuzhe1983/esp-online-tester", "checkedAt": evidence["recordCreatedAtUTC"][:10]}, public("schematic"), public("bsp")],
            "limitations": fields["diagnosticLimitations"]}]
    compatibility = {"status": "reference-only", "productIds": [pid], "simulationVerified": False,
        "hardwareValidated": False, "note": "原厂启动尚未完成；项目原创诊断独立列出。"}
    watch = [{"id": board + "-official", "name": patch["name"] + " official", "category": "factory",
        "upstreamRepo": manifest["repository"], "metadataUrl": "https://api.github.com/repos/" + manifest["repository"] + "/releases/latest",
        "releaseUrl": "https://github.com/" + manifest["repository"] + "/releases", "version": factory_version,
        "checkedAt": utc(factory["fetchedAtUTC"]),
        "assets": [{"name": factory_name, "url": factory["url"], "sha256": factory["sha256"], "bytes": factory["bytes"], "format": "merged"}],
        "compatibility": compatibility, "storage": "remote-links-only", "catalogTargets": [],
        "monitoringNote": "已下载的是固定仓库内镜像；release feed 仅作后续观察入口，未声称存在同名 release asset。无自动替换目标。"}]
    for related in patch.get("relatedFirmware", []):
        assert related["role"] == "xiaozhi" and not related["simulationVerified"]
        watch_id = "xiaozhi-" + board
        firmware_entries.append({"id": board + "-xiaozhi-reference", "name": fields["shortName"] + " · 小智 " + related["version"] + " 发布参考",
            "productIds": [pid], "version": related["version"], "status": "reference", "source": None, "role": "alternative",
            "publisher": "78/xiaozhi-esp32", "publisherGroup": "开源项目", "simulationVerified": False,
            "hardwareValidated": False, "publicationUrl": related["releaseUrl"], "watchlistId": watch_id,
            "sources": [{"id": "xiaozhi", "title": related["assetName"], "url": related["url"], "checkedAt": related["selectionCreatedAtUTC"][:10]}],
            "limitations": ["精确型号 ZIP 来自上游发布记录；本批未下载、未执行。publishedDigest 不冒充实际下载 SHA。"]})
        watch.append({"id": watch_id, "name": patch["name"] + " xiaozhi", "category": "voice", "upstreamRepo": "78/xiaozhi-esp32",
            "metadataUrl": related["metadataSource"], "releaseUrl": related["releaseUrl"], "version": related["version"],
            "checkedAt": utc(related["selectionCreatedAtUTC"]),
            "assets": [{"name": related["assetName"], "url": related["url"], "bytes": related["publishedBytes"],
                "publishedDigest": related["publishedDigest"], "format": "zip"}],
            "compatibility": compatibility, "storage": "remote-links-only", "catalogTargets": [],
            "monitoringNote": "独立第三方项目；仅精确 target，未下载运行。"})
    internal_fixtures = [f for f in firmware_entries if f["publisher"] == "esp-online-tester"]
    public_firmwares = [f for f in firmware_entries if f["publisher"] != "esp-online-tester"]
    return {"product": product, "knowledge": knowledge, "firmwares": public_firmwares, "internal-fixtures": internal_fixtures, "watchlist": watch,
        "index-entry": {"id": pid, "kind": "product", "catalogId": pid, "name": product["name"], "url": "./" + pid + ".json", "updatedAt": timestamp},
        "integration": {"schemaVersion": "1", "productId": pid, "generatedAt": timestamp,
            "sharedCatalogsModified": False, "modelFieldsPreserved": preserved, "omittedSourceEntries": omitted,
            "binaryCopies": [{"from": diagnostic["files"]["app"], "to": "web/platform/" + binary_url.removeprefix("./"), "bytes": diagnostic["appBytes"], "sha256": diagnostic["appSha256"]}],
            "mergeNotes": ["Upsert exact IDs; preserve independently reviewed model, dimension and model-* source/fact fields.",
                "Native acceptance does not establish public Worker/browser acceptance. Update those claims only from subsequent execution.",
                "Central CLI/WASM/JS routes and exact capacities need separate integration; this generator does not modify runtime code."]}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("patch", type=Path)
    parser.add_argument("--fields", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--existing-root", type=Path)
    parser.add_argument("--at", help="UTC generation timestamp; must not precede source retrieval")
    args = parser.parse_args()
    timestamp = utc(args.at or datetime.datetime.now(datetime.timezone.utc).isoformat())
    result = generate(args.root.resolve(), read(args.patch), read(args.fields), timestamp,
        args.existing_root.resolve() if args.existing_root else None)
    args.output.mkdir(parents=True, exist_ok=True)
    for name, value in result.items():
        (args.output / (name + ".json")).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    print("Generated", result["product"]["id"], "into", args.output, "without changing shared catalogs.")


if __name__ == "__main__":
    main()
