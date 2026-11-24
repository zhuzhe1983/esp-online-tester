// Exact runtime profiles; memory and physical TF presence are separate from full peripheral acceptance.
// Source and runtime limits: docs/evidence/waveshare-* and each product knowledge record.
export const WAVESHARE_BOARD_CAPABILITIES=Object.freeze(Object.assign(Object.create(null),{
  "waveshare-s2-pico": {
    "chip": "esp32s2",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "flashMb": 4,
    "psramMb": 8,
    "boardButtons": []
  },
  "waveshare-s2-lcd096": {
    "chip": "esp32s2",
    "display": true,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "flashMb": 4,
    "psramMb": 8,
    "boardButtons": []
  },
  "waveshare-s3-touch-amoled143c": {
    "chip": "esp32s3",
    "flashMb": 8,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c5-lcd273": {
    "chip": "esp32c5",
    "flashMb": 32,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c5-touch35": {
    "chip": "esp32c5",
    "flashMb": 32,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-lcd085": {
    "chip": "esp32c6",
    "flashMb": 8,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-lcd19": {
    "chip": "esp32c6",
    "flashMb": 8,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-lcd19": {
    "chip": "esp32c6",
    "flashMb": 8,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-lcd169": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c6-touch-lcd147": {
    "chip": "esp32c6",
    "flashMb": 8,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-epaper133e6": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-lcd128": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 2,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd128": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 2,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-zero": {
    "chip": "esp32s3",
    "flashMb": 4,
    "psramMb": 2,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-pico": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 2,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-nano": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-devkit-n8r8": {
    "chip": "esp32s3",
    "flashMb": 8,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd21": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd21b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-lcd28b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd28b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-lcd28c": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd28c": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c5-lcd147": {
    "chip": "esp32c5",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c5-touch169": {
    "chip": "esp32c5",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c5-touch28": {
    "chip": "esp32c5",
    "flashMb": 32,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-p4-lcd35-v13": {
    "chip": "esp32p4",
    "display": true,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 100
  },
  "waveshare-p4-lcd35-v32": {
    "chip": "esp32p4",
    "display": true,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-core": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": false,
    "rgb": false,
    "sdCard": false,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-core-32mb": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": false,
    "rgb": false,
    "sdCard": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-module-16mb": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": false,
    "rgb": false,
    "sdCard": false,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-module-32mb": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": false,
    "rgb": false,
    "sdCard": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-module-16mb-ipex": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": false,
    "rgb": false,
    "sdCard": false,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-module-32mb-ipex": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": false,
    "rgb": false,
    "sdCard": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-core-dev-kit": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-nano": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-module-dev-kit": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-dev-kit": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-eth": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-poe-eth": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-pico": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-poe-eth": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-db": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-nano-wifi6-db": {
    "chip": "esp32p4",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-s3-rgb-matrix-p2-64x64": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rgb-matrix-p2-64x64-b": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rgb-matrix-p3-64x64": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rgb-matrix-p3-64x64-f": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rgb-matrix-p5-64x32": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rgb-matrix-p25-64x32": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rgb-matrix-p25-64x32-b": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rgb-matrix-p25-96x48-f": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-lr1121-epaper29": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": [
      "boot",
      "key"
    ]
  },
  "waveshare-s3-lr1121-oled154": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": [
      "boot",
      "power",
      "menu",
      "ptt"
    ]
  },
  "waveshare-s3-touch-lcd185b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-touch-amoled206": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": [
      "boot",
      "power"
    ]
  },
  "waveshare-c6-touch-lcd128": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-lcd128": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-lcd273": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch28": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-lcd154": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-lcd154": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s31-touch-lcd43c": {
    "chip": "esp32s31",
    "display": true,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "led": true,
    "imu": true,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 16
  },
  "waveshare-p4-knob-amoled175": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd43-v13": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 100
  },
  "waveshare-p4-lcd43-v32": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd5-v32": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd5-v13": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 100
  },
  "waveshare-s3-touch-amoled216": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-lcd154": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd154": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-lcd085": {
    "chip": "esp32s3",
    "flashMb": 8,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c3-lcd147": {
    "chip": "esp32c3",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-epaper154": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-amoled216": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-amoled164": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd7c": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-epaper154g": {
    "chip": "esp32s3",
    "flashMb": 8,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-epaper397": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd43c": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-amoled175c": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s31-zero-n16r16": {
    "chip": "esp32s31",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 16,
    "psramMb": 16
  },
  "waveshare-s31-zero-n16r32": {
    "chip": "esp32s31",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 16,
    "psramMb": 32
  },
  "waveshare-s31-zero-n32r16": {
    "chip": "esp32s31",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 32,
    "psramMb": 16
  },
  "waveshare-s31-zero-n32r32": {
    "chip": "esp32s31",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 32,
    "psramMb": 32
  },
  "waveshare-p4-lcd8": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd7": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd7b": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd4b-v32": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd4b-v13": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 100
  },
  "waveshare-p4-panel-eth2ro-v32": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-panel-eth2ro-v13": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 100
  },
  "waveshare-p4-lcd34c": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-lcd4c": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "imu": false,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-nano-wifi6-db-kit-101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-db-kit-101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-poe-eth-kit-101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-c6-geek": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-geek-v1": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-lcd13": {
    "chip": "esp32c6",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-amoled18": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-amoled18-v1": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-lcd183": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-amoled132": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-rlcd42": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-amoled132": {
    "chip": "esp32s3",
    "flashMb": 8,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd183": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd349-v2": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-epaper154v2": {
    "chip": "esp32s3",
    "flashMb": 8,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-epaper154v2": {
    "chip": "esp32s3",
    "flashMb": 8,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-touch-amoled206": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c6-touch-amoled143": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-lcd169": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-dualeye-lcd128": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-dualeye-touch-lcd128": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd4b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-lcd316": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd7b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-lcd7b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-knob-touch-lcd18": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-p4-poe-eth-kit-101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-dev-kit-7": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-dev-kit-101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-kit-101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 32,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-module-dev-kit-7": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-p4-module-dev-kit-101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-c5-mini-kit": {
    "chip": "esp32c5",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c5-pico": {
    "chip": "esp32c5",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c5-zero": {
    "chip": "esp32c5",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c5-wifi6-n16r8": {
    "chip": "esp32c5",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c5-wifi6-n32r8": {
    "chip": "esp32c5",
    "flashMb": 32,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-p4-nano-kit-101": {
    "chip": "esp32p4",
    "display": false,
    "touch": true,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 16,
    "psramMb": 32,
    "virtualRevision": 301
  },
  "waveshare-servo-driver-esp32": {
    "chip": "esp32",
    "display": true,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 4,
    "psramMb": 0
  },
  "waveshare-esp32-devkit-wroom-32e-n4": {
    "chip": "esp32",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 4,
    "psramMb": 0
  },
  "waveshare-esp32-devkit-wroom-32ue-n16": {
    "chip": "esp32",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 16,
    "psramMb": 0
  },
  "waveshare-esp32-devkit-wrover-e-n16r8": {
    "chip": "esp32",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 16,
    "psramMb": 8
  },
  "waveshare-esp32-devkit-wrover-ie-n16r8": {
    "chip": "esp32",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": false,
    "flashMb": 16,
    "psramMb": 8
  },
  "waveshare-general-driver-robots": {
    "chip": "esp32",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "flashMb": 4,
    "psramMb": 0
  },
  "waveshare-epaper-esp32-driver-v3": {
    "chip": "esp32",
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "flashMb": 4,
    "psramMb": 0
  },
  "waveshare-c3-lcd071": {
    "chip": "esp32c3",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c6-lcd147": {
    "chip": "esp32c6",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-c6-zero-b": {
    "chip": "esp32c6",
    "flashMb": 8,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c6-zero": {
    "chip": "esp32c6",
    "flashMb": 8,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c6-zero-fh4": {
    "chip": "esp32c6",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-lcd147b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd35b": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": [
      "power"
    ]
  },
  "waveshare-s3-touch-lcd147": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-amoled175": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": [
      "power"
    ]
  },
  "waveshare-s3-touch-amoled164-rev11": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-rs485-wled": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-can-2ch": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-can-2ch-u": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rgb-matrix": {
    "chip": "esp32s3",
    "flashMb": 32,
    "psramMb": 16,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-cam-ov5640": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot",
      "power"
    ]
  },
  "waveshare-s3-cam-ov3660": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot",
      "power"
    ]
  },
  "waveshare-s3-cam-gc2145": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot",
      "power"
    ]
  },
  "waveshare-s3-cam-gc0308": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot",
      "power"
    ]
  },
  "waveshare-s3-lr1121-hf": {
    "chip": "esp32s3",
    "flashMb": 4,
    "psramMb": 2,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-lr1121-hf-kit": {
    "chip": "esp32s3",
    "flashMb": 4,
    "psramMb": 2,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-lr1121-lf": {
    "chip": "esp32s3",
    "flashMb": 4,
    "psramMb": 2,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-lr1121-lf-kit": {
    "chip": "esp32s3",
    "flashMb": 4,
    "psramMb": 2,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-c6-pico": {
    "chip": "esp32c6",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c6-dev-kit-n8": {
    "chip": "esp32c6",
    "flashMb": 8,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c6-dev-kit-n16": {
    "chip": "esp32c6",
    "flashMb": 16,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-c3-zero": {
    "chip": "esp32c3",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-h2-zero": {
    "chip": "esp32h2",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-h2-dev-kit-n4": {
    "chip": "esp32h2",
    "flashMb": 4,
    "psramMb": 0,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-esp32-touch35": {
    "chip": "esp32",
    "flashMb": 16,
    "psramMb": 2,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-thermal-camera": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-lcd19": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd19": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd35-v2": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": [
      "power"
    ]
  },
  "waveshare-s3-lcd13": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd185c-v2": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-amoled241-v2": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-audio-board": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": true,
    "boardButtons": [
      "boot",
      "key-1",
      "key-2",
      "key-3"
    ]
  },
  "waveshare-s3-audio-board-en": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": true,
    "boardButtons": [
      "boot",
      "key-1",
      "key-2",
      "key-3"
    ]
  },
  "waveshare-s3-rs485-can": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-rs485-can-u": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-relay-1ch": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-relay-1ch-u": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": false,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ]
  },
  "waveshare-s3-poe-eth-8di-8do": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": true,
    "boardButtons": [
      "boot",
      "di1",
      "di2",
      "di3",
      "di4",
      "di5",
      "di6",
      "di7",
      "di8"
    ]
  },
  "waveshare-s3-eth-8di-8ro-c": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": true,
    "boardButtons": [
      "boot",
      "di1",
      "di2",
      "di3",
      "di4",
      "di5",
      "di6",
      "di7",
      "di8"
    ]
  },
  "waveshare-s3-poe-eth-8di-8ro-c": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "display": false,
    "touch": false,
    "gpio": true,
    "rgb": true,
    "sdCard": true,
    "boardButtons": [
      "boot",
      "di1",
      "di2",
      "di3",
      "di4",
      "di5",
      "di6",
      "di7",
      "di8"
    ]
  },
  "waveshare-s3-touch-lcd28-v1": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd28-v2": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-lcd28": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd169": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-matrix8": {
    "chip": "esp32s3",
    "flashMb": 4,
    "psramMb": 2,
    "sdCard": false,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd7": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-lcd7": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  },
  "waveshare-s3-touch-lcd7-external": {
    "chip": "esp32s3",
    "flashMb": 16,
    "psramMb": 8,
    "sdCard": true,
    "boardButtons": []
  }
}));
