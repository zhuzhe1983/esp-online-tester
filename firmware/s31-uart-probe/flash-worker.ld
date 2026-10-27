/* Explicit fixture mapping, not a bootable flash image. */
ENTRY(_start)
MEMORY { FLASH (rx) : ORIGIN = 0x40000000, LENGTH = 64K }
SECTIONS { .text : { *(.text.start) *(.text*) } > FLASH }
