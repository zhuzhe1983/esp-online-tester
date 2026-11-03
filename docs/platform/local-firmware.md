# 本地 ESP-IDF 构建加载

可以在浏览器选择完整 merged BIN，也可以读取同一次构建的 `flasher_args.json` 与其引用的文件。文件只由浏览器读取；不上传到本站服务器，也不写入远程固件缓存。单独的应用 BIN 缺少启动链，不能当作完整 Flash 镜像启动。

布局依据是 ESP-IDF 官方生成的 [`flasher_args.json`](https://github.com/espressif/esp-idf/blob/v5.5.1/components/esptool_py/flasher_args.json.in) 中的 `flash_files`。命令行参数只作数据检查，不执行。浏览器目录选择保留相对路径；普通文件多选只在文件名唯一时匹配。不同构建目录、同名冲突、缺少文件均需先解决。

```js
import {inspectLocalBuild} from './local-firmware.js';
const files = [...fileInput.files];
const layout = await inspectLocalBuild(files);
// layout: {layoutFile, chip, flashSizeBytes, segments:[{file,offset,path}], missingFiles, warnings}
await espTester.loadFirmware({format:'build', files});
await espTester.start();
```

也可明确指定每段地址，地址应来自该构建的烧录信息，不能凭文件名猜测：

```js
await espTester.loadFirmware({format:'segments', files:[
  {file:bootloaderFile, offset:0x0},
  {file:partitionTableFile, offset:0x8000},
  {file:applicationFile, offset:0x10000},
]});
```

上例只是一个 ESP32-S3 布局。应用与分区表可以使用其他合法地址；加载器读取二进制分区表，再验证每个文件恰好位于声明的分区起点、长度未超界、分区和文件不重叠、启动镜像芯片一致。缺少 bootloader、分区表或 app 均失败，不启用 `appDirect`。加密镜像不受支持。已存在的具体产品硬件模型仍是运行前提，不能只凭芯片一致推断产品兼容。

限制为最多 32 个 Flash 段、单文件（含 flasher_args.json）256 MiB、镜像总输入 256 MiB，并且不能超过所选产品容量。构建目录最多检查 4096 个文件名，仅读取 JSON 及其引用的镜像。取消会停止后续解析和初始化；浏览器正在进行的单次 File 读取在返回后丢弃。

`observe().firmware.source` 为 `local`，`localLayout` 记录偏移、字节数与 SHA-256；各镜像没有远程 URL。本地固件不能通过分享 URL 或录制 URL 携带给他人。Web Serial 的本地分段烧录复用相同校验，只写入用户提供的分段，不把模拟内存中填充的 0xff 空洞写入硬件；仍需点击选择串口并核对实物芯片、Flash 容量与安全状态。分段加载测试未连接或刷写硬件。

**烧录到设备**位于运行按钮旁，打开独立的烧录窗口，设备进度、结果与日志都在窗口内显示。它独立读取当前选择的远程或本地固件，不要求模拟器先加载或启动成功。开始一个批次前，需确认型号、固件来源，以及已自行备份并承担刷写风险；该确认用于本批次明确选择的设备。完整 merged BIN（包括 `flasher_args.json` 中只列出地址 0 完整镜像的构建）按合并方式写入；分区构建按清单真实偏移逐段写入。独立应用 BIN 会提示补齐烧录清单、启动镜像和分区表，不推测应用地址。只有资料链接、没有固件文件的条目仍不能刷机。

窗口支持三种方式：

- **单台烧录**：选择一台设备，写入后保留校验结果。
- **多台并行**：分别点击“添加设备”并在浏览器中授权，然后一次开始，所有已选设备同时写入，平台不限制设备数量。每台设备都有独立的进度、日志和结果；失败可单独重试。
- **逐台连续**：保留已校验的固件，开启后写完一台再换插下一台，点击“选择下一台”授权并继续。本批已选择但尚未完成的设备重连后可点击“重试此设备”继续；成功设备重插不会自动重复写入。浏览器要求每个新设备单独授权，不能自动刷写任意插入的 USB 设备。

窗口还可选择或编辑 [UIFlow1 / UIFlow2 网络配置](flash-configuration.md)，填写自定义数据服务器和 Wi-Fi，并随固件写入。默认不启用配置；批次固定每台设备的配置，连续模式可在空闲时切换供下一台使用。

选错设备时，可在尚未开始、失败或已取消的设备行点击**更换设备**，重新选择 USB 端口，无需新建批次或重新下载固件。更换后保留本批固件和原设备日志，等待点击**烧录此设备**（尚未开始的批次仍使用底部“开始烧录”）；选择端口本身不会连接或写入。取消选择会保留原设备，选择原端口或队列已有端口会提示重新选择。正在烧录或已成功的设备不能被替换。芯片不匹配会在 Flash 写入前停止，并提示实际芯片、所需芯片及更换入口；新设备仍须通过芯片、容量和安全状态检查。

批次准备完成后固定固件内容和 SHA-256，工作台切换型号或固件不会改变它。收起窗口会保留队列，入口显示进度，可随时返回；“停止本批”会取消等待中的设备并请求停止正在写入的设备，端口关闭后才可新建批次。请保持页面打开；正在写入时直接关闭页面可能留下不完整镜像。完成或停止后使用“新建批次”读取工作台当前选择。

写入成功表示 Flash 内容已通过校验，不代表应用已启动或产品功能已验收。多设备队列的自动化测试使用注入的串口替身，实际并行能力还受 USB 集线器供电、线材与设备状态影响。

写入校验结束后，窗口依次显示复位、关闭连接，串口关闭成功才结束该设备任务。平台补齐 esptool-js 0.7.0 缺失的 RTS 复位脉冲；对于已确认使用原生 USB-Serial/JTAG、启动引脚仍锁存在下载模式的 ESP32-S3，按乐鑫的 RTC 看门狗复位流程重新采样启动状态。此路径只在写入校验完成后执行，不修改 eFuse；其他芯片与串口适配器使用普通复位。依据：[esptool S3 实现](https://github.com/espressif/esptool/blob/v5.3.0/esptool/targets/esp32s3.py)、[USB 复位无法退出手动下载模式的说明](https://github.com/espressif/esp-idf/issues/13287)。

拔线或写入异常时会释放写锁，并给串口关闭设置总等待上限。若内容已校验但关闭失败，窗口保留这一事实并提示重新连接；不会将其当作全部成功，也不会让尚未释放的端口被另一任务重复打开。`node tools/platform-flash-batch-test.mjs` 覆盖真实 vendored loader 的复位调用、写流异常、延迟关闭及批次隔离。

验证：

- `node tools/platform-local-firmware-test.mjs`：路径冲突、缺文件、自定义分区地址、芯片、布局、取消检查通过。
- `node tools/platform-runtime-test.mjs`：内部诊断固件的手动分段和 ESP-IDF 构建模式均经真实 ROM/bootloader 启动并产生帧；分享明确拒绝本地源。
- `node tools/platform-flasher-test.mjs`：分段保持原长度和地址，不写入空洞；使用注入的串口测试替身，没有访问硬件。
- `node tools/probe-local-build.mjs --build <user-build-directory>`：可显式测试其他构建，不复制镜像；只输出匿名化统计与哈希。当前用户构建在 30.15 秒宿主时间内推进到 6.5 秒 guest 时间、收到 2 帧，但未确认应用就绪。此前较长观察后的暂停回执超时也保留为限制，不能据此声称应用功能已可用。见 [探针记录](evidence/local-build/user-build-probe.json)。

## 远程 BIN 下载失败后恢复

站点配置[固件下载中继](firmware-relay.md)后，目录内固定 hash 的 GitHub Release 与 Meshtastic BIN 可自动下载。下面的手动入口用于未配置中继、网络故障或中继不可用时恢复。

当跨域 BIN 下载因网络或 CORS 限制失败，且当前固件有固定 SHA-256 时，工作台与 mini 页面会打开恢复对话框。点击原始下载地址保存文件，选回下载好的 BIN，再点击“校验并继续”。校验通过的文件按原始 URL 和 hash 存入现有浏览器缓存，随后继续检查芯片、容量和镜像布局。当前版本、历史与分享来源仍指向原始远程固件；分享接收者若遇到同样的上游限制，也需要下载后导入。

此入口由用户下载并选回文件，不能绕过 CORS，也不会将固件镜像到本站。浏览器缓存最多接受 64 MiB 的单个文件；选错版本、ZIP 或网页内容会因 hash 不符而被拒绝，取消不会启动执行。清单、未固定 hash 的地址、缺失 ROM、上游 HTTP 错误和镜像校验失败不走这条恢复流程。agent API 保持非交互，仍返回 `FETCH_FAILED` 或 `FIRMWARE_RELAY_FAILED`。

验证：`node --test tools/platform-firmware-recovery-test.mjs` 覆盖精确来源匹配、hash 不符、文件大小、取消、缓存不可用，以及恢复后仍须通过镜像布局检查。

2026-10-28 无头 Chromium 实测：在工作台选择 T-Deck 的 Meshtastic 2.7.26.54e0d8d、在 mini 页选择 Bruce 1.16.1，点击运行后均复现上游 CORS 失败。使用对话框的原始链接下载并选回文件，分别校验 2,306,304 字节（`f55469e588ea79ded5fa4fce58900bc0252021ab02e79329dc77bedfce40ab13`）和 4,262,592 字节（`16b57f1371756df6ee6bbc4500bc5ae4c288f6d78a8c25edb55062d7ad120664`），恢复 ROM 启动并产生串口输出；Bruce 输出 320×240 framebuffer。错误文件被 hash 校验拒绝，Bruce 刷新重启时未再次请求远程 BIN。此检查验证下载恢复，不代表固件完整兼容或实机刷写通过。
