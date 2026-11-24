// Exact runtime profiles; memory and physical TF presence are separate from full peripheral acceptance.
// Source and runtime limits: docs/evidence/waveshare-* and each product knowledge record.
export const WAVESHARE_BOARD_CAPABILITIES=Object.freeze(Object.assign(Object.create(null),{
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
    "flashMb": 16,
    "psramMb": 32,
    "sdCard": false,
    "boardButtons": [],
    "virtualRevision": 301
  },
  "waveshare-p4-core-32mb": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": false,
    "boardButtons": [],
    "virtualRevision": 301
  },
  "waveshare-p4-module-16mb": {
    "chip": "esp32p4",
    "flashMb": 16,
    "psramMb": 32,
    "sdCard": false,
    "boardButtons": [],
    "virtualRevision": 301
  },
  "waveshare-p4-module-32mb": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": false,
    "boardButtons": [],
    "virtualRevision": 301
  },
  "waveshare-p4-module-16mb-ipex": {
    "chip": "esp32p4",
    "flashMb": 16,
    "psramMb": 32,
    "sdCard": false,
    "boardButtons": [],
    "virtualRevision": 301
  },
  "waveshare-p4-module-32mb-ipex": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": false,
    "boardButtons": [],
    "virtualRevision": 301
  },
  "waveshare-p4-core-dev-kit": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": false,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-nano": {
    "chip": "esp32p4",
    "flashMb": 16,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-module-dev-kit": {
    "chip": "esp32p4",
    "flashMb": 16,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-dev-kit": {
    "chip": "esp32p4",
    "flashMb": 16,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-eth": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-poe-eth": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
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
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-wifi6-db": {
    "chip": "esp32p4",
    "flashMb": 32,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
    "virtualRevision": 301
  },
  "waveshare-p4-nano-wifi6-db": {
    "chip": "esp32p4",
    "flashMb": 16,
    "psramMb": 32,
    "sdCard": true,
    "boardButtons": [
      "boot"
    ],
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
  }
}));
