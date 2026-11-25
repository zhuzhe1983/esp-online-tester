# LCD273 官方源码例运行证据

在实现版本 `99d166829e5835d274a8396bbf2f294b4baa0afd` 的生产 WASM interpreter 中，两个独立源码例均得到 `referenceObserved:true`。03 已从 ROM 启动到原 `app_main`，挂载克隆卡，写入并读回 71 字节，走到原程序的 `memcmp` 成功分支；导出 FAT32 镜像中的 `/test.txt` 长度及内容哈希也吻合。02 已输出实际 LCD 帧，但画面是 **“LV_USE_PERF_MONITOR is not enabled” 提示页**。这证明了该配置的文字、图形和显示链路输出；没有验收完整 benchmark UI、三键响应或速度。

原 `FactoryOnly` 的 `originalFactoryAccepted` 始终为 **false**。源码例与其独立，不能凭二者都叫 `01_factory`、descriptor 版本或相同 BSP 名称互认。所有数值、输入和原始输出的字节数及 SHA-256 见 [result.json](result.json)。本收据限定于这两次 Node reference 运行；真实浏览器的独立结果见[浏览器收据](../c5-lcd273-browser-2026-11-25/README.md)，CPU 和最终整合检查另行记录。

| 例程 | 实际结果 | 仍未证明 |
| --- | --- | --- |
| `03_sd_card` | UART 的原源码顺序标记及 PASS 均出现；导出盘 `/test.txt` 为 71B，SHA-256 `b0944679cf35c8ac6566d654a769567cc9292e0812cdf471e4cae1f2cd45a91c` | 每字节均新近经 MISO 接收、ACK 消费、flush、重启及浏览器持久化 |
| `02_lvgl_demo` | 320×320 实际帧，102135 个非黑像素、18 种 RGB 色；模型面板累计写像素计数 134400，RAMWR 10 次 | 运行中的 benchmark、FPS、吞吐、三键交互、实物电气和时序 |

SD 的 `48 / 8 / 6` 分别是 **published snapshot delta**：`nativeCommandCalls`、`storageBlocksLoaded`、`storageBlocksWritten`。它们只是在同 worker/card 条件下对已发布计数快照作差；不是全部 SPI 线命令数、完整传输次数、guest mount 或文件 flush 的计数。guest 原程序的写读校验与 host 对导出 FAT 文件的只读检查分列；`transportCompletionProven:false` 保留，FATFS 写后读缓存的可能性没有被排除。输入盘在 host 只读，guest 运行使用独立内存副本；03 会以 `wb` 覆盖副本的 `/test.txt`，不要把已有卡或唯一副本当作运行输出。

两个运行都没有 `failure` 字段，CLI 小结为 `failure:null`，最终 runtime `lastError:null`，helper 进程退出码为 0。清理记录为 `runtimeDisposed:true`、`activeWorkers:0`、`workersExited:true`，同时记录了 worker 的退出码 **1**。这是显式处置和 `Worker.terminate()` 清理路径记录的 worker 状态，不能把它改写成 guest 或 helper 失败，也不能把 1 藏掉。原始退出值和无 failure 的事实同时保留。

厂家源码固定在 [418de818](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/tree/418de818f4f40c5a04c4292dbe293bfaa1351068/example/ESP-IDF-V553)，真正的 root Git tree 为 `5282f0112760f1d53a79092548303df96602d4b8`。03/02 子树分别为 `f3a52a48572bffab114f5ce2065ab2fb83c7a7a2` / `6d9c1ecd1b899ab2e545fa48bfb002f8c6d49bdf`。独立构建使用 ESP-IDF `5.5.3@2c211b236707889e8400c4dc5644dd5c4ee071e0` 及推荐 GCC `esp-14.2.0_20251107`。已有 `docs/evidence/c5-lcd273-idf553-examples-2026-11-25/` 保存了原样 source/config/lock、工具、构建产物和静态拒绝检查；本次输入的 build receipt 字节与其中 03/02 收据吻合，merged 哈希也一致。**`binaryBuildProvenanceVerified:false` 不改为 true**：runtime helper 校验调用者 pin 与实际加载字节，不反向证明完整构建链；源码与产物的联系要联合独立静态证据审核。重新构建会受时间、路径及自动版本影响，不保证二进制逐字相同。

本次配置禁用 `CONFIG_LV_USE_SYSMON`，虽启用 benchmark demo 和 BSP 按键注册，实际仍是未启用性能监视器的提示页。注册 PREV/NEXT/ENTER 不能推导出已验证三键 UI；两次运行都没有注入按键，`buttonResponseVerified:false`。原始两份结果的 `limits[5]` 仍有旧句 “02 executes its unchanged benchmark UI”，这句对实测画面的描述错误；原结果没有重写，其哈希照录。实跑 helper 为 `7a23691ff707978feb4152afe21808c05eb293fb51f229a3ea6ca3169348c1dd`，后续仅纠正文案的快照为 `ad9e2f3701c22f2f418439dff088a39025076a87619bf12e9c6262a19eb892bf`。初始黑帧、未完成的交互/速度验收及已有静态构建负结果继续保留；没有虚构一次“三键测试失败”。

该实现版本的默认 release/fat-LTO 全 Rust 测试为 **3488 passed、0 failed、0 ignored、169 filtered**，原日志 247 段汇总独立相加吻合。完整日志为 392692B，SHA-256 `7c96a060b0fe24a92256bd81a46e420f7120c86aa3a0a5e9950888a199ebee0a`。408.659 秒是 build 加 tests 操作耗时，可能与其它正确性、浏览器及 input build 工作重叠，不能用作 CPU 性能证据。

重现时将下列 alias 设为调用者自己的文件路径。`C5_RUNTIME_ROOT` 指向对应实现 checkout；`C5_WASM`、`C5_ROM`、`C5_SD_MASTER` 为匹配收据哈希的生产 WASM、rev100 ROM 和 64MiB 原盘；`C5_SD_MERGED` / `C5_DISPLAY_MERGED` 与各自 build receipt 分开。两个 `*_RUN_OUT` 都必须尚不存在。以下是可重现的 alias 命令，不是含私有路径的原 argv 抄本；这些命令是重现模板。

```sh
cd "$C5_RUNTIME_ROOT"
node tools/c5-lcd273-reference-run.mjs --run --kind sd \
  --runtime-root "$C5_RUNTIME_ROOT" --out "$C5_SD_RUN_OUT" \
  --wasm "$C5_WASM" --wasm-sha256 752427fc94772b948e3320fa56ec187d502fc5c537c71d7de02d74955cd86997 \
  --rom "$C5_ROM" --rom-sha256 49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a \
  --merged "$C5_SD_MERGED" --merged-sha256 0f6d7646eaba88caab4708db719412a0b7b261d7cc18c8d601116a7c7c902308 \
  --sd "$C5_SD_MASTER" --sd-sha256 a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6 \
  --build-receipt "$C5_SD_BUILD_RECEIPT" --build-receipt-sha256 a3847a70fcb854c07216b904ce583f0270059e8c6cd105d35bf1c130af1f7238 \
  --guest-seconds 15 --wall-timeout-ms 180000

node tools/c5-lcd273-reference-run.mjs --run --kind display \
  --runtime-root "$C5_RUNTIME_ROOT" --out "$C5_DISPLAY_RUN_OUT" \
  --wasm "$C5_WASM" --wasm-sha256 752427fc94772b948e3320fa56ec187d502fc5c537c71d7de02d74955cd86997 \
  --rom "$C5_ROM" --rom-sha256 49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a \
  --merged "$C5_DISPLAY_MERGED" --merged-sha256 16b8762fb108bf8b305d84a9182d1ce7967f0eb94a2204950ae3441653131eb4 \
  --sd "$C5_SD_MASTER" --sd-sha256 a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6 \
  --build-receipt "$C5_DISPLAY_BUILD_RECEIPT" --build-receipt-sha256 98642fcf0204ad426dc661537bb6072e4aa39309bd9ac3cd198a86fdd156a8bb \
  --guest-seconds 15 --wall-timeout-ms 180000

(
  unset CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS CARGO_PROFILE_RELEASE_LTO
  ESP32SIM_ROM_DIR="$C5_RUNTIME_ROOT/web/wasm/fw" CARGO_BUILD_JOBS=8 \
    cargo test --release --workspace -- --include-ignored --skip external_
)
```

32MiB flash / 8MiB PSRAM、ROM kind0 后 merged kind5、单次创建、`appDirect:false` 和 interpreter ABI 均有实际回执。240MHz 是 nominal counter，48MHz XTAL 是 runtime 要求，`liveObserved:false` 保留。只在 Node 内存后端观察了这次运行，没有浏览器 IndexedDB、跨进程、重置后持久化、Wi-Fi/RF 或实物兼容性验收。

原 UART、空 emulator 流、RGBA、PNG、64MiB 导出盘与完整测试 stdout 仅在忽略目录留存，未复制或公开上传。精简 JSON 仅保留它们的 alias、字节数及哈希；不包含私有绝对路径、PID、session 标识或硬件快照流。无公开下载的事实也保留：远程审阅者需另行获得匹配原件或复现，不能把哈希当成可下载链接。整理仅做只读核对，最终发布前仍需统一隐私检查与审核。
