# C5 Key Manager 初始化与 DPA 配置

`BootSecurity` 仅实现五个精确字：PCR_KM_CONF `60096164`、PCR_KM_PD_CTRL `60096168`、KEYMNG_STATIC `60087018`、KEYMNG_STATE `60087028`、HP_SYSTEM_SEC_DPA_CONF `60095008`。`60087028` 是 Key Manager 状态，不是 XTS_AES 状态。SEC mux、Bus、eFuse、PMU、CLIC、真正加解密及其他 owner 不在此提交内。

基于 `ccf0a725a256d3e03a713178f36ba6fd0a9c08c3`。既有原厂日志已实际越过 SEC mux，停在 `4200255e` read `60096168`：27,039,922 instructions / 27,039,872 nominal cycles / 3,711 UART bytes；完整日志SHA见 checks。下文 KM 后继和组件测试不是新的原厂运行结果，本提交没有 native guest、工作区构建、Cargo、WASM、browser 或 benchmark。

固定 [IDF553 init.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_security/src/init.c#L32) 的 SECONDARY `esp_security_init` 在 flash encryption **未启用**时执行 KM 初始化。原 typed 顺序为：

```text
4200255e..256a  POWER:4 ->0 ->2（清FORCE_PD2、置FORCE_PU1）
42002578..2590  CONF:开clock0 ->assert reset1 ->release reset1
42002592..259a  轮询RO READY2
420025a4/25b2   两次轮询 KEYMNG_STATE&3 ==0（IDLE）
420025b8..25c2  STATIC:1e0 ->1e2（仅选择eFuse FLASH key来源）
420025cc..25e0  DPA:0 ->4 ->5（CFG_SEL2及LEVEL[1:0]）
```

两次 IDLE wait 来自 [C5 key_mgr_ll reset helper](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/key_mgr_ll.h#L85) 与其 caller。没有 START、CONTINUE、key deploy/recovery、KEY_VLD、RESULT、key memory 或 XTS TRIGGER；这些地址全部拒绝，不能借本模型宣称可用密钥或加解密。

`Inputs` 默认全部未知/false：pre-DPA `sec_source_hz:Option<u32>`、`apb_available`、`crypto_domain_available`、`efuse_dpa_level:Option<u8>`、`flash_encryption_enabled:Option<bool>`、`force_km_flash:Option<bool>`。它们必须来自父级实际 Clock/eFuse 和明确声明的 virtual awake-domain 合同，不能由 profile cookie补true。48/480MHz只作可用来源识别，不作为 KM functional rate。

配置与运行资格分开：

- PCR和DPA配置读写须明确 APB=true；默认未知输入连 POR 也不返回。PORN为 POWER4、CONF RW0/RO READY1、STATIC1e0、DPA0。初始 READY1只报告 `header_POR`，完成计数为0；即使其他输入全Known，未执行reset也不能读取 KM STATIC/STATE 作为运行状态。
- 实际 assert reset撤READY资格、恢复KM数字配置；只有真实1→0 release且 PU/!PD、gate、domain、APB、pre-DPA来源与eFuse全部满足，才能安排有限事件。所有eFuse必须证明 DPA0、flash encryption=false、force-KM-flash=false；寄存器DPA level也必须0。release不合资格或deadline溢出原子拒绝，保持reset-held。
- 完成策略是显式 **virtual reset protocol 1us =240 nominal scheduler cycles**。它不是一个KM edge、硬件延迟、SRAM电压settling、DPA分频或crypto周期。到deadline的有限事件才产生新的released/IDLE和完成计数；反复读、构造器或RO bus值不会制造完成。RO READY2写值忽略，不能force状态。
- 旧Inputs先拥有已过时间，再锁存新观察。deadline前丢失任何资格则取消，恢复Inputs不自动补完成，须新assert→release。正在初始化时改变power/gate或DPA为非off也取消。正常事件同刻先按旧观察结算；watchdog同刻优先由父级先仲裁/dispatch，HP/Chip reset本身不会先pump完成该边界事件。
- KM STATIC/STATE读须已完成上述事件，且当前power/gate/domain/APB/source/eFuse仍可观察。STATE0来自从未接受START的idle不变量；不合资格返回Unsupported，不伪造BUSY/IDLE。STATIC仅bit1可改，其他命名POR保持；没有创建密钥。
- DPA mask7仅保存named配置。完成后 DPA5保留已有released/IDLE读回；它不授予functional rate，新reset在非off DPA下不能完成。实际clock consumer、START/CONTINUE、deploy/valid/result/data/IRQ/locks仍拒绝；不模拟随机divider或加密安全效果。

时间戳倒退不改变状态，deadline用checked add，诊断用饱和计数；未支持地址/字段无alias。拒绝的寄存器写不提交字段或write count；访问前合法的时间/Input推进独立保留。`pump`在取消active未知输入或倒退时返回false，状态观察会明确拒绝；取消后的PCR配置仍可在APB可用时供新的reset恢复。

CPU/LP-only保留HP配置及pending事件；HP/Chip恢复全部PORN、撤deadline和观察。此为明确functional retention，非per-flop硅片保证。raw/SRAM deselect由父级撤optional owner；此模块不修改legacy路径。

时钟与公开限制：固定 [esp_crypto_clk.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_security/src/esp32c5/esp_crypto_clk.h#L12) 选择480MHz SPLL root；TRM1.1 §9.2却说明CRYPTO_CLK最高160MHz并有DPA随机divider，因此禁止把source480M当KM频率。固定 HP_SYSTEM header 的 CFG_SEL注释与SDK写法给1=register，但同header LEVEL注释和TRM9.2文字存在相反说明；当前register/eFuse level均0时解释无差异。此模型只在该KnownOff条件完成reset，DPA其余值仅是配置，不声称硅片分频得到验证。

来源：两个固定 SDK revisions `2c211b236707889e8400c4dc5644dd5c4ee071e0` / `735507283d5b2f9fb363a1901172dbd9e847945d`；相关完整文件逐字相同，见 [sources.json](sources.json)。TRM1.1 Register9.78/9.79、31.16/31.20定义本次PCR/STATIC/STATE。官方ROM key_mgr/km头定义IDLE/LOAD/GAIN/BUSY和加密开启时的key-recovery API，没有给本次reset延迟；没有借ROM头补key-valid或真实key值。

独立复现：

```sh
python3 docs/evidence/c5-security-init-2026-11-25/run-owned.py \
  --repo . --output-dir "$C5_SECURITY_CHECK_DIR"
```

脚本仅用stable rustc/clippy-driver检查本模块。组件覆盖真实配置顺序、deadline前/同刻、输入取消/恢复需新reset、old-input顺序、RO不可force、DPA5边界、source availability非rate、溢出、无alias及CPU/HP/LP/Chip retention；实际结果见 checks/owned-tests。原BIN、ROM及eFuse未改，父级Bus接线/完整集成检查另行完成。

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
