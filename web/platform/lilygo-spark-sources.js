// Same-byte upstream sources for entries whose mutable paths were deleted.
// See docs/platform/lilygo-spark-loading.md for source and hash verification.
const sources = {
  "f6c96c4e9b89cc5dd05a835c43316d6f7c89733da9b077fece222a7f55bce73a": {
    "originalUrl": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/master/firmware/examples/cc1101_send_irq.bin",
    "url": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/68ef3042980e51711c0318efd6cc944999e688db/firmware/examples/cc1101_send_irq.bin"
  },
  "032acb31ff09b04715e04b5db10edf46d8338bb0db5168f119c9e13a16ebc6fa": {
    "originalUrl": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/master/firmware/examples/encode_test.bin",
    "url": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/68ef3042980e51711c0318efd6cc944999e688db/firmware/examples/encode_test.bin"
  },
  "d225a2e21fb629815969f93af83c82686330074f10cb41453044f6a2454108d2": {
    "originalUrl": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/master/firmware/examples/infrared_recv_test.bin",
    "url": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/68ef3042980e51711c0318efd6cc944999e688db/firmware/examples/infrared_recv_test.bin"
  },
  "faf376fada17df5bb67de6b4fd01179757869493504dbacfba5cb54a2ac6ff2d": {
    "originalUrl": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/master/firmware/examples/infrared_send_test.bin",
    "url": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/68ef3042980e51711c0318efd6cc944999e688db/firmware/examples/infrared_send_test.bin"
  },
  "cc63e806e9187c490725df54094edf597cf5c225d512302465742bac611634be": {
    "originalUrl": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/master/firmware/examples/nfc_test.bin",
    "url": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/68ef3042980e51711c0318efd6cc944999e688db/firmware/examples/nfc_test.bin"
  },
  "623d80f759f9eba5615e25ddaf5b2ad5445bbd10f49645efa977a93eeac8aa34": {
    "originalUrl": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/master/firmware/examples/test_display.bin",
    "url": "https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/68ef3042980e51711c0318efd6cc944999e688db/firmware/examples/test_display.bin"
  }
};
export function pinnedSparkSource(url,sha256) {const match=sources[sha256];return match?.originalUrl===url?match.url:url;}
