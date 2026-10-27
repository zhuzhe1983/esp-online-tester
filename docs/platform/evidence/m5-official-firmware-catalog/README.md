# 官方固件补录证据

- `mapping.json`：人工核对的项目、型号、版本分支；同类别不同硬件不自动互配。
- `sources.json`：首次 GET 的 URL、最终地址、UTC 秒、媒体类型、字节数和原始响应 SHA-256。
- `metadata.json`：用于离线重放的精选公开字段，每个项目保留前 10 个版本。原始响应作者、账号、图片、统计和无关描述已删除，不提交私有原始文件。来源 hash 指向原始响应；本文件 hash 另见 `checks.json`，二者不应相同。
- `sync-requests.json`：最终真实 provider 流程的 114 个元数据 GET 收据；没有请求 `/download`。
- `negative-results.json`：列表筛选失效、旧版本非 `.bin` 名称及初次本地缺输入问题；保留失败而不改 golden。
- `browser.json` / `stopwatch-library.png`：实际 UI 选择与确认取消检查，未运行任何固件。
- `unmapped.json`：没有足够依据的项目/SKU，后续按具体型号继续核查。

复现目录与同步逻辑：

```sh
python tools/validate-platform-catalog.py
node --test tools/platform-m5burner-v1-sync-test.mjs tools/platform-firmware-sync-test.mjs tools/platform-watchlist-test.mjs
node tools/sync-firmware-catalog.mjs --snapshot-only --dry-run
# 访问真实发布元信息；只检查，不下载 BIN、不改目录：
node tools/sync-firmware-catalog.mjs --dry-run
```

完整 Node 检查需要项目文档所列本地 ROM/WASM 输入：`ESP32SIM_ROM_DIR=<roms> node --test tools/platform-*-test.mjs`。本轮复用了本地已构建 WASM，只供替身 worker 测试读取；没有重新编译或执行客体。

浏览器复现：按 AGENTS 通过 Playwright wrapper 以独立 session/headless 打开工作台，然后 `run-code "$(cat docs/platform/evidence/m5-official-firmware-catalog/browser-check.js)"`。脚本只选择固件、打开确认框并取消；结束用同 wrapper/session 关闭。HTTP 端口由调用者选择，不复用用户会话。

无 publisher SHA 的新官方版本可以自动推进为未验证 trial，固定版本 ID 不代表内容不可变；实际摘要、芯片和启动布局由用户浏览器加载时检查。同版本已校验来源保留，其他 provider 的缺 hash 候选不自动推进。没有实际固件/实机兼容性验收。
