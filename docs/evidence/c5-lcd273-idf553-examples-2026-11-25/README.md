# 原样 LCD273 示例：IDF 5.5.3 构建输入

已用精确官方 SDK、推荐 GCC 和厂家原锁定组件，顺序构建 Waveshare `03_sd_card`、`02_lvgl_demo`，取得 merged BIN、app BIN、匹配 ELF/map。源码、`dependencies.lock` 与原/有效 `sdkconfig` 全部逐字不变。镜像、分区及 ELF 静态检查通过；本构建没有运行 guest、LVGL benchmark、浏览器、flash/monitor 或实机。

这是两个独立的 source-built examples。它们都沿用厂家 `project(01_factory)`，不是既有 `FactoryOnly_20260724.bin`，不能改变原厂整机、Wi-Fi/RF、LCD、SD 或按键的运行验收。551/602 hello 的既有产物、失败结果和 helper 保持不变。

## 来源与环境

- 厂家：[ESP32-C5-LCD-2.73@418de818f4f40c5a04c4292dbe293bfaa1351068](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/tree/418de818f4f40c5a04c4292dbe293bfaa1351068)，完整项目路径为 `example/ESP-IDF-V553/{03_sd_card,02_lvgl_demo}`。03 为 68 files/1705249B、tree `f3a52a48572bffab114f5ce2065ab2fb83c7a7a2`；02 为 65 files/1702154B、tree `6d9c1ecd1b899ab2e545fa48bfb002f8c6d49bdf`。另保留根 LICENSE/README。每份源码按固定 raw URL、字节数、Git blob SHA1 和 SHA256 校验。
- 厂家 commit 的实际根 tree 为 `5282f0112760f1d53a79092548303df96602d4b8`，已由[官方 commit 元数据](https://api.github.com/repos/waveshareteam/ESP32-C5-LCD-2.73/commits/418de818f4f40c5a04c4292dbe293bfaa1351068)及 382 个 Git tree object 重算核对。原递归缓存的 `sha` 字段保留请求 commit ref；缓存 byte SHA 与真实 tree object 身份分别记录，不混用。
- SDK：[ESP-IDF v5.5.3@2c211b236707889e8400c4dc5644dd5c4ee071e0](https://github.com/espressif/esp-idf/tree/2c211b236707889e8400c4dc5644dd5c4ee071e0)，浅主仓及完整递归子模块；27 条 gitlink 均精确匹配，构建前后 tracked clean。SDK 全部放私有 target，没有更改现有安装。
- GCC：[553 tools.json](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/tools/tools.json)推荐 `esp-14.2.0_20251107`，实际 `riscv32-esp-elf-gcc ... 14.2.0`。官方 macOS arm64 包 288327120B，SHA256 `cedfcdaf8f165bd3e11ca4ee46b71d5b561fab93407eb8203812f9c14b1c7714`；先核发布元数据、下载大小和 SHA，再解压执行。官方 license 字段为 `GPL-3.0-with-GCC-exception`，包内 77 份 LICENSE/COPYING/NOTICE 文件均保留。
- Python：controller 为 Python 3.15.2；SDK 使用新的私有 venv，实际 Python 3.11.15、esptool 4.14.0、Component Manager 2.5.2，按 SDK core requirements 和官方 v5.5 constraints 检查通过；完整包版本见 [inputs.json](inputs.json)。CMake 4.4.5、Ninja 1.13.2，macOS 27.2.1/arm64。没有全局安装或修改原 Python 环境。
- 组件：原 lock 固定 15 个 registry 包，归档总计 122624548B；包括 adapter **0.5.1**、LVGL **9.5.0**、button **4.1.7**。每包先核 registry metadata 的 component_hash，再用官方 Component Manager 验证解出目录 hash 和 CHECKSUMS；归档 SHA、下载 URL、许可证及校验结果逐项见 inputs。component_hash 与 ZIP SHA256 分列。

根 SDK/厂家许可证、各组件许可证及 GCC 包内各自许可证随原输入留存；摘要不把它们合并成一个许可证，也未上传二进制或源码归档。

## 原配置与产物

两例原始和生成有效 `sdkconfig` 都是 90677B，SHA256 `5cb65d2d95d876a898a7729d3590120c4a87a1b9509be496cdbc1f6eaac94423`，CONFIG value delta 为空。原 lock 7066B，SHA256 `f6f8e566fddb6b3cfadf2b150913291c31140d469b919c716bf6cde356087d1a`，构建后不变。

helper 先复制全部原样项目，再把原 `sdkconfig` 逐字复制到 case 根；IDF 使用 `-D SDKCONFIG=$CASE_DIR/sdkconfig`，没有执行 set-target。C5、32MiB flash、DIO/80MHz flash args，240MHz CPU，quad/80MHz PSRAM、boot init、memtest、malloc 和 32768B internal reserve 均保留。原文件同时含 QIO choice 与派生字符串 `dio`；没有手工纠正它，实际生成 flash args 和镜像 mode=2 均为 DIO。

XTAL 为原样 `CONFIG_XTAL_FREQ_AUTO=y`、`CONFIG_XTAL_FREQ=0`，不是编译强制 48MHz。PSRAM 容量在运行时探测，配置没有强制 8MiB。后续 48MHz/8MiB 场景是否真实被检测，属于运行证据。厂家显示 BSP 的 50 行 buffer、`use_psram=true`、双 buffer、PPA disabled 也保留，没有改成 internal buffer。

| Case | 文件 | 字节 | SHA256 |
| --- | --- | ---: | --- |
| 03_sd_card | merged.bin | 422096 | `0f6d7646eaba88caab4708db719412a0b7b261d7cc18c8d601116a7c7c902308` |
| 03_sd_card | 01_factory.elf | 5414120 | `9032775e689027e030458437db46e40641342050a223289905b9f92e876ac544` |
| 03_sd_card | 01_factory.map | 5055669 | `a170471fcc037316ce0176df247d678263af7f168fd8803e23f061bced813882` |
| 02_lvgl_demo | merged.bin | 1046720 | `16b8762fb108bf8b305d84a9182d1ce7967f0eb94a2204950ae3441653131eb4` |
| 02_lvgl_demo | 01_factory.elf | 10627044 | `51e6bc6c20d2bd2a659a8cf39577fa302e0dbb38f845ff7454352253981a4132` |
| 02_lvgl_demo | 01_factory.map | 5223928 | `e92c4a7bf24266134f30a1f638ddf7beca6ce153dc7d1613e18aca924b2a8238` |

各 case 的 app、bootloader、partition、OTA initial data、段、校验、config、source hash 与原始日志 hash 分别见 [03-sd-card.json](03-sd-card.json)、[02-lvgl-demo.json](02-lvgl-demo.json)。私有完整产物位于各 case 根的 `build/`，没有进 Git。

两例均由本次 `flasher_args.json` 合并：bootloader `0x2000`、partition table `0x8000`、OTA initial data `0xd000`、factory app `0x20000`。核对每个 flash 输入逐字相等、全 prefix/gaps 为 FF、无 overlap、merged 恰终止于最后文件且不超过 32MiB；boot/app chip_id=23、revision100..199、image XOR 与附加 SHA、partition MD5、ELF32/RISC-V entry 和 descriptor 内完整 ELF SHA 都通过。

**身份限制：**原项目没有 PROJECT_VER；本次 blob snapshot 位于 integration checkout 内，IDF 自动版本实际为 `v0.3.0-1578-gc6ceccd8`，来自外围 Git describe，不能当厂家版本。两例编译时间不同，均如实保留。固定源码 Git tree/main/lock/config 与产物 SHA 才是本次身份依据；新构建因时间、路径及自动版本可能产生不同二进制。

静态符号用于后续真实执行定位：03 `app_main=42009356`、`bsp_sdcard_mount=4200991a`；02 `app_main=4200954a`、`bsp_display_start=42009af2`、`bsp_display_backlight_on=42009936`、`lv_demo_benchmark=42034fee`。这些是本次 map 地址，不表示已经执行。

03 原程序运行时会以 wb 写 `/sdcard/test.txt`，随后读回比较；本构建没有挂卡或写卡。02 原程序调用 LVGL benchmark demo，本次只编译它，没有运行或计时。两者 main 没有显式 Wi-Fi init/start，但公共配置及 SDK 启动仍含无线支持，不能预先承诺不会触及无线相关启动寄存器。

## 复核与复现

[helper](../../../tools/c5-lcd273-reference-build.py) 分离 acquire、python-env、dependencies、build、inspect。所有输出只允许当前 checkout ignored target 下**尚不存在**的路径：existing file/dir、symlink、symlink ancestor、target 根及 parent traversal 都拒绝，leaf 用原子 mkdir 预留。失败目录保留，下一次必须换 fresh 名称。

使用 caller 提供的 Python 3.11、CMake/Ninja 路径和 fresh 目录；下载树可从固定官方 URL 获取：

```sh
curl --fail --location --output "$TREE_JSON" \
  'https://api.github.com/repos/waveshareteam/ESP32-C5-LCD-2.73/git/trees/5282f0112760f1d53a79092548303df96602d4b8?recursive=1'
python3 tools/c5-lcd273-reference-build.py acquire \
  --tree "$TREE_JSON" --output "$INPUTS"
curl --fail --location --output "$CONSTRAINTS" \
  https://dl.espressif.com/dl/esp-idf/espidf.constraints.v5.5.txt
python3 tools/c5-lcd273-reference-build.py python-env \
  --base-python "$PYTHON311" --sdk "$INPUTS/esp-idf" \
  --constraints "$CONSTRAINTS" \
  --constraints-sha256 412e4e309c10d076d147737cbfc1fa5528910512c262d06f0c8eb82362f97069 \
  --output "$PY_ROOT"
"$PY_ROOT/venv/bin/python" -B tools/c5-lcd273-reference-build.py dependencies \
  --lock "$INPUTS/vendor/example/ESP-IDF-V553/03_sd_card/dependencies.lock" \
  --output "$COMPONENTS"
python3 -B tools/c5-lcd273-reference-build.py build --case 03_sd_card \
  --inputs "$INPUTS" --components "$COMPONENTS" --python-root "$PY_ROOT" \
  --cmake "$CMAKE" --ninja "$NINJA" --jobs 8 --output "$CASE03"
python3 -B tools/c5-lcd273-reference-build.py build --case 02_lvgl_demo \
  --inputs "$INPUTS" --components "$COMPONENTS" --python-root "$PY_ROOT" \
  --cmake "$CMAKE" --ninja "$NINJA" --jobs 8 --output "$CASE02"
python3 -B tools/c5-lcd273-reference-build.py inspect --output "$CASE03"
python3 -B tools/c5-lcd273-reference-build.py inspect --output "$CASE02"
python3 -B docs/evidence/c5-lcd273-idf553-examples-2026-11-25/verify-static.py \
  --reference "$CASE03" --reference "$CASE02" --inputs "$INPUTS" --scratch "$FRESH_CHECK_DIR"
```

本次 build 的精确 aliased argv 逐项保存在各 case JSON。outer build `--parallel 8`，嵌套 bootloader 通过任务私有 Ninja wrapper 使用 jobs1；这是构建参数，不是全程严格总并发上限或 CPU 性能证据。每阶段原始 PID/PGID、命令、stdout 和 state 留在私有 control/case 根；全部 owned process groups 已核无剩余。

实际构建 helper SHA256 `e2d12082dba010c92da8901b391d92d9caa3db3ac15e6c0676c68bd444e18186`。构建后先加强 acquire 的 Git tree 验证，使其重算全部 tree object、接受等价 JSON 格式；随后收紧 inspect 的原样配置合同。实际 acquire/environment/dependency/build、`04b166c7` verifier 和当前 verifier 各份身份分别列在 inputs，不混称同一文件。两套固件及 SDK 没有修改或重建，当前 verifier 的只读回验通过。

## 保护检查与负结果

初版 [23 项静态检查](static-checks.json)通过：9 类 fresh 路径拒绝及 canary 不变、原子预留的第二 writer 拒绝、382 个 tree object 与等价 JSON 验证、篡改 tree child 拒绝、两例只读 source/artifact 回验、RAW append/prefix/gap/missing bootloader/overlap 拒绝、image XOR/SHA、partition MD5 及 ELF machine 的破坏副本拒绝。

`04b166c7` 的 inspect 仅要求少数固定 CONFIG 值，将其余 value delta 作为信息记录，没有强制有效配置与厂家副本逐字相同。后继现显式要求 `sdkconfig_value_delta == {}`，并要求两文件的 byte count/SHA256 相等。新增两个纯副本负例分别改变非白名单 `CONFIG_SPIRAM_USE_MALLOC`、只追加注释；两者都由完整 inspect 拒绝。副本通过只读 symlink 引用现有 build，仅自身配置写入 fresh scratch，真实两套配置和产物均保持原 hash。最新 [26 项检查](static-checks-config-bytes.json)全部通过，全程没有重建或执行 guest。

保留两次前置负结果及原捕获 hash：原已安装 5.5 Python 的 esptool4.12.0 不满足官方 `~=4.14`，所以另建私有 venv；首次组件获取因 helper 复用 CM2.5.2 缓存的一次性 storage_clients generator，在 button 后收到空 VersionNotFound。官方后续归档存在；仅 helper 改为每包新读 client，使用 fresh 组件目录成功，没有改 manager、SDK 或原 lock，也没有降版本。

只提交紧凑事实、hash、alias 命令和复核脚本。原路径、进程标识、原 stdout、所有源码/工具/ZIP、ELF/debug metadata 和 BIN 都留 ignored target。没有替换或重写原捕获；当前 helper、旧 551/602 helper 和所有历史产物 hash 分别保留。原始绝对路径省略不影响输入及产物身份核对。
