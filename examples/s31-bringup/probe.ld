/* ESP-IDF v6.1 S31 SOC_IRAM_LOW/HIGH, not the P4 SRAM window. */
ENTRY(_start)
MEMORY { SRAM (rwx) : ORIGIN = 0x2f000000, LENGTH = 512K }
SECTIONS {
    .text : { *(.text.start) *(.text*) *(.rodata*) } > SRAM
    .data : { *(.data*) } > SRAM
    .bss (NOLOAD) : { *(.bss*) *(COMMON) } > SRAM
}
