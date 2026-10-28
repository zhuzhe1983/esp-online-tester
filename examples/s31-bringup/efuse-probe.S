/* S31 v6.1 eFuse read controller, with explicit synthetic decoded image from host.
 * No programming command and no chip-identity/secret fields are selected.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20715000
    li t0, 0x5aa5
    sw t0, 0x1cc(s1)
    li t0, 1
    sw t0, 0x1e4(s1)
    sw t0, 0x1d8(s1)
1:  lw t0, 0x1d8(s1)
    andi t0, t0, 1
    bnez t0, 1b
    lw t0, 0x1d4(s1)
    li t1, 0x1c01                   /* explicit valid-bit-count7 and idle state1 */
    bne t0, t1, fail
    lw t0, 0x34(s1)
    li t1, 0x80000                  /* WDT_DELAY_SEL bit19 */
    bne t0, t1, fail
    lw t0, 0x88(s1)
    li t1, 0x5a17f00d               /* explicit synthetic user data word */
    bne t0, t1, fail
    sw zero, 0x88(s1)               /* RO shadow must not be changed by a write */
    lw t0, 0x88(s1)
    bne t0, t1, fail
    lw t0, 0x1e0(s1)
    li t1, 1
    bne t0, t1, fail
    sw t1, 0x1e8(s1)
    lw t0, 0x1e0(s1)
    bnez t0, fail
    la s0, pass
    jal ra, puts
2:  wfi
    j 2b
fail:
    la s0, failed
    jal ra, puts
    ebreak
puts:
    li t3, 0x2038a000
3:  lbu t4, 0(s0)
    beqz t4, 4f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 3b
4:  ret
.section .rodata,"a",@progbits
pass: .asciz "S31 EFUSE READ PASS\n"
failed: .asciz "S31 EFUSE FAIL\n"
