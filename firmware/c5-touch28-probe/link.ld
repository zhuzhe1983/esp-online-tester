ENTRY(_start)
SECTIONS {
  . = 0x40800000;
  .text : { *(.text.start) *(.text*) }
  .rodata ALIGN(4) : { *(.rodata*) }
  .data ALIGN(4) : { *(.data*) }
  .bss ALIGN(16) (NOLOAD) : { *(.bss*) . += 4096; stack_top = .; }
}
