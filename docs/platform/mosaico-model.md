# ESP-Mosaico V1.2 产品资料与模型

条目 `espressif-esp-mosaico` 对应乐鑫 **CoreBoard V1.2 / BaseBoard V1.2**。外观为照片与显示屏图纸推算，**不是厂家 CAD，没有通过整机 1:1 或制造尺寸验收**。产品运行和固件输出由[独立运行报告](mosaico-runtime.md)记录；本页的黑色屏幕截图仅用于结构检查。

## 官方来源与版本

- [硬件指南固定提交](https://github.com/espressif/esp-dev-kits/blob/1b4c83bcd8662b8339d5b975a088407ed6a09e7c/docs/en/esp-mosaico/user_guide.rst)区分 V1.0/V1.2：V1.2 内部 I2C 为 SDA56/SCL3，扩展 I2C 为 SDA0/SCL1；LCD RST44、SCL42。不能混用两个版次。
- V1.2 使用 ESP32-S31NRV16，16 MiB NOR、16 MiB 封装内 PSRAM、128 MiB SPI NAND；480×480 CO5300 QSPI AMOLED 与 CST9220 触控；ES8311/NS4150B 音频、BMI270、双 BMM150、BQ27220、65 mAh 电池。没有把 NAND 声明成 TF 卡槽。
- [官方 FAQ](https://mosaico.ai/en/faq)将外壳文件指向 [ESP_MOSAICO 的 MakerWorld 账户](https://makerworld.com/zh/@ESP_MOSAICO)。2026-11-14 正常直接请求、浏览工具和独立无头浏览器均遇 HTTP 403 / 安全验证，未取得原文件。没有尝试绕过验证。官方 esp-dev-kits、BSP、utils、vibe、claw 仓库树也未找到适用外壳 STEP/STL 或板级 CAD。
- `H0216F002AMT004-1 V1` 显示屏图纸给出有效区 **38.39×38.39 mm**，TP 盖板 **43.3×43.3×0.7 mm**。整机制造尺寸没有找到；外壳 45×45×12.5 mm 是照片约束下的估计，XY 约 ±1 mm、厚度约 ±2 mm，不是公差。

所有实际取得资料的原始 URL、最终 URL、UTC、字节数、SHA-256，以及仅发现但未取得的 CAD 入口，见 [sources.json](evidence/mosaico-sources/sources.json)和[产品知识记录](../../web/platform/knowledge/espressif-esp-mosaico.json)。原始资料与照片保留厂商权利；本站没有复制参考照片作产品纹理。生成几何和自行排版的两张文字 SVG 使用本仓库许可，商标仍属于乐鑫。

## 外观和未知结构

模型以毫米工作；+X 为屏幕右方，+Y 为上方，+Z 为屏幕朝外。没有导入后拉伸或用照片覆盖固件显示。`front-cover` 上表面 Z=6.55，`live-display` Z=6.56，显示面位于盖板之上 0.01 mm，直接使用调用者的 framebuffer 纹理。

BOOT 位于底部 USB 左侧，AI 位于顶部右侧；分别关联真实 GPIO61 和 GPIO7。底部右侧 POWER 只呈现外观，因为它连接 SAM8108 电源控制器，不能当普通 GPIO。按键长宽、位置、凸起高度、顶面孔洞和外壳圆角均为照片估算，未认证其精确轮廓。背板文本朝外可读；QR 码没有伪造。四颗背部螺丝只保留可见头部近似，不补猜隐藏螺纹、磁铁或安装方向。

左右 2×10 孔采用官方 **2.54 mm 节距**；绝对位置、孔宽和深度只是视觉估计。USB 具有独立口沿和暗色插入区域，但通用可视口不代表实物连接器型号或插合深度。所有连接均保持 `allowConnections: false`，没有未经核实的线缆。左右口的**逐针编号**单独在产品 interfaces 中按官方表记录，不以背面照片的镜像顺序作线序。

`pcb-main`、`pcb-base` 为两个稳定的 **UNKNOWN** 占位层。未推测内部元件、走线、板厚、板形、 speaker/battery 装配尺寸。分开查看时显示/两层板按厚度方向分层，外壳另移到侧下方；偏移只是查看器安排，不宣称真实拆装距离。侧孔与顶孔的浅表外观标记随外壳移动。反复展开/复位 20 次测试无累积位移。截图 [`mosaico-expanded.png`](evidence/mosaico-model/mosaico-expanded.png)及[紫色透明视图](evidence/mosaico-model/mosaico-expanded-purple.png)均使用完整装配，无隐藏遮挡件，两层 UNKNOWN 可见。

## 尺寸和照片检查

[核验回执](evidence/mosaico-model/receipt.json)绑定最终模型和二进制 hash。Three.js 完整装配包围盒为约 **45.22×46.10×13.185 mm**，包括按键、端口外观和显示辅助面；它与 45×45×12.5 mm 的壳体约束范围不同。没有独立整机图纸，不能把这个数值作为“尺寸通过”。盖板导出顶点为 43.3×43.3 mm，显示面为 38.39×38.39 mm，与独立图纸值的数值差小于 0.001 mm。

以两张官方 V1.2 照片的盖板直边外推角点标定透视相机，未改变模型几何。手工角点不确定度约 ±5 px；参考角点、相机矩阵、投影与误差保存在 [alignment.json](evidence/mosaico-model/alignment.json)。正斜视盖板拟合 RMS 为 6.77 px，壳体四角平均偏差 9.64 px；另一斜视分别为 3.39 px 和 6.84 px。见[正斜视轮廓叠图](evidence/mosaico-model/front-outline-overlay.svg)、[另一斜视叠图](evidence/mosaico-model/isometric-outline-overlay.svg)和对应 `mosaico-fit-*.png`。灰线是标定用盖板，青线是独立外壳照片角点，橙线为模型投影。

这些只是外轮廓地标检查，**没有通过完整照片验收**：参考图带透视/标注，机壳厚度、按钮细节、侧面印字、孔形、颜色、材料和内部结构仍有差异；背面与底部做了人工方向/数量核对，没有取得独立正交照片用于定量对齐。不把同一模型投影充作参考照片。可从固定来源下载原图并核对 hash，在相同像素坐标叠加自己的渲染。

无头浏览器检查了原色、金色、半透明紫色、前后/底部/斜视、展开复位和无模型载入错误；只有外壳随材质变化，盖板、按键、USB、PCB 与背字保留自身材质。入口 [`mosaico-model-review.html`](../../web/platform/mosaico-model-review.html)为可复查的独立查看器。运行场景的 2D/3D、分享和 mini 验收属于运行适配任务，不从此页静态检查推断。

## 固件调查

官方恢复页当前发布 **Hello 0.1.3 + Vibe Mode 0.3.0**，`firmware/4`，2026-11-12，16,777,216 字节，SHA-256 `7562ff67cad8280fe8fdd06f3be7d6c91f44e5a8cc63fe596721f48da428af9b`。官网 FAQ 同时称预装应用为 Mosaico-Claw；当前恢复镜像与那个应用不是同一构建。目录把官方恢复项作为默认，仍标 `trial`，不将来源可信或能够启动等同完整兼容。

[恢复审计](evidence/mosaico-sources/factory-audit.json)保留调查与 CORS 限制。官方端点针对 GitHub Pages Origin 没有返回 Access-Control-Allow-Origin；若“下载并运行”提示跨域失败，请打开[官方恢复页](https://mosaico.ai/en/firmware)下载完整 BIN，再点“本地文件”选择该 BIN 并运行。本站不提供镜像或绕过跨域的代理。

关注列表通过专用 `espressif-mosaico` 适配器读取官方 `/api/v1/firmware` 元数据，校验 S31 目标、镜像名称和对应下载 ID；不把 utils 源码仓库的 Release 当作恢复固件版本。请求不携带 GitHub 凭据，不下载 BIN，不留存发布说明或下载计数。官方元数据没有 SHA-256，故只监测版本，自动替换继续禁用，目录仍使用本轮核验的固定镜像摘要。元数据出错时保留上次结果；小智条目同样锁定已调查 ZIP，自动更新未启用。

小智主项目 v2.6.0 有精确 `espressif-esp-mosaico` ZIP，内含单个 `merged-binary.bin`；镜像 SHA-256 `440797fe7dbf952dc79126002e32e9a2d9ec4f2ad7752989ad74e7ed0dac9eb2`，11,374,509 字节，bootloader 位于 0x2000，chip ID=32，作为独立 `trial` 提供，未验证执行/云服务/实机。ZIP 与内部镜像摘要分别记录在[小智回执](evidence/mosaico-sources/xiaozhi-firmware-receipt.json)，不会把 ZIP hash 当内部 BIN hash。UIFlow 官方仓库 6,830 个路径中未找到 mosaico 目标；这只界定本轮公开来源搜索，不推断未来适配不存在。

## 复现

```sh
python tools/build-mosaico-model.py --output output/mosaico-rebuild
cmp web/platform/assets/espressif-esp-mosaico/model.json output/mosaico-rebuild/model.json
cmp web/platform/assets/espressif-esp-mosaico/geometry.bin output/mosaico-rebuild/geometry.bin
# 独立环境安装 numpy / scipy 后：
python tools/verify-mosaico-model.py
node --test tools/platform-mosaico-model-test.mjs tools/platform-knowledge-test.mjs
python tools/validate-platform-catalog.py
# buildless 预览：
python -m http.server 8179 --directory web
```

访问 `/platform/mosaico-model-review.html`，选择“分开查看”和“分层斜视”可复查 UNKNOWN。浏览器使用 Playwright skill wrapper 的独立无头会话并在完成后关闭。模型自检覆盖真实纹理引用、按键最近射线命中、电源键边界、未知层稳定 ID、复位和禁止未经核实连接，不写模拟成功的假回执。
