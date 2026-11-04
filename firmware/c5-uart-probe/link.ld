ENTRY(_start)
SECTIONS {
  . = 0x40800000;
  .text : { *(.text*) }
  .rodata ALIGN(4) : { *(.rodata*) }
  .data : { *(.data*) }
  .bss (NOLOAD) : { *(.bss*) }
}
