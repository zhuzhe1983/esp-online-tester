# Meshtastic 2.5.11：显示方向、颜色与原始启动边界

本记录调查未修改的 SenseCAP Indicator Meshtastic 2.5.11.8e2a3e5 镜像在
WASM `abb53cb6a9f768aa811b48aa89ccacb2774d5207e33388a116e352cd1972c995`
上的三次真实捕获。它们已经含有像素，但存在跨帧字节相位漂移；不能作为稳定 UI
通过的证据。来源摘要、完整帧比较和原始捕获哈希见
[rotation-color-startup.json](evidence/seeed-indicator-lora-20261122/rotation-color-startup.json)。

## 固件颜色与 180° 方向

[固定发行源码 TFTDisplay.cpp](https://github.com/meshtastic/firmware/blob/v2.5.11.8e2a3e5/src/graphics/TFTDisplay.cpp)
将默认前景定义为 `COLOR565(0x67, 0xEA, 0x94)`。标准 RGB565 量化以及所调查的
[LovyanGFX color565 实现](https://github.com/mverch67/LovyanGFX/blob/894e4340c94d78b1fb56d180185f773cc1f45099/src/lgfx/v1/misc/colortype.hpp)
均得到 **`0x6752`**。11.06 秒捕获恰好只有该绿色与黑色。

同一份固定发行源码对 `SENSECAP_INDICATOR` 明确执行 `setRotation(2)`。
[Panel_FrameBufferBase](https://github.com/mverch67/LovyanGFX/blob/894e4340c94d78b1fb56d180185f773cc1f45099/src/lgfx/v1/panel/Panel_FrameBufferBase.cpp)
在默认旋转偏移为 0 时，将写像素的坐标变为 `x = 479 − x`、`y = 479 − y`，
随后直接写入帧缓冲。这条旋转路径不发送 MADCTL；所调查的
[Panel_ST7701 初始化表](https://github.com/mverch67/LovyanGFX/blob/894e4340c94d78b1fb56d180185f773cc1f45099/src/lgfx/v1/platforms/esp32s3/Panel_RGB.cpp)
也没有写入 `0x36` 或 `0xC7`。因此，画面 180° 方向与客体自己的软件旋转一致，
本次没有额外翻转宿主图像或更改固件。这不等于完成了实体面板安装方向的校准。

来源限制仍然适用：Meshtastic 固定发行配置只依赖 `LovyanGFX#develop`，未锁定
构建提交。`894e4340` 是发布日前的历史候选源码，不能冒充已经证实的二进制
构建依赖。每份取回的源码正文均核验了 Git blob SHA，并保留 SHA-256、字节数、
URL 与获取时间。

## 不是另一种配色：整帧的奇数字节循环偏移

三次捕获的 RGB565 色值计数如下。RGBA 到 RGB565 的恢复逐像素通过反向展开
检查，可准确恢复这批颜色。

| 捕获 | 客体秒 | `0x0000` | `0x6752` | `0x5267` | `0x0067` | `0x5200` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 启动 | 8.00 | 228,221 | 0 | 1,039 | 570 | 570 |
| 显式数字对端包后 | 11.06 | 228,791 | 1,609 | 0 | 0 | 0 |
| 短按后 | 12.10 | 228,221 | 0 | 1,039 | 570 | 570 |

正确前景的字节对是 `[0x67, 0x52]`。在相差一个字节的位置重新配对，会得到
`0x5267`；前景与黑色边界则会产生 `0x0067` 和 `0x5200`。这解释了黄褐色及
另外两种只在边界出现的颜色。

比颜色观察更强的证据是**全部 460,800 字节的精确匹配**。将每个捕获的像素
恢复为 RGB565，并统一按高字节、低字节序列化，以 11.06 秒捕获为参考：

| 比较 | 参考字节流的循环左移 | 等价有符号偏移 | 全部字节相等 |
| --- | ---: | ---: | --- |
| 启动捕获 | 27 字节 | +27 字节 | 是，460,800/460,800 |
| 短按后捕获 | 460,643 字节 | −157 字节 | 是，460,800/460,800 |

两种偏移均为奇数。这说明颜色交换和横向位置漂移属于同一字节流相位问题，
不需要通过改主题、改面板旋转或动态调整 GPIO 数据线顺序解释。

精确比较算法如下；它不修改、裁剪、插值或旋转输入图像：

```python
def canonical_rgb565(rgba):
    result = bytearray()
    for i in range(0, len(rgba), 4):
        r, g, b, alpha = rgba[i:i + 4]
        value = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)
        assert alpha == 255
        assert (r, g, b) == (
            ((value >> 11) * 255 + 15) // 31,
            (((value >> 5) & 63) * 255 + 31) // 63,
            ((value & 31) * 255 + 15) // 31,
        )
        result.extend(value.to_bytes(2, "big"))
    return bytes(result)

reference = canonical_rgb565(reference_rgba)
candidate = canonical_rgb565(candidate_rgba)
offset = (reference + reference).find(candidate)
assert 0 <= offset < len(reference)
assert (reference + reference)[offset:offset + len(reference)] == candidate
```

原始截图和 RGBA 中可能包含固件生成的设备标签，未提交原始载荷。收据保留其
精确哈希、颜色计数、比较算法与偏移，负结果不被后续修正替换。

## 启动画面和按键：源码预期与实际进展分别记录

[Screen.cpp](https://github.com/meshtastic/firmware/blob/v2.5.11.8e2a3e5/src/graphics/Screen.cpp)
的 `logo_timeout` 常量是 **5,000 ms**。计时起点在屏幕初始化的两次
`ui->update()` 完成后才设置为 `serialSinceMsec = millis()`；不是从仿真重置
瞬间计时。在本次串口中，这一初始化阶段约为客体日志的 5 秒。随后必须由
`Screen::runOnce()` 实际执行到超时判断，才打印 `Done with boot screen...`
并切换帧。LoRa region 为 `UNSET` 时，源码会改为只有一个欢迎帧的帧集；
短按并不保证产生视觉不同的下一页。

[ButtonThread.cpp](https://github.com/meshtastic/firmware/blob/v2.5.11.8e2a3e5/src/ButtonThread.cpp)
按低有效方式读取 GPIO38，任务轮询周期为 50 ms，消抖为 1 ms。
[ButtonThread.h](https://github.com/meshtastic/firmware/blob/v2.5.11.8e2a3e5/src/ButtonThread.h)
设置 250 ms 单击判定窗口。短按经 `EVENT_PRESS` 到
[PowerFSM](https://github.com/meshtastic/firmware/blob/v2.5.11.8e2a3e5/src/PowerFSM.cpp)
和 `screen->onPress()`，在画面处于固定帧状态时才调用 `nextFrame()`。
长按为 5 秒，且长按回调另有启动后 30 秒的屏蔽条件；不应把启动早期的长按
当作一定能退出 logo 的操作。

截至 12.1 客体秒，本次串口只观察到 Screen 在日志秒 5 启动、最后一次
`haveGlyphs` 在日志秒 7、NimBLE 初始化标记在日志秒 8，没有 boot-screen
退出标记或短按 `press!` 标记。显式数字 LoRa 包的接收日志则确实出现。
这些是不同的执行路径，不能把无线接收成功推导为页面或物理按键已经验证。

源码中，PowerFSM 在设置后等待 3 秒，从 BOOT 进入 USB POWER 状态，调用
`setBluetoothEnable(true)`。
[NimbleBluetooth::setup](https://github.com/meshtastic/firmware/blob/v2.5.11.8e2a3e5/src/nimble/NimbleBluetooth.cpp)
打印该初始化标记后调用 `NimBLEDevice::init()`。**最后一个串口标记不能证明
实际阻塞 PC 或蓝牙控制器故障**；本记录没有做 PC/栈采样，不将此推测写成定位
结论，也没有禁用 BLE 或预填 region 来制造页面成功。

后续有界验收可以保留 8 秒初次捕获，再以 2 客体秒为一阶段观察到 10、12、14、
16 秒，检查真正的超时标记与画面。看到正常页面进展后，GPIO38 拉低 200 ms，
释放并再运行至少 1 秒，检查固件自己的按键标记。若默认镜像仍无 Screen 或按键
进展，应保留未验证结论；这里只提出该验收条件，没有宣称已执行。
