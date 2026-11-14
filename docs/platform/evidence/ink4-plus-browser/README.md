# INK4 Plus 浏览器执行验收

2026-11-15，以未修改的厂家完整 BIN、S31 rev0 ROM 和真实 WASM worker 验收。
输入文件、构建哈希、实际客体时间和结果见 [verification.json](verification.json)；
截图与原始回执的大小、SHA-256 见 [artifacts.json](artifacts.json)。
Flash 为 16 MiB，PSRAM 的 16 MiB 是模拟配置，实物容量仍未知。

## 结果与边界

| 构建 / 固件 | 实际结果 |
| --- | --- |
| UART 修复前 / 2.0.68 | 显示 684×1216 中文主页并挂载 64 MiB SD；初始化停在约 3.483 客体秒。有效长度的触控与按键没有引起新输出，未验收交互。两次不足 1 ms 的早期触控不用于判断固件行为。 |
| UART 修复后 / 2.0.68 | API 与 2D 画布触控打开设置；盖板 HOME 返回主页、PREV 返回上级；工具栏音量加减使固件音量变为 80、70。NEXT 在显示测试页触发独立的新测试刷新。SD 的 26 个修改页保存后，由新 worker 恢复，完整 64 MiB 导出镜像 SHA-256 一致。 |
| 最终双门控构建 `56afb196` / 2.0.79 | 显示 684×1216 中文主页；客体输出 SD 挂载成功、名称 SIMSD、容量 64 MiB，产生 9 个修改页。设置触控保持 0.191725 客体秒，固件报告 13 个设置页签，帧号从 7 变为 15；原生截图已目视检查。 |

2.0.68 的设置触控刚松开时帧号仍为 10；等待客体刷新后为 20，设置页截图确认实际显示。
NEXT 的证据也与首次进入测试页区分：进入页面后在 244873 ms 完成普通全屏刷新；
随后于 245.723–245.921 秒按下并松开 NEXT，在 248323 ms 产生新的清屏测试刷新，
帧号从 62 变为 77。[固定版本源码的按键处理](https://github.com/CloudZao/Metalio-E-INK4-Plus/blob/ad8edf4150a688737c0d7c00cae4413079b262ef/main/display/screen/settings_screen/settings_test/settings_test_display_screen.cc)
将 NEXT 映射到 `RunGray()`；页面创建本身只请求全屏提交。

这些结果不证明完整整机兼容。显示测试的灰阶尚未校准，未验收模拟波形、残影或实物刷新时序。
音量设置变化不证明播放成功，外接音频模块仍报告 `mode_ack` 失败。
显示测试页持有 `StandbyInhibit` 时短按 POWER 未出现新输出，电源休眠与唤醒仍未验收。
按键结论只涉及工具栏和 API 的语义输入，3D 侧键的物理顺序未验收。
SD 恢复验证的是显式保存后重新配置并加载新 worker，不涉及浏览器崩溃或断电。
最终 2.0.79 只重复主页、SD 挂载和设置触控；其余按键与恢复结论属于 2.0.68。
网络中继已关闭，没有联网、SoftAP 或云端功能结论。

## 复现步骤

按平台文档准备对应 WASM 与 ROM，用静态 HTTP 服务提供 `web/`。
在独立、无既有设备数据的浏览器上下文中运行；不要清空用户保存的 Flash 或 SD。
以下 `PREVIEW_URL` 由调用者指定为该部署的 `platform/index.html` 地址。
Playwright 包装器默认以无头模式启动；使用同一个任务会话，并在失败时同样关闭。

```sh
command -v npx
PWCLI="$HOME/.codex/skills/playwright/scripts/playwright_cli.sh"
: "${PREVIEW_URL:?Set the platform preview URL}"
"$PWCLI" open "$PREVIEW_URL"
"$PWCLI" snapshot
# 完成下列 UI / API 检查后：
"$PWCLI" close
"$PWCLI" list
```

2.0.68 首轮通过工作台选择 INK4 Plus、官方 2.0.68，点击下载并尝试运行并确认。
2.0.79 使用公共 API 指定回执中的完整 BIN 地址与 SHA-256：

```js
const api = window.espTester;
const {loadCatalog, createDefaultScene} = await import('./catalog.js');
const catalog = await loadCatalog();
const scene = createDefaultScene(catalog, 'metalio-ink4-plus');
scene.firmware = {
  url: 'https://cloudzaoai.oss-rg-china-mainland.aliyuncs.com/OTA/xiaozhi/aaaee01e-1595-49fc-995d-43a31935312d.bin',
  sha256: '56786c7407405993fe1b049446d8535f4cc7e72c99953fff5f2662c7aa2cfbc0',
};
await api.configure(scene);
await api.configureNetwork({enabled: false});
await api.loadFirmware();
await api.start();
```

通过包装器的 `run-code` 在 `page.evaluate` 中执行页面 API。等待实际主页及
`SdCardManager: SD ok at /sdcard`，读取 `observe().firmware` 核对文件哈希。
用 `capture({format:'png'})` 保存原生设备像素，不能拿产品 3D 预览代替执行结果。
以下触控方法按客体时间保持输入，记录实际回执，超时后仍释放输入：

```js
const before = await api.observe();
const down = await api.input({type:'touch', x:545, y:1015, down:true});
const deadline = performance.now() + 60000;
try {
  while ((await api.observe()).stats.time < down.guestTime + 0.16) {
    if (performance.now() >= deadline) throw new Error('Guest-time hold timed out');
    await new Promise(resolve => setTimeout(resolve, 50));
  }
} finally {
  const up = await api.input({type:'touch', x:545, y:1015, down:false});
  console.log({guestHoldSeconds:up.guestTime-down.guestTime});
}
// 等待新的 SettingsScreen 日志及帧更新，再保存 capture({format:'png'})。
```

按键使用 `input({type:'button', id, pressed:true|false})`，同样按客体时间保持约
0.16 秒。用 `home` 从设置页返回，`previous` 从设置页后退；工具栏操作
`volume-up` / `volume-down` 后核对新的固件音量输出。为观察 `next`，在设置页点击
测试页签 `(100,820)`，再点显示测试 `(440,490)`，待首次页面刷新完成后单独按 NEXT。
这些坐标和测试页路径属于固定 2.0.68，不推定其他版本相同。
每次操作前记录串口位置与帧号，分别检查新日志和实际画面；排队成功不算验收通过。

保存恢复步骤为 `pause()` → `flushDeviceStorage()` → `flushSdCard()` →
`exportSdCard()`，计算全部镜像字节的 SHA-256；然后 `configure(scene)`、
`loadFirmware()`，在再次 `start()` 之前导出并比较哈希与恢复页数。
期间不导入或重新创建空卡。检查完成后暂停模拟，关闭本任务浏览器并验证会话退出，
停止本任务静态服务器。

## 证据保留

原生截图与详细回执保留在清单列出的本地、忽略目录 `output/playwright/`，未上传或提交。
复查或清理工作区前必须先保存这些文件并核对清单；不能删除唯一副本。
提交的摘要不含完整串口流和逐事件数组，省略设备 UUID 与会话标识，不影响输入、
画面变化和镜像完整性结论。运行顺序、失败结果、输入来源与构建哈希均保留。
本次全部浏览器会话和静态服务器已关闭。
