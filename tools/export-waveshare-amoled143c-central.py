#!/usr/bin/env python3
"""Export reviewed 1.43C entries; never mutate the shared platform catalogs."""
import copy
import datetime
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ARCHIVE = "docs/research/waveshare-amoled143c-sources"
manifest = json.loads((ROOT / ARCHIVE / "manifest.json").read_text())
patch = json.loads((ROOT / "docs/research/waveshare-amoled143c-runtime-patch.json").read_text())
now = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
base_id = "waveshare-esp32-s3-touch-amoled-1-43c"
firmware_id = base_id + "-factory"
watch_id = base_id + "-official"
fw = manifest["firmwares"][0]["receipt"]
records = {s["receipt"]["url"]: s for s in manifest["sources"]}
prefix = "https://raw.githubusercontent.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.43C/8cea11ddae6167dd591d1f5465fd7b615080360f/"
bsp = "examples/factory/components/esp32_s3_touch_amoled_1_43c/"
source_specs = [
    ("overview", "官方精确型号、SKU 与包装关系", "https://docs.waveshare.com/ESP32-S3-Touch-AMOLED-1.43C", "official-reference"),
    ("resources", "官方资源目录", "https://docs.waveshare.com/ESP32-S3-Touch-AMOLED-1.43C/Resources-And-Documents", "official-reference"),
    ("schematic", "精确板电路及 PCB 正反面", prefix + "hardware/schematic/ESP32-S3-Touch-AMOLED-1.43C-Schematic.pdf", "official-schematic"),
    ("bsp-header", "官方 QSPI、I2C 和按键引脚", prefix + bsp + "include/bsp/esp32_s3_touch_amoled_1_43c.h", "official-source"),
    ("lcd-driver", "官方显示窗口、方向与启动亮度", prefix + bsp + "esp32_s3_touch_amoled_1_43c.cpp", "official-source"),
    ("touch-driver", "CST820 原厂报告解析与复位", prefix + bsp + "esp_lcd_touch_cst820.cpp", "official-source"),
    ("factory-guide", "原厂九应用操作与 BOOT 退出说明", prefix + "examples/factory/README.md", "official-reference"),
    ("factory-readme", "原始出厂恢复镜像与地址", prefix + "firmware/README.md", "official-firmware"),
]


def source_record(spec):
    identity, title, url, kind = spec
    row = records[url]
    receipt = row["receipt"]
    fetched = receipt.get("completedAtUTC", receipt.get("fetchedAt"))
    return {
        "id": identity, "title": title, "url": url, "kind": kind, "status": "fetched",
        "resolvedUrl": receipt.get("finalUrl", receipt.get("resolvedUrl", url)),
        "fetchedAt": datetime.datetime.fromisoformat(fetched.replace("Z", "+00:00")).isoformat(timespec="seconds").replace("+00:00", "Z"), "bytes": row["bytes"],
        "sha256": row["sha256"], "archiveName": Path(row["archive"]).name.removesuffix(".gz"),
        "notes": f"原始响应保留于 {ARCHIVE}/{row['archive']}；bytes/SHA-256 对应解压后的原始字节，不是 gzip 包装。",
    }


sources = [source_record(s) for s in source_specs]
sources.append({
    "id": "factory-binary", "title": "固定提交的未修改原厂镜像", "url": fw["url"],
    "kind": "official-firmware", "status": "fetched", "resolvedUrl": fw["finalUrl"],
    "fetchedAt": datetime.datetime.fromisoformat(fw["completedAtUTC"].replace("Z", "+00:00")).isoformat(timespec="seconds").replace("+00:00", "Z"), "bytes": fw["bytes"],
    "sha256": fw["sha256"], "archiveName": Path(fw["path"]).name,
    "notes": "原始 BIN 仅用于本地验收并保留下载收据；仓库采用 remote-links-only，不含此 BIN。完整合并镜像从 0x0 经原始 ROM 启动。",
})
catalog_sources = [{"id": s["id"], "title": s["title"], "url": s["url"], "checkedAt": s["fetchedAt"][:10]} for s in sources]
chronology = {"date": None, "basis": "unknown", "sourceId": None, "sourceUrl": None,
              "confidence": "unknown", "notes": "20260420 和 20261027 是官方固件文件名日期；不是硬件发布日期，也不以仓库创建时间代替发布日期。"}
limitations = [
    "原生原厂验证：466×466 九应用桌面、CST820 实际画线与 BOOT 退出；11.22 guest 秒零重启、零 QSPI reject。",
    "同时间无绘图对照：画线新增 2493 红像素／总差 2710；BOOT 退出恢复原桌面差 0；静止桌面差 0。",
    "公开 Worker／生产 WASM、浏览器 WebGL 和实物验收尚未完成，原生通过不等于这些环节通过。",
    "保留原厂 Wi-Fi 校准与未连接告警；无线网络、Xiaozhi 在线服务、音频 PCM 和模拟电气未验收。",
    "ES8311／ES7210 仅数字控制子集；GPIO18 浮空按原理图 R18 上拉供电，模拟电源、充电与电池状态未实现。",
    "显示与触摸采用官方 BSP 数字坐标；USB 相对文字方位、光学颜色、触摸扫描周期未实物标定。",
    "TE 引脚已记载但没有生成波形；QPI、四线地址、SPI 读回和 MADCTL 位 5 坐标交换未实现。",
    "当前详细机械模型未在本批交付，默认只显示已知分辨率的屏幕调试视图。",
]


def fact(identity, label, value, refs, notes=None):
    row = {"id": identity, "label": label, "value": value, "basis": "official", "sourceIds": refs}
    if notes:
        row["notes"] = notes
    return row


products, knowledge, index = [], [], []
for item in patch["products"]:
    suffix = "-en" if item["name"].endswith("-EN") else ""
    identity = base_id + suffix
    summary = "1.43 英寸 466×466 圆形 AMOLED，8 MiB Flash / 8 MiB Octal PSRAM；原生原厂桌面、画线与 BOOT 返回已验证。"
    battery = item["packaging"]["batteryIncluded"]
    product = {
        "id": identity, "name": "Waveshare " + item["name"], "manufacturer": "Waveshare",
        "sku": item["sku"], "chip": "esp32s3", "hardwareRevision": "unspecified", "summary": summary,
        "knowledgeUrl": f"./knowledge/{identity}.json",
        "family": {"id": "waveshare-s3-qspi-amoled", "name": "Waveshare ESP32-S3 QSPI AMOLED"},
        "runtime": {"board": "waveshare-s3-touch-amoled143c", "flashMb": 8, "psramMb": 8, "status": "partial", "touch": True},
        "displayStatus": "known", "display": {"width": 466, "height": 466, "type": "AMOLED", "bus": "QSPI", "diagonalInches": 1.43,
                                              "touch": True, "controller": "CO5300", "shape": "circle"},
        "controls": copy.deepcopy(item["controls"]),
        "interfaces": [
            {"id": "usb-c", "name": "USB-C · 编程与供电", "type": "USB-C", "external": True,
             "pins": [{"name": "D-", "gpio": 19}, {"name": "D+", "gpio": 20}], "sourceIds": ["schematic"]},
            {"id": "i2c-pads", "name": "I2C 调试焊盘", "type": "I2C", "external": True,
             "pins": [{"name": "SDA", "gpio": 47}, {"name": "SCL", "gpio": 48}], "sourceIds": ["schematic"],
             "notes": "与板载触摸及音频共享电气总线；外接任意器件未验收。"},
            {"id": "uart-pads", "name": "UART0 调试焊盘", "type": "UART", "external": True,
             "pins": [{"name": "TX", "gpio": 43}, {"name": "RX", "gpio": 44}], "sourceIds": ["schematic"]},
        ],
        "sources": copy.deepcopy(catalog_sources), "limitations": limitations + ["包装包含锂电池；执行模型采用明确的 USB 供电数字场景。" if battery else "-EN 包装不含锂电池；执行模型采用明确的 USB 供电数字场景。"],
        "chronology": copy.deepcopy(chronology), "defaultFirmwareId": firmware_id,
    }
    if suffix:
        product["firmwareTargetId"] = base_id
    products.append(product)
    facts = [
        fact("identity", "精确型号、包装和存储", {"sku": item["sku"], "model": item["name"], "mcu": "ESP32-S3-PICO-1-N8R8", "flashMiB": 8, "psramMiB": 8, "psramBus": "octal", "batteryIncluded": battery}, ["overview", "schematic"]),
        fact("display", "屏幕与触摸", {"width": 466, "height": 466, "shape": "round", "controller": "CO5300", "touchController": "CST820", "bus": "QSPI", "runtimePixelFormat": "RGB565"}, ["overview", "lcd-driver", "touch-driver"]),
        fact("pins", "精确板载信号", item["pins"], ["schematic", "bsp-header"]),
        fact("tf-slot", "物理 TF 卡槽", False, ["schematic"], "原理图及 PCB 正反面没有 TF 插座；不要向此板自动配置 SD 卡。"),
        fact("codec-power", "音频数字电源", {"enableGpio": 18, "pullUpOhms": 10000, "pullUpReference": "R18", "ldo": "RT9193 U4"}, ["schematic"]),
        fact("factory", "原始出厂镜像", {"name": Path(fw["path"]).name, "bytes": fw["bytes"], "sha256": fw["sha256"], "offset": 0, "repositoryCommit": fw["commit"]}, ["factory-readme", "factory-binary"], "日期表示固件文件来源，不能作为硬件发布日期。"),
    ]
    unknowns = [
        {"id": "release-date", "label": "硬件发布日期", "value": None, "basis": "unknown", "sourceIds": ["overview"], "notes": chronology["notes"]},
        {"id": "pcb-revision", "label": "当前量产 PCB 版次", "value": None, "basis": "unknown", "sourceIds": ["schematic"], "notes": "已按精确命名原理图接线；未将 PDF 生成日期认证为量产版次。"},
        {"id": "physical-alignment", "label": "实物光学与 USB 方位校准", "value": None, "basis": "unknown", "sourceIds": ["lcd-driver", "schematic"], "notes": "已使用 BSP 数字方向，不声称实物坐标已标定。"},
        {"id": "mechanical-model", "label": "详细机械模型交付", "value": None, "basis": "unknown", "sourceIds": ["resources"], "notes": "本批仅交执行配置；未把未调查完整的官方 CAD 宣称为不存在。"},
    ]
    doc = {
        "schemaVersion": "1", "id": identity, "kind": "product", "catalogId": identity,
        "title": product["name"], "summary": summary, "summarySourceIds": ["overview", "schematic"],
        "updatedAt": now, "hardwareRevision": "unspecified", "facts": facts, "revisions": [],
        "sources": copy.deepcopy(sources),
        "assets": [{"id": "schematic", "sourceId": "schematic", "role": "schematic", "url": source_specs[2][2]}],
        "unknowns": unknowns,
        "collectionNotes": product["limitations"] + [
            "本次原生结果及逐文件原始 capture 哈希：docs/evidence/waveshare-amoled143c-2026-11-23/result.json 与 artifacts.json。",
            "原始 UART/PNG/RGB565 留 ignored output 归档；公开 Worker 验收未在此记录中发生。",
            "官方同一页面明确区分 34383 与 34384-EN 的电池包装；两者共享本板数字执行配置，不代表电池功能已模拟。",
        ],
    }
    knowledge.append(doc)
    index.append({"id": identity, "kind": "product", "catalogId": identity, "name": product["name"], "url": f"./{identity}.json", "updatedAt": now})

firmware = {
    "id": firmware_id, "name": "Waveshare ESP32-S3-Touch-AMOLED-1.43C · 官方原厂九应用桌面",
    "productIds": [p["id"] for p in products], "status": "trial",
    "source": {"url": fw["url"], "sha256": fw["sha256"], "bootMode": "rom"},
    "role": "factory", "publisher": "Waveshare", "publisherGroup": "官方",
    "version": Path(fw["path"]).stem, "simulationVerified": False, "nativeVerified": True,
    "publicWorkerVerified": False, "hardwareValidated": False, "storage": "remote-links-only", "compatibility": "partial",
    "publicationUrl": prefix + "firmware/README.md", "sources": catalog_sources,
    "limitations": limitations, "watchlistId": watch_id,
}
watch = {
    "id": watch_id, "name": "Waveshare ESP32-S3-Touch-AMOLED-1.43C 官方出厂固件", "category": "factory",
    "upstreamRepo": "waveshareteam/ESP32-S3-Touch-AMOLED-1.43C",
    "metadataUrl": "https://api.github.com/repos/waveshareteam/ESP32-S3-Touch-AMOLED-1.43C/releases/latest",
    "releaseUrl": "https://github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.43C/releases",
    "version": Path(fw["path"]).stem, "checkedAt": datetime.datetime.fromisoformat(fw["completedAtUTC"].replace("Z", "+00:00")).isoformat(timespec="seconds").replace("+00:00", "Z"),
    "assets": [{"name": Path(fw["path"]).name, "url": fw["url"], "sha256": fw["sha256"], "bytes": fw["bytes"], "format": "merged"}],
    "compatibility": {"status": "reference-only", "productIds": [p["id"] for p in products], "simulationVerified": False,
                      "hardwareValidated": False, "note": "原生原厂桌面、画线和 BOOT 已通过；公开 Worker 未验收。新 Release 不能继承这一固定镜像的验证。"},
    "storage": "remote-links-only", "catalogTargets": [],
    "monitoringNote": "保留已下载核验的固定原厂文件。此记录没有声称最新 Release 已执行，也不自动替换为不同应用／版本资产。",
}
bundle = {"schema": "waveshare-reviewed-central-entries-v1", "createdAtUTC": now,
          "boardCommit": "ba8caee5", "products": products, "knowledge": knowledge, "knowledgeIndexEntries": index,
          "firmwares": [firmware], "watchlistEntries": [watch]}
output = ROOT / "docs/research/waveshare-amoled143c-central-entries.json"
output.write_text(json.dumps(bundle, ensure_ascii=False, indent=2) + "\n")
print(output)
