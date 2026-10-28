/* TIMG0 XTAL/target measurement using real guest polling. Fixed S31 IDF6.1
 * rtc_time.c / clk_tree_ll.h. Counts are nominal functional clocks, not silicon. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f06f000
    li s1, 0x20580000
    li s2, 0x20587118
    li t0, (1 | (12 << 1) | (9 << 5))
    sw t0, 0(s2)                  /* XTAL /10 */
    li t0, ((10000 << 7) | (3 << 3))
    sw t0, 0x80(s1)
    li t0, (3 << 16)
    sw t0, 0x68(s1)               /* stop cycling */
    li t1, 0x80000000
    or t0, t0, t1
    sw t0, 0x68(s1)
1:  lw t0, 0x68(s1)               /* no host clock input: stays here */
    li t1, 0x8000
    and t0, t0, t1
    beqz t0, 1b
    lw t0, 0x6c(s1)
    li t1, (30 << 7)
    bne t0, t1, fail
    /* Missing target due to its gate: reference clock reaches real timeout. */
    li t0, (12 << 1)
    sw t0, 0(s2)
    li t0, (1 << 16)
    sw t0, 0x68(s1)
    li t1, 0x80000000
    or t0, t0, t1
    sw t0, 0x68(s1)
2:  lw t0, 0x80(s1)
    andi t0, t0, 1
    beqz t0, 2b
    lw t0, 0x68(s1)
    li t1, 0x8000
    and t0, t0, t1
    bnez t0, fail
    /* 17 nominal RC_SLOW cycles = exactly5000 XTAL ticks at136kHz. */
    li t0, (1 | (8 << 1))
    sw t0, 0(s2)
    li t0, (17 << 16)
    sw t0, 0x68(s1)
    li t1, 0x80000000
    or t0, t0, t1
    sw t0, 0x68(s1)
3:  lw t0, 0x68(s1)
    li t1, 0x8000
    and t0, t0, t1
    beqz t0, 3b
    lw t0, 0x6c(s1)
    li t1, (5000 << 7)
    bne t0, t1, fail
    lw t0, 0x80(s1)
    andi t0, t0, 1
    bnez t0, fail
    li t0, 0x2f060000
    li t1, 1
    sw t1, 0(t0)
    la s0, pass_msg
    call puts
4:  wfi
    j 4b
fail:
    la s0, fail_msg
    call puts
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
pass_msg: .asciz "S31 RTC CALIBRATION COUNT TIMEOUT PASS\n"
fail_msg: .asciz "S31 RTC CALIBRATION FAIL\n"
