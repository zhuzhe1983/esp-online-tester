.section .text.start
.global _start
_start:
    la sp, stack_top
    call main
1:  wfi
    j 1b
