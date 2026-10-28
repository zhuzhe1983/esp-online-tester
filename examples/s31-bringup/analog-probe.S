/* S31 IDF v6.1 internal analog-I2C transaction and MPLL shutdown fixture.
 * Independent host BIAS input [0x17,0x29]; no silicon-reset or voltage claim.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x2010f800
    li s2, 0x2010f000
    li t0, 4
    sw t0, 0x18(s2)
    sw t0, 0x1c(s2)
    li t1, 0x20109c04
    lw t0, 0(t1)
    li t2, 0x1000
    or t0, t0, t2
    sw t0, 0(t1)
    li t0, 0xffffbf
    sw t0, 0x1c(s1)
    li t0, 0x6a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0x17
    bne t1, t2, fail
    li t0, 0x01a7006a
    sw t0, 4(s1)
    li t0, 0x6a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0xa7
    bne t1, t2, fail
    li t0, 0x16a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0x29
    bne t1, t2, fail
    li t0, 0x012a016a
    sw t0, 4(s1)
    li t0, 4
    sw t0, 0x24(s2)             /* reset digital master, preserve analog data */
    sw zero, 0x24(s2)
    li t0, 0x6a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0xa7
    bne t1, t2, fail
    li t0, 0x16a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0x2a
    bne t1, t2, fail
    li t0, 0x104                /* route BIAS to host0; same slave storage */
    sw t0, 0x20(s1)
    li t0, 0x6a
    sw t0, 0(s1)
    lw t1, 0(s1)
    srli t1, t1, 16
    li t2, 0xa7
    bne t1, t2, fail
    li t0, 0x207040f4
    li t1, 0x800
    sw t1, 0(t0)
    li t1, 0x88
    sw t1, 0(t0)
    lw t1, 0(t0)
    bnez t1, fail               /* WT command bits must not remain latched */
    li t0, 0x20589000
    lw t1, 0(t0)
    li t2, 0x7fffffff
    and t1, t1, t2
    sw t1, 0(t0)
    la s0, pass
    jal ra, puts
1:  wfi
    j 1b
fail:
    la s0, failed
    jal ra, puts
    ebreak
puts:
    li t3, 0x2038a000
2:  lbu t4, 0(s0)
    beqz t4, 3f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 2b
3:  ret
.section .rodata,"a",@progbits
pass: .asciz "S31 ANALOG ROUTE RESET MPLL PASS\n"
failed: .asciz "S31 ANALOG FAIL\n"
