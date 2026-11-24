# P4 revision 3 bit-manipulation gate

The [Espressif revision guide](https://documentation.espressif.com/esp32-p4-chip-revision-v3.x_user_guide_en.pdf), version 1.1 section 1.1, specifies the addition of Zb on revision 3. The revision-3 CPU now enables the existing Zba/Zbb/Zbs decoder; the earlier P4 and C3/C6 configurations retain illegal-instruction traps.

The existing bit-manipulation arithmetic cases now run on both S31 and P4 revision 3. An additional BSETI program checks the destination register, program counter, reset retention and exception metadata on unsupported cores. This does not enable Zc/PIE or establish compatibility of an entire manufacturer firmware image. `source.json` identifies the retrieved guide; `verification.json` binds the checks to source hashes.
