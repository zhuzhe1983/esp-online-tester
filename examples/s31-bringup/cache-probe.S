/* S31 IDF v6.1 cache_reg.h, hp_sys_clkrst_reg.h, hp_alive_sys_reg.h.
 * Source SRAM guest verifies cache/MMU control transitions and reads independent
 * Flash input. Functional no-cache-line model; no timing or product boot claim.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x2c000000
    li t0, 0x20500000
    li t1, 0x40000001
    sw t1, 0x3c(t0)
    sw zero, 0x380(t0)
    li t1, 0x1000
    sw t1, 0x37c(t0)
    li t0, 0x2058902c
    li t1, 13                    /* software light-retention, MMU blocked */
    sw t1, 0(t0)
    lw t2, 0(t0)
    bne t1, t2, fail
    li t1, 9                     /* power back on; table retained */
    sw t1, 0(t0)
    li t0, 0x40000000
    lw t1, 0(t0)
    li t2, 0x17293c5a
    bne t1, t2, fail
    li t0, 0x13                  /* actual I0/I1/D targets */
    sw t0, 0xa0(s1)
    li t0, 0x40000000
    sw t0, 0xa4(s1)
    li t0, 4
    sw t0, 0xa8(s1)
    li t0, 1                     /* invalidate over the functional direct model */
    sw t0, 0x9c(s1)
    lw t1, 0x9c(s1)
    li t2, 16
    bne t1, t2, fail
    li t0, 0x10000               /* D-cache freeze */
    sw t0, 0x30(s1)
    lw t1, 0x30(s1)
    li t2, 0x50000
    bne t1, t2, fail
    sw zero, 0x30(s1)
    lw t1, 0x30(s1)
    bnez t1, fail
    li t0, 0x40000000
    lw t1, 0(t0)
    li t2, 0x17293c5a
    bne t1, t2, fail
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
pass: .asciz "S31 CACHE POWER SYNC FREEZE PASS\n"
failed: .asciz "S31 CACHE FAIL\n"
