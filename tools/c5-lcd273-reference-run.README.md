# LCD273 官方源码例 reference runtime helper

此工具只观察固定厂家源码例 `03_sd_card`（`--kind sd`）或 `02_lvgl_demo`
（`--kind display`）的独立构建。`originalFactoryAccepted` **始终为 false**。
原 `FactoryOnly_20260724` 原字节、其 PINNED 和验收门槛仍由原
`c5-lcd273-acceptance.mjs` 管理；这里拒绝该 FactoryOnly 的已知 SHA，不修改
或调用它的运行流程，只复用其纯函数 `c5CounterDelta`。

固定来源为 [厂家 418de818](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/tree/418de818f4f40c5a04c4292dbe293bfaa1351068/example/ESP-IDF-V553)。
预期 SDK 是 ESP-IDF `5.5.3@2c211b236707889e8400c4dc5644dd5c4ee071e0`。

| kind | 厂家项目/tree | main.c SHA-256 |
| --- | --- | --- |
| sd | `03_sd_card` / `f3a52a48572bffab114f5ce2065ab2fb83c7a7a2` | `8c4b53d535a2714c2a82d77eef2133417a99bbc43abc452380e6eae68d021a49` |
| display | `02_lvgl_demo` / `6d9c1ecd1b899ab2e545fa48bfb002f8c6d49bdf` | `d7cdaa690e35557ef7f5ee3cb9c25ab4b51f60440f5c109942168dcb7784f3e1` |

两个项目的 CMake 名字都为 `01_factory`；名字、descriptor 或相同 BSP 名称
不能证明它们是原 FactoryOnly，也不能区分这两个源码例。所有输入摘要由
调用方独立提供，helper 校验实际字节并保留 runtime 的真实加载 receipt、
Flash layout 和 metadata。可附带构建来源 receipt 的 path/hash；其内容不会
复制进结果。helper 不从二进制 hash 单独推导源码或 SDK 未改，
`binaryBuildProvenanceVerified:false` 明确保留这个边界；构建来源需与独立
build receipt 一起审查。

## 运行

Node.js 20+。`--help` 和纯导入不读输入、不创建 runtime/worker/WASM。
`--check-inputs` 只读/hash 输入和解析输入盘，不载入/启动 guest，不验证运行
ABI。只有 `--run` 才通过生产 `createPlatformRuntime` 和真实
`web/wasm/worker.js` 执行 caller 指定的生产 WASM。

```sh
node tools/c5-lcd273-reference-run.mjs --help

node tools/c5-lcd273-reference-run.mjs --check-inputs --kind sd \
  --runtime-root "$C5_RUNTIME_ROOT" --out "$C5_SD_INPUT_CHECK_OUTPUT" \
  --wasm "$C5_PRODUCTION_WASM" --wasm-sha256 "$C5_PRODUCTION_WASM_SHA256" \
  --rom "$C5_REV100_ROM" --rom-sha256 "$C5_REV100_ROM_SHA256" \
  --merged "$C5_SD_REFERENCE_MERGED" --merged-sha256 "$C5_SD_REFERENCE_MERGED_SHA256" \
  --sd "$C5_ORIGINAL_SD" --sd-sha256 "$C5_ORIGINAL_SD_SHA256"

node tools/c5-lcd273-reference-run.mjs --run --kind sd \
  --runtime-root "$C5_RUNTIME_ROOT" --out "$C5_SD_REFERENCE_OUTPUT" \
  --wasm "$C5_PRODUCTION_WASM" --wasm-sha256 "$C5_PRODUCTION_WASM_SHA256" \
  --rom "$C5_REV100_ROM" --rom-sha256 "$C5_REV100_ROM_SHA256" \
  --merged "$C5_SD_REFERENCE_MERGED" --merged-sha256 "$C5_SD_REFERENCE_MERGED_SHA256" \
  --sd "$C5_ORIGINAL_SD" --sd-sha256 "$C5_ORIGINAL_SD_SHA256" \
  --guest-seconds 15 --wall-timeout-ms 180000
```

display 使用 `--kind display`、02 的独立 merged/hash 和新的 out。可附加
`--build-receipt <path-or-url> --build-receipt-sha256 <hash>`，两者须成对。
WASM、ROM、merged 和 SD 均接受 caller 文件或 HTTP(S) URL；没有默认私有
主机或远端 emulator。ROM 必须符合生产 runtime 的 MP rev100 固定摘要
`49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a`。

out 必须不存在，建议位于忽略的 `output/`。原始 64 MiB SD 只在 host 读取，
传入独立 memory storage 的副本。**03 的原样 `wb` 会覆盖克隆卡中的
`/test.txt`**；不要把 master 卡当作运行输出。原卡 hash 和原有 test.txt 的
状态会保留，`sd-after.img` 永远新建，不覆盖原卡；无 host format/clear。

## 证据与判断

加载必须是独立 LCD273 C5 32 MiB Flash / 8 MiB PSRAM、interpreter、
`appDirect:false`，一次创建，ROM kind0 后 merged kind5，实际加载字节匹配
输入 pin。worker `clockHz` 为 240 MHz nominal counter；48 MHz XTAL 是
独立 runtime 要求，创建回执没有 live XTAL 字段。拒绝 stubs、实验开关、
app-direct 和意外资源 fallback，不修改 SDK、固件、内核或原卡。

`referenceObserved:true` 的含义仅限以下组合事实：

- sd：真实启动回执和 ABI 成立；新 UART 出现 app_main，依原源码顺序出现
  mount/start/path、71B write、71B read、原 read-content 和 PASS；没有该例
  failure 或 panic，完整 UART 未超限。暂停后从真实后端导出盘，独立只读
  解析 FAT32 根目录 `/test.txt`，长度和内容精确吻合下面的 71B payload；
  同 worker/card epoch 的真实 store-load 和 store-write 增量均大于零。
- display：真实 app_main、新 worker framebuffer 含非黑像素和至少两种 RGB
  颜色，实际 LCD reset/sleep-out/on/backlight 可见且 pixels/RAMWR 计数非零。
  最多保留三份真实 `.rgba`；早期 blank 不会占满所有有效帧名额。02 原样
  调用原 benchmark demo；固定配置关闭 LV_USE_PERF_MONITOR，实际只显示配置提示页，不能称为运行中的 benchmark。本工具不计算 FPS/吞吐或给出速度结论，不注入按键，
  `buttonResponseVerified:false`。

03 payload（CRLF，71B，不含终止 NUL）：

```text
ESP32-C5 TF card read/write test\r\n
Write and read verification passed.\r\n
```

SHA-256 为 `b0944679cf35c8ac6566d654a769567cc9292e0812cdf471e4cae1f2cd45a91c`。
源码在 `fwrite`、`fclose`、`fread`、长度/错误及 `memcmp` 检查之后才打印 PASS。
FAT32 inspector 仅支持 MBR 中唯一 FAT32 分区或 sector0 FAT32 BPB，只读查询
根目录 `test.txt`；有界检查 FAT、cluster chain、重复目标、文件大小和内容，
不列出其它私有文件，也不模拟 guest 文件系统。遇不支持/坏结构保留明确错误
和完整导出盘，不能泛称 guest mount 成功。

UART mount 日志是 guest 报告；FAT32 inspector 是 host 导出盘检查，两者分列。
`nativeCommandCalls` 不含所有 SPI 特判 CMD；`storageBlocksLoaded` 早于完整
reply/MISO delivery；`storageBlocksWritten` 不证明 ACK 消费或文件 flush。
所有真实 u64 字符串和完整冷 snapshot 保留，差值复用已有 helper，只有同
worker/card 与真实 publication 绑定时计算。新 attach 仅在成功回执后更新
epoch；缺失快照不补零。FATFS 缓存仍可能服务写后读取，
`transportCompletionProven:false`。Node memory backend 没有浏览器 IndexedDB
或跨进程 durability 证据；本工具不做 reset/reload 持久化验收。

结果将 input/load/ABI 失败与执行阻塞、UART 分支、真实 frame、SD 导出/FAT
事实分开。退出 0 为 `reference-observed` 或 `inputs-checked`，1 为输入/
加载/ABI/清理错误，2 为 blocked，3 为 partial。执行后续出错时仍保留已观察
到的局部事实，但不会给出完整 reference status。无论结果如何，
`originalFactoryAccepted:false`，物理硬件兼容、原厂完整功能与浏览器持久化
不继承 reference 结果。

所有 task-owned workers 在 finally 中处置、等待退出并记录剩余数；不用
browser、native emulator 或额外服务。`uart0.log`、`emulator.log`、RGBA 和
SD 导出盘是私有原始证据，留在忽略目录，不加入 Git 或公开发布。

## 无 guest 的检查

```sh
node --check tools/c5-lcd273-reference-run.mjs
node tools/c5-lcd273-reference-run.mjs --help
node --input-type=module -e "await import('./tools/c5-lcd273-reference-run.mjs')"
node --test tools/c5-lcd273-reference-run-test.mjs
```

测试仅用内存 UART/FAT32 fixtures 验证顺序分支、payload identity、只读 FAT
解析与循环/截断/重复目标拒绝、精度和 reference 判定，不创建 runtime、
worker、WASM 或浏览器。生产 reference 实跑由集成方统一执行。

初始提交 `3b23235e` 的短文件名识别使用 `Buffer.toString('ascii')`，会剥掉
输入字节的最高位，例如把 `D4` 错认成 `54`（T），造成伪目标匹配。这是已
保留的审查负结果。后继修复改为 11 个原始字节逐一比较，只将明确的
`61..7A`（ASCII 小写）映射到大写，任何高位字节均不得匹配。纯 fixture
覆盖根目录首字节及其它短名位置设置高位后的拒绝、跨 root/file cluster 的
精确读取、跨 root cluster 的重复名拒绝，并保留循环及截断 chain 负例；
没有修改真实输入镜像。
