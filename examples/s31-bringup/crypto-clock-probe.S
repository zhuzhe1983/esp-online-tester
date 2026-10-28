/* S31 actual SHA-256 over three documented crypto roots. The guest itself writes
 * clocks, resets SHA, supplies the padded abc block and verifies all digest words.
 * Mode mailbox0 runs all;1/2/3 start RC_FAST/PLL240/reserved for negative inputs. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20587000
    li t0, 7
    sw t0, 0x5c(s1)
    lw t1, 0x5c(s1)
    bne t0, t1, fail
    li t0, 5
    sw t0, 0x5c(s1)
    lw t1, 0x5c(s1)
    bne t0, t1, fail
    li s2, 0x20506000
    lw t0, 0x28(s2)
    bnez t0, fail
    lw t0, 0x18(s2)
    li t1, 0x1e0
    bne t0, t1, fail
    ori t1, t1, 2
    sw t1, 0x18(s2)
    li t0, 2
    sw t0, 0x1c(s2)
    sw zero, 0x1c(s2)
    sw zero, 0x18(s2)
    lw t0, 0x18(s2)
    li t1, 2
    bne t0, t1, fail
    lw t0, 0x1c(s2)
    bne t0, t1, fail
    li t0, 7
    sw t0, 0x5c(s1)
    li t0, 5
    sw t0, 0x5c(s1)
    lw t0, 0x18(s2)
    li t1, 0x1e0
    bne t0, t1, fail
    lw t0, 0x1c(s2)
    bnez t0, fail
    lw t0, 0x2c(s2)
    bnez t0, fail
    lw t0, 0x30(s2)
    bnez t0, fail
    li t0, 0x2f070000
    lw s0, 0(t0)
    li t0, 6
    bltu s0, t0, hash_setup
    addi t0, s0, -5
    sw t0, 0x24(s2)          /* Real unsupported START/CONTINUE must fail. */
    j fail
hash_setup:
    li s2, 0x20509000
loop:
    slli t0, s0, 20
    ori t0, t0, 0xc5
    sw t0, 0x58(s1)
    xori t0, t0, 0x80
    sw t0, 0x58(s1)
    lw t1, 0x58(s1)
    bne t0, t1, fail
    li t0, 2
    sw t0, 0(s2)
    addi t0, s2, 0x80
    li t1, 16
clear:
    sw zero, 0(t0)
    addi t0, t0, 4
    addi t1, t1, -1
    bnez t1, clear
    li t0, 0x80636261
    sw t0, 0x80(s2)
    li t0, 0x18000000
    sw t0, 0xbc(s2)
    li t0, 1
    sw t0, 0x10(s2)
wait:
    lw t0, 0x18(s2)
    bnez t0, wait
    la t0, digest
    addi t1, s2, 0x40
    li t2, 8
check:
    lw t3, 0(t0)
    lw t4, 0(t1)
    bne t3, t4, fail
    addi t0, t0, 4
    addi t1, t1, 4
    addi t2, t2, -1
    bnez t2, check
    li t0, 0x2f060000
    lw t1, 0(t0)
    addi t1, t1, 1
    sw t1, 0(t0)
    addi s0, s0, 1
    li t0, 3
    bltu s0, t0, loop
    la s0, pass_text
    jal ra, puts
sleep:
    wfi
    j sleep
fail:
    la s0, fail_text
    jal ra, puts
    ebreak
puts:
    li t3, 0x2038a000
1:  lbu t4, 0(s0)
    beqz t4, 2f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 1b
2:  ret
.section .rodata,"a",@progbits
.balign 4
digest: .word 0xbf1678ba,0xeacf018f,0xde404141,0x2322ae5d,0xa36103b0,0x9c7a1796,0x61ff10b4,0xad1500f2
pass_text: .asciz "S31 CRYPTO ROOTS REAL SHA PASS\n"
fail_text: .asciz "S31 CRYPTO ROOTS FAIL\n"
