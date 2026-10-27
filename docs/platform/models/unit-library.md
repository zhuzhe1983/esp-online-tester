# Unit Button、Angle、RGB 的来源与验证

本批增加 U027、U005、U003 三个独立配件。厂家 STL、照片与尺寸资料已归档；GPIO、ADC 和 SK6812 模块行为尚未实现。不能把插线动画或旋钮外观当成固件可使用配件的证据。

| 配件 | 型号 | 官方接口事实 | 后续最小行为范围 |
| --- | --- | --- | --- |
| [Button](m5stack-unit-button-u027.md) | U027 | 黑 GND、红 5V、黄 NC、白数字输出 | 按下接地、释放恢复分压电平；绑定所选主机端实际 GPIO，测试客户固件输出 |
| [Angle](m5stack-unit-angle-u005.md) | U005 | 黑 GND、红 5V、黄 NC、白模拟输出 | 旋钮值转换为 ADC 输入电压；页面标 0–2500 mV，但两张电路图有 4.7kΩ/10kΩ 冲突，需明确修订或参数 |
| [RGB](m5stack-unit-rgb-u003.md) | U003 | 黑 GND、红 5V、黄灯珠数据输入、白线表格标 NC | 接入实际 GPIO/RMT 波形、解码三颗 SK6812，再以解码输出驱动三处光源；串级白线 DI 与表格有冲突，暂不模拟输出端 |

三款外壳源网格保真及 24×32×8 mm 检查通过。正视轮廓 IoU：Button 0.98742、RGB 0.98492，通过 0.95；Angle 0.94510，保留未通过结果。所有完整模型验收仍为 partial：连接器隐藏尺寸、Angle 旋钮和 PCB 内部没有完整的厂家 CAD。原厂色、金色、紫透、独立纸贴、原生 M5 浮雕、未知 PCB 和插接近景分别保留证据。

离线重建命令示例：

```sh
python tools/rebuild-unit-library.py m5stack-unit-button-u027 --scratch output/unit-rebuild/button
```

安装 `tools/requirements-model.txt` 后运行。脚本验证输入 hash，输出到指定 scratch，不写目录或知识记录。三款 `geometry.bin`、原始 STL 和贴纸均已在独立 scratch 重建并逐字节比较；parts、derivedParts、坐标变换、贴图规则和观察相机也一致。

浏览器证据使用实际通用模型加载器和实际 Grove 插头/线缆函数。启动项目本地 HTTP 服务后，可将 `tools/unit-library-browser-check.js` 中的函数传给已安装 Playwright wrapper 的 `run-code`。下面的独立调用确保结束后关闭自己的浏览器；不要复用他人的 session 名称。

```python
from pathlib import Path
import subprocess

wrapper = Path.home() / '.codex/skills/playwright/scripts/playwright_cli.sh'
command = [str(wrapper), '-s=unit-library-recheck']
try:
    subprocess.run(command + ['open',
        'http://127.0.0.1:8088/platform/review-lane-01.html?id=m5stack-unit-button-u027'], check=True)
    subprocess.run(command + ['run-code',
        Path('tools/unit-library-browser-check.js').read_text()], check=True)
finally:
    subprocess.run(command + ['close'], check=False)
    subprocess.run([str(wrapper), 'list'], check=False)
```

此命令重新生成三个配件自己的截图，不更新来源、模型或现有验证结论。截图改变后应重新审阅并更新对应 `verification.json`，不能沿用旧模型 hash 的结论。摄影透视、采光和材料差异须保留；不要通过拉伸图片或缩放网格提高轮廓分数。
