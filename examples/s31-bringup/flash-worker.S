.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li t0, 0x50000000
    li a0, 0x5a3c1729
    sw a0, 0(t0)
    ret
