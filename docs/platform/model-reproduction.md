# 从公开文件重建产品模型

已有模型可通过 `tools/rebuild-product-model.py` 重建，不需要 `.butler` 任务目录、历史队列、`output/source` 缓存或原作者机器。它读取已提交的 `assets/<产品ID>/model-plan.json`、`model.json` 和 `docs/platform/evidence/<产品ID>/source-audit.json`，默认只使用模型资产目录中保留的原厂 CAD、原始照片及许可文件。

## 安装与执行

推荐 Python 3.11。首次使用安装模型工具的固定依赖：

```sh
python3.11 -m venv .venv-model
.venv-model/bin/python -m pip install -r tools/requirements-model.txt
```

先查看输入清单。此命令只用 Python 标准库，不下载文件、不导入 CAD：

```sh
python3 tools/rebuild-product-model.py plan m5stack-c008
```

在新建临时目录中完整重建 ATOM Lite：

```sh
MODEL_REBUILD_DIR="$(mktemp -d)"
.venv-model/bin/python tools/rebuild-product-model.py build m5stack-c008 \
  --scratch "$MODEL_REBUILD_DIR"
cat "$MODEL_REBUILD_DIR/comparison/m5stack-c008.json"
```

将 `m5stack-c008` 换成其他具有通用 `model-plan.json` 的产品 ID 即可，例如 `m5stack-k132`、`m5stack-k132-v11`。这两款已提交的计划包含逐键三角形分区，正常重建无需再次运行历史分键脚本。STL、STEP、分组、刚体变换、透明贴纸遮罩及照片透视校正均沿用该产品的明确计划，不自动拟合或缩放厂家几何。

如只需恢复经核对的输入，使用 `hydrate`：

```sh
.venv-model/bin/python tools/rebuild-product-model.py hydrate m5stack-k132 \
  --scratch "$MODEL_REBUILD_DIR"
```

`hydrate` 可重复运行并复核缓存；`build` 不覆盖已有重建输出，重复比较请换一个新的临时目录。`--scratch` 不允许指向仓库的产品、工具、知识、证据或任务状态目录。

## 缺失来源与校验

每个必需输入均核对回执中固定的字节数与 SHA-256。离线源缺失会明确列出来源 ID、URL 和预期哈希；只有显式传入 `--allow-network` 才下载缺失的公开来源：

```sh
.venv-model/bin/python tools/rebuild-product-model.py build m5stack-c008 \
  --scratch "$MODEL_REBUILD_DIR" --allow-network
```

下载仅接受 HTTPS，单文件上限 64 MiB，网络读取超时 45 秒；返回内容的字节数或哈希变化即失败，不自动采用新版本。已存在但损坏的本地源也直接失败，不静默换源。KiCad 内嵌 STEP 优先复用仓库中保留的 payload；若缺失，恢复并验证原容器后执行 Base64/Zstandard 解码与 KiCad 校验，再核对 payload 的固定 SHA-256。

本轮80份通用计划已检查：78份的全部必需来源都保留在仓库；`m5stack-k056` 与 `m5stack-c056-b` 各有一张仅用于纹理的原始照片需要 `--allow-network`，其URL、字节数和哈希已在公开回执中固定。

只恢复构建实际使用的 CAD、参考照片、纹理源和许可文件，不抓取所有历史页面/PDF，更不会下载固件。来源原始抓取时间保持不变，临时目录中的 hydration 回执另记本次取得或核对时间。

## 输出与通过条件

所有新文件都写入指定临时目录：

| 路径 | 内容 |
| --- | --- |
| `sources/<ID>/` | 经验证的输入和恢复出的 prepared/components 数据 |
| `hydration/<ID>.json` | 本次输入取得方式、时间、固定哈希 |
| `rebuilt/<ID>/` | 重新导出的几何、纹理和模型元信息 |
| `evidence/<ID>/` | 本次导出的尺寸检查及源记录，不覆盖原验证证据 |
| `comparison/<ID>.json` | 与已提交模型的逐资产、逐配置比较 |

`build` 返回成功要求：实际重新生成的几何、原始参考文件、许可、纹理哈希，以及 parts、坐标变换、派生表面、接口、屏幕和相机等模型配置均与已提交版本一致。`modelJsonByteIdentical` 另报元信息文件是否字节一致；历史遗留但本次计划不使用的资产列在 `unusedOriginalArtifacts`，不伪装为本次生成结果。

这证明可从固定输入重建现有模型，不会把原有尺寸失败或 `partial` 状态变成通过，也不替代重新做浏览器照片对照、固件或真实硬件测试。没有通用计划的旧定制渲染器，以及没有模型的 FacesKit，会明确拒绝此入口，应使用各自记录的生成流程。

原 `onboard-lane-*.py prepare/publish` 是首次批量资料收集和提交的历史编排入口，依赖当时的队列和网页语料，不是公开模型重建入口。新建产品仍按 `skills/esp-product-onboarding/SKILL.md` 完成来源、模型、知识和行为审核；已有模型复现使用本页命令。

## 测试

```sh
python3 tools/test-rebuild-product-model.py
```

该组测试不访问网络、不导入 CAD，覆盖离线恢复与缓存、修改后的源、来源回执冲突、缺源时默认禁网、HTTPS/凭据限制、目录和符号链接越界，以及生成文件的真实字节比较。另已实际离线重建 `m5stack-c008`：5 个生成资产及模型配置一致；原产品的 14 个资产、证据、知识文件哈希保持不变。
