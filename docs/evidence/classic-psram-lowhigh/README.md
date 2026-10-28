# Classic PSRAM low/high mapping

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

Source: ESP32 TRM v5.8 section 4.3.2.2, tables 4.3-14/16/18, printed pages 86-88.
PDF SHA-256: `4ba58e9fa0405ec2bf80b912a29b483f6edc8c4b2b1058201913a2fe37e582f0`.
For either CPU, 0x3f800000-0x3f9fffff uses PRO local slots 1152-1215,
MMIO 0x3ff11200-0x3ff112fc. 0x3fa00000-0x3fbfffff uses APP local slots
1216-1279, MMIO 0x3ff13300-0x3ff133fc. Upper PRO and lower APP slots are unused.
Disabling APP cache removes the upper half, not the lower half, for either CPU.
All bytes of a cross-half store are validated before modifying backing.

ESP-IDF v4.4.7 `38eeba213aa695aabfd6d89aa9f5078dbe5a94c3`:
`components/esp_hw_support/port/esp32/cache_sram_mmu.c:19-27,69-71,90-110`,
`spiram.c:31-38,128-138,254-306`, `spiram_psram.c:1090-1105`, and
`components/soc/esp32/include/soc/dport_reg.h:186-326,426-554`.
Both masks clear and page mode zero on the owning cache are conservative
qualification restrictions. SPLIT/even-odd, mixed live normal/HL modes and
HL+SPLIT are unsupported. No per-requester routing or himem API claim.

Stock 80/80 MHz Arduino selects single-byte EB/38; no wire timing is modelled.
The linked weak himem reservation returns zero, even with reserve=8 configured.
The IDF4 inclusive-end heap is 4194303 bytes, not the physical 4194304 bytes.
The original stock A2 prerequisite replay remains an external test, invoked
with P5_FIRMWARE_DIR and ESP32SIM_ROM_DIR. It observes the real memory test,
all 131072 stride-32 words and absent ID without guest stubs or MMU preparation.
Run `cargo +1.99.0 test --release -p esp32sim --test classic_psram_lowhigh -- --ignored`.
Historical self-consistent slot/shift probe survivors are not counted as kills;
the independent native mapping suite detects those errors.
