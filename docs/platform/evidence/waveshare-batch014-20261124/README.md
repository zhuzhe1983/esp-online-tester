# Waveshare batch 014 evidence

Five P4 controller families, 24 exact SKU profiles and eight original manufacturer CAD models. The Module DEV KIT combines separately retained native module and carrier geometries with explicitly UNKNOWN assembly alignment. No sibling PCB is substituted. All eight four-view CAD reports were reviewed.

Three native RISC-V guest tests pass, including exact BOOT and codec absence/ACK/NACK across 27 platform profiles. Four production WASM registry/wafer/chip gates pass.

Four representative untouched factory images run for 3.00 to 3.08 guest seconds through the public Platform/Worker. UART reaches app_main, SPIFFS, codec control initialization and MIPI DSI PHY power. No display frames are produced. DSI pixels, Ethernet, Wi-Fi coprocessor RF and audio PCM remain unsupported; this is a bounded original-image observation, not full device acceptance.

Raw UART, original-image identity and WASM hashes are retained under each original-attempt directory. No vendor binary is mirrored in the repository or the download site. Browser WebGL and physical hardware acceptance are separate and not claimed.
