# GitHub Pages 模型资源发布

仓库保留厂家 CAD、原始模型、来源清单和重建输入。Pages 使用独立的发布目录，保留运行时实际引用的几何、贴图、参考照片、来源记录、许可，以及已有模型审阅页提供的下载文件。发布过程不修改 `web/` 内的原件。

```sh
node tools/stage-platform-pages.mjs web target/pages-new
node tools/check-staged-model-assets.mjs web target/pages-new target/pages-model-check.json
```

目标目录必须不存在。脚本拒绝覆盖源码或已有发布目录，并把发布目录限制在 1,000,000,000 字节以内。GitHub Pages workflow 在上传前对同一个目录运行模型完整性检查。

## 无损几何传输

通用模型加载器支持原始二进制、`gzip`、`gzip-xor-shuffle-24` 和 `gzip-split-indexed-12`。后两种格式只改变传输方式，不改变顶点、法线、三角形顺序、精度或浮点位模式。

- `gzip-xor-shuffle-24`：每个顶点的六个 float32 共 24 字节。按字节列排列，各列相邻行异或后 gzip；解码时逆向恢复。
- `gzip-split-indexed-12`：解压后的前 12 字节是三个 little-endian uint32，分别为原始字节数、位置字典条目数、法线字典条目数。随后依次是位置字典、法线字典、位置索引和法线索引。字典以 12 字节为一行，索引以 4 字节为一行，各自按上述列排列与异或规则编码。字典以原始字节判重，解码后恢复原始的 24 字节顶点流。

发布器比较压缩结果，选择较小的格式；三个既有专用加载器仍使用原始二进制。浏览器分别限制下载、解压及重建后的大小，默认上限均为 64 MiB，并校验步长、字典长度与索引范围。元数据中的负零和原始来源哈希保持不变，额外记录发布文件的 SHA-256。

检查器逐个比较源码与发布目录的解码后几何字节，并检查元数据、来源哈希、贴图及参考照片。声明了 `sharedAssetProductId` 的共用参考照片只有在 `sourceSha256` 一致时才可生成发布别名。

## 缓存与复现

发布器在自有模块的 import、Worker、WASM URL 和 HTML 入口资源上附加相同提交版本，确保新元数据与对应解码器一起加载。第三方 vendor 文件保持原样。

相关验证：

```sh
node --test tools/platform-model-binary-test.mjs tools/platform-pages-stage-test.mjs
```

模型可加载仅表示几何和配置有效。实际固件执行应另行检查 UART、帧缓冲及输入后的输出，不能以模型预览代替。
