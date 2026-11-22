#!/usr/bin/env python3
"""Write the current LilyGO inventory report from reviewed integration inputs."""
from collections import Counter
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/platform/research'
WEB = ROOT / 'web/platform'


def read(path):
    return json.loads(path.read_text())


products = json.loads(subprocess.run(['node', '--input-type=module', '-e',
    "import fs from 'node:fs';import {compareProductChronology} from './web/platform/catalog-selection.js';"
    "const p=JSON.parse(fs.readFileSync('web/platform/catalog/products.json')).items;"
    "console.log(JSON.stringify(p.filter(x=>x.manufacturer==='LILYGO').sort(compareProductChronology)));"],
    cwd=ROOT, text=True, capture_output=True, check=True).stdout)
profiles = {}
profile_documents = []
for path in sorted(RESEARCH.glob('lilygo-*-profiles-2026-11-22.json')):
    document = read(path)
    profile_documents.append(path.name)
    for item in document['items']:
        assert item['productId'] not in profiles
        profiles[item['productId']] = item
        profiles[item['productId']]['reportSource'] = path.name
model_report = read(RESEARCH / 'lilygo-model-expansion-2026-11-22.json')
models = {item['id']: item for item in model_report['items']}
verification_path = RESEARCH / 'lilygo-integration-verification-2026-11-22.json'
verification = read(verification_path) if verification_path.exists() else {'status': 'pending', 'checks': []}
runtime_count = sum(bool(product['runtime']['board']) for product in products)
new_count = sum(product['runtime'].get('status') == 'diagnostic' for product in products)
model_counts = Counter(product['model'].get('mode', product['model'].get('basis', product['model']['type'])) for product in products)
inline_c5 = {'lilygo-t7-c5', 'lilygo-t-dongle-c5'}
assert all(product['id'] in profiles or product['id'] in inline_c5
           for product in products if product['runtime'].get('status') == 'diagnostic')
chip_counts = Counter(profiles[product['id']]['chip'] if product['id'] in profiles else 'esp32c5'
    for product in products if product['runtime'].get('status') == 'diagnostic')
chronology_counts = Counter(product.get('chronology', {}).get('basis') if product.get('chronology', {}).get('date') else 'unknown' for product in products)

def cell(text):
    return str(text).replace('|', '\\|').replace('\n', ' ')


def display_text(product):
    display = product.get('display')
    if not display:
        return '无内置屏幕' if product.get('displayStatus') == 'none' else '未确认'
    text = f"{display['width']}×{display['height']} {display['controller']}"
    if display.get('physicalPanels'):
        text += f"；{len(display['physicalPanels'])} 片合成调试画布"
    profile = profiles.get(product['id'])
    if profile and (not profile.get('display') or profile['display'].get('implemented') is False):
        text += '（实体屏幕预览，尚无帧输出）'
    return text


lines = [
    '# LILYGO 全目录适配记录 · 2026-11-22', '',
    f'本轮处理现有 LILYGO 类目的 **{len(products)} 个商品条目**：保留原有 5 个部分支持的运行器，新增 **{new_count} 个有明确范围的诊断配置**，当前 **{runtime_count} 个条目可进入芯片／板级调试**。全部条目都有可检查的 3D 预览。这里的范围是项目已经收录的 Spark 商品 ID，不代表厂家所有历史硬件选项。', '',
    '**诊断适配的含义：** 执行真实 CPU 指令，按所选硬件资料接入已经实现的外设路径；它不表示任意出厂固件、全部通信功能或实物电气行为已经验证。每款的所选版次、引脚、未实现功能、来源冲突和测试范围保存在独立知识页中。', '',
    '## 发布时间顺序', '',
    '工作台、搜索和下表共用 `compareProductChronology`：已确认的官方硬件版次／公告日期优先采用；缺少首发资料时，明确标注官方上架时间作为排序代用。日期不明的设备置后。同名不同预装软件条目保留原 ID；仓库最近提交时间不被当成产品发布日。', '',
    '下表日期标记：**硬件**＝官方硬件版次日期，**公告**＝官方公告，**上架代用**＝官方商品页发布时间，**仓库代用**＝最早官方记录。硬件版次日期也不自动等于零售首发日。', '',
    f'当前日期依据：**{chronology_counts.get("hardware-release", 0)} 项硬件版次日期、{chronology_counts.get("storefront-publication", 0)} 项上架日期代用、{chronology_counts.get("unknown", 0)} 项未知**。上架日期可能反映重新上架或预装软件版本，不能据此确认首次零售发布日。', '',
    '## 运行与模型覆盖', '',
    '| 项目 | 当前范围 |', '| --- | --- |',
    f'| 原有运行器 | 5；原有固件目标与默认映射保留 |',
    f'| 新增诊断配置 | {new_count}；' + '、'.join(f'{chip}: {count}' for chip, count in sorted(chip_counts.items())) + ' |',
    f'| 新增模型 | {len(models)}；官方 CAD 部件 {model_report["summary"]["modeCounts"].get("manufacturer-cad", 0)}，官方照片轮廓模型 {model_report["summary"]["modeCounts"].get("photo-derived", 0)} |',
    '| 全目录模型 | 原有 4 个官方 CAD、1 个有限照片模型与本轮新增模型；均保留 partial 状态 |',
    '| 新增出厂固件兼容声明 | 0；本轮的协议／自编程序测试不提升为原厂固件验证 |',
    '| 实机／机械装配校准 | 未进行；CAD 导出精度与实物精度分别记录 |', '',
    '### 有意保留的边界', '',
    '- **T-Radar**：官方资料描述的是 A121 雷达扩展，需要外部主机；目录已纠正主控标签，保留该商品模型与资料，不为它虚构独立 ESP32-S3。',
    '- **S2 与 C5**：独立芯片后端的 SRAM app-direct 调试；普通 ROM、Flash/MMU、ESP-IDF／UIFlow 启动不在已实现范围内。S2 的 IRAM 和 DRAM 是同一 320 KiB SRAM 的别名，加载器拒绝物理重叠、越界和错误入口。',
    '- **P4**：所选 T-Display-P4 V1.0 的 GPIO、XL9535 I2C0 和原生 SD 路径；HI8561 屏幕、C6 协处理器和完整出厂启动没有被协议测试证明可用。',
    '- **多处理器和未知内存**：T-Pico 的 C6、T-BAO 的 ESP32、Watch Keyboard C3 的 MCU 子集与其他处理器／键盘／屏幕分开。模拟 Flash/PSRAM 容量不替代尚未知的物理容量。',
    '- **不同屏幕和版次**：T-Touch Bar 选择 H621 双 JD9613 AMOLED；T-Keyboard S3 Pro 选择所述 STM32 固件与 4 片 GC9107；T-Connect Pro 选择 LoRa+屏幕版；T-RGB 选择 2.1 英寸 Full Circle。各自资料中未选的硬件选项不继承适配声明。',
    '- **电子纸**：SSD1680、SSD1681、UC81xx、IL91874 和 ED047TC1 保留各自的显存、尺寸与刷新规则。原始 ED047TC1 屏幕当前为黑白数字子集，模拟电压、完整灰阶与实物刷新时间未验证。',
    '- **通信与电源**：已实现的数字 LoRa、SD 传输和按键状态单独声明；不能由主芯片可执行推断蜂窝网络、GNSS、RF、摄像头、模拟电池、USB 或音频已经可用。', '',
    '### 3D 来源选择', '',
    '优先使用匹配所选型号的官方 STEP/STL，保留原始文件哈希、固定 Git 版本、单位说明和刚体变换。外壳零件、PCB、支架与分散打印布局按实际内容标注；不把部件边界写成已测整机尺寸。STL 本身不编码单位，未标定的毫米解释保持明确。', '',
    '没有确认适合所选版次的 CAD 时，使用保留的官方主要视图建立投影轮廓粗模，其他角度用于比对；个别商品只有一张正反面合成图。照片模型不宣称为摄影测量重建：厚度、绝对尺度、被遮挡结构和屏幕位置仍可能未知。实时屏幕与尚未校准位置的按钮使用独立标记的调试面板；照片中的演示画面不会伪装成固件输出。', '',
    '官方仓库候选也逐个区分：T-Circle 的 `.OBJ` 是编译器目标文件，KiCad 封装不等于整机模型，其他尺寸的 T-RGB、旧版 T-Embed、TWR Plus 外壳不能自动用于当前变体。详见 [CAD 选择审阅](research/lilygo-cad-selection-review-2026-11-22.json)、[92 个仓库快照](research/lilygo-cad-discovery-2026-11-22.json)及[模型导出报告](research/lilygo-model-expansion-2026-11-22.json)。', '',
    '## 全部 127 项，按当前日期顺序', '',
    '“诊断”只表示本表所链接资料中已实现的范围；“原有部分支持”沿用之前单独记录的验证。屏幕尺寸是所选运行画布或明确标注的实体预览，不是其他硬件选项的兼容承诺。', '',
    '| # | 设备／逐款资料 | 日期及依据 | 运行配置 | 屏幕／调试画布 | 模型来源 |',
    '| ---: | --- | --- | --- | --- | --- |',
]
basis_names = {'hardware-release': '硬件', 'official-announcement': '公告', 'storefront-publication': '上架代用', 'repository-first-seen': '仓库代用'}
inventory = []
for index, product in enumerate(products, 1):
    pid = product['id']
    chronology = product.get('chronology', {})
    date = (chronology.get('date') or '未知') + (' · ' + basis_names.get(chronology.get('basis'), '资料') if chronology.get('date') else '')
    runtime = product['runtime']
    runtime_label = '诊断' if runtime.get('status') == 'diagnostic' else '原有部分支持' if runtime.get('board') else '扩展模块模型／资料' if pid == 'lilygo-t-radar' else '模型／资料；运行待补'
    if runtime.get('board'):
        runtime_label += ' · ' + (profiles[pid]['chip'] if pid in profiles else product['chip'])
    mode = product['model'].get('mode', product['model'].get('basis', product['model']['type']))
    model_label = {'manufacturer-cad': '官方 CAD 部件', 'photo-derived': '照片轮廓粗模', 'photo-proxy': '原有有限照片模型'}.get(mode, mode)
    lines.append(f'| {index} | [{cell(product["name"])}](../../web/platform/knowledge/{pid}.json) | {cell(date)} | {cell(runtime_label)} | {cell(display_text(product))} | [{model_label}](../../web/platform/assets/{pid}/model.json) |')
    inventory.append({'id': pid, 'rank': index, 'chronology': chronology, 'runtime': runtime,
                      'modelMode': mode, 'selectedHardware': product['hardwareRevision'],
                      'profileSource': profiles.get(pid, {}).get('reportSource') or ('lilygo-c5-sources-2026-11-22.json' if pid in inline_c5 else None),
                      **({'profileGenerator': 'tools/apply-lilygo-adaptations.py:c5_profiles'} if pid in inline_c5 else {}),
                      'knowledge': f'web/platform/knowledge/{pid}.json'})
lines += ['', '## 验证与发布', '',
          '分开记录原生 CPU／外设测试、生产 WASM、公共 Worker 输入、目录／模型合约、真实 Chromium 渲染与性能比较。测试只证明所运行的程序和路径；新条目不会因此获得“出厂固件已验证”标记。', '',
          '| 检查 | 结果 |', '| --- | --- |']
for check in verification.get('checks', []):
    lines.append(f'| {cell(check["name"])} | {cell(check["status"])} · {cell(check.get("detail", ""))} |')
if not verification.get('checks'):
    lines.append('| 最终集成检查 | 待完成；各板型阶段验证见其 profile 来源文件 |')
lines += ['', '实际命令、产物哈希、性能条件和发布状态见 [集成验证回执](research/lilygo-integration-verification-2026-11-22.json)。', '',
          '### 可复现维护入口', '', '```sh',
          'python3 tools/apply-lilygo-adaptations.py',
          'python3 tools/sync-lilygo-model-runtime-hints.py',
          'python3 tools/build-lilygo-expansion-models.py --output web/platform/assets',
          'python3 tools/apply-lilygo-adaptations.py',
          'python3 tools/report-lilygo-expansion-models.py',
          'python3 tools/report-lilygo-adaptations.py',
          'python3 tools/validate-platform-catalog.py',
          'node tools/verify-lilygo-expansion-models.mjs',
          'node --test tools/platform-*-test.mjs', '```', '',
          '模型重建的本次已验证依赖记录在 `tools/requirements-lilygo-expansion-models.txt`，逐文件重现结果见 [重现回执](research/lilygo-model-reproduction-2026-11-22.json)；以上流程不抓取或重分发新的出厂固件。最终 Rust、WASM、性能与浏览器命令以集成验证回执为准。', '']
(ROOT / 'docs/platform/lilygo-adaptation-2026-11-22.md').write_text('\n'.join(lines))
(RESEARCH / 'lilygo-adaptation-coverage-2026-11-22.json').write_text(json.dumps({
    'schemaVersion': 1, 'evidenceDate': '2026-11-22', 'catalogCount': len(products),
    'runtimeCount': runtime_count, 'newDiagnosticCount': new_count,
    'preservedRuntimeCount': runtime_count-new_count, 'newRuntimeChips': dict(chip_counts),
    'chronologyBasisCounts': dict(chronology_counts),
    'allModelModes': dict(model_counts), 'newModelSummary': model_report['summary'],
    'factoryCompatibilityNewlyVerified': 0, 'physicalHardwareVerified': False,
    'verificationStatus': verification['status'], 'items': inventory}, ensure_ascii=False, indent=2) + '\n')
print(f'Reported {len(products)} products, {runtime_count} runtimes and {len(models)} new models.')
