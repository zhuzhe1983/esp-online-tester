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

限制为最多 32 个 Flash 段、单文件 32 MiB、总输入 64 MiB，并且不能超过所选产品容量。构建目录最多检查 4096 个文件名，仅读取 JSON 及其引用的镜像。取消会停止后续解析和初始化；浏览器正在进行的单次 File 读取在返回后丢弃。

`observe().firmware.source` 为 `local`，`localLayout` 记录偏移、字节数与 SHA-256；各镜像没有远程 URL。本地固件不能通过分享 URL 或录制 URL 携带给他人。Web Serial 的本地分段烧录复用相同校验，只写入用户提供的分段，不把模拟内存中填充的 0xff 空洞写入硬件；仍需点击选择串口并核对实物芯片、Flash 容量与安全状态。本轮测试未连接或刷写硬件。

验证：

- `node tools/platform-local-firmware-test.mjs`：路径冲突、缺文件、自定义分区地址、芯片、布局、取消检查通过。
- `node tools/platform-runtime-test.mjs`：内部诊断固件的手动分段和 ESP-IDF 构建模式均经真实 ROM/bootloader 启动并产生帧；分享明确拒绝本地源。
- `node tools/platform-flasher-test.mjs`：分段保持原长度和地址，不写入空洞；使用注入的串口测试替身，没有访问硬件。
- `node tools/probe-local-build.mjs --build <user-build-directory>`：可显式测试其他构建，不复制镜像；只输出匿名化统计与哈希。当前用户构建在 30.15 秒宿主时间内推进到 6.5 秒 guest 时间、收到 2 帧，但未确认应用就绪。此前较长观察后的暂停回执超时也保留为限制，不能据此声称应用功能已可用。见 [探针记录](evidence/local-build/user-build-probe.json)。
