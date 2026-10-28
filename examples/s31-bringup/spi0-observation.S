/* Emulator-only SPI0 reserved-word observation; never flash to a physical device. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20500000
    la s0, before
    jal ra, puts
    lw a0, 0x24(s1)
    jal ra, hex
    lw a0, 0x28(s1)
    jal ra, hex
    li t0, 0xa55a03ff
    sw t0, 0x24(s1)
    sw t0, 0x28(s1)
    la s0, after
    jal ra, puts
    lw a0, 0x24(s1)
    jal ra, hex
    lw a0, 0x28(s1)
    jal ra, hex
    la s0, done
    jal ra, puts
1:  wfi
    j 1b
hex:
    li t3, 0x2038a000
    li t4, 8
2:  srli t0, a0, 28
    addi t0, t0, 48
    li t1, 58
    blt t0, t1, 3f
    addi t0, t0, 39
3:  sw t0, 0(t3)
    slli a0, a0, 4
    addi t4, t4, -1
    bnez t4, 2b
    li t0, 10
    sw t0, 0(t3)
    ret
puts:
    li t3, 0x2038a000
4:  lbu t4, 0(s0)
    beqz t4, 5f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 4b
5:  ret
.section .rodata,"a",@progbits
before: .asciz "SPI0 BEFORE\n"
after: .asciz "SPI0 AFTER\n"
done: .asciz "SPI0 OBSERVATION DONE\n"
