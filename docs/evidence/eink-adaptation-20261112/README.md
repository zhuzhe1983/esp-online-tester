# 四款墨水屏基础功能、显示与联网验收

2026-11-12。基线 `7f0498df`，最终 Rust 源码 `2891d72c`；之后的提交只包含探针、显示核对、目录与文档。最终生产 WASM SHA-256 为 `8d8ccce696666eeb7a355cdbe15fd398eaec9ddb9ae9ade85544d91639b4fb20`，5,568,210 字节。完整检查与限制见 [checks.json](checks.json)，产品列表见 [适配清单](../../platform/eink-adaptation-status.md)。

- PaperS3：官方 v0.5 的 TF 实际文件写入和 reset/reload、BM8563 日历连续递增、触摸/IMU/扫描通过；三个静音图标各 7,480 像素与固定官方素材及对应数字 LUT/Bayer 结果完全一致。仍是黑白数字模型，不能据此证明 16 灰阶光学效果。EDCBook 下载验证返回错误，现有站点 OCR 路径 20 秒超时，未取得镜像，未验收其阅读和联网。
- PaperMono 与 Lite：官方 1.2.0 两型号首次 BMI270 初始化、实际值与恢复、按键/触摸/TF/RGB 通过。两款都完成关联、DHCP、双向 SNTP，原固件显示 TIME UPDATED；全部四灰阶像素、方向、局刷覆盖范围及恢复均有确定数值断言。不同构建的验收范围在产品收据中分开记录，最终构建再验完整版灰阶、两型号输入及联网。大 BMP 栈溢出和偶发扫描失败仍保留。
- 普通 INK4：官方 2.0.53 TF 挂载、文件读写、浏览器重载持久化与录放通过；SSD1677 Mode 2 在实际刷新完成后交换两块 RAM，消除旧标题及旧页残留。成功页兼容固件声明的 `/reboot` POST 后，Node 与真实无头浏览器均自动配网并收到实际 OTA 响应；账户激活和语音未完成。

[PaperS3 TF](../../platform/evidence/papers3-factory-tf/README.md)、[RTC](../../platform/papers3-rtc.md)、[显示](../../platform/papers3-display.md)；[Mono IMU](../../platform/evidence/papermono-imu/README.md)、[灰阶与联网](../../platform/evidence/papermono-network-display/README.md)；[INK4 TF](../../platform/evidence/ink4-tf/README.md)、[显示](../../platform/evidence/ink4-display/README.md)、[联网](../../platform/evidence/ink4-network/README.md)。各收据保留固定来源、原始文件摘要、失败和复现命令；不提交固件二进制、用户媒体、原始串口、网络 token 或账户激活数据。

## 检查

Rust stable 1.99.1。两项 Clippy 均以 `-D warnings` 通过；CI 风格 Rust 测试 1,957 项、171 组通过（含 ignored，跳过 `external_`）；平台测试 387 项通过。生产 WASM 无 DWARF、保留 name section。无 golden 重建。之前 worktree 整合的 JIT/其他检查见其独立收据，不作为本轮重新运行的结果。

```sh
cargo +stable clippy --workspace --all-targets -- -D warnings
cargo +stable clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
ESP32SIM_ROM_DIR=<complete-rom-directory> cargo +stable test --release --workspace -- --include-ignored --skip external_
node --test tools/platform-*-test.mjs
tools/wasm-build.sh
node tools/check-wasm-sections.mjs web/wasm/esp32sim.wasm
```

检查针对干净集成分支。主目录已有的 Dial/NFC/relay 未提交工作单独保留，未混入本次提交或上述测试身份。

## CPU 对照

沿用 EX201 的 GPIO/I²C 物理总线契约：这次改变是硬件 I²C 事务末尾的真实 GPIO STOP 必须通知连接的器件，释放 RTC burst 锁；重复 START 不视为 STOP。同时包含 Mono 的客体时钟 IMU 与 INK4 显示 RAM 修复。关联 EX250 的采样条件，仅作整个改动的回归记录，不提出新优化机制。

基线保留自前一轮干净构建：`3a0a9a00` 的运行代码与仅改文档的 `7f0498df` 一致；候选使用上述最终 Rust 源码。两边 release 二进制 hash、固定输入 hash、原始数字样本、计数与输出 hash 均保留在 [cpu.json](cpu.json)。[环境](environment.json)记录通用硬件/工具链、汇总负载和限制，不含进程清单或用户身份。

```sh
python3 tools/compare-display-cpu.py \
  --baseline <baseline-release-bin-dir> --candidate <candidate-release-bin-dir> \
  --firmware <fixed-firmware-dir> --roms <complete-rom-dir> \
  --rounds 3 --output <receipt.json>
```

每个工作负载运行 30 客体秒，三对 AB/BA 交替，共 24 个样本；没有单独预热。测量期间没有任务自有的编译、浏览器或其他模拟器负载，保留无关应用。三组 hello 完成后发现 Pocket Tank 的本地模型文件缺失，该负载未启动；补齐固定模型后只运行未完成的 Pocket Tank，保留全部已测样本。短样本、共享宿主与样本数限制因果归因；不宣称提速、零开销、硬件时序或无回归。

| 工作负载 | 基线 CPU 中位数 / s | 候选 CPU 中位数 / s | 变化 | 候选较慢的对数 |
| --- | ---: | ---: | ---: | ---: |
| hello | 0.217073 | 0.218686 | +0.743% | 2/3 |
| c3-hello | 2.393268 | 2.469541 | +3.187% | 2/3 |
| c6-hello | 1.571193 | 1.538701 | -2.068% | 1/3 |
| pocket-tank | 38.811015 | 38.860147 | +0.127% | 2/3 |

全部 24 个样本的指令数、周期、异常、中断和 stdout SHA-256 一致。C3 的 +3.187% 中位数增量尚未归因，不能消去或称为无回归；各负载的基线/候选范围重叠，没有为取得更好数字重跑。
