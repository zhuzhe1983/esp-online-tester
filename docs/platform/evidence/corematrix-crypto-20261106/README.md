# CoreMatrix 原版固件加密向量验证

原版 UIFlow 2.5.4 的 109 组加密测试全部匹配预先固定的结果。此前独立复现的 8 KiB SHA-256 错误，在 C61 提交 `fac00a34` 修正 SHA DMA 的 PSRAM/MMU 访问后消失。详细输入、输出和构建哈希见 [receipt.json](receipt.json)、[修复前结果](before-fix.txt)和[修复后结果](guest-output.txt)。

复现命令及测试源在 `tools/fixtures/corematrix-crypto/`。程序经实际 USB raw REPL 上传，在原版固件的 MicroPython 内运行；所有哈希、加密和断言都由客体 CPU 执行。运行总计 335,000,000 条指令。该快照未打印 CPU 异常计数，因此本记录不宣称具体异常次数。

| 验证范围 | 结果 |
| --- | --- |
| SHA-256 / SHA-1 / MD5，含 padding、DMA、4 KiB边界和多种分块 | 99组全部匹配 |
| RFC 4231 HMAC-SHA256，含超长密钥 | 3组全部匹配 |
| RFC 5246 P_SHA256，128字节输出 | 1组匹配独立主机参考 |
| AES-128/256 ECB、AES-128 CBC加解密 | 6组全部匹配 |
| `cryptolib` CTR mode 6 | 原版API返回 `ValueError: mode`，记录为未暴露 |
| 原生mbedTLS HMAC/PRF入口、GCM/GHASH、TLS握手/证书 | 本测试未直接覆盖，使用独立网络验收 |

修复前，公开输入 `bytes(range(256))*32` 的 SHA-256 返回 `1322c28eb5f9fa9043d40f1dac099aec15ffefe06aa04bd836b8776c08f891e8`，预期为 `dc404a613fedaeb54034514bc6505f56b933caa5250299ba7d094377a51caa46`。短输入129字节的 SHA-256、SHA-1、MD5 已正确。修复后包括此8 KiB输入在内的全部边界和分块输入通过，支持 DMA 访问路径是该错误原因的判断。共享哈希压缩算法、AES算法和ECC数学无需修改。

硬件依据采用与固件 SDK 匹配的 [ESP-IDF v5.5.4 C61 能力定义](https://github.com/espressif/esp-idf/blob/v5.5.4/components/soc/esp32c61/include/soc/soc_caps.h)和 [SHA寄存器定义](https://github.com/espressif/esp-idf/blob/v5.5.4/components/soc/esp32c61/register/soc/sha_reg.h)。C61启用SHA、ECC及PSRAM DMA能力；没有通用 `SOC_AES_SUPPORTED` 或 `SOC_HMAC_SUPPORTED`。Flash XTS-AES是独立用途，不能因此添加一个不存在的通用AES控制器。SHA模式0/1/2、六位DMA块数、DMA_START/CONTINUE以及摘要/消息窗口均按C61头文件核对；此次修复位于芯片DMA内存适配器。

公开向量来源为 [RFC 4231](https://www.rfc-editor.org/rfc/rfc4231.txt)、[RFC 5246 §5](https://www.rfc-editor.org/rfc/rfc5246.html#section-5)、[FIPS 197](https://csrc.nist.gov/pubs/fips/197/final)和 [NIST SP 800-38A](https://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication800-38a.pdf)。来源抓取时间与哈希见 [sources.json](sources.json)。测试不使用网络，也不调整RTC；官方固件的证书时间问题和TLS端到端结果由网络记录单独说明。
