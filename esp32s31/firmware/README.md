# ESP32-S31 firmware

`tools/fetch-s31-firmware.sh` builds the IDF v6.1 (`fff9895c82d7`) `hello_world` images here
from source with `tools/s31-hello/sdkconfig.defaults` and checks their pinned SHA-256s. The
`*.bin` files are not committed. See `docs/evidence/s31-upstream/README.md`.

