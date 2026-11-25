# 独立 LCD273 按键与共用 SPI 诊断输入

已构建独立 `c5_lcd273_diagnostic` 0.1.0，取得完整 merged BIN、匹配 ELF/map。它是本仓库自有 MIT 测试程序，显示三键计数、最近事件和 TF 状态；不是厂家 02/03 或 FactoryOnly，不继承它们的运行验收。本次只有构建及静态检查，没有运行 guest、native emulator、浏览器、benchmark、flash/monitor 或硬件。

源码和协议见 [firmware README](../../../firmware/c5-lcd273-diagnostic/README.md)，实现位于同目录 `main/main.c`。新项目的 main、CMake、明确版本和许可证与原厂家来源分列；原固定 BSP、local components、partitions、sdkconfig、dependency lock 和空 main manifest 均保留。原 02/03 的所有已记录产物重新核 hash 不变，旧工作树也保持 clean。

## 固定输入与真实接口

源基线为 `99d166829e5835d274a8396bbf2f294b4baa0afd`。沿用 [IDF553 输入收据](../c5-lcd273-idf553-examples-2026-11-25/README.md)：厂家 commit `418de818f4f40c5a04c4292dbe293bfaa1351068`、SDK `2c211b236707889e8400c4dc5644dd5c4ee071e0`、GCC **esp-14.2.0_20251107**/14.2.0、私有 Python3.11.15/esptool4.14.0/CM2.5.2。SDK 前后 tracked clean、gitlinks 匹配。SDK、GCC、Python 和原 source/package cache 只读；Component Manager cache、tools state、TMPDIR 均另复制/创建在新输出。

原 sdkconfig 90677B，SHA256 `5cb65d2d95d876a898a7729d3590120c4a87a1b9509be496cdbc1f6eaac94423`；原 lock7066B，SHA256 `f6f8e566fddb6b3cfadf2b150913291c31140d469b919c716bf6cde356087d1a`。原/有效 config、原 lock 均逐字一致，没有 set-target、MINIMAL_BUILD 或新增 SDK/library flags。32MiB DIO/80MHz flash、quad80MHz PSRAM/memtest、50行 PSRAM 双显示 buffer 均保留；XTAL AUTO 和 PSRAM 容量探测没有被改成48MHz/8MiB常量。

15 个实际 `managed_components` 在构建后再次离线验证目录 component_hash 和各文件 CHECKSUMS，全部通过。其中 button4.1.7、adapter0.5.1、LVGL9.5.0，与原 lock 相同。输入、源码及组件 hash 见 [inputs.json](inputs.json)。

[固定厂家 BSP](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/blob/418de818f4f40c5a04c4292dbe293bfaa1351068/example/ESP-IDF-V553/02_lvgl_demo/components/waveshare__esp32_c5_lcd_2_73/esp32_c5_lcd_2_73.c)创建 GPIO2/3/28 active-low button 句柄，并注册 [adapter0.5.1](https://components.espressif.com/components/espressif/esp_lvgl_adapter/versions/0.5.1/readme) 的 ENCODER indev。该 adapter 的 LVGL9 read callback 明确给出 LEFT/RIGHT/ENTER 与 PRESSED/RELEASED。诊断通过公开 getter 取得此 callback，包装后先调用原实现、原样透传其 data，只观察单键 driver state 边沿，不重复创建 GPIO 驱动或修改 debounce/diff。LVGL 正常消费同一输入移动 group 焦点；三键 label/recent label 的更新是程序响应输入的路径，尚需实际帧验证。

原 adapter 有 PREV > NEXT > BOOT 优先级，因此本程序只接受逐个单键测试。优先级在保持按下期间变化会报告 `INPUT_AMBIGUOUS`，不会编造新物理 release/press。短于 debounce/read 周期的电气脉冲和组合键不在边沿计数合同内。UART 每个观察到的 DOWN/UP 都有唯一递增序号，但 UART 本身不证明 LCD transfer 已完成。

共用 SPI 顺序依据 [固定 IDF553 sdspi_share.rst](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/docs/en/api-reference/peripherals/sdspi_share.rst)：先令 CS8/9 高，调用公共 `bsp_spi_init(48000)`，原 BSP mount 令卡先进入 SPI mode，再 CH32 reset/LCD/adapter start。SCLK6/MOSI7/MISO5、两 CS 及 SPI2 均保留真实驱动。UI 开始200 guest ms后和每次 BOOT release 后，由 main task 用 rb 读取现有 `/sdcard/test.txt`，最多128B加 EOF probe；不创建/覆盖文件，不 format。71B完整原03 payload才标 MATCH；缺文件正常显示 absent，其他错误仍显示失败。是否实际共用、是否无磁盘写入，均留给运行日志、真实 SPI/帧与卡 IMG 前后 hash 证明。

## 产物与静态检查

成功输出为私有 `target/c5-lcd273-diagnostic-build-manifest-20261125/build/`：

| 文件 | 字节 | SHA256 |
| --- | ---: | --- |
| merged.bin | 770832 | `7009dda577b13fe2ccfe721b415cfd7f88de487fa452cc2f52e5f20a531c0c0f` |
| c5_lcd273_diagnostic.bin | 639760 | `9fee28660073b560f7869fa74c07ed8faa0a2dc1fdafb8dbff44b4e8a35e7795` |
| c5_lcd273_diagnostic.elf | 10009428 | `61bfa987eba95d9048d048e220abc30c5af3e84b15793b26917fe266a6ce6806` |
| c5_lcd273_diagnostic.map | 5138432 | `ac5d0f4cb51f2e5addece5ee419ecb38d8cd0baeb86f8c4bc6dd458b414bc104` |

descriptor 明确为 `c5_lcd273_diagnostic`/`0.1.0`/`v5.5.3`，不再从外围工作树推断版本。静态 app_main=`420097b0`，entry=`408002b0`；这只是本次 ELF 地址，没有称已执行。

bootloader2000、partition8000、OTA initial dataD000、app20000；完整 merged prefix/gaps 为 FF、各片段逐字匹配、无 overlap/尾巴、app 在 factory partition 内。C5/32MiB image header、XOR、附加SHA、partitionMD5、ELF32/RISC-V entry、descriptor 完整 ELF SHA 均通过。详细镜像、来源和 aliased 命令/日志 hash 见 [result.json](result.json)。原始日志、源与 BIN/ELF/map 均保留 ignored target，没有上传二进制。

[6项纯静态检查](static-checks.json)通过：完整真实产物/源码/config回验；分别改变非白名单 CONFIG、仅注释字节、自有 main 字节、上游 lock 字节的副本都被拒绝；真实产物再验不变。副本仅通过只读 symlink 引用 build，不改真实源码/产物。复核命令：

```sh
python3 -B tools/c5-lcd273-diagnostic-build.py inspect --output "$DIAGNOSTIC_DIR"
python3 -B docs/evidence/c5-lcd273-diagnostic-2026-11-25/verify-static.py \
  --reference "$DIAGNOSTIC_DIR" --scratch "$FRESH_CHECK_DIR"
```

构建 helper 的全部输出要求 fresh target 路径，复用原参考 helper 的原子预留保护。outer8/nested bootloader1 只记录构建 flags，不承诺严格总并发8，也不是性能数据。两次 owned process groups 均已确认无剩余。

后继工具路径修复保留 `1d2fe1ea` 的独立 P2：旧 helper 没有预先解析 Ninja/CMake，`--ninja ninja` 在 wrapper 前置 PATH 后可能再次 exec 自身，显式相对路径也可能在 build cwd 错解。原实际构建使用绝对路径，未受此路径问题影响。当前 helper 在任何输出创建前，以 `shutil.which` 解析 basename、按调用 cwd 解析显式相对路径并 canonicalize，拒绝 missing、directory、nonexec；wrapper env、PATH 和 build argv 只使用解析后的绝对可执行文件。参数保留 `./ninja` 原拼写，避免被当作 PATH basename。

[17 项临时假工具检查](tool-path-checks.json)通过：包含不同 cwd/PATH 下 basename 与显式 `./ninja` 的区别、symlink、relative/absolute、目录/nonexec/missing拒绝、真实生产 wrapper 从 project 和 nested bootloader cwd 各委托假 Ninja 一次并正常结束，以及绝对 CMake 调用。每次假工具运行有3秒超时，没有执行真实构建或 guest。另将既有6项固件静态检查重新运行，全部通过；main、`7009…` merged/ELF/map及原构建日志均不变。actual builder、`1d2fe1ea` helper 和当前 helper 的 SHA 分列在 inputs，新测试没有后台任务。复核命令：

```sh
python3 -B docs/evidence/c5-lcd273-diagnostic-2026-11-25/verify-tool-paths.py \
  --scratch "$FRESH_TOOL_CHECK_DIR"
```

首次 attempt 在 configure 后被拒绝：遗漏原0B `main/idf_component.yml` 改变 manifest hash，CM 自动重解出新版本，造成 lock/config 漂移。该目录、实际 helper 快照及漂移后的 lock/config/log hash 全部保留；没有进行 application build 或 guest run。成功 attempt 使用另一个 fresh 目录，只补复制原空 manifest；原15 pins、SDK、config均未修改，没有通过降版本或改库配置掩盖失败。

未保留到 Git 的内容包括开发者绝对路径、PID/PGID、原 stdout、原 SDK/包源码/归档、二进制及其 debug metadata。actual source/helper/artifact hashes、版本、alias 命令、负结果和检查结果均保留；原捕获没有重写或以净化版本替换。运行后的三键、frame、共用 SPI/TF、卡 hash 或 FactoryOnly 验收必须另写实际运行证据。
