# C5 本地集成检查

固定私有集成源码 `c8232bfcc3464bd62bdafbc93c18d096c169d6f5`（tree `9761c4662bf79e332da300c21536e7492a8d6b1e`）的本地功能检查完成，检查前后源码干净且版本不变。最终公开树与本检查的源码对应见[整合身份](../c5-final-integration-2026-11-25/source-identity.json)。本组件收据不认证远程 CI 或 Pages。

| 检查 | 实际结果 |
| --- | --- |
| Native Clippy / WASM Clippy | 两项均 `-D warnings`、exit 0 |
| release workspace（含ignored，跳过external_） | 3513通过，0失败，0ignored，170filtered |
| 原厂固件具名native用例 | 1通过；显式离线驱动兼容，另含严格路径负断言 |
| Native VQ | 44通过，0失败 |
| Node检查 | 26组命令exit 0；其中network-relay为8通过、2条件跳过 |
| platform套件 | 136文件、554通过、0跳过；四个私有C5输入用例实际执行 |
| WASM JIT差分 | 111126用例、78隔离组，99679 compiled modules released |
| 独立GPIO波形 | 24用例，738 compiled modules released |
| `jit-profile,cpu-profile` WASM编译 | exit 0；未覆盖生产WASM |

这些计数分别列出，不能相加成唯一测试总数。四个私有C5用例验证原始输入/协议、后端拒绝和实际WASM加载，不代表四次完整原厂运行。Node的browser-benchmark组是单元测试，没有启动浏览器性能测量。硬件诊断probe为1个StopWatch虚拟夹具，不是实物验收。

生产WASM为8902103B，SHA256 `dee308ca758e02e5574d1888b3b68d7f7b7071da6d0f193347a4cf8628ceb28f`；Node26组及WASM extra前后均不变。Native CLI/网络backend等产物的原始大小和SHA见 [checks.json](checks.json)，未重新命名或改写其hash。

## 环境与复现

Rust `rustc 1.100.0 (0ef1bc386 2026-11-09)`；Cargo `1.100.0 (3acc79c0c 2026-11-05)`；Node `v26.13.0`；Python `3.15.2`。CI使用Node24，与本地不同，因此本地通过不能代替CI。build jobs=8；Native无自定义RUSTFLAGS/target-dir/release overrides，源码release默认opt3/debug1/fat LTO/codegen-units1。WASM脚本使用仓库固定编译策略（inline-threshold2000、debug0；strip默认debuginfo），实际脚本SHA和Cargo/lock/workflowSHA见checks。原environment未单列WASM_OPT，其脚本默认0；不凭命令缺少参数推断完整父环境。这里没有保存OS/CPU测量，不作性能比较。

在固定源码根运行。`$CHECKOUT`、`$INPUT_DIR`、`$OUTPUT`由调用者指定；INPUT_DIR提供以下原样文件，不从开发者目录复制数据。缺少私有输入时，四个external平台用例会跳过，具名native用例会失败，不能称复现相同覆盖：

| 输入 | 字节 | SHA256 |
| --- | ---: | --- |
| esp32c5_rev100_rom.elf | 472300 | `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a` |
| lcd273-factory.bin | 3346992 | `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942` |
| lcd273-sd64m.img | 67108864 | `a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6` |

```sh
export CARGO_BUILD_JOBS=8 PYTHONDONTWRITEBYTECODE=1
export ESP32SIM_ROM_DIR="$CHECKOUT/web/wasm/fw"
export C5_ROM_ELF="$INPUT_DIR/esp32c5_rev100_rom.elf"
export C5_LCD273_FACTORY="$INPUT_DIR/lcd273-factory.bin"
export C5_SD_IMG="$INPUT_DIR/lcd273-sd64m.img"
export ESP32SIM_C5_ROM="$C5_ROM_ELF" ESP32SIM_C5_FACTORY="$C5_LCD273_FACTORY"
export ESP32SIM_C5_WASM="$CHECKOUT/web/wasm/esp32sim.wasm"
export ESP32SIM_WASM="$ESP32SIM_C5_WASM"
export ESP_NETWORK_BACKEND="$CHECKOUT/target/release/esp-network-backend"
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release --workspace
cargo test --release --workspace -- --include-ignored --skip external_
cargo test --release -p esp32c5 external_original_factory_offline_driver_reaches_its_menu_with_real_event_queue -- --ignored --nocapture
ESP32SIM_VQ_NATIVE=1 cargo test --release -p esp32s3 --test machine --test virtual_stops
cargo build --release -p esp32sim --bins
cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
tools/wasm-build.sh
node --test --test-reporter=tap --test-concurrency=1 tools/platform-*-test.mjs
tools/wasm-jit-test.sh
( . ./tools/wasm-rustflags.sh; cargo build --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-profile,cpu-profile )
```

原platform命令完整展开为136文件；固定源码下上述glob与原argv逐项相等，清单按每项后一个LF的UTF-8 SHA256为 `231f542336098dd24442b3afe63fbddd4d43f3834b41988a0c6ccc40e9a084ff`。包含完整argv的原raw manifest为28754B / `7d62447f914de3cecaf57b83cb5043fcf5bee01a08ec65f230b91c6dba2b4da7`。其余25组Node命令、各组计数及全部36份最终日志的大小/SHA见checks与 [curation.json](curation.json)。保留原清单身份，未复制raw log或冗长文件展开。

network-relay的public TCP和public DNS两例分别缺少caller提供的`ESP_RELAY_TEST_TARGET`、`ESP_RELAY_TEST_DNS`而跳过；没有公开目标值，也没有把它们计作通过。需另提供合法外部目标才能增加该覆盖。

## 保留的负结果与限制

`06a8bad6e92ff0a5c5c999566ae3995d094dfeb1` 的一次原厂具名测试使用非限定名加`--exact`，exit0但实际0用例/598filtered，明确不计覆盖；去掉`--exact`后独立执行1个正确具名用例通过（597filtered）。当前c823最终检查再次执行并通过该1例（603filtered）。三份各自原日志hash分别保留，未以成功记录替换零用例结果。

更早 `6e4581a17f41eebb07cbcdec4f24023e97ce2886` 的workspace检查在编译期间为源码更新而有意中断，尚无test execution/result，signal终止不等于断言失败；它不计覆盖，最终完整运行另列。当前workspace仍出现同名`bringup`/`bringup.dSYM` example输出路径碰撞警告，未隐藏；两项Clippy本身均成功。

各`functionalWallSeconds`包含编译、Cargo锁及并行功能作业的等待，**不是CPU性能样本**。最终CPU对比另见[独立 CPU 收据](../c5-final-cpu-2026-11-25/README.md)；远程CI/Pages由该公开提交的Actions另行验证。离线HLE用例通过不等于严格Wi-Fi硬件、RF或完整FactoryOnly验收。[Sensor 时序及端序失败](../c5-factory-sensor-2026-11-25/README.md)另列，Sensor数值未纳入通过范围。没有从这些检查推导真实浏览器或物理硬件验收。

原始捕获保持原字节留在私有ignored目录；这里只发布精简事实。绝对路径、进程/会话标识及无关命令被省略，参数以调用者alias表示；所有测量值、结果、负面和原artifact/hash保留。原始与本组件记录的映射、hash及删减影响见curation。
